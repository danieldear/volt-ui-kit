//! Action-driven motion: finite animations, stable controls and quiet idle.
use super::{pages::Section, App};
use volt_ui_kit::ui::prelude::*;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Curve {
    #[default]
    Smooth,
    Spring,
    Instant,
}
#[derive(Default)]
pub struct State {
    pub curve: Curve,
    pub moved: bool,
    pub revision: u64,
    pub actions: u32,
    pub rotated: bool,
    pub transform_actions: u32,
    pub compact: bool,
    pub reordered: bool,
    pub selected: usize,
    pub timeline_revision: u64,
    pub playback: TimelinePlayback,
    pub seek: f32,
    pub text_changed: bool,
    pub text_revision: u64,
    pub number: i64,
}
fn timing(curve: Curve) -> Animation {
    match curve {
        Curve::Smooth => Animation::ease_out().duration_ms(320),
        Curve::Spring => Animation::spring().duration_ms(520),
        Curve::Instant => Animation::linear().duration_ms(0),
    }
}
fn quiet(s: impl Into<String>) -> volt_ui_kit::ui::Text<App> {
    text(s).font_size(13.).color(ColorToken::Muted)
}
pub fn workspace(app: &App) -> Section {
    let status = if app.motion.moved {
        "Ready"
    } else {
        "In review"
    };
    let surface = vstack![
        hstack![
            text("Delivery workflow").font_size(17.).font_weight(600),
            spacer(),
            badge(if app.motion.curve == Curve::Instant {
                "Instant"
            } else {
                "Paint only"
            })
            .font_size(11.)
        ]
        .align(Align::Center),
        quiet("Move the task, then reverse it mid-flight. Its layout stays in place."),
        segmented_control([
            (Curve::Smooth, "Smooth"),
            (Curve::Spring, "Spring"),
            (Curve::Instant, "Instant")
        ])
        .key("motion-curve")
        .accessible_label("Motion preview curve")
        .selected(app.motion.curve)
        .on_select(|app: &mut App, curve| {
            app.motion.curve = curve;
            app.motion.revision = app.motion.revision.wrapping_add(1);
        }),
        vstack![
            hstack![quiet("Review"), spacer(), quiet("Ready to ship")],
            panel![vstack![
                text("Catalog refresh").font_size(14.).font_weight(600),
                text(status).font_size(12.).color(ColorToken::Primary)
            ]
            .gap(8., Axis::Vertical)]
            .key("motion-task")
            .width(142.)
            .height(72.)
            .padding(14., Edge::All)
            .rounded(10.)
            .background(if app.motion.moved {
                ColorToken::Primary.alpha(13)
            } else {
                ColorToken::Base100.into()
            })
            .border_color(if app.motion.moved {
                ColorToken::Primary
            } else {
                ColorToken::Border
            })
            .border_width(1.)
            .shadow(
                Shadow::new()
                    .color(ColorToken::BaseContent.alpha(8))
                    .blur(8.)
                    .offset(0., 3.)
            )
            .offset(if app.motion.moved { 64. } else { 0. }, 0.)
            .animation(timing(app.motion.curve)),
        ]
        .gap(18., Axis::Vertical)
        .height(154.)
        .padding(16., Edge::All)
        .background(ColorToken::Base200)
        .rounded(12.),
        hstack![
            button(if app.motion.moved {
                "Return to review"
            } else {
                "Move to ready"
            })
            .key("motion-move")
            .variant(ButtonVariant::Primary)
            .height(36.)
            .font_size(13.)
            .on_click(|app: &mut App| app.motion.moved = !app.motion.moved),
            spacer(),
            text("No layout ticks")
                .font_family("SF Mono")
                .font_size(11.)
                .color(ColorToken::Muted)
        ]
        .align(Align::Center)
        .gap(12., Axis::Horizontal),
    ]
    .width_fraction(1.)
    .max_width(620.)
    .font_family("SF")
    .gap(18., Axis::Vertical)
    .padding(24., Edge::All)
    .background(ColorToken::Base100)
    .border_width(1.)
    .border_color(ColorToken::Border)
    .rounded(14.)
    .into();
    Section{id:"fx-motion-workspace",title:"Motion that explains a change",note:"Compare smooth, spring and instant timing. Reverse the task while it moves; hit targets, focus and layout never follow decorative paint offsets.",demo:surface,
 code:r#"panel![task_content(app)].key("task")
    .offset(if app.ready { 64. } else { 0. }, 0.)
    .animation(Animation::spring().duration_ms(520));
// Or tune the response without adding a timer:
// Animation::spring_with(Spring::new().damping_ratio(0.7))
// Retargeting preserves painted position, not physical velocity."#}
}
pub fn sequence(app: &App) -> Section {
    let rows = vstack(
        [
            ("Design approved", "Interface / Catalog"),
            ("Checks passed", "Tests / Rendering"),
            ("Ready for review", "Delivery / Native"),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (title, detail))| -> Element<App> {
            hstack![
                text("✓")
                    .font_size(15.)
                    .color(ColorToken::Success)
                    .width(22.),
                vstack![
                    text(title).font_size(14.).font_weight(500),
                    quiet(detail).font_size(12.)
                ]
                .gap(5., Axis::Vertical),
                spacer(),
                text(format!(
                    "+{} ms",
                    if app.motion.curve == Curve::Instant {
                        0
                    } else {
                        index * 70
                    }
                ))
                .font_family("SF Mono")
                .font_size(11.)
                .color(ColorToken::Muted),
            ]
            .key(format!("motion-step-{index}"))
            .align(Align::Center)
            .gap(10., Axis::Horizontal)
            .padding(16., Edge::All)
            .background(ColorToken::Base200)
            .rounded(10.)
            .transition(
                Transition::slide(0., 10.)
                    .animation(timing(app.motion.curve))
                    .stagger(
                        index,
                        if app.motion.curve == Curve::Instant {
                            0
                        } else {
                            70
                        },
                    ),
            )
            .transition_revision(app.motion.revision)
            .into()
        }),
    )
    .gap(10., Axis::Vertical);
    let surface=vstack![
  hstack![text("A small release, in order").font_size(17.).font_weight(600),spacer(),button("Replay").key("motion-replay").variant(ButtonVariant::Outline).height(32.).font_size(12.).on_click(|app:&mut App|app.motion.revision=app.motion.revision.wrapping_add(1))].align(Align::Center).gap(10.,Axis::Horizontal),
  quiet("Three finite entrances, 70 ms apart. Replay changes a revision, never the keys."),rows,
  quiet("Ordinary edits do not restart this sequence. Reduced motion settles it immediately.").font_size(12.),
 ].width_fraction(1.).max_width(620.).font_family("SF").padding(24.,Edge::All).gap(18.,Axis::Vertical).background(ColorToken::Base100).border_width(1.).border_color(ColorToken::Border).rounded(14.).into();
    Section{id:"fx-motion-sequence",title:"A sequence, not a loop",note:"Stagger related updates and replay an entrance explicitly. When the last item settles, this preview stops requesting animation frames.",demo:surface,
 code:r#"row.key(format!("step-{index}"))
    .transition(Transition::slide(0., 10.)
        .animation(Animation::ease_out().duration_ms(320))
        .stagger(index, 70))
    .transition_revision(app.replay_revision);
// Replay button: app.replay_revision = app.replay_revision.wrapping_add(1);
// Unchanged revision keeps the current transition's clock."#}
}
pub fn feedback(app: &App) -> Section {
    let surface = vstack![
        text("Responsive, not distracting")
            .font_size(17.)
            .font_weight(600),
        quiet("Hover to lift. Press to settle. Tab to reveal the focus treatment."),
        hstack![
                button("Save changes")
                    .key("motion-feedback")
                    .variant(ButtonVariant::Outline)
                    .height(42.)
                    .font_size(14.)
                    .hover_style(|s| s
                        .offset(0., -2.)
                        .border_color(ColorToken::Primary)
                        .glow(Glow::new().color(ColorToken::Primary.alpha(25)).blur(12.)))
                    .pressed_style(|s| s.offset(0., 1.).opacity(0.8))
                    .focus_style(|s| s.border_color(ColorToken::Primary))
                    .animate_on(Interaction::Hover, Animation::spring().duration_ms(280))
                    .animate_on(Interaction::Pressed, Animation::ease_out().duration_ms(90))
                    .on_click(
                        |app: &mut App| app.motion.actions = app.motion.actions.saturating_add(1)
                    ),
                quiet(format!("Saved {} times", app.motion.actions)).key("motion-feedback-result"),
            ]
        .gap(18., Axis::Horizontal)
        .align(Align::Center),
    ]
    .width_fraction(1.)
    .max_width(620.)
    .font_family("SF")
    .padding(24., Edge::All)
    .gap(18., Axis::Vertical)
    .background(ColorToken::Base100)
    .border_width(1.)
    .border_color(ColorToken::Border)
    .rounded(14.)
    .into();
    Section{id:"fx-motion-feedback",title:"Tactile state feedback",note:"A restrained glow and two-pixel lift. Press and hover can interrupt one another; the callback runs only when the button activates.",demo:surface,
 code:r#"button("Save changes").key("save")
    .hover_style(|s| s.offset(0., -2.)
        .glow(Glow::new().color(ColorToken::Primary.alpha(25)).blur(12.)))
    .pressed_style(|s| s.offset(0., 1.).opacity(0.8))
    .animate_on(Interaction::Hover, Animation::spring().duration_ms(280))
    .animate_on(Interaction::Pressed, Animation::ease_out().duration_ms(90))
    .on_click(save_changes);"#}
}

/// Interactive native motion recipes. Controls remain in the established Catalog shell.
pub fn transforms(app: &App) -> Section {
    let angle = if app.motion.rotated { 12. } else { 0. };
    let scale = if app.motion.rotated { 1.12 } else { 1. };
    let demo = vstack![
        quiet("The button and its clickable area move together. Reverse while it is moving."),
        hstack![
            spacer(),
            button(format!("Pressed {} times", app.motion.transform_actions))
                .key("motion-affine-button")
                .width(180.)
                .height(48.)
                .variant(ButtonVariant::Primary)
                .transform(Transform2D::new().rotate(angle).scale(scale, scale))
                .animation(timing(app.motion.curve))
                .on_click(|app: &mut App| app.motion.transform_actions =
                    app.motion.transform_actions.saturating_add(1)),
            spacer()
        ]
        .height(120.)
        .align(Align::Center),
        button(if app.motion.rotated {
            "Return to rest"
        } else {
            "Rotate and scale"
        })
        .key("motion-transform-toggle")
        .variant(ButtonVariant::Outline)
        .on_click(|app: &mut App| app.motion.rotated = !app.motion.rotated),
    ]
    .gap(16., Axis::Vertical)
    .max_width(640.)
    .into();
    Section{id:"fx-transforms",title:"Transforms that keep input aligned",note:"Logical 2D scale and rotation, shaped GPU text and inverse-mapped pointer input at fractional DPI.",demo,
        code:r#"button("Press me").key("action")
    .transform(Transform2D::new().rotate(angle).scale(zoom, zoom))
    .animation(Animation::ease_out().duration_ms(320))
    .on_click(|app: &mut App| app.presses += 1)"#}
}
pub fn layout(app: &App) -> Section {
    let tile = |key: &str, name: &str| -> Element<App> {
        vstack![text(name).font_weight(600), quiet("Stable identity")]
            .key(key)
            .width(if app.motion.compact { 116. } else { 156. })
            .height(80.)
            .padding(14., Edge::All)
            .background(ColorToken::Primary.alpha(10))
            .border_color(ColorToken::Border)
            .border_width(1.)
            .rounded(12.)
            .layout_animation(timing(app.motion.curve))
            .into()
    };
    let items = if app.motion.reordered {
        vec![
            tile("motion-card-b", "Sources"),
            tile("motion-card-a", "Workspace"),
        ]
    } else {
        vec![
            tile("motion-card-a", "Workspace"),
            tile("motion-card-b", "Sources"),
        ]
    };
    let tabs = hstack((0..3).map(|i| -> Element<App> {
        let indicator: Element<App> = if app.motion.selected == i {
            vstack![]
                .key(format!("motion-indicator-{i}"))
                .shared_element("motion-indicator")
                .layout_animation(timing(app.motion.curve))
                .height(4.)
                .background(ColorToken::Primary)
                .rounded(2.)
                .into()
        } else {
            spacer().height(4.).flex(0.).into()
        };
        vstack![
            button(["Overview", "Activity", "Files"][i])
                .key(format!("motion-layout-tab-{i}"))
                .fill_width()
                .on_click(move |app: &mut App| app.motion.selected = i),
            indicator
        ]
        .width(110.)
        .gap(8., Axis::Vertical)
        .into()
    }))
    .gap(12., Axis::Horizontal);
    let demo = vstack![
        hstack(items).gap(16., Axis::Horizontal),
        hstack![
            button("Reorder cards")
                .key("motion-reorder")
                .variant(ButtonVariant::Outline)
                .on_click(|app: &mut App| app.motion.reordered = !app.motion.reordered),
            button("Resize cards")
                .key("motion-resize")
                .variant(ButtonVariant::Outline)
                .on_click(|app: &mut App| app.motion.compact = !app.motion.compact)
        ]
        .gap(8., Axis::Horizontal),
        quiet("A shared indicator moves between different parents."),
        tabs
    ]
    .gap(20., Axis::Vertical)
    .max_width(680.)
    .into();
    Section{id:"fx-layout",title:"Layout and shared geometry",note:"Keyed cards interpolate position and size. One shared identity connects the indicator across tab containers.",demo,
        code:r#"card.key("workspace")
    .layout_animation(Animation::ease_out().duration_ms(320));
indicator.key(format!("indicator-{selected}"))
    .shared_element("active-indicator")
    .layout_animation(Animation::ease_out().duration_ms(320));"#}
}
pub fn gestures(_: &App) -> Section {
    let demo = vstack![
        quiet("Drag the card horizontally, then release. Its momentum stays inside the track."),
        vstack![vstack![
            text("Drag me").font_weight(600),
            quiet("Release to coast").no_wrap(true)
        ]
        .key("motion-inertia-card")
        .width(140.)
        .height(64.)
        .padding(12., Edge::All)
        .background(ColorToken::Primary.alpha(15))
        .border_color(ColorToken::Primary.alpha(35))
        .border_width(1.)
        .rounded(12.)
        .cursor(Cursor::Grab)
        .drag_motion(DragMotion::new().bounds([0., 0.], [128., 0.]).friction(7.))]
        .width(300.)
        .height(104.)
        .padding(16., Edge::All)
        .rounded(16.)
        .clip_contents(true)
        .background(ColorToken::Base200)
        .border_color(ColorToken::Border)
        .border_width(1.),
        quiet("Reduced motion disables momentum, not direct dragging.")
    ]
    .gap(16., Axis::Vertical)
    .max_width(640.)
    .into();
    Section{id:"fx-gestures",title:"Direct manipulation and momentum",note:"Captured pointer gestures in logical coordinates, with analytical frame-rate-independent inertia and explicit bounds.",demo,
        code:r#"card.key("draggable")
    .drag_motion(DragMotion::new()
        .bounds([0., 0.], [128., 0.])
        .friction(7.)); // .momentum(false) for direct manipulation only"#}
}
pub fn timeline(app: &App) -> Section {
    let sequence = Timeline::new(1600)
        .keyframe(0., MotionPose::new())
        .keyframe(
            0.3,
            MotionPose::new()
                .translate(100., 0.)
                .rotate(-12.)
                .scale(1.08, 1.08),
        )
        .keyframe(
            0.65,
            MotionPose::new()
                .translate(150., 0.)
                .rotate(12.)
                .scale(0.95, 0.95),
        )
        .keyframe(1., MotionPose::new())
        .easing(Easing::EaseInOut)
        .revision(app.motion.timeline_revision)
        .playback(app.motion.playback);
    let demo = vstack![
        vstack![vstack![text("Volt").font_size(17.).font_weight(600)]
            .key("motion-timeline-card")
            .width(80.)
            .height(56.)
            .padding(14., Edge::All)
            .rounded(12.)
            .background(ColorToken::Primary.alpha(14))
            .border_color(ColorToken::Border)
            .border_width(1.)
            .timeline(sequence)]
        .width(300.)
        .height(112.)
        .padding(24., Edge::All),
        hstack![
            button("Replay")
                .key("motion-replay-timeline")
                .variant(ButtonVariant::Primary)
                .on_click(|app: &mut App| {
                    app.motion.timeline_revision = app.motion.timeline_revision.wrapping_add(1);
                    app.motion.playback = TimelinePlayback::Play;
                }),
            button("Pause")
                .key("motion-pause-timeline")
                .variant(ButtonVariant::Outline)
                .on_click(|app: &mut App| app.motion.playback = TimelinePlayback::Paused),
            button("Resume")
                .key("motion-resume-timeline")
                .variant(ButtonVariant::Outline)
                .on_click(|app: &mut App| app.motion.playback = TimelinePlayback::Play),
            button("Reverse")
                .key("motion-reverse-timeline")
                .variant(ButtonVariant::Outline)
                .on_click(|app: &mut App| app.motion.playback = TimelinePlayback::Reverse)
        ]
        .gap(8., Axis::Horizontal),
        quiet("Seek position — moving this slider pauses the timeline."),
        slider(app.motion.seek)
            .key("motion-seek-timeline")
            .accessible_label("Timeline seek position")
            .range(0. ..=100.)
            .width(280.)
            .on_change(|app: &mut App, value| {
                app.motion.seek = value;
                app.motion.playback = TimelinePlayback::Seek(value / 100.);
            })
    ]
    .gap(14., Axis::Vertical)
    .max_width(680.)
    .into();
    Section{id:"fx-timeline",title:"One clock, a composed sequence",note:"Finite keyframe sequences with pause, resume, reverse, seek and explicit replay. No app-owned animation timer.",demo,
        code:r#"card.key("sequence").timeline(Timeline::new(1600)
    .keyframe(0., MotionPose::new())
    .keyframe(0.5, MotionPose::new().translate(150., 0.).rotate(12.))
    .keyframe(1., MotionPose::new())
    .playback(TimelinePlayback::Play)
    .revision(replay_revision));"#}
}
pub fn text_presets(app: &App) -> Section {
    let word = if app.motion.text_changed {
        "Ready to ship"
    } else {
        "Made for native apps"
    };
    let demo = vstack![
        quiet("Typing · grapheme-safe reveal"),
        typing_text("Beautiful motion, native Rust.")
            .key("motion-typing")
            .text_effect(
                TextEffect::typing()
                    .duration_ms(1400)
                    .revision(app.motion.text_revision)
            )
            .font_size(24.)
            .font_weight(600),
        quiet("Morph · shaped-text crossfade"),
        morphing_text(word)
            .key("motion-morph")
            .font_size(24.)
            .font_weight(600),
        hstack![
            vstack![
                quiet("Rolling number"),
                rolling_number(app.motion.number)
                    .key("motion-number")
                    .font_size(32.)
                    .font_weight(600)
            ],
            spacer(),
            vstack![
                quiet("Shimmer · live status"),
                shimmer_text("Preparing your workspace")
                    .key("motion-shimmer")
                    .font_size(18.)
            ]
        ],
        hstack![
            button("Replay typing")
                .key("motion-replay-text")
                .variant(ButtonVariant::Outline)
                .on_click(|app: &mut App| app.motion.text_revision =
                    app.motion.text_revision.wrapping_add(1)),
            button("Change text and number")
                .key("motion-change-text")
                .variant(ButtonVariant::Primary)
                .on_click(|app: &mut App| {
                    app.motion.text_changed = !app.motion.text_changed;
                    app.motion.number = if app.motion.text_changed { 2048 } else { 128 };
                })
        ]
        .gap(8., Axis::Horizontal)
    ]
    .gap(14., Axis::Vertical)
    .max_width(680.)
    .into();
    Section{id:"fx-text",title:"Text that stays text",note:"Selectable semantic text remains intact. Typing reveals whole graphemes, morphing crossfades shaped text, numbers roll and shimmer sweeps opacity across glyphs.",demo,
        code:r#"typing_text("Hello, native world").key("typing")
    .text_effect(TextEffect::typing().duration_ms(1400).revision(replay));
morphing_text(status).key("status");
rolling_number(count).key("count");
shimmer_text("Preparing workspace").key("working");"#}
}
