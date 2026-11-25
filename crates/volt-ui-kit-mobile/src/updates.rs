//! Changes to app state that arrive from other threads: a network reply, a
//! sync pass, a timer. Work runs wherever the app likes; the change itself
//! runs on the UI thread, and the host draws the result.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

type Change<C> = Box<dyn FnOnce(&mut C) + Send>;
/// Called from any thread when the first change of a batch is queued; the
/// host then asks the application to apply them on its UI thread.
pub(crate) type Wake = Arc<dyn Fn() + Send + Sync>;

pub(crate) struct Inbox<C> {
    changes: Mutex<Vec<Change<C>>>,
    wake: Mutex<Option<Wake>>,
    /// A wake is outstanding: later changes join its batch.
    pending: AtomicBool,
    /// The application is gone; changes are dropped.
    closed: AtomicBool,
}

/// Sends changes to the app's state from any thread. Clone it freely and
/// move it into background work. Each change runs once, on the UI thread, in
/// the order sent, and the page is drawn after each batch.
///
/// ```rust,ignore
/// let updates = app.updater();
/// std::thread::spawn(move || {
///     let reply = fetch();
///     updates.update(move |s: &mut State| s.reply = Some(reply));
/// });
/// ```
pub struct Updater<C> {
    inbox: Arc<Inbox<C>>,
}
impl<C> Clone for Updater<C> {
    fn clone(&self) -> Self {
        Self {
            inbox: self.inbox.clone(),
        }
    }
}
impl<C> std::fmt::Debug for Updater<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Updater")
            .field("open", &self.is_open())
            .finish()
    }
}
impl<C> Updater<C> {
    /// Queue a change. Returns false, dropping it, once the app has closed.
    pub fn update(&self, change: impl FnOnce(&mut C) + Send + 'static) -> bool {
        if self.inbox.closed.load(Ordering::Acquire) {
            return false;
        }
        match self.inbox.changes.lock() {
            Ok(mut changes) => changes.push(Box::new(change)),
            Err(_) => return false,
        }
        if !self.inbox.pending.swap(true, Ordering::AcqRel) {
            let wake = self.inbox.wake.lock().ok().and_then(|w| w.clone());
            if let Some(wake) = wake {
                wake();
            }
        }
        true
    }
    /// Whether the app is still running to receive changes.
    pub fn is_open(&self) -> bool {
        !self.inbox.closed.load(Ordering::Acquire)
    }
}

impl<C> Inbox<C> {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            changes: Mutex::new(Vec::new()),
            wake: Mutex::new(None),
            pending: AtomicBool::new(false),
            closed: AtomicBool::new(false),
        })
    }
    pub(crate) fn updater(self: &Arc<Self>) -> Updater<C> {
        Updater {
            inbox: self.clone(),
        }
    }
    /// Install the host's wake. Changes queued before it are woken now.
    pub(crate) fn set_wake(&self, wake: Option<Wake>) {
        if let Ok(mut slot) = self.wake.lock() {
            *slot = wake.clone();
        }
        let waiting = self.changes.lock().map(|c| !c.is_empty()).unwrap_or(false);
        if waiting {
            self.pending.store(true, Ordering::Release);
            if let Some(wake) = wake {
                wake();
            }
        }
    }
    /// Take every queued change; a later change wakes the host again.
    pub(crate) fn take(&self) -> Vec<Change<C>> {
        self.pending.store(false, Ordering::Release);
        self.changes
            .lock()
            .map(|mut c| std::mem::take(&mut *c))
            .unwrap_or_default()
    }
    pub(crate) fn has_changes(&self) -> bool {
        self.changes.lock().map(|c| !c.is_empty()).unwrap_or(false)
    }
    pub(crate) fn close(&self) {
        self.closed.store(true, Ordering::Release);
        if let Ok(mut wake) = self.wake.lock() {
            *wake = None;
        }
        if let Ok(mut changes) = self.changes.lock() {
            changes.clear();
        }
    }
}
