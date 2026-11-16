//! Relay: a desktop coding-agent client in the style of today's agent apps.
//! Threads grouped by project, a streaming transcript with collapsible work
//! steps and file changes, a composer with model/effort/permission menus and
//! an inspector card. The "agent" is a local script played on a worker thread
//! (or, in the browser, on the frame clock), so the whole flow — streaming,
//! cancel, retry — runs offline.
pub mod icons;
mod script;
#[cfg(all(test, feature = "native"))]
mod tests;
mod view;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
#[cfg(feature = "native")]
use std::time::Duration;
pub use view::view;
#[cfg(feature = "native")]
use volt_ui_kit::native::{UiSendError, UiSender};
use volt_ui_kit::ui::{Palette, ThemeMode, UiTheme};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    Pro,
    Standard,
    Mini,
}
impl Model {
    pub const ALL: [Model; 3] = [Model::Pro, Model::Standard, Model::Mini];
    pub fn label(self) -> &'static str {
        match self {
            Model::Pro => "Relay 2 Pro",
            Model::Standard => "Relay 2",
            Model::Mini => "Relay 2 Mini",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effort {
    Low,
    Medium,
    High,
}
impl Effort {
    pub const ALL: [Effort; 3] = [Effort::Low, Effort::Medium, Effort::High];
    pub fn label(self) -> &'static str {
        match self {
            Effort::Low => "Low",
            Effort::Medium => "Medium",
            Effort::High => "High",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Ask,
    AutoEdit,
    Full,
}
impl Access {
    pub const ALL: [Access; 3] = [Access::Ask, Access::AutoEdit, Access::Full];
    pub fn label(self) -> &'static str {
        match self {
            Access::Ask => "Ask first",
            Access::AutoEdit => "Auto edit",
            Access::Full => "Full access",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    User,
    Agent,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub label: String,
    pub detail: String,
    pub done: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct FileChange {
    pub path: String,
    pub added: u32,
    pub removed: u32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub role: Role,
    pub text: String,
    pub steps: Vec<Step>,
    pub files: Vec<FileChange>,
    /// Seconds of work, once finished.
    pub worked: Option<u32>,
    pub expanded: bool,
    pub streaming: bool,
    pub liked: bool,
}
impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            text: text.into(),
            steps: vec![],
            files: vec![],
            worked: None,
            expanded: false,
            streaming: false,
            liked: false,
        }
    }
    pub fn agent(text: impl Into<String>) -> Self {
        Self {
            role: Role::Agent,
            ..Self::user(text)
        }
    }
}
#[derive(Clone, Debug)]
pub struct Thread {
    pub id: u32,
    pub project: String,
    pub title: String,
    pub pinned: bool,
    pub unread: bool,
    pub messages: Vec<Message>,
}
impl Thread {
    pub fn changes(&self) -> (u32, u32) {
        self.messages
            .iter()
            .flat_map(|m| &m.files)
            .fold((0u32, 0u32), |(a, r), f| {
                (a.saturating_add(f.added), r.saturating_add(f.removed))
            })
    }
}

/// Fully transparent pixels let clicks fall through to the window behind on
/// macOS, so the tint never goes below this.
pub const TINT_MIN: u8 = 10;
// An offline example is not a persistent, virtualized conversation database.
// Bound retained state and rendered declarations before the 4096-node limit.
pub const MAX_THREADS: usize = 16;
pub const MAX_MESSAGES: usize = 16;
pub const MAX_PROMPT_BYTES: usize = 8192;
const MAX_REPLY_BYTES: usize = 8192;
const MAX_STEPS: usize = 8;
const MAX_FILES: usize = 8;

fn append_bounded(target: &mut String, value: &str, limit: usize) {
    let mut end = value.len().min(limit.saturating_sub(target.len()));
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    target.push_str(&value[..end]);
}

/// Worker events, tagged so a cancelled run can never touch a newer one.
#[derive(Debug)]
pub struct Event {
    pub run: u64,
    pub kind: EventKind,
}
#[derive(Clone, Debug)]
pub enum EventKind {
    Step(String),
    StepDone(String),
    Text(String),
    File(FileChange),
    Done(u32),
}

struct Run {
    id: u64,
    thread: u32,
    cancel: Arc<AtomicBool>,
}

pub struct App {
    pub threads: Vec<Thread>,
    pub active: u32,
    pub draft: String,
    pub query: String,
    pub search_open: bool,
    pub model: Model,
    pub effort: Effort,
    pub access: Access,
    pub model_open: bool,
    pub effort_open: bool,
    pub access_open: bool,
    pub thread_menu_open: bool,
    pub settings_open: bool,
    pub inspector: bool,
    pub sidebar: bool,
    pub enter_sends: bool,
    pub mode: ThemeMode,
    pub glass: bool,
    /// Sidebar tint percentage in glass mode.
    pub tint: u8,
    pub toast: Option<(u32, String)>,
    next_toast: u32,
    next_thread: u32,
    #[cfg(feature = "native")]
    pub sender: Option<UiSender<Event>>,
    /// Browser builds: the current reply's events still to play, each with
    /// the milliseconds to wait before it.
    #[cfg(not(feature = "native"))]
    scheduled: std::collections::VecDeque<(u64, Event)>,
    /// Time waited towards the next event. A `Cell`, so the frame clock can
    /// move it without rebuilding the app (see `elapse`).
    #[cfg(not(feature = "native"))]
    clock: std::cell::Cell<f64>,
    run: Option<Run>,
    next_run: u64,
}

impl App {
    pub fn new() -> Self {
        let threads = script::sample_threads();
        Self {
            active: threads[0].id,
            next_thread: threads.iter().map(|t| t.id).max().unwrap_or(0) + 1,
            threads,
            draft: String::new(),
            query: String::new(),
            search_open: false,
            model: Model::Pro,
            effort: Effort::High,
            access: Access::Full,
            model_open: false,
            effort_open: false,
            access_open: false,
            thread_menu_open: false,
            settings_open: false,
            inspector: true,
            sidebar: true,
            enter_sends: true,
            mode: ThemeMode::Dark,
            glass: false,
            tint: 72,
            toast: None,
            next_toast: 0,
            #[cfg(feature = "native")]
            sender: None,
            #[cfg(not(feature = "native"))]
            scheduled: Default::default(),
            #[cfg(not(feature = "native"))]
            clock: Default::default(),
            run: None,
            next_run: 0,
        }
    }
    pub fn thread(&self) -> &Thread {
        self.threads
            .iter()
            .find(|t| t.id == self.active)
            .unwrap_or(&self.threads[0])
    }
    fn thread_mut(&mut self, id: u32) -> Option<&mut Thread> {
        self.threads.iter_mut().find(|t| t.id == id)
    }
    pub fn running(&self) -> bool {
        self.run.as_ref().is_some_and(|r| r.thread == self.active)
    }
    pub fn running_thread(&self) -> Option<u32> {
        self.run.as_ref().map(|r| r.thread)
    }
    pub fn notify(&mut self, message: impl Into<String>) {
        self.next_toast = self.next_toast.wrapping_add(1);
        self.toast = Some((self.next_toast, message.into()));
    }
    fn close_menus(&mut self) {
        self.model_open = false;
        self.effort_open = false;
        self.access_open = false;
        self.thread_menu_open = false;
    }
    pub fn open(&mut self, id: u32) {
        self.close_menus();
        self.active = id;
        if let Some(t) = self.thread_mut(id) {
            t.unread = false;
        }
    }
    pub fn new_chat(&mut self) {
        let project = self.thread().project.clone();
        // Reuse an untouched draft thread rather than piling up empties.
        if let Some(t) = self
            .threads
            .iter()
            .find(|t| t.project == project && t.messages.is_empty())
        {
            let id = t.id;
            self.open(id);
            return;
        }
        if self.threads.len() >= MAX_THREADS {
            self.notify("Demo thread limit reached; delete a thread before creating another.");
            return;
        }
        let id = self.next_thread;
        self.next_thread += 1;
        self.threads.insert(
            0,
            Thread {
                id,
                project,
                title: "New chat".into(),
                pinned: false,
                unread: false,
                messages: vec![],
            },
        );
        self.open(id);
        self.draft.clear();
    }
    pub fn toggle_pin(&mut self) {
        let id = self.active;
        if let Some(t) = self.thread_mut(id) {
            t.pinned = !t.pinned;
            let pinned = t.pinned;
            self.notify(if pinned { "Pinned" } else { "Unpinned" });
        }
        self.thread_menu_open = false;
    }
    pub fn delete_thread(&mut self) {
        self.thread_menu_open = false;
        if self.threads.len() == 1 {
            self.notify("Keep at least one thread");
            return;
        }
        let id = self.active;
        if self.running_thread() == Some(id) {
            self.stop();
        }
        self.threads.retain(|t| t.id != id);
        self.active = self.threads[0].id;
        self.notify("Thread deleted");
    }
    /// Send the composer draft (or a suggestion) to the active thread.
    pub fn send(&mut self, prompt: Option<String>) {
        let prompt = prompt.unwrap_or_else(|| self.draft.clone());
        let prompt = prompt.trim().to_owned();
        if prompt.is_empty() || self.run.is_some() {
            return;
        }
        if prompt.len() > MAX_PROMPT_BYTES {
            self.notify("Demo messages are limited to 8 KiB; shorten the message to send.");
            return;
        }
        self.close_menus();
        self.draft.clear();
        let id = self.active;
        let Some(thread) = self.thread_mut(id) else {
            return;
        };
        if thread.messages.is_empty() {
            thread.title = script::title_for(&prompt);
        }
        thread.messages.push(Message::user(prompt.clone()));
        let mut reply = Message::agent("");
        reply.streaming = true;
        thread.messages.push(reply);
        let removed = thread.messages.len().saturating_sub(MAX_MESSAGES);
        if removed > 0 {
            thread.messages.drain(..removed);
            self.notify(
                "Older messages removed (demo history limit). No history is saved to disk.",
            );
        }
        // Most recent conversation first in Recents.
        if let Some(i) = self.threads.iter().position(|t| t.id == id) {
            let t = self.threads.remove(i);
            self.threads.insert(0, t);
        }
        self.start(id, &prompt);
    }
    /// Re-run the last prompt of the active thread.
    pub fn retry(&mut self) {
        if self.run.is_some() {
            return;
        }
        let id = self.active;
        let Some(thread) = self.thread_mut(id) else {
            return;
        };
        let Some(prompt) = thread
            .messages
            .iter()
            .rev()
            .find(|m| m.role == Role::User)
            .map(|m| m.text.clone())
        else {
            return;
        };
        if thread
            .messages
            .last()
            .is_some_and(|m| m.role == Role::Agent)
        {
            thread.messages.pop();
        }
        let mut reply = Message::agent("");
        reply.streaming = true;
        thread.messages.push(reply);
        self.start(id, &prompt);
    }
    fn start(&mut self, thread: u32, prompt: &str) {
        self.next_run = self.next_run.wrapping_add(1);
        let cancel = Arc::new(AtomicBool::new(false));
        self.run = Some(Run {
            id: self.next_run,
            thread,
            cancel: cancel.clone(),
        });
        let run = self.next_run;
        let events = timeline(script::reply_for(prompt, self.effort));
        #[cfg(feature = "native")]
        {
            let Some(sender) = self.sender.clone() else {
                // Headless snapshots have no worker; show the run as in progress.
                return;
            };
            std::thread::spawn(move || {
                for (wait, kind) in events {
                    if wait > 0 {
                        std::thread::sleep(Duration::from_millis(wait));
                    }
                    if !deliver(&cancel, || {
                        sender.send(Event {
                            run,
                            kind: kind.clone(),
                        })
                    }) {
                        return;
                    }
                }
            });
        }
        #[cfg(not(feature = "native"))]
        {
            self.scheduled = events
                .into_iter()
                .map(|(wait, kind)| (wait, Event { run, kind }))
                .collect();
            self.clock.set(0.);
        }
    }
    /// Browser builds have no worker thread, so the host plays the reply from
    /// its frame clock. Returns whether anything changed.
    #[cfg(not(feature = "native"))]
    pub fn advance(&mut self, milliseconds: f64) -> bool {
        let mut clock = self.clock.get() + milliseconds;
        let mut changed = false;
        while let Some(&(wait, _)) = self.scheduled.front() {
            if clock < wait as f64 {
                break;
            }
            clock -= wait as f64;
            if let Some((_, event)) = self.scheduled.pop_front() {
                update(self, event);
                changed = true;
            }
        }
        self.clock
            .set(if self.scheduled.is_empty() { 0. } else { clock });
        changed
    }
    /// Let time pass on the frame clock without changing anything on screen;
    /// true once the next event is due, which `advance(0.)` then plays. A
    /// host calls this every frame and rebuilds the app only when it's true.
    #[cfg(not(feature = "native"))]
    pub fn elapse(&self, milliseconds: f64) -> bool {
        self.clock.set(self.clock.get() + milliseconds);
        self.scheduled
            .front()
            .is_some_and(|&(wait, _)| self.clock.get() >= wait as f64)
    }
    /// Whether a browser build still has reply events to play.
    #[cfg(not(feature = "native"))]
    pub fn busy(&self) -> bool {
        !self.scheduled.is_empty()
    }
    pub fn stop(&mut self) {
        let Some(run) = self.run.take() else {
            return;
        };
        run.cancel.store(true, Ordering::Relaxed);
        #[cfg(not(feature = "native"))]
        self.scheduled.clear();
        if let Some(m) = self
            .thread_mut(run.thread)
            .and_then(|t| t.messages.last_mut())
        {
            m.streaming = false;
            for s in &mut m.steps {
                s.done = true;
            }
            if m.text.is_empty() {
                m.text = "Stopped.".into();
            }
        }
        self.notify("Stopped");
    }
}
impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
impl Drop for App {
    fn drop(&mut self) {
        if let Some(run) = &self.run {
            run.cancel.store(true, Ordering::Relaxed);
        }
    }
}

/// A scripted reply as timed events: milliseconds to wait, then the event.
fn timeline(script: script::Reply) -> Vec<(u64, EventKind)> {
    let mut events = Vec::new();
    let mut worked = 2;
    for (label, detail, ms) in script.steps {
        events.push((180, EventKind::Step(label.into())));
        events.push((ms, EventKind::StepDone(detail.into())));
        worked += (ms / 300) as u32 + 1;
    }
    events.extend(
        script
            .files
            .into_iter()
            .map(|file| (0, EventKind::File(file))),
    );
    // Stream the answer a few words at a time, like a token stream.
    let words: Vec<&str> = script.answer.split_inclusive(' ').collect();
    events.extend(
        words
            .chunks(3)
            .map(|chunk| (28, EventKind::Text(chunk.concat()))),
    );
    events.push((0, EventKind::Done(worked)));
    events
}

/// Every event matters (text chunks are not coalescable), so a full queue
/// waits on the worker thread; cancellation or a closed window ends it.
#[cfg(feature = "native")]
pub(crate) fn deliver(
    cancel: &AtomicBool,
    mut send: impl FnMut() -> Result<(), UiSendError>,
) -> bool {
    loop {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        match send() {
            Ok(()) => return true,
            Err(UiSendError::Closed) => return false,
            Err(UiSendError::Full) => std::thread::sleep(Duration::from_millis(4)),
        }
    }
}

/// Worker events arrive on the UI thread in order.
pub fn update(app: &mut App, event: Event) {
    let Some(run) = app.run.as_ref().filter(|r| r.id == event.run) else {
        return;
    };
    let (thread, active) = (run.thread, app.active);
    let Some(t) = app.thread_mut(thread) else {
        app.run = None;
        return;
    };
    let Some(m) = t.messages.last_mut().filter(|m| m.streaming) else {
        app.run = None;
        return;
    };
    let mut finished = false;
    match event.kind {
        EventKind::Step(label) => {
            if m.steps.len() < MAX_STEPS {
                let mut bounded = String::new();
                append_bounded(&mut bounded, &label, 512);
                m.steps.push(Step {
                    label: bounded,
                    detail: String::new(),
                    done: false,
                });
            }
        }
        EventKind::StepDone(detail) => {
            if let Some(s) = m.steps.last_mut() {
                s.detail.clear();
                append_bounded(&mut s.detail, &detail, 1024);
                s.done = true;
            }
        }
        EventKind::File(mut file) => {
            if m.files.len() < MAX_FILES {
                let mut path = String::new();
                append_bounded(&mut path, &file.path, 512);
                file.path = path;
                m.files.push(file);
            }
        }
        EventKind::Text(chunk) => append_bounded(&mut m.text, &chunk, MAX_REPLY_BYTES),
        EventKind::Done(secs) => {
            m.streaming = false;
            m.worked = Some(secs);
            finished = true;
        }
    }
    if finished {
        if thread != active {
            t.unread = true;
        }
        app.run = None;
    }
}

fn rgba(v: u32) -> [f32; 4] {
    [
        ((v >> 16) & 255) as f32 / 255.,
        ((v >> 8) & 255) as f32 / 255.,
        (v & 255) as f32 / 255.,
        1.,
    ]
}

/// Quiet graphite surfaces with an indigo accent.
pub fn theme(app: &App, system: ThemeMode) -> UiTheme {
    let dark = match app.mode {
        ThemeMode::System => system == ThemeMode::Dark,
        mode => mode == ThemeMode::Dark,
    };
    let mut p = if dark {
        Palette::dark()
    } else {
        Palette::light()
    };
    p.name = "Relay".into();
    if dark {
        p.base = [rgba(0x111216), rgba(0x1A1B20), rgba(0x24262C)];
        p.content = rgba(0xECECEE);
        p.muted = rgba(0x8E9099);
        p.border = rgba(0x2A2C33);
    } else {
        p.base = [rgba(0xFFFFFF), rgba(0xF4F4F6), rgba(0xE9E9ED)];
        p.content = rgba(0x1C1C1F);
        p.muted = rgba(0x6E6F78);
        p.border = rgba(0xE4E4E9);
    }
    p.success = rgba(if dark { 0x4ADE80 } else { 0x16A34A });
    p.error = rgba(if dark { 0xF87171 } else { 0xDC2626 });
    p.warning = rgba(if dark { 0xF5A524 } else { 0xD97706 });
    UiTheme::from_palette(p.with_accent(rgba(if dark { 0x8B95FF } else { 0x4F5BD5 })))
}
