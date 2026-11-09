//! What differs between a desktop window and a browser page.
//!
//! The maps code is shared by the desktop example (built with the kit's
//! `native` feature) and the website's live apps (built without it). On the
//! desktop, downloads run on worker threads and delayed messages on timer
//! threads; in a browser there are no threads, so the page fetches what the
//! app queued and drives timers and camera motion from its animation frames.
//! The clock follows: the desktop reads the system clock, the browser is
//! told the frame time.

/// Milliseconds on a clock that only moves forwards.
#[cfg(not(target_arch = "wasm32"))]
pub fn now() -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_secs_f64()
        * 1000.
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static NOW: std::cell::Cell<f64> = const { std::cell::Cell::new(0.) };
}

/// Milliseconds: the time of the latest animation frame.
#[cfg(target_arch = "wasm32")]
pub fn now() -> f64 {
    NOW.with(std::cell::Cell::get)
}

/// The browser host sets the clock at the start of each frame.
#[cfg(target_arch = "wasm32")]
pub fn set_now(ms: f64) {
    NOW.with(|now| now.set(ms));
}

/// MAPS_DEBUG: log messages, tiles and frames (desktop only).
pub fn debug() -> bool {
    cfg!(feature = "native") && std::env::var_os("MAPS_DEBUG").is_some()
}
