//! Sign in with Apple: the system sheet, then the person's stable Apple ID
//! user identifier and a signed identity token for your server.
//!
//! Apple allows this only for an app that carries the Sign in with Apple
//! entitlement and an embedded provisioning profile from your developer
//! account, so `cargo run` binaries get [`AppleIdError::NotEntitled`]. Package
//! with `cargo volt release --provisioning-profile FILE --entitlements FILE`.
//!
//! On macOS Apple grants the entitlement to Mac App Store apps and to
//! development builds (an Apple Development certificate and a development
//! profile listing the Mac), but not to apps signed with Developer ID. Apps
//! distributed outside the App Store sign in with Apple through the web flow
//! instead.
//!
//! Treat the result as a claim until your server has checked the identity
//! token: its signature (Apple's published keys), issuer, audience (your
//! bundle identifier), expiry and nonce. Email and name arrive only the first
//! time a person signs in to your app; save them then.
//!
//! ```no_run
//! use volt_ui_kit_security::apple_id::{self, AppleIdRequest};
//!
//! // From a button's callback, on the main thread.
//! fn start() -> Result<(), apple_id::AppleIdError> {
//!     let request = AppleIdRequest::new().full_name().email();
//!     apple_id::sign_in(&request, |result| {
//!         // Send to the UI with a message; post the identity token to your server.
//!         let _ = result.map(|credential| credential.user);
//!     })
//! }
//! ```
use crate::secrets::Secret;
use std::fmt;

/// The longest nonce or state accepted, in bytes.
pub const MAX_VALUE_LEN: usize = 256;

/// What to ask for. Email and name are offered to the person, who may hide
/// their email behind a private relay address or edit their name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppleIdRequest {
    full_name: bool,
    email: bool,
    nonce: Option<String>,
    state: Option<String>,
}
impl AppleIdRequest {
    pub fn new() -> Self {
        Self::default()
    }
    /// Ask for the person's name (first sign-in only).
    pub fn full_name(mut self) -> Self {
        self.full_name = true;
        self
    }
    /// Ask for the person's email (first sign-in only).
    pub fn email(mut self) -> Self {
        self.email = true;
        self
    }
    /// Copied into the identity token so your server can reject replays. Use
    /// the SHA-256 hex of a fresh random value your server issued.
    pub fn nonce(mut self, nonce: impl Into<String>) -> Self {
        self.nonce = Some(nonce.into());
        self
    }
    /// Returned unchanged in the credential, to match a reply to its request.
    pub fn state(mut self, state: impl Into<String>) -> Self {
        self.state = Some(state.into());
        self
    }
    fn validate(&self) -> Result<(), AppleIdError> {
        for value in [&self.nonce, &self.state].into_iter().flatten() {
            if value.trim().is_empty()
                || value.len() > MAX_VALUE_LEN
                || value.chars().any(char::is_control)
            {
                return Err(AppleIdError::InvalidRequest);
            }
        }
        Ok(())
    }
}

/// Apple's guess at whether a real person signed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RealUser {
    /// This platform can't tell.
    Unsupported,
    Unknown,
    LikelyReal,
}

/// A successful sign-in. `Debug` hides the token and code.
pub struct AppleIdCredential {
    /// Stable for this person across your team's apps. Use it as the account key.
    pub user: String,
    /// A signed JWT for your server to verify. Not proof until it has.
    pub identity_token: Option<Secret>,
    /// A single-use code, valid for five minutes, that your server can trade
    /// with Apple for refresh tokens.
    pub authorization_code: Option<Secret>,
    /// First sign-in only. May be a private relay address.
    pub email: Option<String>,
    /// First sign-in only.
    pub full_name: Option<String>,
    /// The request's state, unchanged.
    pub state: Option<String>,
    pub real_user: RealUser,
}
impl fmt::Debug for AppleIdCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppleIdCredential")
            .field("user", &self.user)
            .field("identity_token", &self.identity_token)
            .field("authorization_code", &self.authorization_code)
            .field("email", &self.email.as_ref().map(|_| "<present>"))
            .field("full_name", &self.full_name.as_ref().map(|_| "<present>"))
            .field("state", &self.state)
            .field("real_user", &self.real_user)
            .finish()
    }
}

/// Whether a user identifier still has access to your app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CredentialState {
    Authorized,
    /// The person stopped using Sign in with Apple with your app: sign them out.
    Revoked,
    NotFound,
    /// The app moved to another developer team; the identifier must be migrated.
    Transferred,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppleIdCapabilities {
    /// [`sign_in`] and [`credential_state`] are compiled in for this platform.
    pub sign_in: bool,
}

pub fn capabilities() -> AppleIdCapabilities {
    AppleIdCapabilities {
        sign_in: cfg!(target_os = "macos"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppleIdError {
    /// The nonce or state is blank, too long or has control characters.
    InvalidRequest,
    /// Not available on this platform.
    Unsupported,
    /// `sign_in` must be called on the main thread.
    WrongThread,
    /// The running app lacks the Sign in with Apple entitlement: it wasn't
    /// packaged with a provisioning profile that grants it.
    NotEntitled,
    /// The app has no window to show the sheet over.
    NoWindow,
    /// Another sign-in sheet is already open.
    Busy,
    /// The person closed the sheet.
    Canceled,
    /// Apple refused the request.
    Failed,
    /// Apple's reply couldn't be read.
    InvalidResponse,
    /// The request wasn't handled.
    NotHandled,
    /// The sheet can't be shown right now.
    NotInteractive,
    /// Any other failure, with the framework's error code (1000 is Apple's
    /// "unknown", often a provisioning or account problem).
    NativeFailure(isize),
}
impl fmt::Display for AppleIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest => write!(f, "the nonce and state must be 1 to {MAX_VALUE_LEN} bytes, without control characters"),
            Self::Unsupported => f.write_str("Sign in with Apple isn't available on this platform"),
            Self::WrongThread => f.write_str("start Sign in with Apple on the main thread"),
            Self::NotEntitled => f.write_str("this app isn't signed with the Sign in with Apple entitlement; package it with a provisioning profile"),
            Self::NoWindow => f.write_str("there is no window to show the sign-in sheet over"),
            Self::Busy => f.write_str("a sign-in sheet is already open"),
            Self::Canceled => f.write_str("sign-in was canceled"),
            Self::Failed => f.write_str("Apple refused the sign-in"),
            Self::InvalidResponse => f.write_str("Apple's reply couldn't be read"),
            Self::NotHandled => f.write_str("the sign-in request wasn't handled"),
            Self::NotInteractive => f.write_str("the sign-in sheet can't be shown right now"),
            Self::NativeFailure(code) => write!(f, "sign-in failed (error {code})"),
        }
    }
}
impl std::error::Error for AppleIdError {}

/// Whether the running app carries the Sign in with Apple entitlement. A
/// `false` here means `sign_in` will fail with `NotEntitled`.
pub fn entitled() -> bool {
    backend::entitled()
}

/// Show Apple's sign-in sheet over the app's key window. Call on the main
/// thread. The callback runs once, on the main thread, after the person
/// finishes or cancels; immediate errors are returned instead.
pub fn sign_in(
    request: &AppleIdRequest,
    callback: impl FnOnce(Result<AppleIdCredential, AppleIdError>) + Send + 'static,
) -> Result<(), AppleIdError> {
    request.validate()?;
    if !capabilities().sign_in {
        return Err(AppleIdError::Unsupported);
    }
    backend::sign_in(request, Box::new(callback))
}

/// Ask Apple whether `user` still has access. Check at launch for a
/// signed-in person and sign them out when it says `Revoked`. The callback
/// runs on a framework thread.
pub fn credential_state(
    user: &str,
    callback: impl FnOnce(Result<CredentialState, AppleIdError>) + Send + 'static,
) -> Result<(), AppleIdError> {
    if user.trim().is_empty() || user.len() > MAX_VALUE_LEN || user.chars().any(char::is_control) {
        return Err(AppleIdError::InvalidRequest);
    }
    if !capabilities().sign_in {
        return Err(AppleIdError::Unsupported);
    }
    backend::credential_state(user, Box::new(callback))
}

type Reply = Box<dyn FnOnce(Result<AppleIdCredential, AppleIdError>) + Send>;
type StateReply = Box<dyn FnOnce(Result<CredentialState, AppleIdError>) + Send>;

#[cfg(not(target_os = "macos"))]
mod backend {
    use super::*;
    pub(super) fn entitled() -> bool {
        false
    }
    pub(super) fn sign_in(_: &AppleIdRequest, _: Reply) -> Result<(), AppleIdError> {
        Err(AppleIdError::Unsupported)
    }
    pub(super) fn credential_state(_: &str, _: StateReply) -> Result<(), AppleIdError> {
        Err(AppleIdError::Unsupported)
    }
}

/// AuthenticationServices: an `ASAuthorizationController` with a delegate,
/// written in Rust, that also anchors the sheet to the key window.
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod backend {
    use super::*;
    use block2::RcBlock;
    use objc2::{
        define_class, msg_send,
        rc::Retained,
        runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject},
        AllocAnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, Message,
    };
    use objc2_app_kit::{NSApplication, NSWindow};
    use objc2_authentication_services::{
        ASAuthorization, ASAuthorizationAppleIDCredential, ASAuthorizationAppleIDProvider,
        ASAuthorizationAppleIDProviderCredentialState as State, ASAuthorizationController,
        ASAuthorizationControllerDelegate, ASAuthorizationControllerPresentationContextProviding,
        ASAuthorizationError, ASAuthorizationErrorDomain, ASAuthorizationScopeEmail,
        ASAuthorizationScopeFullName, ASPresentationAnchor, ASUserDetectionStatus,
    };
    use objc2_core_foundation::CFString;
    use objc2_foundation::{NSArray, NSError, NSString};
    use objc2_security::SecTask;
    use std::{
        cell::{Cell, RefCell},
        sync::Mutex,
    };

    const ENTITLEMENT: &str = "com.apple.developer.applesignin";

    pub(super) fn entitled() -> bool {
        // SAFETY: the current task, no allocator; a null error pointer is
        // allowed. The copied value, if any, is owned and released here.
        let Some(task) = (unsafe { SecTask::from_self(None) }) else {
            return false;
        };
        let name = CFString::from_static_str(ENTITLEMENT);
        unsafe { task.value_for_entitlement(&name, std::ptr::null_mut()) }.is_some()
    }

    fn error(error: &NSError) -> AppleIdError {
        // SAFETY: a framework constant, valid for the process.
        if *error.domain() != *unsafe { ASAuthorizationErrorDomain } {
            return AppleIdError::NativeFailure(error.code());
        }
        match ASAuthorizationError(error.code()) {
            ASAuthorizationError::Canceled => AppleIdError::Canceled,
            ASAuthorizationError::Failed => AppleIdError::Failed,
            ASAuthorizationError::InvalidResponse => AppleIdError::InvalidResponse,
            ASAuthorizationError::NotHandled => AppleIdError::NotHandled,
            ASAuthorizationError::NotInteractive => AppleIdError::NotInteractive,
            ASAuthorizationError(code) => AppleIdError::NativeFailure(code),
        }
    }

    fn credential(authorization: &ASAuthorization) -> Result<AppleIdCredential, AppleIdError> {
        // SAFETY: plain getters on objects the framework handed us.
        let credential = unsafe { authorization.credential() };
        let object: &AnyObject = credential.as_ref();
        let apple = object
            .downcast_ref::<ASAuthorizationAppleIDCredential>()
            .ok_or(AppleIdError::InvalidResponse)?;
        let text = |s: Option<Retained<NSString>>| s.map(|s| s.to_string());
        let full_name = unsafe { apple.fullName() }.and_then(|name| {
            let parts: Vec<String> = [name.givenName(), name.middleName(), name.familyName()]
                .into_iter()
                .flatten()
                .map(|s| s.to_string())
                .filter(|s| !s.trim().is_empty())
                .collect();
            (!parts.is_empty()).then(|| parts.join(" "))
        });
        Ok(AppleIdCredential {
            user: unsafe { apple.user() }.to_string(),
            identity_token: unsafe { apple.identityToken() }.map(|d| Secret::new(d.to_vec())),
            authorization_code: unsafe { apple.authorizationCode() }
                .map(|d| Secret::new(d.to_vec())),
            email: text(unsafe { apple.email() }),
            full_name,
            state: text(unsafe { apple.state() }),
            real_user: match unsafe { apple.realUserStatus() } {
                ASUserDetectionStatus::LikelyReal => RealUser::LikelyReal,
                ASUserDetectionStatus::Unknown => RealUser::Unknown,
                _ => RealUser::Unsupported,
            },
        })
    }

    struct Ivars {
        reply: RefCell<Option<Reply>>,
        anchor: Retained<NSWindow>,
    }

    define_class!(
        // SAFETY: NSObject has no subclassing requirements and Delegate
        // doesn't implement Drop. Main-thread-only, as both protocols require.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[ivars = Ivars]
        struct Delegate;

        unsafe impl NSObjectProtocol for Delegate {}

        // SAFETY: the signatures match the protocol's declarations.
        unsafe impl ASAuthorizationControllerDelegate for Delegate {
            #[unsafe(method(authorizationController:didCompleteWithAuthorization:))]
            fn completed(
                &self,
                _controller: &ASAuthorizationController,
                authorization: &ASAuthorization,
            ) {
                self.finish(credential(authorization));
            }

            #[unsafe(method(authorizationController:didCompleteWithError:))]
            fn failed(&self, _controller: &ASAuthorizationController, failure: &NSError) {
                self.finish(Err(error(failure)));
            }
        }

        // SAFETY: as above.
        unsafe impl ASAuthorizationControllerPresentationContextProviding for Delegate {
            #[unsafe(method_id(presentationAnchorForAuthorizationController:))]
            fn anchor(
                &self,
                _controller: &ASAuthorizationController,
            ) -> Retained<ASPresentationAnchor> {
                Retained::into_super(Retained::into_super(self.ivars().anchor.clone()))
            }
        }
    );

    impl Delegate {
        fn new(mtm: MainThreadMarker, reply: Reply, anchor: Retained<NSWindow>) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(Ivars {
                reply: RefCell::new(Some(reply)),
                anchor,
            });
            // SAFETY: NSObject's designated initializer.
            unsafe { msg_send![super(this), init] }
        }
        fn finish(&self, result: Result<AppleIdCredential, AppleIdError>) {
            // Alive until this returns, even if the callback starts another
            // sign-in that replaces the stored delegate.
            let _alive = self.retain();
            SHOWING.with(|showing| showing.set(false));
            let reply = self.ivars().reply.borrow_mut().take();
            if let Some(reply) = reply {
                reply(result);
            }
        }
    }

    thread_local! {
        static SHOWING: Cell<bool> = const { Cell::new(false) };
        /// The controller holds its delegate weakly, so the latest flow is
        /// kept here until the next one replaces it.
        static LATEST: RefCell<Option<(Retained<ASAuthorizationController>, Retained<Delegate>)>> =
            const { RefCell::new(None) };
    }

    pub(super) fn sign_in(request: &AppleIdRequest, reply: Reply) -> Result<(), AppleIdError> {
        let mtm = MainThreadMarker::new().ok_or(AppleIdError::WrongThread)?;
        if !entitled() {
            return Err(AppleIdError::NotEntitled);
        }
        if SHOWING.with(Cell::get) {
            return Err(AppleIdError::Busy);
        }
        let app = NSApplication::sharedApplication(mtm);
        let anchor = app
            .keyWindow()
            .or_else(|| app.mainWindow())
            .or_else(|| app.windows().firstObject())
            .ok_or(AppleIdError::NoWindow)?;
        // SAFETY: ordinary object creation and property setters; scope
        // constants are framework statics valid for the process.
        let apple = unsafe { ASAuthorizationAppleIDProvider::new().createRequest() };
        let mut scopes: Vec<&NSString> = Vec::new();
        if request.full_name {
            scopes.push(unsafe { ASAuthorizationScopeFullName });
        }
        if request.email {
            scopes.push(unsafe { ASAuthorizationScopeEmail });
        }
        unsafe {
            apple.setRequestedScopes(Some(&NSArray::from_slice(&scopes)));
            apple.setNonce(request.nonce.as_deref().map(NSString::from_str).as_deref());
            apple.setState(request.state.as_deref().map(NSString::from_str).as_deref());
        }
        let requests =
            NSArray::from_retained_slice(&[Retained::into_super(Retained::into_super(apple))]);
        let controller = unsafe {
            ASAuthorizationController::initWithAuthorizationRequests(
                ASAuthorizationController::alloc(),
                &requests,
            )
        };
        let delegate = Delegate::new(mtm, reply, anchor);
        // SAFETY: the delegate outlives the flow: it's kept in LATEST.
        unsafe {
            controller.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
            controller.setPresentationContextProvider(Some(ProtocolObject::from_ref(&*delegate)));
        }
        SHOWING.with(|showing| showing.set(true));
        LATEST.with(|latest| latest.replace(Some((controller.clone(), delegate))));
        // SAFETY: on the main thread, with a delegate and anchor set.
        unsafe { controller.performRequests() };
        Ok(())
    }

    pub(super) fn credential_state(user: &str, reply: StateReply) -> Result<(), AppleIdError> {
        if !entitled() {
            return Err(AppleIdError::NotEntitled);
        }
        let slot = Mutex::new(Some(reply));
        let block = RcBlock::new(move |state: State, failure: *mut NSError| {
            let result = match state {
                State::Authorized => Ok(CredentialState::Authorized),
                State::Revoked => Ok(CredentialState::Revoked),
                // Apple also passes an error with NotFound; the state says it.
                State::NotFound => Ok(CredentialState::NotFound),
                State::Transferred => Ok(CredentialState::Transferred),
                // SAFETY: the framework passes a valid NSError or null.
                _ => Err(unsafe { failure.as_ref() }.map_or(AppleIdError::InvalidResponse, error)),
            };
            crate::local_auth::once(&slot, result);
        });
        // SAFETY: a valid user string; the block is copied by the framework
        // and only touches a Mutex and a `Send` callback.
        unsafe {
            ASAuthorizationAppleIDProvider::new()
                .getCredentialStateForUserID_completion(&NSString::from_str(user), &block)
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_reject_blank_long_and_control_values() {
        assert!(AppleIdRequest::new()
            .full_name()
            .email()
            .nonce("ab12")
            .state("s1")
            .validate()
            .is_ok());
        for wrong in ["", " ", "a\nb", &"a".repeat(MAX_VALUE_LEN + 1)] {
            assert_eq!(
                AppleIdRequest::new().nonce(wrong).validate(),
                Err(AppleIdError::InvalidRequest)
            );
            assert_eq!(
                credential_state(wrong, |_| unreachable!()),
                Err(AppleIdError::InvalidRequest)
            );
        }
    }

    #[test]
    fn credentials_never_print_tokens_or_personal_details() {
        let credential = AppleIdCredential {
            user: "001234.abcd".into(),
            identity_token: Some(Secret::new("eyJhbGciOi")),
            authorization_code: Some(Secret::new("c0de")),
            email: Some("someone@privaterelay.appleid.com".into()),
            full_name: Some("Someone".into()),
            state: None,
            real_user: RealUser::LikelyReal,
        };
        let printed = format!("{credential:?}");
        for hidden in ["eyJhbGciOi", "c0de", "someone@", "Someone"] {
            assert!(!printed.contains(hidden), "{printed}");
        }
        assert!(printed.contains("001234.abcd"));
    }

    /// A test binary isn't packaged with a provisioning profile, so it must
    /// say so up front instead of opening a sheet that fails.
    #[test]
    fn unpackaged_programs_are_not_entitled() {
        assert!(!entitled());
        let expected = if cfg!(target_os = "macos") {
            AppleIdError::NotEntitled
        } else {
            AppleIdError::Unsupported
        };
        assert_eq!(
            credential_state("001234.abcd", |_| unreachable!()),
            Err(expected)
        );
    }
}
