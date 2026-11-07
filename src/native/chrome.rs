//! Window state and pure custom-frame hit testing. No platform resources here.
use super::WindowChrome;
use winit::window::ResizeDirection;

/// A snapshot reported by the native window, not an optimistic UI toggle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct WindowState {
    pub maximized: bool,
    pub fullscreen: bool,
    pub focused: bool,
    pub resizable: bool,
    pub chrome: WindowChrome,
}

/// Linux compositors need not round an undecorated surface for us. Keep the
/// client mask separate from OS decoration policy (macOS owns its own frame).
#[cfg(any(feature = "native-gpu", test))]
pub(super) fn client_corner_radius(
    chrome: WindowChrome,
    linux: bool,
    maximized: bool,
    fullscreen: bool,
) -> u32 {
    if linux
        && !maximized
        && !fullscreen
        && matches!(chrome, WindowChrome::Custom | WindowChrome::Borderless)
    {
        12
    } else {
        0
    }
}

#[cfg(feature = "native-gpu")]
pub(super) fn round_frame(
    batches: &mut Vec<crate::declarative::PaintBatch<super::raster::Shape, super::raster::Glyph>>,
    viewport: crate::Viewport,
    radius: u32,
    background: [f32; 4],
) -> [f32; 4] {
    if radius == 0 {
        return background;
    }
    use super::raster::Shape;
    use crate::{declarative::PaintBatch, graphics::ClipRegion, Rect};
    let bounds = [0., 0., viewport.width, viewport.height];
    let radius = (radius as f32 * viewport.scale)
        .min(viewport.width / 2.)
        .min(viewport.height / 2.);
    let clip = Rect::new(0., 0., viewport.width, viewport.height).expect("valid viewport");
    // The clear itself must be clipped too. Clear transparent, then paint the
    // original background *inside* one shared structural clip. Neither text nor
    // child controls are faded; no CPU framebuffer or extra opacity layer.
    batches.insert(
        0,
        PaintBatch {
            clip,
            shapes: vec![
                Shape::ClipBegin(ClipRegion::Rounded {
                    bounds,
                    radii: [radius; 4],
                }),
                Shape::Rectangle(bounds, background),
            ],
            glyphs: vec![],
        },
    );
    batches.push(PaintBatch {
        clip,
        shapes: vec![Shape::ClipEnd],
        glyphs: vec![],
    });
    [0.; 4]
}

pub(super) fn resize_hit(
    size: [u32; 2],
    point: [f32; 2],
    scale: f64,
    border: f32,
    enabled: bool,
) -> Option<ResizeDirection> {
    if !enabled || !scale.is_finite() || scale <= 0. || !border.is_finite() || border <= 0. {
        return None;
    }
    let [x, y] = point;
    let [w, h] = size.map(|v| v as f32);
    if !x.is_finite() || !y.is_finite() || x < 0. || y < 0. || x >= w || y >= h {
        return None;
    }
    let b = ((border * scale as f32).min(w / 2.)).min(h / 2.);
    let left = x < b;
    let right = x >= w - b;
    let top = y < b;
    let bottom = y >= h - b;
    use ResizeDirection::*;
    match (left, right, top, bottom) {
        (true, _, true, _) => Some(NorthWest),
        (_, true, true, _) => Some(NorthEast),
        (true, _, _, true) => Some(SouthWest),
        (_, true, _, true) => Some(SouthEast),
        (true, _, _, _) => Some(West),
        (_, true, _, _) => Some(East),
        (_, _, true, _) => Some(North),
        (_, _, _, true) => Some(South),
        _ => None,
    }
}
pub(super) fn resize_cursor(direction: ResizeDirection) -> winit::window::CursorIcon {
    use winit::window::{CursorIcon as C, ResizeDirection::*};
    match direction {
        North => C::NResize,
        South => C::SResize,
        East => C::EResize,
        West => C::WResize,
        NorthEast => C::NeResize,
        NorthWest => C::NwResize,
        SouthEast => C::SeResize,
        SouthWest => C::SwResize,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_floating_linux_client_frames_are_rounded() {
        for chrome in [
            WindowChrome::Native,
            WindowChrome::Overlay,
            WindowChrome::CompactOverlay,
            WindowChrome::Custom,
            WindowChrome::Borderless,
        ] {
            for linux in [false, true] {
                for maximized in [false, true] {
                    for fullscreen in [false, true] {
                        let expected = if linux
                            && !maximized
                            && !fullscreen
                            && matches!(chrome, WindowChrome::Custom | WindowChrome::Borderless)
                        {
                            12
                        } else {
                            0
                        };
                        assert_eq!(
                            client_corner_radius(chrome, linux, maximized, fullscreen),
                            expected
                        );
                    }
                }
            }
        }
    }

    #[cfg(feature = "native-gpu")]
    fn frame_fixture(
        scale: f32,
        background: [f32; 4],
    ) -> (
        super::super::RasterPainter,
        Vec<
            crate::declarative::PaintBatch<
                super::super::raster::Shape,
                super::super::raster::Glyph,
            >,
        >,
        [f32; 4],
    ) {
        use super::super::raster::Shape;
        let vp = crate::Viewport::new(100. * scale, 80. * scale, scale).unwrap();
        let clip = crate::Rect::new(0., 0., vp.width, vp.height).unwrap();
        // Two separately clipped content batches must remain under the frame
        // mask, including an opaque bottom row reaching the last corner.
        let mut batches = vec![
            crate::declarative::PaintBatch {
                clip,
                shapes: vec![Shape::Rectangle(
                    [30. * scale, 30. * scale, 20. * scale, 20. * scale],
                    [0., 1., 0., 1.],
                )],
                glyphs: vec![],
            },
            crate::declarative::PaintBatch {
                clip: crate::Rect::new(0., 60. * scale, vp.width, 20. * scale).unwrap(),
                shapes: vec![Shape::Rectangle(
                    [0., 60. * scale, vp.width, 20. * scale],
                    [1., 0., 0., 1.],
                )],
                glyphs: vec![],
            },
        ];
        let clear = round_frame(&mut batches, vp, 12, background);
        (super::super::RasterPainter::new(vp), batches, clear)
    }

    #[cfg(feature = "native-gpu")]
    fn assert_frame(p: &tiny_skia::Pixmap, scale: f32, alpha: f32) {
        let [w, h] = [p.width(), p.height()];
        for [x, y] in [[0, 0], [w - 1, 0], [0, h - 1], [w - 1, h - 1]] {
            assert_eq!(p.pixel(x, y).unwrap().alpha(), 0, "outer corner {x},{y}");
        }
        assert!(
            (i32::from(p.pixel(w / 2, 10).unwrap().alpha()) - (alpha * 255.).round() as i32).abs()
                <= 1
        );
        assert_eq!(
            p.pixel((35. * scale) as u32, (35. * scale) as u32)
                .unwrap()
                .alpha(),
            255,
            "content remains opaque"
        );
        assert_eq!(
            p.pixel(w / 2, h - 1).unwrap().alpha(),
            255,
            "bottom content reaches straight edge"
        );
        let antialiased = (0..(12. * scale) as u32).any(|y| {
            (0..(12. * scale) as u32).any(|x| {
                let a = p.pixel(x, y).unwrap().alpha();
                a > 0 && f32::from(a) < alpha * 255. - 1.
            })
        });
        assert!(antialiased, "rounded boundary is antialiased at {scale}x");
    }

    #[cfg(feature = "native-gpu")]
    #[test]
    fn frame_mask_clips_clear_and_multiple_content_batches_at_fractional_scale() {
        for scale in [1., 1.25, 2.] {
            for alpha in [0.45, 1.] {
                let (mut painter, batches, clear) = frame_fixture(scale, [0.2, 0.3, 0.4, alpha]);
                assert_eq!(clear, [0.; 4]);
                let pixels = painter.rasterize(&batches, clear).unwrap();
                assert_frame(&pixels, scale, alpha);
                let estimate =
                    super::super::gpu_scene::estimate_gpu_scene(&mut painter, &batches).unwrap();
                assert!(estimate.over_budget.is_none());
            }
        }
    }

    #[cfg(feature = "native-gpu")]
    #[test]
    #[ignore = "requires a real GPU; explicit local rendering validation"]
    fn rounded_frame_gpu_pixels() {
        let mut gpu = super::super::gpu::GpuOffscreenRenderer::new().unwrap();
        for scale in [1., 1.25, 2.] {
            for alpha in [0.45, 1.] {
                let (mut painter, batches, clear) = frame_fixture(scale, [0.2, 0.3, 0.4, alpha]);
                let pixels = gpu.render(&mut painter, &batches, clear).unwrap();
                assert_frame(&pixels, scale, alpha);
            }
        }
    }
    #[test]
    fn corners_edges_and_interior_at_fractional_scale() {
        use ResizeDirection::*;
        for scale in [1., 1.25, 2.] {
            for (p, expected) in [
                ([1., 1.], Some(NorthWest)),
                ([799., 1.], Some(NorthEast)),
                ([1., 599.], Some(SouthWest)),
                ([799., 599.], Some(SouthEast)),
                ([400., 1.], Some(North)),
                ([400., 599.], Some(South)),
                ([1., 300.], Some(West)),
                ([799., 300.], Some(East)),
                ([400., 300.], None),
            ] {
                assert_eq!(resize_hit([800, 600], p, scale, 6., true), expected);
            }
        }
        assert_eq!(
            resize_hit([800, 600], [7., 300.], 1.25, 6., true),
            Some(West)
        );
        assert_eq!(resize_hit([800, 600], [8., 300.], 1.25, 6., true), None);
    }
    #[test]
    fn native_maximized_fullscreen_disabled_and_invalid_inputs_do_not_hit() {
        for (p, scale, border, enabled) in [
            ([1., 1.], 1., 6., false),
            ([1., 1.], 1., 0., true),
            ([1., 1.], 0., 6., true),
            ([f32::NAN, 1.], 1., 6., true),
            ([-1., 1.], 1., 6., true),
            ([800., 1.], 1., 6., true),
            ([1., 1.], 1., f32::NAN, true),
        ] {
            assert_eq!(resize_hit([800, 600], p, scale, border, enabled), None);
        }
    }
}
