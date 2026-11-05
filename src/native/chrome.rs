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
