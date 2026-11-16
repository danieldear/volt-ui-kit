//! Secure storage and device-owner confirmation for Volt UI Kit apps.
//!
//! - [`secrets`]: keep API keys, tokens and passwords in the system's store
//!   (the login Keychain on macOS) instead of files or settings.
//! - [`local_auth`]: ask the person at the computer to confirm with Touch ID or
//!   their account password before something sensitive.
//! - [`apple_id`]: Sign in with Apple, for apps packaged with a provisioning
//!   profile that grants it.
//!
//! macOS is supported. Other platforms compile and report `Unsupported`; their
//! stores (Secret Service on Linux, Credential Manager and Windows Hello on
//! Windows) are planned.
//!
//! Every callback runs on a background thread. Send its result to your UI
//! with a message (`UiSender` in volt-ui-kit), never edit UI state from it.
//!
//! Unsafe code is limited to the macOS backend modules, which call the
//! Security, LocalAuthentication and AuthenticationServices frameworks; each
//! call states why it is sound.
#![deny(unsafe_code)]

pub mod apple_id;
pub mod local_auth;
pub mod secrets;
