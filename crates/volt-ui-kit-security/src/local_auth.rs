//! Ask the person at the computer to confirm it's them: Touch ID, or their
//! account password, before showing or using something sensitive.
//!
//! macOS shows its own sheet: "<App> is trying to <reason>." Pass a short,
//! lowercase reason that completes that sentence, such as "show your saved
//! keys". Other platforms report [`LocalAuthError::Unsupported`].
//!
//! A successful confirmation proves the person could unlock this Mac just now.
//! It is a gate in your app, not encryption: store the data itself with
//! [`crate::secrets`].
//!
//! ```no_run
//! use volt_ui_kit_security::local_auth::{self, Policy};
//!
//! fn reveal() -> Result<(), local_auth::LocalAuthError> {
//!     local_auth::confirm("show your saved keys", Policy::BiometricsOrPassword, |result| {
//!         // Runs on a framework thread: send the result to the UI as a message.
//!         let _ = result;
//!     })
//! }
//! ```
use std::{
    fmt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

/// The longest reason accepted, in characters.
pub const MAX_REASON_CHARS: usize = 150;

/// How the person may confirm.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Policy {
    /// Touch ID or a paired Apple Watch, with the account password as the
    /// fallback. Works on every Mac, including ones without Touch ID.
    #[default]
    BiometricsOrPassword,
    /// Touch ID only; the sheet offers no password. Fails with
    /// [`LocalAuthError::NotAvailable`] on Macs without a sensor.
    Biometrics,
}

/// The biometric sensor the device has, when one is set up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Biometry {
    /// None, or none usable: confirmation uses the password.
    None,
    TouchId,
    FaceId,
    OpticId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalAuthCapabilities {
    /// [`confirm`] and [`check`] are compiled in for this platform.
    pub confirm: bool,
}

/// What this build can do, not whether a sensor is set up: use [`check`].
pub fn capabilities() -> LocalAuthCapabilities {
    LocalAuthCapabilities {
        confirm: cfg!(target_os = "macos"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalAuthError {
    /// The reason is blank, longer than [`MAX_REASON_CHARS`] or has control
    /// characters.
    InvalidReason,
    /// No confirmation service is compiled in for this platform.
    Unsupported,
    /// No sensor, or it is disconnected (for [`Policy::Biometrics`]).
    NotAvailable,
    /// The sensor has no fingerprints set up.
    NotEnrolled,
    /// The account has no password set.
    NoPassword,
    /// Too many failed attempts; the system requires the password first.
    Locked,
    /// The person, the system or the app closed the sheet.
    Canceled,
    /// The person didn't confirm: wrong finger or password too many times.
    Failed,
    /// The sheet can't be shown right now.
    NotInteractive,
    /// Another confirmation is already on screen.
    Busy,
    /// Any other failure, with the framework's error code.
    NativeFailure(isize),
}
impl fmt::Display for LocalAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidReason => write!(
                f,
                "the reason must be 1 to {MAX_REASON_CHARS} characters, without control characters"
            ),
            Self::Unsupported => {
                f.write_str("confirming the device owner isn't available on this platform yet")
            }
            Self::NotAvailable => f.write_str("this device has no usable biometric sensor"),
            Self::NotEnrolled => f.write_str("no fingerprints are set up"),
            Self::NoPassword => f.write_str("the account has no password set"),
            Self::Locked => {
                f.write_str("too many failed attempts; enter the password to unlock biometrics")
            }
            Self::Canceled => f.write_str("the confirmation was canceled"),
            Self::Failed => f.write_str("the person didn't confirm"),
            Self::NotInteractive => f.write_str("the confirmation sheet can't be shown right now"),
            Self::Busy => f.write_str("another confirmation is already on screen"),
            Self::NativeFailure(code) => write!(f, "confirmation failed (error {code})"),
        }
    }
}
impl std::error::Error for LocalAuthError {}

/// Whether `policy` can be used right now, and which sensor it would use.
/// Shows nothing. Ask again when you need it; the answer can change.
pub fn check(policy: Policy) -> Result<Biometry, LocalAuthError> {
    backend::check(policy)
}

/// Show the system's confirmation sheet. The callback runs once, on a
/// framework thread, with `Ok(())` when the person confirmed. On an
/// immediate error (invalid reason, unsupported, busy) it isn't called.
pub fn confirm(
    reason: &str,
    policy: Policy,
    callback: impl FnOnce(Result<(), LocalAuthError>) + Send + 'static,
) -> Result<(), LocalAuthError> {
    validate_reason(reason)?;
    if !capabilities().confirm {
        return Err(LocalAuthError::Unsupported);
    }
    if SHOWING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err(LocalAuthError::Busy);
    }
    let callback = Mutex::new(Some(callback));
    let finish = move |result| {
        SHOWING.store(false, Ordering::Release);
        once(&callback, result);
    };
    backend::confirm(reason, policy, Box::new(finish)).inspect_err(|_| {
        SHOWING.store(false, Ordering::Release);
    })
}

/// One sheet at a time: a second request while one is up would be refused or
/// stacked by the system, so it fails early with `Busy` instead.
static SHOWING: AtomicBool = AtomicBool::new(false);

type Finish = Box<dyn Fn(Result<(), LocalAuthError>) + Send>;

/// Runs the callback at most once, even if a framework block is called again,
/// and releases the lock before user code runs.
pub(crate) fn once<T, F: FnOnce(T)>(slot: &Mutex<Option<F>>, value: T) {
    let callback = slot.lock().unwrap_or_else(|p| p.into_inner()).take();
    if let Some(callback) = callback {
        callback(value);
    }
}

fn validate_reason(reason: &str) -> Result<(), LocalAuthError> {
    if reason.trim().is_empty()
        || reason.chars().count() > MAX_REASON_CHARS
        || reason.chars().any(char::is_control)
    {
        Err(LocalAuthError::InvalidReason)
    } else {
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
mod backend {
    use super::*;
    pub(super) fn check(_: Policy) -> Result<Biometry, LocalAuthError> {
        Err(LocalAuthError::Unsupported)
    }
    pub(super) fn confirm(_: &str, _: Policy, _: Finish) -> Result<(), LocalAuthError> {
        Err(LocalAuthError::Unsupported)
    }
}

/// LocalAuthentication's `LAContext`: one context per check or confirmation.
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod backend {
    use super::*;
    use block2::RcBlock;
    use objc2::{rc::Retained, runtime::Bool};
    use objc2_foundation::{NSError, NSString};
    use objc2_local_authentication::{LABiometryType, LAContext, LAError, LAErrorDomain, LAPolicy};

    fn native(policy: Policy) -> LAPolicy {
        match policy {
            Policy::BiometricsOrPassword => LAPolicy::DeviceOwnerAuthentication,
            Policy::Biometrics => LAPolicy::DeviceOwnerAuthenticationWithBiometrics,
        }
    }

    fn error(error: &NSError) -> LocalAuthError {
        // SAFETY: a framework constant, valid for the process.
        if *error.domain() != *unsafe { LAErrorDomain } {
            return LocalAuthError::NativeFailure(error.code());
        }
        match LAError(error.code()) {
            LAError::UserCancel
            | LAError::UserFallback
            | LAError::SystemCancel
            | LAError::AppCancel => LocalAuthError::Canceled,
            LAError::AuthenticationFailed => LocalAuthError::Failed,
            LAError::PasscodeNotSet => LocalAuthError::NoPassword,
            LAError::BiometryNotAvailable
            | LAError::BiometryNotPaired
            | LAError::BiometryDisconnected
            | LAError::CompanionNotAvailable => LocalAuthError::NotAvailable,
            LAError::BiometryNotEnrolled => LocalAuthError::NotEnrolled,
            LAError::BiometryLockout => LocalAuthError::Locked,
            LAError::NotInteractive => LocalAuthError::NotInteractive,
            LAError(code) => LocalAuthError::NativeFailure(code),
        }
    }

    pub(super) fn check(policy: Policy) -> Result<Biometry, LocalAuthError> {
        // SAFETY: `new` returns an owned context; it may be used from any
        // thread. `canEvaluatePolicy` fills `biometryType`, read afterwards.
        let context = unsafe { LAContext::new() };
        unsafe { context.canEvaluatePolicy_error(native(policy)) }.map_err(|e| error(&e))?;
        Ok(match unsafe { context.biometryType() } {
            LABiometryType::TouchID => Biometry::TouchId,
            LABiometryType::FaceID => Biometry::FaceId,
            LABiometryType::OpticID => Biometry::OpticId,
            _ => Biometry::None,
        })
    }

    pub(super) fn confirm(
        reason: &str,
        policy: Policy,
        finish: Finish,
    ) -> Result<(), LocalAuthError> {
        // SAFETY: as in `check`.
        let context = unsafe { LAContext::new() };
        if policy == Policy::Biometrics {
            // An empty title hides the sheet's "Enter Password" button.
            // SAFETY: a valid string; the setter copies it.
            unsafe { context.setLocalizedFallbackTitle(Some(&NSString::from_str(""))) };
        }
        // The framework cancels an evaluation whose context is released, so
        // the reply keeps it until it runs, then lets it go: no cycle.
        let keep: Mutex<Option<Retained<LAContext>>> = Mutex::new(Some(context.clone()));
        let reply = RcBlock::new(move |confirmed: Bool, failure: *mut NSError| {
            let result = if confirmed.as_bool() {
                Ok(())
            } else {
                // SAFETY: the framework passes a valid NSError or null.
                Err(unsafe { failure.as_ref() }.map_or(LocalAuthError::NativeFailure(0), error))
            };
            let context = keep.lock().unwrap_or_else(|p| p.into_inner()).take();
            finish(result);
            drop(context);
        });
        // SAFETY: the reason was validated as non-empty (an empty one raises an
        // Objective-C exception). The block is `Send`-safe: it only touches a
        // Mutex and a `Send` callback, and runs once on a framework queue.
        unsafe {
            context.evaluatePolicy_localizedReason_reply(
                native(policy),
                &NSString::from_str(reason),
                &reply,
            )
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reasons_must_fit_the_system_sentence() {
        assert!(validate_reason("show your saved keys").is_ok());
        assert!(validate_reason(&"a".repeat(MAX_REASON_CHARS)).is_ok());
        for wrong in ["", "   ", "two\nlines", &"a".repeat(MAX_REASON_CHARS + 1)] {
            assert_eq!(validate_reason(wrong), Err(LocalAuthError::InvalidReason));
            assert_eq!(
                confirm(wrong, Policy::default(), |_| unreachable!()),
                Err(LocalAuthError::InvalidReason)
            );
        }
    }

    #[test]
    fn the_callback_runs_once_and_unlocks_before_running() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        let count = Arc::new(AtomicUsize::new(0));
        let counted = count.clone();
        type Callback = Box<dyn FnOnce(()) + Send>;
        let slot: Arc<Mutex<Option<Callback>>> = Arc::default();
        let weak = Arc::downgrade(&slot);
        *slot.lock().unwrap() = Some(Box::new(move |()| {
            assert!(weak.upgrade().unwrap().try_lock().unwrap().is_none());
            counted.fetch_add(1, Ordering::Relaxed);
        }));
        once(&slot, ());
        once(&slot, ());
        assert_eq!(count.load(Ordering::Relaxed), 1);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn other_platforms_say_unsupported_instead_of_pretending() {
        assert!(!capabilities().confirm);
        assert_eq!(check(Policy::default()), Err(LocalAuthError::Unsupported));
        assert_eq!(
            confirm("unlock", Policy::default(), |_| unreachable!()),
            Err(LocalAuthError::Unsupported)
        );
    }

    /// Asks the framework without showing anything: every Mac can confirm
    /// with the account password, unless it has none (some CI images).
    #[cfg(target_os = "macos")]
    #[test]
    fn a_mac_reports_what_it_can_confirm_with() {
        match check(Policy::BiometricsOrPassword) {
            Ok(_) | Err(LocalAuthError::NoPassword | LocalAuthError::NotInteractive) => {}
            other => panic!("unexpected: {other:?}"),
        }
        match check(Policy::Biometrics) {
            Ok(Biometry::TouchId)
            | Err(
                LocalAuthError::NotAvailable
                | LocalAuthError::NotEnrolled
                | LocalAuthError::Locked
                | LocalAuthError::NoPassword
                | LocalAuthError::NotInteractive,
            ) => {}
            other => panic!("unexpected: {other:?}"),
        }
    }
}
