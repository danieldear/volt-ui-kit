use super::{gpu_scene::GpuSceneEncoder, raster::Shape, RasterPainter};
use crate::{declarative::PaintBatch, Rect, Viewport};

fn batch(shapes: Vec<Shape>) -> PaintBatch<Shape, super::raster::Glyph> {
    PaintBatch {
        shapes,
        glyphs: vec![],
        clip: Rect::new(0., 0., 64., 64.).unwrap(),
    }
}
#[test]
fn scene_is_reset_and_shape_commands_are_not_cpu_framebuffer_uploads() {
    let mut encoder = GpuSceneEncoder::default();
    let mut painter = RasterPainter::new(Viewport::new(64., 64., 1.).unwrap());
    encoder
        .encode(
            &[batch(vec![Shape::Rectangle(
                [4., 4., 24., 24.],
                [1., 0., 0., 1.],
            )])],
            &mut painter,
        )
        .unwrap();
    assert_eq!(encoder.stats().shapes, 1);
    assert_eq!(encoder.stats().framebuffer_upload_bytes, 0);
    encoder.encode(&[], &mut painter).unwrap();
    assert_eq!(encoder.stats().shapes, 0);
}
#[test]
fn gpu_layers_and_clips_reject_unbalanced_or_over_limit_streams() {
    let mut encoder = GpuSceneEncoder::default();
    let mut painter = RasterPainter::new(Viewport::new(64., 64., 1.).unwrap());
    for shapes in [
        vec![Shape::LayerEnd],
        vec![Shape::ClipEnd],
        vec![Shape::LayerBegin(0.5)],
        vec![Shape::LayerBegin(f32::NAN)],
        vec![Shape::LayerBegin(0.5); 25],
    ] {
        assert!(encoder.encode(&[batch(shapes)], &mut painter).is_err());
    }
    encoder
        .encode(
            &[batch(vec![
                Shape::LayerBegin(0.5),
                Shape::Rectangle([0., 0., 30., 30.], [1., 1., 1., 1.]),
                Shape::LayerEnd,
            ])],
            &mut painter,
        )
        .unwrap();
    assert_eq!(encoder.stats().layers, 1);
}

#[test]
fn cached_text_and_images_reuse_gpu_resources_without_framebuffer_uploads() {
    use crate::{declarative::DocumentPainter, graphics::Image};
    let mut encoder = GpuSceneEncoder::default();
    let mut painter = RasterPainter::new(Viewport::new(64., 64., 1.).unwrap());
    let image = Image::from_rgba(2, 2, vec![255; 16]).unwrap().background();
    let mut b = batch(vec![Shape::Image([0., 0., 16., 16.], image, 1.)]);
    painter.ui_text(&mut b.glyphs, "SF Aé", [0., 20., 64., 24.], 16., [1.; 4]);
    encoder.encode(&[b.clone()], &mut painter).unwrap();
    assert!(encoder.stats().resource_bytes > 0);
    assert!(encoder.stats().glyphs > 0);
    encoder.encode(&[b], &mut painter).unwrap();
    assert_eq!(encoder.stats().resource_bytes, 0);
    assert_eq!(encoder.stats().framebuffer_upload_bytes, 0);
}

#[test]
fn invalid_gpu_resources_and_crossed_structural_scopes_are_rejected() {
    use crate::graphics::{ClipRegion, Image};
    let image = Image::from_rgba(1, 1, vec![255; 4]).unwrap();
    let mut encoder = GpuSceneEncoder::default();
    let mut painter = RasterPainter::new(Viewport::new(64., 64., 1.).unwrap());
    for s in [
        Shape::Image(
            [0., 0., 10., 10.],
            image.clone().background().opacity(f32::NAN),
            1.,
        ),
        Shape::Image([0., 0., 10., 10.], image.background(), 0.),
    ] {
        assert!(encoder.encode(&[batch(vec![s])], &mut painter).is_err());
    }
    assert!(encoder
        .encode(
            &[batch(vec![
                Shape::LayerBegin(0.5),
                Shape::ClipBegin(ClipRegion::Circle {
                    bounds: [0., 0., 20., 20.]
                }),
                Shape::LayerEnd,
                Shape::ClipEnd
            ])],
            &mut painter
        )
        .is_err());
}

#[test]
#[ignore = "requires an actual GPU; run with --ignored --nocapture"]
fn actual_gpu_readback_matches_software_shapes_text_images_clips_and_opacity() {
    use super::GpuOffscreenRenderer;
    use crate::{
        declarative::{DocumentPainter, TextStyle},
        graphics::{ClipRegion, Image, ImageFit, ImagePosition, VectorPath},
    };
    let mut gpu = GpuOffscreenRenderer::new().expect("GPU required for explicit acceptance test");
    eprintln!("GPU acceptance adapter: {}", gpu.adapter_name);
    for scale in [1., 2.] {
        let mut painter =
            RasterPainter::new(Viewport::new(128. * scale, 128. * scale, scale).unwrap());
        let s = |b: [f32; 4]| b.map(|v| v * scale);
        let pixels = vec![
            255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 255, 255, 255, 255, 0,
        ];
        let image = Image::from_rgba(2, 2, pixels).unwrap();
        let mut b = PaintBatch {
            clip: Rect::new(0., 0., 128. * scale, 128. * scale).unwrap(),
            glyphs: vec![],
            shapes: vec![
                Shape::Rectangle(s([2., 2., 18., 18.]), [0.8, 0.3, 0.2, 0.7]),
                Shape::Corners(
                    s([26., 2., 36., 24.]),
                    [8. * scale, 0., 6. * scale, 0.],
                    [0.2, 0.6, 0.8, 1.],
                ),
                Shape::Outline(
                    s([68., 2., 40., 24.]),
                    [6. * scale; 4],
                    2. * scale,
                    [0.9, 0.7, 0.3, 1.],
                ),
                Shape::Line(
                    [2. * scale, 32. * scale],
                    [108. * scale, 32. * scale],
                    2. * scale,
                    [0.7; 4],
                ),
                Shape::ClipBegin(ClipRegion::Circle {
                    bounds: s([2., 40., 32., 32.]),
                }),
                Shape::LayerBegin(0.5),
                Shape::Rectangle(s([2., 40., 32., 32.]), [1., 0., 0., 1.]),
                Shape::Rectangle(s([10., 48., 16., 16.]), [0., 1., 0., 1.]),
                Shape::LayerEnd,
                Shape::ClipEnd,
                Shape::Image(
                    s([40., 40., 24., 32.]),
                    image
                        .clone()
                        .background()
                        .fit(ImageFit::Cover)
                        .position(ImagePosition::Right)
                        .opacity(0.8),
                    scale,
                ),
                Shape::Image(
                    s([72., 40., 36., 32.]),
                    image.background().fit(ImageFit::Tile),
                    scale,
                ),
                Shape::Path(
                    VectorPath::ellipse(16. * scale, 96. * scale, 12. * scale, 8. * scale),
                    [0.5, 0.9, 0.8, 1.],
                    None,
                ),
            ],
        };
        painter.ui_text_styled(
            &mut b.glyphs,
            "SF café",
            s([36., 84., 84., 30.]),
            &TextStyle {
                family: "SF".into(),
                size: 18. * scale,
                ..Default::default()
            },
            [0.9, 0.85, 0.8, 0.75],
        );
        let batches = [b];
        let background = [0.07, 0.1, 0.12, 1.];
        let cpu = painter.rasterize(&batches, background).unwrap();
        let actual = gpu.render(&mut painter, &batches, background).unwrap();
        let mae = cpu
            .data()
            .iter()
            .zip(actual.data())
            .map(|(a, b)| a.abs_diff(*b) as f64)
            .sum::<f64>()
            / cpu.data().len() as f64;
        eprintln!("GPU/software geometry {scale}x MAE={mae:.3}");
        assert!(
            mae < 4.,
            "GPU/software geometry mismatch at {scale}x: {mae}"
        );
        assert_eq!(gpu.framebuffer_upload_bytes(), 0);
        gpu.render(&mut painter, &batches, background).unwrap();
        assert_eq!(gpu.resource_bytes(), 0);
        // Same-winding hole must remain empty under the public even-odd clip contract.
        let p = VectorPath::rectangle(8., 8., 48., 48.);
        let mut p = p;
        p.commands
            .extend(VectorPath::rectangle(20., 20., 24., 24.).commands);
        let hole = [batch(vec![
            Shape::ClipBegin(ClipRegion::Path(p)),
            Shape::Rectangle([0., 0., 64., 64.], [1.; 4]),
            Shape::ClipEnd,
        ])];
        let result = gpu.render(&mut painter, &hole, [0.; 4]).unwrap();
        let px = |x: usize, y: usize| {
            &result.data()
                [(y * result.width() as usize + x) * 4..(y * result.width() as usize + x) * 4 + 4]
        };
        assert_eq!(px(30, 30), [0, 0, 0, 0]);
        assert_eq!(px(12, 12), [255; 4]);
        // Group opacity is applied once to overlapping siblings, across batches.
        let groups = [
            batch(vec![
                Shape::LayerBegin(0.5),
                Shape::Rectangle([4., 4., 40., 40.], [1., 0., 0., 1.]),
            ]),
            batch(vec![
                Shape::Rectangle([16., 16., 40., 40.], [0., 1., 0., 1.]),
                Shape::LayerEnd,
            ]),
        ];
        let result = gpu.render(&mut painter, &groups, [0.; 4]).unwrap();
        let index = (24 * result.width() as usize + 24) * 4;
        eprintln!("group opacity pixel {:?}", &result.data()[index..index + 4]);
        assert!(result.data()[index] < 3);
        assert!((125..=130).contains(&result.data()[index + 1]));
        assert!((125..=130).contains(&result.data()[index + 3]));
        let shadows = [batch(vec![Shape::Shadow(
            [20., 20., 24., 24.],
            [8.; 4],
            8.,
            0.,
            [0., 0.],
            [0., 1., 0., 0.8],
        )])];
        let result = gpu.render(&mut painter, &shadows, [0.; 4]).unwrap();
        let alpha = |x: usize, y: usize| result.data()[(y * result.width() as usize + x) * 4 + 3];
        assert!(alpha(16, 32) > 0 && alpha(47, 32) > 0);
        assert!(alpha(16, 32).abs_diff(alpha(47, 32)) <= 3);
        assert!(alpha(32, 32) > 180);
        let corners = [batch(vec![Shape::Shadow(
            [20., 20., 24., 24.],
            [8., 0., 8., 0.],
            8.,
            0.,
            [0., 0.],
            [1., 0., 0., 0.8],
        )])];
        gpu.render(&mut painter, &corners, [0.; 4]).unwrap();
    }
}

/// Calibrates the blend-spill estimate against real Vello behaviour: nested
/// full-window rounded clips at Retina size, one level deeper each round.
#[test]
#[ignore = "requires an actual GPU; run with --ignored --nocapture"]
fn actual_gpu_deep_nesting_estimate_matches_vello_spill_limit() {
    use super::GpuOffscreenRenderer;
    use crate::graphics::ClipRegion;
    let mut gpu = GpuOffscreenRenderer::new().expect("GPU required");
    let (w, h) = (2640_f32, 1800_f32);
    for depth in 3..=8 {
        let mut painter = RasterPainter::new(Viewport::new(w, h, 2.).unwrap());
        let mut shapes = Vec::new();
        for i in 0..depth {
            let inset = i as f32 * 2.;
            shapes.push(Shape::ClipBegin(ClipRegion::Rounded {
                bounds: [inset, inset, w - 2. * inset, h - 2. * inset],
                radii: [24.; 4],
            }));
        }
        // A distinct colour per depth: a dropped Vello frame leaves the previous
        // frame's pixels in the target, so "something was drawn" is not enough.
        let red = depth as f32 / 10.;
        shapes.push(Shape::Rectangle([0., 0., w, h], [red, 1., 0., 1.]));
        shapes.extend(std::iter::repeat_n(Shape::ClipEnd, depth));
        let batch = PaintBatch {
            clip: Rect::new(0., 0., w, h).unwrap(),
            glyphs: vec![],
            shapes,
        };
        let pixmap = gpu
            .render(&mut painter, &[batch], [0., 0., 1., 1.])
            .unwrap();
        let center = pixmap.pixel(1320, 900).unwrap();
        let drawn =
            center.green() > 200 && (i32::from(center.red()) - (red * 255.) as i32).abs() <= 2;
        let estimate = gpu.last_spill_slots();
        eprintln!("depth={depth} estimate={estimate} drawn={drawn}");
        // Frames Vello would drop are flagged and drawn in software instead,
        // so every depth must show this frame's colour, never a stale one.
        assert!(
            drawn,
            "stale or blank frame at depth {depth} (estimate {estimate})"
        );
        assert_eq!(estimate > super::gpu_scene::SPILL_SLOT_LIMIT, depth > 4);
    }
}

#[test]
#[ignore = "requires an actual GPU; explicit resize/virtual-list stress acceptance"]
fn actual_gpu_virtual_list_resize_stress_uses_bounded_scenes() {
    use super::GpuOffscreenRenderer;
    use crate::{
        declarative::{Bindings, Runtime},
        ui::prelude::*,
        UiTheme,
    };
    let mut gpu = GpuOffscreenRenderer::new().expect("explicit GPU acceptance");
    eprintln!("Stress adapter: {}", gpu.adapter_name);
    let mut painter = RasterPainter::new(Viewport::new(640., 480., 1.).unwrap());
    let mut max_resources = 0;
    let mut elapsed = vec![];
    for frame in 0..24usize {
        let scale = if frame % 2 == 0 { 1. } else { 2. };
        let width = if frame % 3 == 0 { 480. } else { 640. };
        painter.set_viewport(Viewport::new(width * scale, 480. * scale, scale).unwrap());
        let start = (frame * 397) % 9_984;
        let ui: Element<()> = vstack![
            text("Virtual GPU stress — SF glyphs, clipping, 10k model").font_size(16.),
            virtual_list(10_000, 28., |id| data_item(id, format!("Record {id:05}"))
                .key(format!("stress-{id}")))
            .key("stress-list")
            .window(start..start + 16)
            .height(300.)
            .on_select(|_: &mut (), _| {})
            .on_window_change(|_: &mut (), _| {}),
            progress(frame as f32 / 24.).width(300.),
        ]
        .padding(12., Edge::All)
        .gap(12., Axis::Vertical)
        .into();
        let (document, handlers) = ui.build().unwrap().into_parts();
        let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
        // Initial keyed list scroll is initialized from the controlled window.
        // This fixture exercises the scene/target resize path independently.
        runtime
            .layout(painter.gpu_viewport(), &Bindings::default(), &mut painter)
            .unwrap();
        assert!(runtime
            .boxes()
            .iter()
            .any(|b| b.id == format!("stress-{start}")));
        let batches = runtime.paint_ui(&mut painter, &UiTheme::dark());
        let begin = std::time::Instant::now();
        let pixmap = gpu
            .render(&mut painter, &batches, UiTheme::dark().palette.base[0])
            .unwrap();
        elapsed.push(begin.elapsed().as_secs_f64() * 1000.);
        assert_eq!(pixmap.width(), (width * scale) as u32);
        assert_eq!(pixmap.height(), (480. * scale) as u32);
        assert_eq!(gpu.framebuffer_upload_bytes(), 0);
        max_resources = max_resources.max(gpu.resource_bytes());
        assert!(gpu.last_spill_slots() < 1_000_000);
    }
    elapsed.sort_by(f64::total_cmp);
    eprintln!("24 alternating resize frames incl readback: p50={:.3}ms p95={:.3}ms max_new_cpu_asset_bytes={} (not GPU memory)",elapsed[12],elapsed[22],max_resources);
}

/// Synthesised italic (a face with no italic style) must lean the same way on
/// GPU vector glyphs as on software bitmaps: rightward towards the top.
#[test]
#[ignore = "requires an actual GPU; run with --ignored --nocapture"]
fn actual_gpu_fake_italic_leans_like_software() {
    use super::GpuOffscreenRenderer;
    use crate::declarative::{DocumentPainter, TextStyle};
    let mut gpu = GpuOffscreenRenderer::new().expect("GPU required");
    let lean = |pixmap: &tiny_skia::Pixmap| {
        let centroid = |rows: std::ops::Range<u32>| {
            let (mut sum, mut weight) = (0f64, 0f64);
            for y in rows {
                for x in 0..pixmap.width() {
                    let ink = f64::from(pixmap.pixel(x, y).unwrap().red());
                    sum += f64::from(x) * ink;
                    weight += ink;
                }
            }
            sum / weight.max(1.)
        };
        centroid(20..40) - centroid(80..100)
    };
    for family in ["Arial Unicode MS", "Verdana"] {
        let mut painter = RasterPainter::new(Viewport::new(160., 120., 1.).unwrap());
        let style = TextStyle {
            family: family.into(),
            size: 96.,
            weight: 400,
            italic: true,
        };
        let mut glyphs = vec![];
        painter.ui_text_styled(
            &mut glyphs,
            "I",
            [40., 0., 100., 120.],
            &style,
            [1., 1., 1., 1.],
        );
        let batch = PaintBatch {
            clip: Rect::new(0., 0., 160., 120.).unwrap(),
            shapes: vec![],
            glyphs,
        };
        let cpu = painter
            .rasterize(std::slice::from_ref(&batch), [0., 0., 0., 1.])
            .unwrap();
        let gpu_pixels = gpu
            .render(&mut painter, &[batch], [0., 0., 0., 1.])
            .unwrap();
        let (cpu_lean, gpu_lean) = (lean(&cpu), lean(&gpu_pixels));
        eprintln!("{family}: software lean={cpu_lean:.2}px gpu lean={gpu_lean:.2}px");
        assert!(
            cpu_lean > 2.,
            "{family}: expected a rightward software slant"
        );
        assert!(gpu_lean > 2. && (gpu_lean - cpu_lean).abs() < cpu_lean * 0.5);
    }
}

#[test]
fn over_budget_frames_name_the_vello_buffer_they_would_overflow() {
    use crate::graphics::Image;
    let (w, h) = (2640_f32, 1800_f32);
    let mut painter = RasterPainter::new(Viewport::new(w, h, 2.).unwrap());
    let frame = |shapes: Vec<Shape>| PaintBatch {
        clip: Rect::new(0., 0., w, h).unwrap(),
        glyphs: vec![],
        shapes,
    };
    let mut encoder = GpuSceneEncoder::default();
    // An ordinary frame stays on the GPU.
    encoder
        .encode(
            &[frame(vec![Shape::Rectangle(
                [0., 0., w, h],
                [0.1, 0.1, 0.1, 1.],
            )])],
            &mut painter,
        )
        .unwrap();
    assert_eq!(encoder.stats().over_budget, None);
    // ~18.5k tiles per full-window fill: 150 of them exceed the tile buffer.
    let fills = vec![Shape::Rectangle([0., 0., w, h], [0.1, 0.1, 0.1, 1.]); 150];
    encoder.encode(&[frame(fills)], &mut painter).unwrap();
    assert_eq!(encoder.stats().over_budget, Some("tiles"));
    // A 9000px-wide image cannot fit Vello's 8192px atlas.
    let wide = Image::from_rgba(9000, 1, vec![255; 9000 * 4]).unwrap();
    encoder
        .encode(
            &[frame(vec![Shape::Image(
                [0., 0., w, 10.],
                wide.background(),
                2.,
            )])],
            &mut painter,
        )
        .unwrap();
    assert_eq!(encoder.stats().over_budget, Some("image atlas"));
}
