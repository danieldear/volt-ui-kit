//! The Chat page: a composer that grows and rides the keyboard, and replies
//! that stream in from another thread.
use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub struct Message {
    pub mine: bool,
    pub text: String,
}

#[derive(Default)]
pub struct Chat {
    pub messages: Vec<Message>,
    pub draft: String,
    pub replying: bool,
    /// Set to stop the reply being written.
    pub stop: Option<Arc<AtomicBool>>,
}

const REPLY: &str = "Here's a start. Keep the composer at the foot of the page, \
under the transcript, and let it ride the keyboard. Send lights up only when \
there's something to send, and turns into Stop while a reply comes in. \
The reply eases in piece by piece, and the transcript follows it down unless \
you scroll up to read.";

pub fn send(a: &mut App) {
    let text = a.chat.draft.trim().to_owned();
    if text.is_empty() || a.chat.replying {
        return;
    }
    a.chat.draft.clear();
    a.chat.messages.push(Message { mine: true, text });
    a.chat.messages.push(Message {
        mine: false,
        text: String::new(),
    });
    a.chat.replying = true;
    let stop = Arc::new(AtomicBool::new(false));
    a.chat.stop = Some(stop.clone());
    let Some(updates) = a.updates.clone() else {
        // No worker in tests: the whole reply at once.
        if let Some(last) = a.chat.messages.last_mut() {
            last.text = REPLY.into();
        }
        a.chat.replying = false;
        return;
    };
    std::thread::spawn(move || {
        let words: Vec<&str> = REPLY.split_inclusive(' ').collect();
        for (i, chunk) in words.chunks(2).enumerate() {
            std::thread::sleep(std::time::Duration::from_millis(60 + (i as u64 % 3) * 30));
            if stop.load(Ordering::Relaxed) {
                return;
            }
            let piece: String = chunk.concat();
            let last = (i + 1) * 2 >= words.len();
            let open = updates.update(move |a: &mut App| {
                if !a.chat.replying {
                    return;
                }
                if let Some(m) = a.chat.messages.last_mut() {
                    m.text.push_str(&piece);
                }
                if last {
                    a.chat.replying = false;
                }
            });
            if !open {
                return;
            }
        }
    });
}

fn stop(a: &mut App) {
    if let Some(flag) = a.chat.stop.take() {
        flag.store(true, Ordering::Relaxed);
    }
    a.chat.replying = false;
}

fn bubble(i: usize, m: &Message, live: bool) -> Element<App> {
    let body: Element<App> = if m.mine {
        text(m.text.clone())
            .font_size(17.)
            .color(ColorToken::PrimaryContent)
            .into()
    } else if m.text.is_empty() {
        shimmer_text("Thinking")
            .font_size(17.)
            .color(ColorToken::Muted)
            .into()
    } else if live {
        live_text(m.text.clone()).font_size(17.).into()
    } else {
        text(m.text.clone()).font_size(17.).into()
    };
    let b = vstack![body]
        .key(format!("chat-message-{i}"))
        .padding(10., Edge::Vertical)
        .padding(14., Edge::Horizontal)
        .rounded(20.);
    if m.mine {
        hstack![spacer().width(48.), b.background(ColorToken::Primary)]
            .justify(Justify::End)
            .into()
    } else {
        // Replies run the width of the page, as in most assistants.
        hstack![b.flex(1.)].into()
    }
}

pub fn page(app: &App) -> Container<App> {
    let c = &app.chat;
    let n = c.messages.len();
    let mut items: Vec<Element<App>> = Vec::new();
    if n == 0 {
        items.push(
            vstack![
                text("Ask anything").font_size(22.).font_weight(600),
                text("Type below and send. The reply streams in from another thread.")
                    .font_size(15.)
                    .color(ColorToken::Muted),
            ]
            .key("chat-empty")
            .gap(6., Axis::Vertical)
            .padding(32., Edge::Top)
            .into(),
        );
    }
    for (i, m) in c.messages.iter().enumerate() {
        items.push(bubble(i, m, i + 1 == n && !m.mine));
    }
    let transcript = vstack![vstack(items)
        .gap(12., Axis::Vertical)
        .padding(16., Edge::Vertical)]
    .key("chat-transcript")
    .padding(16., Edge::Horizontal)
    .flex(1.)
    .scroll(Scroll::Vertical)
    .anchor_end(true);
    vstack![
        transcript,
        composer(c.draft.clone())
            .key("chat-composer")
            .placeholder("Message")
            .on_change(|a: &mut App, text| a.chat.draft = text)
            .on_send(send)
            .busy(c.replying)
            .on_stop(stop)
            .on_attach(|_: &mut App| {}),
    ]
    .key("chat-screen")
    .flex(1.)
}
