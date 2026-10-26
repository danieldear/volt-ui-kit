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
