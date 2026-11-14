#![cfg(feature = "rendering")]
use std::time::Duration;
use volt_ui_kit::{
    declarative::{Bindings, Handlers, Runtime, ValueKey},
    rendering::RasterPainter,
    ui::prelude::*,
    Viewport,
};
fn mount<C: 'static>(
    e: Element<C>,
    scale: f32,
    w: f32,
    h: f32,
) -> (Runtime, Handlers<'static, C>, RasterPainter) {
    let (d, f) = e.build().unwrap().into_parts();
    let mut r = Runtime::new(d, &f, &Bindings::default()).unwrap();
    let vp = Viewport::new(w * scale, h * scale, scale).unwrap();
    let mut p = RasterPainter::new(vp);
    r.layout(vp, &Bindings::default(), &mut p).unwrap();
    (r, f, p)
}
#[test]
fn blend_modes_and_mask_operations_have_distinct_pixel_semantics() {
    for (mode, expected) in [
        (BlendMode::Normal, [51, 153, 204]),
        (BlendMode::Multiply, [41, 61, 41]),
        (BlendMode::Screen, [214, 194, 214]),
        (BlendMode::Difference, [153, 51, 153]),
    ] {
        let tree: Element<()> = zstack![
            vstack![].width(100.).height(50.).background("#CC6633"),
            vstack![]
                .width(100.)
                .height(50.)
                .background("#3399CC")
                .blend_mode(mode)
        ]
        .width(100.)
        .height(50.)
        .isolate(true)
        .into();
        let (r, _, mut p) = mount(tree, 1., 120., 70.);
        let b = r.paint_ui(&mut p, &UiTheme::light());
        let f = p.rasterize(&b, [0.; 4]).unwrap();
        let c = f.pixel(20, 20).unwrap();
        for (a, b) in [c.red(), c.green(), c.blue()].into_iter().zip(expected) {
            assert!((i32::from(a) - b).abs() <= 2, "{mode:?}: {c:?}");
        }
    }
    let mask = |x: f32| -> Element<()> {
        canvas(move |c| c.fill(VectorPath::rectangle(x, 0., 60., 40.), "#FFFFFF"))
            .width(100.)
            .height(40.)
    };
    for (op, expected) in [
        (0, [true, true, true]),
        (1, [false, true, false]),
        (2, [true, false, false]),
        (3, [true, false, true]),
    ] {
        let m = match op {
            0 => mask_union([mask(0.), mask(40.)]),
            1 => mask_intersection(mask(0.), mask(40.)),
            2 => mask_difference(mask(0.), mask(40.)),
            _ => mask_exclude(mask(0.), mask(40.)),
        };
        let tree: Element<()> = zstack![vstack![].width(100.).height(40.).background("#FF0000")]
            .width(100.)
            .height(40.)
            .mask(m, MaskMode::Alpha)
            .into();
        let (r, _, mut p) = mount(tree, 1., 120., 60.);
        let b = r.paint_ui(&mut p, &UiTheme::light());
        let f = p.rasterize(&b, [0.; 4]).unwrap();
        for (x, visible) in [20, 50, 80].into_iter().zip(expected) {
            assert_eq!(
                f.pixel(x, 20).unwrap().alpha() > 200,
                visible,
                "operation {op}, x={x}"
            );
        }
    }
}
#[derive(Default)]
struct App {
    value: f32,
    calls: usize,
    slide: usize,
    open: bool,
}
#[test]
fn rotary_slider_uses_angular_pointer_and_existing_keyboard_semantics() {
    for scale in [1., 1.25, 2.] {
        let e: Element<App> = knob(0.5)
            .key("knob")
            .accessible_label("Gain")
            .step(0.1)
            .on_change(|a: &mut App, v| {
                a.value = v;
                a.calls += 1;
            })
            .into();
        let (mut r, mut h, mut p) = mount(e, scale, 200., 160.);
        let mut a = App::default();
        r.paint_ui(&mut p, &UiTheme::light());
        r.pointer_down_outcome(100. * scale, 56. * scale, &mut h, &mut a)
            .unwrap();
        assert!((a.value - 0.8).abs() < 0.001, "DPI {scale}: {}", a.value);
        r.pointer_up(100. * scale, 56. * scale, &mut h, &mut a)
            .unwrap();
        r.slider_key(ValueKey::Increase, &mut h, &mut a).unwrap();
        assert!((a.value - 0.9).abs() < 0.001);
        r.slider_key(ValueKey::Last, &mut h, &mut a).unwrap();
        assert_eq!(a.value, 1.);
        assert!(r
            .accessibility_nodes()
            .iter()
            .any(|n| n.id == "knob" && n.numeric.is_some()));
    }
}
fn carousel_view(index: usize) -> Element<App> {
    carousel(
        (0..3).map(|i| -> Element<App> {
            button(format!("Slide {i}"))
                .key(format!("slide-button-{i}"))
                .height(60.)
                .on_click(move |a: &mut App| {
                    a.calls += 1;
                    a.slide = i;
                })
                .into()
        }),
        index,
    )
    .key("carousel")
    .height(100.)
    .slide_animation(Animation::linear().duration_ms(200))
    .on_change(|a: &mut App, i| a.slide = i)
    .controls_below(true)
    .into()
}
#[test]
fn sliding_carousel_moves_input_with_paint_and_excludes_inactive_semantics() {
    for scale in [1., 1.25, 2.] {
        let (mut r, _, mut p) = mount(carousel_view(0), scale, 420., 220.);
        let mut a = App::default();
        r.paint_ui(&mut p, &UiTheme::light());
        assert!(r
            .accessibility_nodes()
            .iter()
            .any(|n| n.id == "slide-button-0"));
        assert!(!r
            .accessibility_nodes()
            .iter()
            .any(|n| n.id == "slide-button-1"));
        let (d, next) = carousel_view(1).build().unwrap().into_parts();
        r.replace_document(d, &next, &Bindings::default()).unwrap();
        let mut h = next;
        r.layout(
            Viewport::new(420. * scale, 220. * scale, scale).unwrap(),
            &Bindings::default(),
            &mut p,
        )
        .unwrap();
        r.paint_ui(&mut p, &UiTheme::light());
        r.advance_animations(Duration::from_millis(250));
        r.paint_ui(&mut p, &UiTheme::light());
        r.pointer_down(210. * scale, 30. * scale);
        r.pointer_up(210. * scale, 30. * scale, &mut h, &mut a)
            .unwrap();
        assert_eq!(a.calls, 1, "DPI {scale}");
        assert_eq!(a.slide, 1);
        assert!(!r
            .accessibility_nodes()
            .iter()
            .any(|n| n.id == "slide-button-0"));
    }
}
#[test]
fn viewport_reveal_waits_for_visibility_runs_once_and_respects_reduced_motion() {
    let view = |revision| -> Element<()> {
        vstack![
            spacer().height(320.).flex(0.),
            text("Reveal target")
                .key("target")
                .height(80.)
                .reveal_on_scroll(Transition::slide(0., 24.).duration_ms(400))
                .transition_revision(revision)
        ]
        .key("scroll")
        .height(140.)
        .scroll(Scroll::Vertical)
        .clip_contents(true)
        .into()
    };
    let (mut r, _, mut p) = mount(view(0), 1., 300., 140.);
    r.advance_animations(Duration::from_secs(10));
    assert!(!r.is_animating());
    r.scroll_at(40., 50., [0., 250.]);
    r.layout(
        Viewport::new(300., 140., 1.).unwrap(),
        &Bindings::default(),
        &mut p,
    )
    .unwrap();
    assert!(r.is_animating());
    assert!(r.accessibility_nodes().iter().any(|n| n.id == "target"));
    r.advance_animations(Duration::from_millis(10600));
    assert!(!r.is_animating());
    for d in [-250., 250.] {
        r.scroll_at(40., 50., [0., d]);
        r.layout(
            Viewport::new(300., 140., 1.).unwrap(),
            &Bindings::default(),
            &mut p,
        )
        .unwrap();
    }
    assert!(
        !r.is_animating(),
        "a seen key must not replay when scrolling back"
    );
    let (d, h) = view(1).build().unwrap().into_parts();
    r.replace_document(d, &h, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(300., 140., 1.).unwrap(),
        &Bindings::default(),
        &mut p,
    )
    .unwrap();
    assert!(r.is_animating());
    r.set_reduced_motion(true);
    assert_eq!(r.animation_wait(), None);
}
#[test]
fn hover_surface_can_pass_wheel_to_scroll_without_losing_pointer_events() {
    let e: Element<App> = vstack![
        vstack![text("Pointer")]
            .key("surface")
            .height(100.)
            .wheel_passthrough(true)
            .on_pointer(|a: &mut App, _| a.calls += 1),
        spacer().height(400.).flex(0.)
    ]
    .key("scroll")
    .height(150.)
    .scroll(Scroll::Vertical)
    .into();
    let (mut r, mut h, _) = mount(e, 1., 300., 150.);
    let mut a = App::default();
    r.pointer_move_outcome(50., 50., &mut h, &mut a).unwrap();
    assert!(a.calls > 0);
    let calls = a.calls;
    let out = r
        .scroll_outcome(50., 50., [0., 60.], &mut h, &mut a)
        .unwrap();
    assert!(out.changed);
    assert_eq!(calls, a.calls);
}
#[test]
fn invalid_dial_and_reveal_parameters_fail_before_rendering() {
    // New reveal bounds must not narrow the established shadow contract.
    assert!(
        Element::from(text::<()>("shadow").shadows([Shadow::new().offset(1024., 0.)]))
            .build()
            .is_ok()
    );
    assert!(
        Element::from(progress_indeterminate::<()>().dial_style(DialStyle::new()))
            .build()
            .is_err()
    );
    for style in [
        DialStyle::new().sweep(f32::NAN),
        DialStyle::new().sweep(0.),
        DialStyle::new().ticks(65),
        DialStyle::new().stroke_width(-1.),
    ] {
        assert!(Element::from(gauge::<()>(0.5).dial_style(style))
            .build()
            .is_err());
    }
    assert!(
        Element::from(text::<()>("invalid").reveal_on_scroll(Transition::slide(f32::NAN, 0.)))
            .build()
            .is_err()
    );
}

#[test]
fn inactive_carousel_portals_cannot_trap_the_active_page() {
    let slides = [
        button("Active")
            .key("active")
            .on_click(|a: &mut App| a.calls += 1)
            .into(),
        dialog(
            button("Hidden modal action")
                .key("hidden-action")
                .on_click(|a: &mut App| a.calls += 10),
        )
        .key("hidden-modal")
        .open(true)
        .on_dismiss(|_: &mut App| {})
        .into(),
    ];
    let e = carousel(slides, 0)
        .key("portal-carousel")
        .height(100.)
        .slide_animation(Animation::ease_out())
        .on_change(|_: &mut App, _| {})
        .into();
    let (mut r, mut h, _) = mount(e, 1., 400., 240.);
    let mut app = App::default();
    assert!(
        r.accessibility_activate("active", &mut h, &mut app)
            .unwrap()
            .activated
    );
    assert!(
        !r.accessibility_activate("hidden-action", &mut h, &mut app)
            .unwrap()
            .activated
    );
    assert_eq!(app.calls, 1);
    assert!(!r.boxes().iter().any(|b| b.id == "hidden-modal"));
}

fn dial_view(open: bool) -> Element<App> {
    speed_dial("quick actions", [(1, "New note"), (2, "Add waypoint")])
        .key("quick")
        .open(open)
        .on_open_change(|app: &mut App, open| app.open = open)
        .on_action(|app: &mut App, action| {
            app.slide = action;
            app.calls += 1;
        })
        .into()
}

#[test]
fn speed_dial_only_mounts_actions_when_open_and_closes_before_dispatch() {
    let (mut r, mut h, mut p) = mount(dial_view(false), 1., 300., 300.);
    let mut app = App::default();
    assert!(
        !r.dispatch_escape(&mut h, &mut app).unwrap(),
        "a closed speed dial must not consume Escape"
    );
    assert!(!r
        .accessibility_nodes()
        .iter()
        .any(|n| n.id == "quick-action-0"));
    let b = r
        .boxes()
        .iter()
        .find(|b| b.id == "quick-toggle")
        .unwrap()
        .bounds
        .bounds();
    r.pointer_down(b[0] + b[2] / 2., b[1] + b[3] / 2.);
    r.pointer_up(b[0] + b[2] / 2., b[1] + b[3] / 2., &mut h, &mut app)
        .unwrap();
    assert!(app.open);
    let (d, next) = dial_view(app.open).build().unwrap().into_parts();
    r.replace_document(d, &next, &Bindings::default()).unwrap();
    h = next;
    r.layout(
        Viewport::new(300., 300., 1.).unwrap(),
        &Bindings::default(),
        &mut p,
    )
    .unwrap();
    let b = r
        .boxes()
        .iter()
        .find(|b| b.id == "quick-action-1")
        .unwrap()
        .bounds
        .bounds();
    r.pointer_down(b[0] + b[2] / 2., b[1] + b[3] / 2.);
    r.pointer_up(b[0] + b[2] / 2., b[1] + b[3] / 2., &mut h, &mut app)
        .unwrap();
    assert!(!app.open);
    assert_eq!((app.slide, app.calls), (2, 1));
}

#[cfg(feature = "rendering-gpu")]
#[test]
#[ignore = "requires a real GPU; compositing pixel parity at fractional DPI"]
fn actual_gpu_blends_match_software_without_framebuffer_uploads() {
    let mut gpu = volt_ui_kit::rendering::GpuOffscreenRenderer::new().unwrap();
    for scale in [1., 1.25, 2.] {
        for mode in [
            BlendMode::Normal,
            BlendMode::Multiply,
            BlendMode::Screen,
            BlendMode::Overlay,
            BlendMode::ColorBurn,
            BlendMode::Difference,
            BlendMode::DestinationOut,
            BlendMode::Xor,
        ] {
            let e: Element<()> = zstack![
                vstack![].width(100.).height(60.).background("#CC9966"),
                vstack![]
                    .width(100.)
                    .height(60.)
                    .background("#80CCE6")
                    .opacity(0.7)
                    .blend_mode(mode),
            ]
            .width(100.)
            .height(60.)
            .isolate(true)
            .into();
            let (r, _, mut p) = mount(e, scale, 120., 80.);
            let batches = r.paint_ui(&mut p, &UiTheme::light());
            let cpu = p.rasterize(&batches, [0.; 4]).unwrap();
            let actual = gpu.render(&mut p, &batches, [0.; 4]).unwrap();
            assert_eq!(gpu.last_over_budget(), None);
            assert_eq!(gpu.framebuffer_upload_bytes(), 0);
            let a = cpu
                .pixel((30. * scale) as u32, (30. * scale) as u32)
                .unwrap();
            let b = actual
                .pixel((30. * scale) as u32, (30. * scale) as u32)
                .unwrap();
            for (a, b) in [a.red(), a.green(), a.blue(), a.alpha()].into_iter().zip([
                b.red(),
                b.green(),
                b.blue(),
                b.alpha(),
            ]) {
                assert!(a.abs_diff(b) <= 3, "{mode:?} at {scale}: {a} vs {b}");
            }
        }
    }
}
