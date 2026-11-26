//! "Jump to latest": a round button over the foot of a chat transcript or a
//! log that the reader scrolled up in, which takes them back to the end.
use super::*;
use crate::declarative::{AccessRole, Transition};

/// The transcript with a button over its foot while `shown`, as one element
/// to put where the transcript was (give it `flex(1.)` in a column). Show
/// it while the transcript's `on_away_from_end` says the reader is away;
/// pressing it should bump the transcript's `scroll_to_end` request. iOS
/// draws a raised circle, Android Material 3's small floating action button.
///
/// ```rust,ignore
/// jump_to_latest(
///     transcript
///         .anchor_end(true)
///         .on_away_from_end(|a: &mut App, away| a.away = away)
///         .scroll_to_end(app.jumps),
///     "chat-jump",
///     app.away,
///     |a: &mut App| a.jumps += 1,
/// )
/// .flex(1.)
/// ```
pub fn jump_to_latest<C: 'static>(
    transcript: impl Into<Element<C>>,
    key: impl Into<String>,
    shown: bool,
    mut f: impl FnMut(&mut C) + 'static,
) -> Container<C> {
    let key = key.into();
    let android = cfg!(target_os = "android");
    let mut parts: Vec<Element<C>> = Vec::new();
    if shown {
        let size = if android { 40. } else { 36. };
        let mut b = zstack![icon(Icon::ArrowDown)
            .size(if android { 20. } else { 18. })
            .stroke(2.)
            .color(ColorToken::BaseContent)]
        .key(key.clone())
        .width(size)
        .height(size)
        .flow(Flow::Center)
        .rounded(if android { 12. } else { size / 2. })
        .background("@base-100|@base-300")
        .border_width(if android { 0. } else { 0.5 })
        .border_color(ColorToken::BaseContent.alpha(20))
        .shadow(
            crate::declarative::effects::Shadow::new()
                .color("#00000026|#00000066")
                .blur(12.)
                .offset(0., 3.),
        )
        .accessible_label("Jump to latest")
        .pressed_style(|s| s.opacity(0.8))
        .on_click_with(move |c: &mut C, _| f(c));
        b.0.node.access_role = Some(AccessRole::Button);
        let mut b: Element<C> = b.into();
        b.node.effects.transition = Some(Transition::slide(0., 12.).duration_ms(200));
        parts.push(b);
    }
    // Stretched layers fill the stack without adding to its natural height,
    // so a column still gives the transcript only the room that's left.
    let mut transcript: Element<C> = transcript.into();
    // A flexible scroll view adds nothing to the stack's natural height,
    // however long the conversation.
    transcript.node.flex = transcript.node.flex.max(1.);
    zstack![
        transcript,
        // At the foot, centred; no spacer, which would make the layer as
        // tall as the screen before it's stretched.
        vstack(parts)
            .key(format!("{key}-layer"))
            .flow(Flow::Bottom)
            .padding(12., Edge::Bottom)
            .scroll_passthrough(true),
    ]
    .key(format!("{key}-stack"))
    .align(Align::Stretch)
}
