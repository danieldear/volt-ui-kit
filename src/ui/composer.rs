//! The message box at the foot of a chat: a field that grows as you type, a
//! send button that only lights up when there is something to send, and a
//! stop button while a reply is coming in. It rides the keyboard.
use super::*;
use crate::declarative::AccessRole;
use std::{cell::RefCell, rc::Rc};

type Press<C> = Rc<RefCell<dyn FnMut(&mut C)>>;
type Change<C> = Box<dyn FnMut(&mut C, String)>;

/// See [`composer`].
pub struct Composer<C> {
    key: Option<String>,
    value: String,
    placeholder: String,
    change: Option<Change<C>>,
    send: Option<Press<C>>,
    stop: Option<Press<C>>,
    attach: Option<Press<C>>,
    busy: bool,
    max_lines: usize,
    above: Vec<Element<C>>,
}

/// A chat composer. Put it last in a page, under the transcript:
///
/// ```rust,ignore
/// vstack![
///     transcript.flex(1.),
///     composer(&app.draft)
///         .key("chat-composer")
///         .placeholder("Message")
///         .on_change(|a: &mut App, text| a.draft = text)
///         .on_send(|a: &mut App| a.send())
///         .busy(app.replying)
///         .on_stop(|a: &mut App| a.stop()),
/// ]
/// ```
pub fn composer<C>(value: impl Into<String>) -> Composer<C> {
    Composer {
        key: None,
        value: value.into(),
        placeholder: "Message".into(),
        change: None,
        send: None,
        stop: None,
        attach: None,
        busy: false,
        max_lines: 6,
        above: Vec::new(),
    }
}

impl<C> Composer<C> {
    /// Required. Parts are keyed `{key}-field`, `{key}-send`, `{key}-stop`
    /// and `{key}-attach`.
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    /// What the empty field says. Default "Message".
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }
    /// Required: the draft changed.
    pub fn on_change(mut self, f: impl FnMut(&mut C, String) + 'static) -> Self {
        self.change = Some(Box::new(f));
        self
    }
    /// Required: send the draft. Clear it in here.
    pub fn on_send(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.send = Some(Rc::new(RefCell::new(f)));
        self
    }
    /// A reply is coming in: Send becomes Stop.
    pub fn busy(mut self, value: bool) -> Self {
        self.busy = value;
        self
    }
    /// Stop the reply. Required with `busy`.
    pub fn on_stop(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.stop = Some(Rc::new(RefCell::new(f)));
        self
    }
    /// A button before the field that adds things: files, photos, context.
    pub fn on_attach(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.attach = Some(Rc::new(RefCell::new(f)));
        self
    }
    /// How tall the field grows before it scrolls, in lines. Default 6.
    pub fn max_lines(mut self, lines: usize) -> Self {
        self.max_lines = lines.clamp(1, 12);
        self
    }
    /// Shown above the field: attachment chips, a reply-to quote.
    pub fn above(mut self, items: impl IntoIterator<Item = Element<C>>) -> Self {
        self.above = items.into_iter().collect();
        self
    }
}

/// Measurements, in points.
const BUTTON: f32 = 36.;
const LINE: f32 = 22.;
/// Single-line field = 44, the height of the buttons beside it, so the text,
/// Add and Send share one centre line.
const FIELD_PAD: f32 = 11.;
/// Roughly how many characters fit on a line of the field, for growing it
/// before the text wraps. The field also scrolls, so a guess is safe.
const CHARS_PER_LINE: usize = 34;

/// Lines the draft takes, by its line breaks and a guess at wrapping.
fn lines(text: &str) -> usize {
    text.split('\n')
        .map(|l| l.chars().count().div_ceil(CHARS_PER_LINE).max(1))
        .sum()
}

fn round_button<C: 'static>(
    key: String,
    glyph: Icon,
    label: &str,
    fill: ColorToken,
    ink: ColorToken,
    enabled: bool,
    press: Option<Press<C>>,
) -> Element<C> {
    let mut b = zstack![icon(glyph).size(18.).stroke(2.4).color(ink)]
        .key(key)
        .width(BUTTON)
        .height(BUTTON)
        .flow(Flow::Center)
        .rounded(Rounding::Full)
        .background(fill)
        .accessible_label(label)
        .opacity(if enabled { 1. } else { 0.4 })
        .pressed_style(|s| s.opacity(0.7));
    if enabled {
        b = b.on_click_with(move |c: &mut C, _| {
            if let Some(f) = &press {
                (f.borrow_mut())(c);
            }
        });
    }
    b.0.node.access_role = Some(AccessRole::Button);
    b.0.node.enabled = enabled;
    // Sending leaves the keyboard up for the next message.
    b.0.node.keeps_keyboard = true;
    b.into()
}

fn invalid<C>(message: &str) -> Element<C> {
    let mut e = Element::new(Kind::Column);
    e.validation_error = Some(message.into());
    e
}

impl<C: 'static> From<Composer<C>> for Element<C> {
    fn from(c: Composer<C>) -> Self {
        let Some(key) = c.key.filter(|k| !k.is_empty()) else {
            return invalid("composer requires a key");
        };
        let (Some(change), Some(send)) = (c.change, c.send) else {
            return invalid("composer requires on_change and on_send");
        };
        if c.busy && c.stop.is_none() {
            return invalid("busy composer requires on_stop");
        }
        let ready = !c.value.trim().is_empty();
        let shown = lines(&c.value).clamp(1, c.max_lines);
        let field_height = shown as f32 * LINE + 2. * FIELD_PAD;

        let submit = send.clone();
        let mut field = text_area(c.value)
            .key(format!("{key}-field"))
            .placeholder(c.placeholder)
            .on_change(change)
            .on_submit(move |a: &mut C| {
                if ready {
                    (submit.borrow_mut())(a);
                }
            })
            .hide_done_button()
            .height(field_height)
            .font_size(17.)
            .background("#00000000")
            .border_width(0.)
            .flex(1.);
        field.0.node.padding_vertical = Some(FIELD_PAD);
        field.0.node.padding_horizontal = Some(14.);

        let mut row: Vec<Element<C>> = Vec::new();
        if let Some(attach) = c.attach {
            // Level with the send button, by the field's last line.
            row.push(
                vstack![round_button(
                    format!("{key}-attach"),
                    Icon::Plus,
                    "Add",
                    ColorToken::Base300,
                    ColorToken::BaseContent,
                    true,
                    Some(attach),
                )]
                .padding(4., Edge::Vertical)
                .into(),
            );
        }
        // The field and its button share one pill; the button sits at the
        // bottom, by the last line, as the field grows.
        let action = if c.busy {
            round_button(
                format!("{key}-stop"),
                Icon::Stop,
                "Stop",
                ColorToken::BaseContent,
                ColorToken::Base100,
                true,
                c.stop,
            )
        } else {
            round_button(
                format!("{key}-send"),
                Icon::ArrowUp,
                "Send",
                ColorToken::Primary,
                ColorToken::PrimaryContent,
                ready,
                Some(send),
            )
        };
        row.push(
            hstack![field, vstack![action].padding(4., Edge::All)]
                .key(format!("{key}-pill"))
                .flex(1.)
                .align(Align::End)
                .rounded(22.)
                .background(ColorToken::Base200)
                .border_color(ColorToken::Border)
                .border_width(1. / 3.)
                .into(),
        );
        let mut parts: Vec<Element<C>> = Vec::new();
        if !c.above.is_empty() {
            parts.push(
                hstack(c.above)
                    .key(format!("{key}-above"))
                    .gap(8., Axis::Horizontal)
                    .padding(4., Edge::Horizontal)
                    .into(),
            );
        }
        parts.push(
            hstack(row)
                .key(format!("{key}-row"))
                .gap(8., Axis::Horizontal)
                .align(Align::End)
                .into(),
        );
        // Under the field: room for the tab bar, home indicator or keyboard,
        // so the composer always sits just above whichever is there.
        let mut foot = vstack(Vec::<Element<C>>::new())
            .key(format!("{key}-foot"))
            .height(8.);
        foot.0.node.ext.above_bars = true;
        parts.push(foot.into());
        vstack(parts)
            .key(key)
            .gap(8., Axis::Vertical)
            .padding(12., Edge::Horizontal)
            .padding(8., Edge::Top)
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::lines;
    #[test]
    fn the_field_grows_by_lines_and_wrapping() {
        assert_eq!(lines(""), 1);
        assert_eq!(lines("Hi"), 1);
        assert_eq!(lines("a\nb\nc"), 3);
        assert_eq!(lines(&"x".repeat(70)), 3);
    }
}
