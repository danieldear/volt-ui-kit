//! Product-like recipes using public toolkit capabilities, also compiled to WASM.
use super::{
    pages::{Doc, Section},
    App,
};
use volt_ui_kit::ui::prelude::*;
#[derive(Clone)]
pub struct State {
    pub slide: usize,
    pub sheet: bool,
    pub dial_open: bool,
    pub expanded: bool,
    pub pointer: [f32; 2],
    pub tracking: bool,
    pub reveals: u64,
    pub temperature: f32,
    pub battery: f32,
    pub blend: BlendMode,
    pub action: String,
}
impl Default for State {
    fn default() -> Self {
        Self {
            slide: 0,
            sheet: false,
            dial_open: false,
            expanded: false,
            pointer: [240., 120.],
            tracking: false,
            reveals: 0,
            temperature: 21.5,
            battery: 0.72,
            blend: BlendMode::Overlay,
            action: "No action yet".into(),
        }
    }
}
fn muted(s: impl Into<String>) -> volt_ui_kit::ui::Text<App> {
    text(s).font_size(13.).color(ColorToken::Muted)
}
fn section(
    id: &'static str,
    title: &'static str,
    note: &'static str,
    demo: impl Into<Element<App>>,
    code: &'static str,
) -> Section {
    Section {
        id,
        title,
        note,
        demo: demo.into(),
        code,
    }
}
fn panel(body: impl Into<Element<App>>) -> Element<App> {
    vstack![body.into()]
        .padding(24., Edge::All)
        .background(ColorToken::Base200)
        .rounded(18.)
        .width_fraction(1.)
        .max_width(640.)
        .font_family("SF")
        .into()
}
fn route_slide(app: &App, title: &str, detail: &str, tint: &str) -> Element<App> {
    zstack![
        vstack![]
            .background_image(app.image.clone().background().fit(ImageFit::Cover))
            .width_fraction(1.)
            .height(240.),
        vstack![]
            .background_gradient(Gradient::linear(
                0.,
                [(0., "#081624F0"), (0.6, "#08162400"), (1., tint)]
            ))
            .height(240.)
            .width_fraction(1.),
        vstack![
            badge("Field collection")
                .background("#FFFFFF24")
                .color("#FFFFFF")
                .font_size(11.),
            spacer(),
            text(title).font_size(29.).font_weight(600).color("#FFFFFF"),
            text(detail).font_size(14.).color("#FFFFFFDA")
        ]
        .gap(8., Axis::Vertical)
        .padding(24., Edge::All)
        .height(240.)
        .width_fraction(1.),
    ]
    .height(240.)
    .rounded(16.)
    .clip_contents(true)
    .into()
}
fn carousel_demo(app: &App) -> Section {
    let slides = [
        route_slide(
            app,
            "The quiet coast",
            "Find your next stopping point.",
            "#28A8D233",
        ),
        route_slide(
            app,
            "Beyond the ridge",
            "A little further from the familiar.",
            "#E7A43655",
        ),
        route_slide(
            app,
            "After the rain",
            "Fresh air. A clear way forward.",
            "#786FE255",
        ),
    ];
    section("studio-carousel","A carousel with real page motion","Retained pages travel by the resolved viewport width. Buttons, focus and hit targets follow the same pose; no autoplay timer.",
        vstack![carousel(slides,app.gallery.slide).key("studio-carousel-control").height(240.)
            .slide_animation(Animation::ease_out().duration_ms(380)).controls_below(true)
            .on_change(|a:&mut App,i|a.gallery.slide=i),muted("Use the arrows or choose a position marker.")]
            .gap(14.,Axis::Vertical).width_fraction(1.).max_width(640.),
        r#"carousel(slides, app.slide).key("journey").height(240.)
    .slide_animation(Animation::ease_out().duration_ms(380))
    .controls_below(true).on_change(|app, i| app.slide = i)"#)
}
fn sheet_demo(app: &App) -> Section {
    let sheet = bottom_sheet(
        vstack![
            hstack![
                text("Trail journal").font_size(18.).font_weight(600),
                spacer(),
                text("$48.00").font_family("SF Mono")
            ],
            muted("A local checkout layout. This example never sends a payment."),
            divider(),
            hstack![text("Pocket notebook × 2"), spacer(), text("$36.00")],
            hstack![text("Shipping"), spacer(), text("$12.00")],
            button("Confirm preview")
                .key("studio-sheet-confirm")
                .height(48.)
                .variant(ButtonVariant::Primary)
                .on_click(|a: &mut App| {
                    a.gallery.action = "Preview confirmed".into();
                    a.gallery.sheet = false;
                }),
        ]
        .gap(18., Axis::Vertical)
        .max_width(560.)
        .width_fraction(1.),
    )
    .key("studio-sheet")
    .title("Review your order")
    .size(340.)
    .open(app.gallery.sheet)
    .on_dismiss(|a: &mut App| a.gallery.sheet = false);
    section("studio-sheet","Bottom sheets, not another window","A real modal: focus containment, Escape, backdrop dismissal and a scrollable body reuse the drawer contract.",
        vstack![panel(hstack![vstack![text("Ready when you are").font_size(22.).font_weight(600),muted("Keep context behind a focused action.")].gap(8.,Axis::Vertical),
            spacer(),button("Review order").key("studio-sheet-open").height(44.).variant(ButtonVariant::Primary).on_click(|a:&mut App|a.gallery.sheet=true)].gap(16.,Axis::Horizontal).wrap(true)),sheet],
        r#"bottom_sheet(order_form()).key("checkout").title("Review your order")
    .size(340.).open(app.checkout_open)
    .on_dismiss(|app| app.checkout_open = false)"#)
}
fn fab_demo(app: &App) -> Section {
    let quick = speed_dial(
        "quick actions",
        [(0, "New note"), (1, "Add waypoint"), (2, "Attach photo")],
    )
    .key("studio-speed-dial")
    .open(app.gallery.dial_open)
    .gap(10.)
    .on_open_change(|a: &mut App, v| a.gallery.dial_open = v)
    .on_action(|a: &mut App, v| {
        a.gallery.action = ["Note created", "Waypoint added", "Photo action selected"][v].into()
    });
    section("studio-fab","Quick actions within reach","A controlled, non-modal speed dial. Touch-sized buttons, staggered entrances, explicit close and Escape; no hidden side effects.",
        panel(zstack![vstack![text("Field notes").font_size(24.).font_weight(600),muted(app.gallery.action.clone())]
            .gap(10.,Axis::Vertical).width_fraction(1.).height(300.).padding(8.,Edge::All),
            vstack![quick].flow(Flow::BottomRight).height(300.).width_fraction(1.)].height(300.)),
        r#"speed_dial("quick actions", [(Action::Note, "New note"), (Action::Pin, "Add waypoint")])
    .key("quick-actions").open(app.quick_open)
    .on_open_change(|app, open| app.quick_open = open)
    .on_action(|app, action| app.perform(action))"#)
}
fn card_demo(app: &App) -> Section {
    let expanded = app.gallery.expanded;
    let mut contents = vec![
        hstack![
            text("Camera kit").font_size(21.).font_weight(600),
            spacer(),
            badge(if expanded { "Ready" } else { "Packed" })
        ]
        .align(Align::Center)
        .into(),
        muted("Everything you need. Nothing in the way.").into(),
    ];
    if expanded {
        contents.push(
            vstack![
                text("✓  Battery charged"),
                text("✓  Memory card checked"),
                text("✓  Weather seal ready")
            ]
            .key("studio-card-details")
            .gap(10., Axis::Vertical)
            .transition(Transition::slide(0., 18.).duration_ms(260))
            .into(),
        );
    }
    contents.push(
        button(if expanded {
            "Close checklist"
        } else {
            "Reveal checklist"
        })
        .key("studio-card-reveal")
        .height(44.)
        .variant(ButtonVariant::Outline)
        .on_click(|a: &mut App| a.gallery.expanded = !a.gallery.expanded)
        .into(),
    );
    let card = vstack(contents)
        .key("studio-reveal-card")
        .padding(24., Edge::All)
        .gap(16., Axis::Vertical)
        .width_fraction(1.)
        .max_width(440.)
        .height(if expanded { 294. } else { 180. })
        .background(ColorToken::Base100)
        .rounded(16.)
        .border_color(ColorToken::Border)
        .border_width(1.)
        .layout_animation(Animation::spring().duration_ms(400))
        .clip_contents(true);
    section("studio-card","Reveal what matters","The card changes size with keyed layout motion. Details slide in and out; removed content cannot receive input.",
        panel(vstack![card].flow(Flow::Center).height(320.)),
        r#"card.key("kit").height(if app.expanded { 294. } else { 180. })
    .layout_animation(Animation::spring().duration_ms(400));
// Mount details with a stable key and a finite enter/exit slide.
// details.key("details").transition(Transition::slide(0., 18.))"#)
}
fn pointer_demo(app: &App) -> Section {
    let pos = if app.gallery.tracking {
        app.gallery.pointer
    } else {
        [240., 120.]
    };
    let glow = Gradient::radial(
        [pos[0] / 480., pos[1] / 240.],
        [
            (0., ColorToken::Primary.alpha(35)),
            (0.7, ColorToken::Primary.alpha(2)),
            (1., ColorToken::Primary.alpha(0)),
        ],
    );
    let card = vstack![
        text("Point. Explore. Discover.")
            .font_size(22.)
            .font_weight(600),
        muted("A quiet response to your movement.")
    ]
    .key("studio-pointer-card")
    .gap(8., Axis::Vertical)
    .padding(20., Edge::All)
    .background(ColorToken::Base100)
    .rounded(14.)
    .border_color(ColorToken::Border)
    .border_width(1.)
    .transform(
        Transform2D::new()
            .translate((pos[0] - 240.) * 0.025, (pos[1] - 120.) * 0.025)
            .rotate((pos[0] - 240.) * 0.012),
    )
    .animation(Animation::ease_out().duration_ms(120));
    let marker = canvas(|c| c.stroke(VectorPath::circle(20., 20., 18.), ColorToken::Primary, 1.5))
        .key("studio-pointer-marker")
        .width(40.)
        .height(40.)
        .transform(Transform2D::new().translate(pos[0] - 20., pos[1] - 20.))
        .animation(Animation::ease_out().duration_ms(95));
    let stage = zstack![
        vstack![]
            .width_fraction(1.)
            .height(240.)
            .background_gradient(glow),
        vstack![card]
            .flow(Flow::Center)
            .height(240.)
            .width_fraction(1.),
        vstack![marker]
            .flow(Flow::TopLeft)
            .height(240.)
            .width_fraction(1.)
    ]
    .key("studio-pointer-surface")
    .fill_width()
    .max_width(480.)
    .height(240.)
    .rounded(18.)
    .clip_contents(true)
    .wheel_passthrough(true)
    .accessible_label("Pointer reaction surface")
    .on_pointer(|a: &mut App, p| match p.phase {
        PointerPhase::Hover | PointerPhase::Press | PointerPhase::Drag => {
            a.gallery.pointer = [p.at[0].clamp(0., 480.), p.at[1].clamp(0., 240.)];
            a.gallery.tracking = true;
        }
        PointerPhase::Leave => a.gallery.tracking = false,
        _ => {}
    });
    section("studio-pointer","Follow, without replacing the cursor","A spotlight, spring-like following ring and restrained 2D card response. Touch/drag works too; wheel input still scrolls the page.",
        vstack![stage,button("Center target").key("studio-pointer-reset").variant(ButtonVariant::Ghost)
            .on_click(|a:&mut App|a.gallery.tracking=false)]
            .gap(12.,Axis::Vertical).fill_width().max_width(480.),
        r#"surface.wheel_passthrough(true).on_pointer(|app, p| match p.phase {
    PointerPhase::Hover | PointerPhase::Press | PointerPhase::Drag => app.pointer = p.at,
    _ => {},
});
// Move retained children using Transform2D + Animation; do not replace the OS cursor."#)
}
fn scroll_demo(app: &App) -> Section {
    let cards = [
        ("Capture", "Start with the view in front of you."),
        ("Review", "Keep the details that tell the story."),
        ("Share", "Send it when you are ready."),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (title, body))| -> Element<App> {
        vstack![text(title).font_size(22.).font_weight(600), muted(body)]
            .key(format!("studio-scroll-reveal-{i}"))
            .gap(8., Axis::Vertical)
            .padding(24., Edge::All)
            .height(126.)
            .background(ColorToken::Base100)
            .border_color(ColorToken::Border)
            .border_width(1.)
            .rounded(14.)
            .reveal_on_scroll(Transition::slide(0., 24.).duration_ms(420))
            .transition_revision(app.gallery.reveals)
            .into()
    });
    let mut content = vec![
        muted("Scroll this preview to reveal each step.").into(),
        spacer().height(150.).flex(0.).into(),
    ];
    content.extend(cards);
    section("studio-scroll-reveal","Reveals that follow your reading","One entrance per key/revision when a box first becomes visible. Offscreen items do not run an animation timer. Reduced motion shows content immediately.",
        vstack![vstack(content).key("studio-reveal-scroller").gap(24.,Axis::Vertical).padding(16.,Edge::All)
            .height(280.).width_fraction(1.).max_width(520.).scroll(Scroll::Vertical).clip_contents(true).background(ColorToken::Base200).rounded(16.),
            button("Replay visible reveals").key("studio-scroll-replay").variant(ButtonVariant::Outline)
                .on_click(|a:&mut App|a.gallery.reveals=a.gallery.reveals.wrapping_add(1))].gap(14.,Axis::Vertical),
        r#"section.key("story-step").reveal_on_scroll(Transition::slide(0., 24.).duration_ms(420))
    .transition_revision(app.replay_revision);
// Normal scrolling stays in control; this is not scroll snapping or scroll hijacking."#)
}
pub fn doc(app: &App) -> Doc {
    Doc {
        sections: vec![
            carousel_demo(app),
            card_demo(app),
            sheet_demo(app),
            fab_demo(app),
            pointer_demo(app),
            scroll_demo(app),
        ],
        api: vec![
            (
                "carousel(...).slide_animation(animation)",
                "Responsive retained slides; shared paint/input transforms",
            ),
            (
                "bottom_sheet(body) / speed_dial(label, actions)",
                "Controlled modal and non-modal touch-sized actions",
            ),
            (
                ".reveal_on_scroll(transition)",
                "One entry reveal per key/revision; never drives the scroll position",
            ),
            (
                ".on_pointer(...) / .wheel_passthrough(true)",
                "Pointer reactions without swallowing wheel scrolling",
            ),
        ],
    }
}
pub fn instruments(app: &App) -> Doc {
    let battery = zstack![
        gauge(app.gallery.battery)
            .key("instrument-battery")
            .accessible_label("Battery charge")
            .width(168.)
            .height(168.)
            .dial_style(DialStyle::new().stroke_width(7.).ticks(13)),
        vstack![
            text(format!("{:.0}%", app.gallery.battery * 100.))
                .font_family("SF Mono")
                .font_size(30.),
            muted("Battery")
        ]
        .gap(6., Axis::Vertical)
    ]
    .width(168.)
    .height(168.)
    .flow(Flow::Center);
    let dial = vstack![
        knob(app.gallery.temperature)
            .key("instrument-temperature")
            .accessible_label("Cabin temperature, degrees Celsius")
            .range(16. ..=30.)
            .step(0.5)
            .width(144.)
            .height(144.)
            .dial_style(DialStyle::new().ticks(15))
            .on_change(|a: &mut App, v| a.gallery.temperature = v),
        text(format!("{:.1} °C", app.gallery.temperature))
            .font_family("SF Mono")
            .font_size(23.),
        muted("Cabin setpoint")
    ]
    .gap(6., Axis::Vertical)
    .align(Align::Center);
    Doc {sections:vec![section("instruments-controls","A panel made for touch","Real progress and slider semantics. The rotary control supports captured pointer input, keyboard steps, disabled state and assistive adjustment. Values here are simulated.",
        panel(vstack![hstack![text("Cabin controls").font_size(24.).font_weight(600),spacer(),badge("Simulation")].align(Align::Center),
            hstack![battery,dial].gap(48.,Axis::Horizontal).wrap(true).align(Align::Center),
            hstack![button("Charge +5%").key("instrument-charge").height(44.).on_click(|a:&mut App|a.gallery.battery=(a.gallery.battery+0.05).min(1.)),
                button("Reset setpoint").key("instrument-reset").height(44.).variant(ButtonVariant::Outline).on_click(|a:&mut App|a.gallery.temperature=21.5)].gap(12.,Axis::Horizontal).wrap(true),
            muted("Portable components are not a certified automotive or real-time device host.")].gap(24.,Axis::Vertical)),
        r#"gauge(app.battery).accessible_label("Battery charge")
    .dial_style(DialStyle::new().ticks(13).stroke_width(7.));
knob(app.temperature).key("temperature").range(16.0..=30.0).step(0.5)
    .accessible_label("Cabin temperature, degrees Celsius")
    .on_change(|app, value| app.temperature = value);"#)],api:vec![
        ("gauge(fraction) / knob(value)","Read-only progress and interactive numeric slider"),
        ("DialStyle::new().start_angle(...).sweep(...).ticks(...)","Validated, bounded logical geometry; normal GPU antialiasing"),
    ]}
}
fn mask_circle(x: f32) -> Element<App> {
    canvas(move |c| c.fill(VectorPath::circle(x, 60., 42.), "#FFFFFF"))
        .width(160.)
        .height(120.)
}
pub fn image_sections(app: &App) -> Vec<Section> {
    let masked = |mask: Element<App>| -> Element<App> {
        zstack![vstack![]
            .width(160.)
            .height(120.)
            .background_image(app.image.clone().background())]
        .width(160.)
        .height(120.)
        .mask(mask, MaskMode::Alpha)
        .rounded(12.)
        .clip_contents(true)
        .into()
    };
    let masks = [
        (
            "Union",
            masked(mask_union([mask_circle(55.), mask_circle(105.)]).into()),
        ),
        (
            "Intersection",
            masked(mask_intersection(mask_circle(55.), mask_circle(105.)).into()),
        ),
        (
            "Difference",
            masked(mask_difference(mask_circle(55.), mask_circle(105.)).into()),
        ),
        (
            "Exclude",
            masked(mask_exclude(mask_circle(55.), mask_circle(105.)).into()),
        ),
    ];
    let operations = hstack(masks.into_iter().map(|(label, art)| -> Element<App> {
        vstack![art, muted(label)].gap(10., Axis::Vertical).into()
    }))
    .gap(20., Axis::Horizontal)
    .wrap(true);
    let fade = zstack![vstack![]
        .width_fraction(1.)
        .height(210.)
        .background_image(app.image.clone().background())]
    .height(210.)
    .width_fraction(1.)
    .max_width(520.)
    .mask(
        vstack![]
            .height(210.)
            .width_fraction(1.)
            .background_gradient(Gradient::linear(
                90.,
                [
                    (0., "#FFFFFF00"),
                    (0.25, "#FFFFFFFF"),
                    (0.75, "#FFFFFFFF"),
                    (1., "#FFFFFF00"),
                ],
            )),
        MaskMode::Alpha,
    );
    let blend = app.gallery.blend;
    let image = zstack![
        vstack![]
            .width_fraction(1.)
            .height(220.)
            .background_image(app.image.clone().background()),
        vstack![]
            .width_fraction(1.)
            .height(220.)
            .background_gradient(Gradient::linear(
                100.,
                [(0., "#103D8CAA"), (1., "#FFD987CC")]
            ))
            .blend_mode(blend)
    ]
    .width_fraction(1.)
    .max_width(520.)
    .height(220.)
    .rounded(16.)
    .clip_contents(true)
    .isolate(true);
    vec![section("image-mask-operations","Cutouts with clear edges","Reusable alpha-coverage union, intersection, subtraction and exclusion. These compose masks; they do not generate boolean vector-path geometry.",operations,
        r#"photo.mask(mask_union([left_mask(), right_mask()]), MaskMode::Alpha);
photo.mask(mask_difference(left_mask(), right_mask()), MaskMode::Alpha);
// Also: mask_intersection(a, b), mask_exclude(a, b)."#),
        section("image-feather","A softer edge","Gradient alpha masks blend an image into its surface without a pre-rendered asset.",fade,
        r##"photo.mask(mask.background_gradient(Gradient::linear(90., [
    (0., "#FFFFFF00"), (0.25, "#FFFFFFFF"),
    (0.75, "#FFFFFFFF"), (1., "#FFFFFF00"),
])), MaskMode::Alpha)"##),
        section("image-blend","Overlays that belong to the image","Normal, multiply, screen, overlay, color burn and difference run on both GPU and software. Isolation keeps compositing inside this preview.",
            vstack![image,hstack([(BlendMode::Normal,"Normal"),(BlendMode::Multiply,"Multiply"),(BlendMode::Screen,"Screen"),(BlendMode::Overlay,"Overlay"),(BlendMode::ColorBurn,"Burn"),(BlendMode::Difference,"Difference")]
                .into_iter().enumerate().map(|(i,(mode,label))| -> Element<App> {
                    button(label).key(format!("image-blend-mode-{i}"))
                        .variant(if blend==mode {ButtonVariant::Primary} else {ButtonVariant::Soft})
                        .on_click(move |a:&mut App|a.gallery.blend=mode).into()
                })).gap(8.,Axis::Horizontal).wrap(true).fill_width().max_width(520.)].gap(16.,Axis::Vertical),
        r#"zstack![photo, tint.blend_mode(BlendMode::ColorBurn)]
    .isolate(true).clip_contents(true);
// BlendMode::Difference is colour difference, not alpha subtraction."#)]
}
