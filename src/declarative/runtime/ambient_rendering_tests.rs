use crate::{
    declarative::{Bindings, PaintBatch, Runtime},
    motion::Affine2D,
    rendering::{
        raster::{Glyph, Shape},
        RasterPainter,
    },
    ui::prelude::*,
    Viewport,
};
use std::time::Duration;

fn fixture(scale: f32) -> (Runtime, RasterPainter) {
    let tree: Element<()> = vstack![text("Floating").key("label").font_size(20.)]
        .key("card")
        .width(180.)
        .height(60.)
        .padding(15., Edge::All)
        .ambient(Ambient::float(6., 4.).period_ms(7000))
        .into();
    let (d, h) = tree.build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    let vp = Viewport::new(300. * scale, 150. * scale, scale).unwrap();
    let mut p = RasterPainter::new(vp);
    r.layout(vp, &Bindings::default(), &mut p).unwrap();
    (r, p)
}

// Match the renderer's pixel-snapped local glyph origin, then apply subtree
// transforms. Fractional world translation must survive that local snapping.
fn text_origin(batches: &[PaintBatch<Shape, Glyph>], parent: Affine2D) -> Option<[f32; 2]> {
    for b in batches {
        if let Some(g) = b.glyphs.iter().find(|g| g.text == "Floating") {
            return Some(parent.point([g.bounds[0].round(), g.bounds[1].round()]));
        }
        for s in &b.shapes {
            if let Shape::Transformed(m, content, _) = s {
                if let Some(p) = text_origin(content, m.then(parent)) {
                    return Some(p);
                }
            }
        }
    }
    None
}

#[test]
fn ambient_float_preserves_subpixel_text_motion_at_high_refresh_and_fractional_dpi() {
    for scale in [1., 1.25, 1.5, 2., 3.] {
        let (mut r, mut p) = fixture(scale);
        let semantics = r.accessibility_nodes();
        let mut previous = None;
        for i in 0..8 {
            let t = Duration::from_secs_f64(f64::from(i) / 120.);
            r.advance_animations(t);
            let batches = r.paint_ui(&mut p, &UiTheme::light());
            let origin = text_origin(&batches, Affine2D::IDENTITY).unwrap();
            if let Some(last) = previous {
                let dx = origin[0] - last;
                assert!(
                    dx > 0.001 * scale && dx < 0.1 * scale,
                    "DPI {scale}, frame {i}: dx={dx}"
                );
            }
            previous = Some(origin[0]);
            assert_eq!(semantics, r.accessibility_nodes());
        }
        r.set_reduced_motion(true);
        assert_eq!(r.animation_wait(), None);
        let a = r.paint_ui(&mut p, &UiTheme::light());
        r.advance_animations(Duration::from_secs(20));
        let b = r.paint_ui(&mut p, &UiTheme::light());
        assert_eq!(
            text_origin(&a, Affine2D::IDENTITY),
            text_origin(&b, Affine2D::IDENTITY)
        );
    }
}

#[test]
fn ambient_clock_keeps_submillisecond_phase_and_long_uptime_precision() {
    let a = Ambient::float(6., 4.).period_ms(7000);
    assert!(a.turns(Duration::from_micros(500)) > a.turns(Duration::ZERO));
    let base = Duration::from_secs(7 * 1_000_000);
    assert!(
        (a.turns(base + Duration::from_micros(500)) - a.turns(Duration::from_micros(500))).abs()
            < 1e-7
    );
}

#[test]
fn fractional_floating_cards_keep_padding_edges_and_real_ancestor_clips() {
    for scale in [1., 1.25, 2.] {
        for clipped in [false, true] {
            let wrapper: Element<()> = vstack![vstack![]
                .key("card")
                .width(100.)
                .height(40.)
                .rounded(6.)
                .background("#ff0000")
                .ambient(Ambient::float(30., 0.).period_ms(4000))]
            .key("wrapper")
            .width(140.)
            .height(80.)
            .padding(20., Edge::All)
            .clip_contents(clipped)
            .into();
            let tree: Element<()> = vstack![wrapper]
                .width(240.)
                .height(140.)
                .padding(30., Edge::All)
                .into();
            let (d, h) = tree.build().unwrap().into_parts();
            let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
            let vp = Viewport::new(300. * scale, 180. * scale, scale).unwrap();
            let mut p = RasterPainter::new(vp);
            r.layout(vp, &Bindings::default(), &mut p).unwrap();
            r.advance_animations(Duration::from_millis(3000));
            let batches = r.paint_ui(&mut p, &UiTheme::light());
            let frame = p.rasterize(&batches, [0.; 4]).unwrap();
            assert!(
                frame
                    .pixel((35. * scale) as u32, (60. * scale) as u32)
                    .unwrap()
                    .alpha()
                    > 200,
                "padding must not clip a floating card: scale={scale}, clip={clipped}"
            );
            let outside_parent = frame
                .pixel((25. * scale) as u32, (60. * scale) as u32)
                .unwrap()
                .alpha();
            if clipped {
                assert_eq!(outside_parent, 0, "real clipping ancestor must stay fixed");
            } else {
                assert!(
                    outside_parent > 200,
                    "padding alone is not a clipping ancestor"
                );
            }
            assert_eq!(
                frame
                    .pixel((125. * scale) as u32, (60. * scale) as u32)
                    .unwrap()
                    .alpha(),
                0
            );
        }
    }
}

#[cfg(feature = "rendering-gpu")]
#[test]
#[ignore = "requires an actual GPU; subpixel floating text coverage, not a frame-rate benchmark"]
fn actual_gpu_ambient_text_changes_between_subpixel_frames_without_uploads() {
    let mut gpu = crate::rendering::GpuOffscreenRenderer::new().unwrap();
    for scale in [1., 1.25, 1.5, 2., 3.] {
        let (mut r, mut p) = fixture(scale);
        let mut last_gpu = None;
        let mut last_cpu = None;
        for t in [Duration::ZERO, Duration::from_secs_f64(1. / 120.)] {
            r.advance_animations(t);
            let batches = r.paint_ui(&mut p, &UiTheme::light());
            let cpu = p.rasterize(&batches, [0.; 4]).unwrap();
            let actual = gpu.render(&mut p, &batches, [0.; 4]).unwrap();
            assert!(cpu.pixels().iter().filter(|p| p.alpha() > 0).count() > 50);
            assert!(actual.pixels().iter().filter(|p| p.alpha() > 0).count() > 50);
            assert_eq!(gpu.last_over_budget(), None);
            assert_eq!(gpu.framebuffer_upload_bytes(), 0);
            let mean_alpha_difference = cpu
                .pixels()
                .iter()
                .zip(actual.pixels())
                .map(|(a, b)| f64::from(a.alpha().abs_diff(b.alpha())))
                .sum::<f64>()
                / cpu.pixels().len() as f64;
            assert!(
                mean_alpha_difference < 3.,
                "DPI {scale}: {mean_alpha_difference}"
            );
            if let Some(last) = last_gpu {
                assert!(
                    actual.pixels() != last,
                    "GPU text must not stick to a whole-pixel origin at DPI {scale}"
                );
            }
            if let Some(last) = last_cpu {
                assert!(
                    cpu.pixels() != last,
                    "software text must preserve fractional motion at DPI {scale}"
                );
            }
            last_gpu = Some(actual.pixels().to_vec());
            last_cpu = Some(cpu.pixels().to_vec());
        }
    }
}

struct LegacyPainter(Viewport);
impl crate::Painter for LegacyPainter {
    type Shape = ();
    type Glyph = [f32; 4];
    fn viewport(&self) -> Viewport {
        self.0
    }
    fn draw_rect(&self, _: &mut Vec<()>, _: f32, _: f32, _: f32, _: f32, _: [f32; 4]) {}
    fn card_triangle(&self, _: &mut Vec<()>, _: [[f32; 2]; 3], _: [f32; 4]) {}
    fn card_text(
        &mut self,
        _: &mut Vec<Self::Glyph>,
        _: &str,
        _: f32,
        _: f32,
        _: f32,
        _: [f32; 4],
    ) {
    }
    fn draw_text_colored(
        &mut self,
        _: &mut Vec<Self::Glyph>,
        _: &str,
        _: f32,
        _: f32,
        _: f32,
        _: f32,
        _: &dyn Fn(usize) -> [f32; 4],
    ) {
    }
}
impl crate::declarative::TextMetrics for LegacyPainter {
    fn measure(&mut self, _: &str, _: f32, _: f32) -> [f32; 2] {
        [80., 25.]
    }
}
impl crate::declarative::DocumentPainter for LegacyPainter {
    fn ui_text(
        &mut self,
        out: &mut Vec<Self::Glyph>,
        _: &str,
        bounds: [f32; 4],
        _: f32,
        _: [f32; 4],
    ) {
        out.push(bounds);
    }
}
#[test]
fn legacy_painters_keep_decorative_box_translation_without_new_hooks() {
    use crate::{declarative::DocumentPainter, Painter};
    for scale in [1., 1.25, 2.] {
        let (mut r, p) = fixture(scale);
        let original = r
            .boxes()
            .iter()
            .find(|b| b.id == "label")
            .unwrap()
            .bounds
            .bounds();
        let mut old = LegacyPainter(p.viewport());
        assert!(!old.ui_supports_transforms());
        r.advance_animations(Duration::from_millis(1750));
        let batches = r.paint_ui(&mut old, &UiTheme::light());
        let label = batches.iter().flat_map(|b| &b.glyphs).next().unwrap();
        assert!((label[0] - original[0] - 6. * scale).abs() < 0.001);
        assert!((label[1] - original[1]).abs() < 0.001);
    }
}
