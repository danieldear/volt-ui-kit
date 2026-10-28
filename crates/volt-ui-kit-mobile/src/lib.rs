//! Experimental reusable mobile host. App state, views and callbacks stay Rust.
//!
//! The safe application/event layer is platform-neutral. Only the iOS adapter
//! currently exists; Android needs its own native-window, IME and lifecycle
//! adapter. This crate is separate so the main toolkit retains forbid(unsafe_code)
//! and unchanged default/desktop dependencies.
#![deny(unsafe_op_in_unsafe_fn)]
mod application;
mod protocol;
pub use application::{Application, HostContext};
pub use protocol::{PlatformEvent, HOST_ABI_VERSION};
pub use volt_ui_kit as ui_kit;

#[cfg(target_os = "ios")]
pub mod ios;

/// Export one Rust application factory to Volt's reusable iOS host.
///
/// The factory takes [`HostContext`] and returns `Result<Application<State>, String>`.
/// Use exactly once in a static-library app crate. The macro supplies the ABI
/// registration symbol; developers write no raw-pointer or UIKit code.
#[macro_export]
macro_rules! export_ios_app {
    ($factory:path) => {
        #[cfg(target_os = "ios")]
        #[no_mangle]
        pub extern "C" fn volt_app_register() -> bool {
            $crate::ios::register($factory)
        }
    };
}
