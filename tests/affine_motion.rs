#![cfg(feature = "rendering")]
use volt_ui_kit::{
    declarative::{Bindings, Runtime},
    rendering::RasterPainter,
    ui::prelude::*,
    Painter, UiTheme, Viewport,
};
fn fixture(
    scale: f32,
) -> (
    Runtime,
    volt_ui_kit::declarative::Handlers<'static, usize>,
    RasterPainter,
) {
    let tree: Element<usize> = vstack![vstack![button("Target")
        .key("target")
        .height(40.)
        .fill_width()
        .on_click(|n: &mut usize| *n += 1)]
    .key("rotated")
    .width(120.)
    .height(40.)
    .rotate(90.)]
    .padding(40., Edge::All)
    .into();
    let (doc, handlers) = tree.build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
    let viewport = Viewport::new(300. * scale, 220. * scale, scale).unwrap();
    let mut p = RasterPainter::new(viewport);
    r.layout(viewport, &Bindings::default(), &mut p).unwrap();
    (r, handlers, p)
}
#[test]
fn rotated_children_activate_at_the_painted_location_at_fractional_dpi() {
    for scale in [1., 1.25, 1.5, 2.] {
        let (mut r, mut handlers, mut p) = fixture(scale);

        let batches = r.paint_ui(&mut p, &UiTheme::dark());
        let frame = p.rasterize(&batches, [0.; 4]).unwrap();
        assert!(frame.pixels().iter().any(|v| v.alpha() > 0));
        let mut count = 0;
        r.pointer_down(100. * scale, 100. * scale);
        r.pointer_up(100. * scale, 100. * scale, &mut handlers, &mut count)
            .unwrap();
        assert_eq!(count, 1, "DPI {scale}");
        r.pointer_down(45. * scale, 65. * scale);
        r.pointer_up(45. * scale, 65. * scale, &mut handlers, &mut count)
            .unwrap();
        assert_eq!(count, 1, "untransformed position must not activate");
    }
}
#[test]
fn invalid_transform_values_are_rejected_before_rendering() {
    for transform in [
        Transform2D::new().scale(f32::INFINITY, 1.),
        Transform2D::new().rotate(f32::NAN),
    ] {
        let tree: Element<()> = vstack![text("x")].transform(transform).into();
        assert!(tree.build().is_err());
    }
}

#[cfg(feature = "rendering-gpu")]
#[test]
#[ignore = "requires a real GPU; fractional affine/text/clipping acceptance"]
fn actual_gpu_affine_and_software_keep_coverage_at_fractional_dpi() {
    let mut gpu = volt_ui_kit::rendering::GpuOffscreenRenderer::new().unwrap();
    for scale in [1., 1.25, 1.5, 2., 3.] {
        let (r, _, mut p) = fixture(scale);
        let batches = r.paint_ui(&mut p, &UiTheme::dark());
        let cpu = p.rasterize(&batches, [0.; 4]).unwrap();
        let actual = gpu.render(&mut p, &batches, [0.; 4]).unwrap();
        assert_eq!(gpu.last_over_budget(), None);
        assert_eq!(gpu.framebuffer_upload_bytes(), 0);
        let delta = cpu
            .pixels()
            .iter()
            .zip(actual.pixels())
            .map(|(a, b)| a.alpha().abs_diff(b.alpha()) as u64)
            .sum::<u64>() as f64
            / cpu.pixels().len() as f64;
        assert!(delta < 4., "coverage mismatch {delta} at {scale}");
        assert!(
            actual
                .pixel((100. * scale) as u32, (100. * scale) as u32)
                .unwrap()
                .alpha()
                > 0
        );
        assert_eq!(
            actual
                .pixel((45. * scale) as u32, (65. * scale) as u32)
                .unwrap()
                .alpha(),
            0
        );
    }
}
fn geometry_view(left: f32, key: &str, shared: bool) -> Element<usize> {
    let mut item = vstack![button("Move")
        .key(format!("{key}-button"))
        .height(40.)
        .fill_width()
        .on_click(|n: &mut usize| *n += 1)]
    .key(key)
    .width(80.)
    .height(40.)
    .layout_animation(Animation::linear().duration_ms(1000));
    if shared {
        item = item.shared_element("selection");
    }
    vstack![item]
        .key("root")
        .padding(left, Edge::Left)
        .padding(40., Edge::Top)
        .into()
}
#[test]
fn automatic_layout_and_shared_geometry_preserve_interrupted_positions() {
    for shared in [false, true] {
        let mut p = RasterPainter::new(Viewport::new(400., 220., 1.).unwrap());
        let (doc, handlers) = geometry_view(40., "first", shared)
            .build()
            .unwrap()
            .into_parts();
        let mut r = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
        r.layout(p.viewport(), &Bindings::default(), &mut p)
            .unwrap();
        r.paint_ui(&mut p, &UiTheme::dark());
        let key = if shared { "second" } else { "first" };
        let (doc, h) = geometry_view(140., key, shared)
            .build()
            .unwrap()
            .into_parts();
        r.replace_document(doc, &h, &Bindings::default()).unwrap();
        r.layout(p.viewport(), &Bindings::default(), &mut p)
            .unwrap();
        r.advance_animations(std::time::Duration::from_millis(500));
        r.paint_ui(&mut p, &UiTheme::dark());
        let node = r
            .accessibility_nodes()
            .into_iter()
            .find(|n| n.id == format!("{key}-button"))
            .unwrap();
        assert!((node.bounds[0] - 90.).abs() < 0.05, "{:?}", node.bounds);
        let (doc, h) = geometry_view(40., key, shared)
            .build()
            .unwrap()
            .into_parts();
        r.replace_document(doc, &h, &Bindings::default()).unwrap();
        r.layout(p.viewport(), &Bindings::default(), &mut p)
            .unwrap();
        let node = r
            .accessibility_nodes()
            .into_iter()
            .find(|n| n.id == format!("{key}-button"))
            .unwrap();
        assert!((node.bounds[0] - 90.).abs() < 0.05, "interruption jumped");
        r.advance_animations(std::time::Duration::from_millis(1500));
        assert!(!r.is_animating());
    }
}
#[test]
fn declarative_timeline_moves_paint_and_input_and_reduced_motion_settles() {
    let tree: Element<usize> = vstack![button("Track")
        .key("track")
        .width(80.)
        .height(40.)
        .on_click(|n: &mut usize| *n += 1)
        .timeline(
            Timeline::new(1000)
                .keyframe(0., MotionPose::new())
                .keyframe(1., MotionPose::new().translate(120., 0.))
        )]
    .key("root")
    .padding(20., Edge::All)
    .into();
    let (doc, mut h) = tree.build().unwrap().into_parts();
    let mut p = RasterPainter::new(Viewport::new(300., 160., 1.).unwrap());
    let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    r.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
    r.advance_animations(std::time::Duration::from_millis(500));
    r.paint_ui(&mut p, &UiTheme::dark());
    let mut n = 0;
    r.pointer_down(100., 40.);
    r.pointer_up(100., 40., &mut h, &mut n).unwrap();
    assert_eq!(n, 1);
    let node = r
        .accessibility_nodes()
        .into_iter()
        .find(|n| n.id == "track")
        .unwrap();
    assert!((node.bounds[0] - 80.).abs() < 0.05);
    r.set_reduced_motion(true);
    assert!(!r.is_animating());
    let node = r
        .accessibility_nodes()
        .into_iter()
        .find(|n| n.id == "track")
        .unwrap();
    assert!((node.bounds[0] - 140.).abs() < 0.05);
}
#[test]
fn declarative_drag_has_bounded_frame_independent_momentum() {
    let tree: Element<usize> = vstack![vstack![text("Drag")]
        .key("drag")
        .width(80.)
        .height(40.)
        .background("#F04040")
        .drag_motion(DragMotion::new().bounds([0., 0.], [200., 0.]))]
    .key("root")
    .padding(20., Edge::All)
    .into();
    let (doc, mut h) = tree.build().unwrap().into_parts();
    let mut p = RasterPainter::new(Viewport::new(400., 180., 1.).unwrap());
    let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    r.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
    let mut ctx = 0;
    r.pointer_down_outcome(40., 40., &mut h, &mut ctx).unwrap();
    r.advance_animations(std::time::Duration::from_millis(50));
    r.pointer_move_outcome(100., 40., &mut h, &mut ctx).unwrap();
    r.pointer_up_outcome(100., 40., &mut h, &mut ctx).unwrap();
    assert!(r.is_animating());
    r.advance_animations(std::time::Duration::from_millis(500));
    let image = r.paint_ui(&mut p, &UiTheme::dark());
    assert!(p.rasterize(&image, [0.; 4]).is_ok());
    r.advance_animations(std::time::Duration::from_secs(10));
    assert!(!r.is_animating());
    r.set_reduced_motion(true);
    assert_eq!(r.animation_wait(), None);
}
#[test]
fn shared_ids_are_unique_and_invalid_timelines_are_rejected() {
    let tree: Element<()> = vstack![
        text("one").shared_element("same"),
        text("two").shared_element("same")
    ]
    .into();
    assert!(tree.build().is_err());
    let tree: Element<()> = text("x")
        .timeline(
            Timeline::new(1000)
                .keyframe(0., MotionPose::new())
                .keyframe(0., MotionPose::new()),
        )
        .into();
    assert!(tree.build().is_err());
}

#[test]
fn nested_layout_tracks_do_not_double_apply_parent_motion_and_reset_on_dpi_change() {
    fn view(shift: f32, padding: f32) -> Element<()> {
        vstack![vstack![button("Child")
            .on_click(|_: &mut ()| {})
            .key("child")
            .width(50.)
            .height(30.)
            .layout_animation(Animation::linear().duration_ms(1000))]
        .key("parent")
        .width(150. + padding * 2.)
        .height(80.)
        .padding(padding, Edge::Left)
        .layout_animation(Animation::linear().duration_ms(1000))]
        .key("root")
        .padding(shift, Edge::Left)
        .padding(20., Edge::Top)
        .into()
    }
    let vp = Viewport::new(500., 200., 1.25).unwrap();
    let mut p = RasterPainter::new(vp);
    let (doc, h) = view(20., 10.).build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    r.layout(vp, &Bindings::default(), &mut p).unwrap();
    r.paint_ui(&mut p, &UiTheme::dark());
    let from = r
        .accessibility_nodes()
        .into_iter()
        .find(|n| n.id == "child")
        .unwrap()
        .bounds;
    let (doc, h) = view(120., 30.).build().unwrap().into_parts();
    r.replace_document(doc, &h, &Bindings::default()).unwrap();
    r.layout(vp, &Bindings::default(), &mut p).unwrap();
    r.advance_animations(std::time::Duration::from_millis(500));
    r.paint_ui(&mut p, &UiTheme::dark());
    let halfway = r
        .accessibility_nodes()
        .into_iter()
        .find(|n| n.id == "child")
        .unwrap()
        .bounds;
    assert!(
        (halfway[0] - (from[0] + 60. * vp.scale)).abs() < 0.1,
        "{from:?} -> {halfway:?}"
    );
    assert!(
        (halfway[2] - 50. * vp.scale).abs() < 0.1,
        "parent scale must not resize an independently tracked child"
    );
    let changed = Viewport::new(800., 400., 2.).unwrap();
    p = RasterPainter::new(changed);
    r.layout(changed, &Bindings::default(), &mut p).unwrap();
    let bounds = r
        .accessibility_nodes()
        .into_iter()
        .find(|n| n.id == "child")
        .unwrap()
        .bounds;
    assert!(
        (bounds[0] - 300.).abs() < 0.1,
        "DPI change must not animate from stale physical coordinates: {bounds:?}"
    );
    assert_eq!(r.animation_wait(), None);
}

#[test]
fn rotated_drag_callbacks_report_local_coordinates_and_zero_scale_cannot_capture() {
    for factor in [1.25, 2.] {
        let vp = Viewport::new(400. * factor, 240. * factor, factor).unwrap();
        let mut p = RasterPainter::new(vp);
        let tree: Element<Vec<volt_ui_kit::ui::Pointer>> = vstack![vstack![]
            .key("surface")
            .width(120.)
            .height(40.)
            .rotate(90.)
            .on_pointer(|events: &mut Vec<_>, e| events.push(e))]
        .padding(40., Edge::All)
        .into();
        let (doc, mut h) = tree.build().unwrap().into_parts();
        let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
        r.layout(vp, &Bindings::default(), &mut p).unwrap();
        r.paint_ui(&mut p, &UiTheme::dark());
        let mut events = vec![];
        r.pointer_down_outcome(100. * factor, 100. * factor, &mut h, &mut events)
            .unwrap();
        r.pointer_move_outcome(100. * factor, 110. * factor, &mut h, &mut events)
            .unwrap();
        r.pointer_up_outcome(100. * factor, 110. * factor, &mut h, &mut events)
            .unwrap();
        assert_eq!(events.len(), 3);
        assert!((events[0].at[0] - 100.).abs() < 0.1);
        assert!((events[1].offset[0] - 10.).abs() < 0.1);
        assert!(events[1].offset[1].abs() < 0.1);
    }
    let tree: Element<usize> = button("Hidden")
        .key("hidden")
        .width(100.)
        .height(40.)
        .scale(0., 1.)
        .on_click(|n: &mut usize| *n += 1)
        .into();
    let (doc, mut h) = tree.build().unwrap().into_parts();
    let mut p = RasterPainter::new(Viewport::new(400., 200., 1.).unwrap());
    let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    r.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
    let mut count = 0;
    r.pointer_down(50., 20.);
    r.pointer_up(50., 20., &mut h, &mut count).unwrap();
    assert_eq!(count, 0);
    let batches = r.paint_ui(&mut p, &UiTheme::dark());
    assert!(p
        .rasterize(&batches, [0.; 4])
        .unwrap()
        .pixels()
        .iter()
        .all(|p| p.alpha() == 0));
}

#[test]
fn automatic_drag_survives_host_recomposition_without_reporting_a_spurious_callback() {
    fn view() -> Element<()> {
        vstack![vstack![text("Drag")]
            .key("card")
            .width(100.)
            .height(50.)
            .drag_motion(DragMotion::new().bounds([0., 0.], [150., 0.]))
            .accessible_label("Draggable card")]
        .key("root")
        .padding(20., Edge::All)
        .into()
    }
    let (d, mut h) = view().build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    let mut p = RasterPainter::new(Viewport::new(500., 200., 1.25).unwrap());
    r.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
    let press = r.pointer_down_outcome(50., 50., &mut h, &mut ()).unwrap();
    assert!(!press.activated, "no application callback ran");
    let (d, next) = view().build().unwrap().into_parts();
    r.replace_document(d, &next, &Bindings::default()).unwrap();
    h = next;
    r.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
    assert!(
        r.pointer_dragging(),
        "stable drag policy must preserve capture"
    );
    r.advance_animations(std::time::Duration::from_millis(50));
    let moved = r.pointer_move_outcome(150., 50., &mut h, &mut ()).unwrap();
    assert!(moved.changed && !moved.activated);
    let card = r
        .accessibility_nodes()
        .into_iter()
        .find(|n| n.id == "card")
        .unwrap();
    assert!((card.bounds[0] - 125.).abs() < 0.1, "{:?}", card.bounds);
    r.pointer_up_outcome(150., 50., &mut h, &mut ()).unwrap();
    assert!(r.is_animating());
}

#[test]
fn wheel_scrolling_is_not_reinterpreted_as_a_layout_tween() {
    let tree: Element<()> = vstack![
        spacer().height(100.),
        text("Card")
            .key("card")
            .height(40.)
            .layout_animation(Animation::ease_out().duration_ms(320)),
        spacer().height(300.)
    ]
    .key("scroll")
    .height(160.)
    .scroll(Scroll::Vertical)
    .into();
    let (doc, h) = tree.build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    let mut p = RasterPainter::new(Viewport::new(400., 200., 1.).unwrap());
    r.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
    r.paint_ui(&mut p, &UiTheme::dark());
    assert!(r.scroll_at(20., 20., [0., 50.]));
    r.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
    let card = r
        .accessibility_nodes()
        .into_iter()
        .find(|n| n.id == "card")
        .unwrap();
    assert!((card.bounds[1] - 50.).abs() < 0.1, "{card:?}");
    assert_eq!(r.animation_wait(), None);
}

#[cfg(feature = "rendering-gpu")]
#[test]
#[ignore = "requires a real GPU; oblique, magnified vector edge and text acceptance"]
fn oblique_nonuniform_transform_preserves_antialiased_edges_and_coverage() {
    let mut gpu = volt_ui_kit::rendering::GpuOffscreenRenderer::new().unwrap();
    for scale in [1., 1.25, 1.5, 2., 3.] {
        let tree: Element<()> = vstack![vstack![text("Sharp vector text").font_size(17.)]
            .key("surface")
            .width(150.)
            .height(65.)
            .padding(8., Edge::All)
            .background("#4080E0")
            .rounded(14.)
            .clip_contents(true)
            .transform(Transform2D::new().rotate(23.).scale(1.37, 0.83))]
        .padding(70., Edge::All)
        .into();
        let (doc, h) = tree.build().unwrap().into_parts();
        let vp = Viewport::new(350. * scale, 250. * scale, scale).unwrap();
        let mut p = RasterPainter::new(vp);
        let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
        r.layout(vp, &Bindings::default(), &mut p).unwrap();
        let batches = r.paint_ui(&mut p, &UiTheme::dark());
        let cpu = p.rasterize(&batches, [0.; 4]).unwrap();
        let actual = gpu.render(&mut p, &batches, [0.; 4]).unwrap();
        assert_eq!(gpu.last_over_budget(), None);
        assert_eq!(gpu.framebuffer_upload_bytes(), 0);
        assert!(
            actual
                .pixels()
                .iter()
                .filter(|p| p.alpha() > 0 && p.alpha() < 255)
                .count()
                > (80. * scale) as usize,
            "edges must retain fractional coverage at {scale}x"
        );
        let mad = cpu
            .pixels()
            .iter()
            .zip(actual.pixels())
            .map(|(a, b)| a.alpha().abs_diff(b.alpha()) as f64)
            .sum::<f64>()
            / cpu.pixels().len() as f64;
        assert!(mad < 4., "coverage difference {mad} at {scale}x");
    }
}

#[test]
fn automatic_drag_of_a_rotated_card_follows_the_pointer_in_world_space() {
    let tree: Element<()> = vstack![vstack![]
        .key("card")
        .accessible_label("Card")
        .width(120.)
        .height(40.)
        .rotate(90.)
        .drag_motion(
            DragMotion::new()
                .bounds([-200.; 2], [200.; 2])
                .momentum(false)
        )]
    .key("root")
    .padding(40., Edge::All)
    .into();
    let (d, mut h) = tree.build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    let mut p = RasterPainter::new(Viewport::new(400., 300., 1.).unwrap());
    r.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
    let before = r
        .accessibility_nodes()
        .into_iter()
        .find(|n| n.id == "card")
        .unwrap()
        .bounds;
    r.pointer_down_outcome(100., 100., &mut h, &mut ()).unwrap();
    r.pointer_move_outcome(100., 160., &mut h, &mut ()).unwrap();
    let after = r
        .accessibility_nodes()
        .into_iter()
        .find(|n| n.id == "card")
        .unwrap()
        .bounds;
    assert!(
        (after[0] - before[0]).abs() < 0.1,
        "{before:?} -> {after:?}"
    );
    assert!(
        (after[1] - before[1] - 60.).abs() < 0.1,
        "{before:?} -> {after:?}"
    );
    r.pointer_up_outcome(100., 160., &mut h, &mut ()).unwrap();
    assert!(!r.is_animating());
}

#[test]
fn transformed_editors_report_world_space_ime_and_selection_bounds_at_fractional_dpi() {
    type App = Option<volt_ui_kit::declarative::TextSelection>;
    fn view(pose: Transform2D) -> Element<App> {
        vstack![text_field("Native text")
            .key("entry")
            .autofocus(true)
            .on_change(|_: &mut App, _| {})
            .on_selection(|a: &mut App, value| *a = Some(value))
            .width(140.)
            .height(40.)
            .transform(pose)]
        .padding(70., Edge::All)
        .into()
    }
    for dpi in [1., 1.25, 1.5, 2.] {
        let vp = Viewport::new(400. * dpi, 250. * dpi, dpi).unwrap();
        let mut p = RasterPainter::new(vp);
        let (doc, mut h) = view(Transform2D::new()).build().unwrap().into_parts();
        let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
        r.layout(vp, &Bindings::default(), &mut p).unwrap();
        let base = r.editor_candidate_bounds().unwrap();
        let b = r
            .boxes()
            .iter()
            .find(|b| b.id == "entry")
            .unwrap()
            .bounds
            .bounds();
        let transform = Transform2D::new()
            .rotate(23.)
            .scale(1.2, 0.9)
            .translate(12., -4.);
        let matrix = transform.matrix(b.map(|v| v / dpi), dpi);
        let (doc, next) = view(transform).build().unwrap().into_parts();
        r.replace_document(doc, &next, &Bindings::default())
            .unwrap();
        h = next;
        r.layout(vp, &Bindings::default(), &mut p).unwrap();
        let expected = matrix.bounds(base);
        let actual = r.editor_candidate_bounds().unwrap();
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!((actual - expected).abs() < 0.1);
        }
        let mut report = None;
        r.dispatch_editor_selections(&mut h, &mut report).unwrap();
        let report = report.unwrap();
        for (caret, expected) in report.caret.into_iter().zip(expected) {
            assert!((caret * dpi - expected).abs() < 0.1);
        }
    }
}

#[test]
fn culling_preserves_an_offscreen_leaf_transformed_into_view_through_its_ancestors() {
    let tree: Element<usize> = vstack![
        text("Top").height(30.),
        spacer().height(260.),
        vstack![button("Moved into view")
            .key("moved")
            .width(150.)
            .height(40.)
            .transform(Transform2D::new().translate(0., -240.))
            .on_click(|n: &mut usize| *n += 1)]
        .key("offscreen-parent")
        .height(50.)
    ]
    .key("scroll")
    .height(180.)
    .scroll(Scroll::Vertical)
    .into();
    let (d, mut h) = tree.build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    let vp = Viewport::new(400., 180., 1.).unwrap();
    let mut p = RasterPainter::new(vp);
    r.layout(vp, &Bindings::default(), &mut p).unwrap();
    let batches = r.paint_ui(&mut p, &UiTheme::dark());
    let frame = p.rasterize(&batches, [0.; 4]).unwrap();
    assert!(frame.pixel(30, 65).unwrap().alpha() > 0);
    let mut count = 0;
    r.pointer_down(30., 65.);
    r.pointer_up(30., 65., &mut h, &mut count).unwrap();
    assert_eq!(
        count, 1,
        "keep moving leaf and its offscreen ancestor groups"
    );
}
