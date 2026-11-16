//! Small secrets kept by the operating system: API keys, tokens, passwords.
//!
//! On macOS items go to the user's login Keychain,
//! one item per [`SecretKey`] (a service and an account name). Other
//! platforms report [`SecretError::Unsupported`] until their stores (Secret
//! Service on Linux, Credential Manager on Windows) are added.
//!
//! Every operation runs in order on one background worker, because the OS may
//! show a prompt and wait for an answer: macOS asks before an app it doesn't
//! recognise reads an item, which includes a development build that was
//! rebuilt since it saved. Callbacks run on that worker, not the UI thread;
//! forward results to the UI with a message.
//!
//! ```no_run
//! use volt_ui_kit_security::secrets::{self, Secret, SecretKey};
//!
//! fn remember(token: String) -> Result<(), secrets::SecretError> {
//!     let key = SecretKey::new("com.example.maps", "mapbox")?;
//!     secrets::store(&key, Secret::new(token), |saved| {
//!         // Marshal into your UI mailbox; this thread must not edit UI state.
//!         let _ = saved;
//!     })?;
//!     secrets::load(&key, |found| {
//!         // Ok(Some(secret)), Ok(None) when nothing is stored, or an error.
//!         let _ = found.map(|s| s.map(|s| s.len()));
//!     })
//! }
//! ```
use std::{
    fmt,
    sync::{
        mpsc::{self, SyncSender, TrySendError},
        Mutex, OnceLock,
    },
};

/// The largest secret accepted, in bytes: the smallest limit among the
/// platform stores (Windows' Credential Manager), so a value that fits on one
/// platform fits on all of them.
pub const MAX_SECRET_LEN: usize = 2560;
/// The longest service or account name accepted, in bytes.
pub const MAX_NAME_LEN: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SecretCapabilities {
    /// Store, load and remove are compiled in for this platform.
    pub storage: bool,
    /// Items copy to the user's other devices. Always false: items stay on
    /// this device.
    pub synced: bool,
}

/// What this build can do, not whether the store is unlocked or allowed.
pub fn capabilities() -> SecretCapabilities {
    SecretCapabilities {
        storage: cfg!(target_os = "macos"),
        synced: false,
    }
}

/// Where a secret lives: a service, normally the app's reverse-DNS id, and an
/// account within it. Keychain Access lists the item under the service name.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SecretKey {
    service: String,
    account: String,
}
impl SecretKey {
    /// Names must be 1 to 128 bytes, not blank and without control characters.
    pub fn new(
        service: impl Into<String>,
        account: impl Into<String>,
    ) -> Result<Self, SecretError> {
        let key = Self {
            service: service.into(),
            account: account.into(),
        };
        for name in [&key.service, &key.account] {
            if name.trim().is_empty()
                || name.len() > MAX_NAME_LEN
                || name.chars().any(char::is_control)
            {
                return Err(SecretError::InvalidKey);
            }
        }
        Ok(key)
    }
    pub fn service(&self) -> &str {
        &self.service
    }
    pub fn account(&self) -> &str {
        &self.account
    }
}

/// Secret bytes. The buffer is overwritten with zeros when dropped and never
/// printed by `Debug`. Copies the OS makes are outside the kit's control.
pub struct Secret(Vec<u8>);
impl Secret {
    /// Takes ownership without copying, so a `String` or `Vec` moves in.
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self(bytes.into())
    }
    pub fn expose(&self) -> &[u8] {
        &self.0
    }
    /// The secret as text, when it is UTF-8.
    pub fn expose_text(&self) -> Option<&str> {
        std::str::from_utf8(&self.0).ok()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
impl Drop for Secret {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        for byte in self.0.iter_mut() {
            // Volatile, so the compiler can't drop the write as dead.
            // SAFETY: `byte` is a valid, aligned, exclusive reference.
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
    }
}
impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretError {
    /// The service or account name is blank, too long or has control characters.
    InvalidKey,
    /// The secret is empty or longer than [`MAX_SECRET_LEN`].
    InvalidSecret,
    /// No secret store is compiled in for this platform.
    Unsupported,
    /// The person declined the system's access prompt.
    Canceled,
    /// The store refused: locked, no prompt allowed, or access denied.
    Denied,
    /// Too many operations are waiting; try again later.
    Busy,
    /// Any other failure, with the platform's status code (0 when it has none).
    NativeFailure(i32),
}
impl fmt::Display for SecretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKey => f.write_str("secret service and account names must be 1 to 128 bytes, not blank, without control characters"),
            Self::InvalidSecret => write!(f, "secrets must be 1 to {MAX_SECRET_LEN} bytes"),
            Self::Unsupported => f.write_str("no secret store is available on this platform yet"),
            Self::Canceled => f.write_str("access to the secret was declined"),
            Self::Denied => f.write_str("the secret store refused access; it may be locked"),
            Self::Busy => f.write_str("too many secret operations are waiting"),
            Self::NativeFailure(code) => write!(f, "the secret store failed (status {code})"),
        }
    }
}
impl std::error::Error for SecretError {}

/// Save `secret` under `key`, replacing what was there. Success means the
/// store wrote it.
pub fn store(
    key: &SecretKey,
    secret: Secret,
    callback: impl FnOnce(Result<(), SecretError>) + Send + 'static,
) -> Result<(), SecretError> {
    if secret.is_empty() || secret.len() > MAX_SECRET_LEN {
        return Err(SecretError::InvalidSecret);
    }
    supported()?;
    let key = key.clone();
    submit(Box::new(move || {
        callback(backend::store(&key, secret.expose()))
    }))
}

/// Read the secret under `key`: `Ok(None)` when nothing is stored there.
pub fn load(
    key: &SecretKey,
    callback: impl FnOnce(Result<Option<Secret>, SecretError>) + Send + 'static,
) -> Result<(), SecretError> {
    supported()?;
    let key = key.clone();
    submit(Box::new(move || callback(backend::load(&key))))
}

/// Delete the secret under `key`. Removing a missing secret succeeds.
pub fn remove(
    key: &SecretKey,
    callback: impl FnOnce(Result<(), SecretError>) + Send + 'static,
) -> Result<(), SecretError> {
    supported()?;
    let key = key.clone();
    submit(Box::new(move || callback(backend::remove(&key))))
}

fn supported() -> Result<(), SecretError> {
    if capabilities().storage {
        Ok(())
    } else {
        Err(SecretError::Unsupported)
    }
}

type Job = Box<dyn FnOnce() + Send>;
/// How many operations may wait before `Busy`.
const QUEUE: usize = 16;

/// One worker, so a store followed by a load sees the stored value, and a
/// prompt waiting for an answer holds up only secret operations.
fn submit(job: Job) -> Result<(), SecretError> {
    static WORKER: OnceLock<Mutex<Option<SyncSender<Job>>>> = OnceLock::new();
    let mut slot = WORKER
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let sender = match slot.as_ref() {
        Some(sender) => sender.clone(),
        None => {
            let (sender, jobs) = mpsc::sync_channel::<Job>(QUEUE);
            std::thread::Builder::new()
                .name("volt-secrets".into())
                .spawn(move || {
                    for job in jobs {
                        // A panicking callback must not stop later operations.
                        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
                    }
                })
                .map_err(|_| SecretError::NativeFailure(0))?;
            slot.insert(sender).clone()
        }
    };
    match sender.try_send(job) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(_)) => Err(SecretError::Busy),
        Err(TrySendError::Disconnected(_)) => {
            *slot = None;
            Err(SecretError::NativeFailure(0))
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod backend {
    use super::*;
    pub(super) fn store(_: &SecretKey, _: &[u8]) -> Result<(), SecretError> {
        Err(SecretError::Unsupported)
    }
    pub(super) fn load(_: &SecretKey) -> Result<Option<Secret>, SecretError> {
        Err(SecretError::Unsupported)
    }
    pub(super) fn remove(_: &SecretKey) -> Result<(), SecretError> {
        Err(SecretError::Unsupported)
    }
}

/// The login Keychain through `SecItem`: generic password items that stay on
/// this Mac. The data-protection Keychain needs a provisioning profile, so
/// unsigned development builds couldn't use it.
#[cfg(target_os = "macos")]
#[allow(unsafe_code, non_upper_case_globals)] // Apple's constant names
mod backend {
    use super::*;
    use objc2_core_foundation::{CFBoolean, CFData, CFDictionary, CFRetained, CFString, CFType};
    use objc2_security::{
        errSecAuthFailed, errSecInteractionNotAllowed, errSecItemNotFound, errSecSuccess,
        errSecUserCanceled, kSecAttrAccount, kSecAttrLabel, kSecAttrService, kSecClass,
        kSecClassGenericPassword, kSecMatchLimit, kSecMatchLimitOne, kSecReturnData, kSecValueData,
        SecItemAdd, SecItemCopyMatching, SecItemDelete, SecItemUpdate,
    };

    fn error(status: i32) -> SecretError {
        match status {
            errSecUserCanceled => SecretError::Canceled,
            errSecAuthFailed | errSecInteractionNotAllowed => SecretError::Denied,
            code => SecretError::NativeFailure(code),
        }
    }
    fn dictionary(pairs: &[(&CFString, &CFType)]) -> CFRetained<CFDictionary<CFString, CFType>> {
        let (keys, values): (Vec<_>, Vec<_>) = pairs.iter().copied().unzip();
        CFDictionary::from_slices(&keys, &values)
    }
    /// The attributes that pick out one item.
    struct Item {
        service: CFRetained<CFString>,
        account: CFRetained<CFString>,
    }
    fn item(key: &SecretKey) -> Item {
        Item {
            service: CFString::from_str(key.service()),
            account: CFString::from_str(key.account()),
        }
    }
    fn cf<T: AsRef<CFType> + ?Sized>(value: &T) -> &CFType {
        value.as_ref()
    }
    fn query<'a>(
        item: &'a Item,
        extra: &[(&'a CFString, &'a CFType)],
    ) -> CFRetained<CFDictionary<CFString, CFType>> {
        // SAFETY (each static): framework constants, valid for the process.
        let mut pairs: Vec<(&CFString, &CFType)> = vec![
            (
                unsafe { kSecClass },
                cf(unsafe { kSecClassGenericPassword }),
            ),
            (unsafe { kSecAttrService }, cf(&*item.service)),
            (unsafe { kSecAttrAccount }, cf(&*item.account)),
        ];
        pairs.extend_from_slice(extra);
        dictionary(&pairs)
    }

    pub(super) fn store(key: &SecretKey, secret: &[u8]) -> Result<(), SecretError> {
        let item = item(key);
        let data = CFData::from_bytes(secret);
        let found = query(&item, &[]);
        let change = dictionary(&[(unsafe { kSecValueData }, cf(&*data))]);
        // SAFETY: both dictionaries hold CFString keys and CF values.
        let status = unsafe { SecItemUpdate(found.as_opaque(), change.as_opaque()) };
        if status != errSecItemNotFound {
            return if status == errSecSuccess {
                Ok(())
            } else {
                Err(error(status))
            };
        }
        let label = CFString::from_str(key.service());
        let add = query(
            &item,
            &[
                (unsafe { kSecAttrLabel }, cf(&*label)),
                (unsafe { kSecValueData }, cf(&*data)),
            ],
        );
        // SAFETY: a valid attribute dictionary; no result is requested.
        match unsafe { SecItemAdd(add.as_opaque(), std::ptr::null_mut()) } {
            errSecSuccess => Ok(()),
            status => Err(error(status)),
        }
    }

    pub(super) fn load(key: &SecretKey) -> Result<Option<Secret>, SecretError> {
        let item = item(key);
        let found = query(
            &item,
            &[
                (unsafe { kSecReturnData }, cf(CFBoolean::new(true))),
                (unsafe { kSecMatchLimit }, cf(unsafe { kSecMatchLimitOne })),
            ],
        );
        let mut result: *const CFType = std::ptr::null();
        // SAFETY: a valid query; on success `result` holds a +1 reference.
        let status = unsafe { SecItemCopyMatching(found.as_opaque(), &mut result) };
        match status {
            errSecSuccess => {}
            errSecItemNotFound => return Ok(None),
            status => return Err(error(status)),
        }
        let result =
            std::ptr::NonNull::new(result.cast_mut()).ok_or(SecretError::NativeFailure(0))?;
        // SAFETY: the Copy rule: SecItemCopyMatching returns an owned reference.
        let result = unsafe { CFRetained::from_raw(result) };
        let data = result
            .downcast::<CFData>()
            .map_err(|_| SecretError::NativeFailure(0))?;
        Ok(Some(Secret::new(data.to_vec())))
    }

    pub(super) fn remove(key: &SecretKey) -> Result<(), SecretError> {
        let item = item(key);
        let found = query(&item, &[]);
        // SAFETY: a valid query dictionary.
        match unsafe { SecItemDelete(found.as_opaque()) } {
            errSecSuccess | errSecItemNotFound => Ok(()),
            status => Err(error(status)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_reject_blank_long_and_control_names() {
        assert!(SecretKey::new("com.example.maps", "mapbox").is_ok());
        assert!(SecretKey::new("com.example.maps", "a".repeat(MAX_NAME_LEN)).is_ok());
        for (service, account) in [
            ("", "mapbox"),
            ("com.example.maps", "  "),
            ("com.example.maps", "map\nbox"),
            ("com.example\0maps", "mapbox"),
        ] {
            assert_eq!(
                SecretKey::new(service, account),
                Err(SecretError::InvalidKey)
            );
        }
        let long = "a".repeat(MAX_NAME_LEN + 1);
        assert_eq!(SecretKey::new(long, "x"), Err(SecretError::InvalidKey));
    }

    #[test]
    fn secrets_never_print_and_stay_within_every_store() {
        let secret = Secret::new("pk.live-token".to_owned());
        assert_eq!(format!("{secret:?}"), "Secret(<redacted>)");
        assert_eq!(secret.expose_text(), Some("pk.live-token"));
        let key = SecretKey::new("com.example.maps", "mapbox").unwrap();
        for wrong in [Vec::new(), vec![1; MAX_SECRET_LEN + 1]] {
            assert_eq!(
                store(&key, Secret::new(wrong), |_| {}),
                Err(SecretError::InvalidSecret)
            );
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn other_platforms_say_unsupported_instead_of_pretending() {
        let key = SecretKey::new("com.example.maps", "mapbox").unwrap();
        assert!(!capabilities().storage);
        assert_eq!(
            store(&key, Secret::new("x"), |_| unreachable!()),
            Err(SecretError::Unsupported)
        );
        assert_eq!(
            load(&key, |_| unreachable!()),
            Err(SecretError::Unsupported)
        );
        assert_eq!(
            remove(&key, |_| unreachable!()),
            Err(SecretError::Unsupported)
        );
    }

    /// Writes one item to the login Keychain and removes it again. Ignored by
    /// default: CI machines have no unlocked login Keychain. Run with
    /// `cargo test secrets -- --ignored` in this crate.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "uses the login Keychain"]
    fn a_secret_round_trips_through_the_keychain_in_order() {
        let key = SecretKey::new("dev.volt.secrets-test", "round-trip").unwrap();
        let (sender, results) = mpsc::channel();
        let step = |label: &'static str| {
            let sender = sender.clone();
            move |result: Result<Option<String>, SecretError>| {
                sender.send((label, result)).unwrap();
            }
        };
        let saved = step("store");
        store(&key, Secret::new("first"), move |r| saved(r.map(|()| None))).unwrap();
        let replaced = step("replace");
        store(&key, Secret::new("second"), move |r| {
            replaced(r.map(|()| None))
        })
        .unwrap();
        let read = step("load");
        load(&key, move |r| {
            read(r.map(|s| s.and_then(|s| s.expose_text().map(str::to_owned))))
        })
        .unwrap();
        let removed = step("remove");
        remove(&key, move |r| removed(r.map(|()| None))).unwrap();
        let gone = step("load again");
        load(&key, move |r| {
            gone(r.map(|s| s.map(|_| "still there".to_owned())))
        })
        .unwrap();
        let got: Vec<_> = (0..5).map(|_| results.recv().unwrap()).collect();
        assert_eq!(
            got,
            [
                ("store", Ok(None)),
                ("replace", Ok(None)),
                ("load", Ok(Some("second".to_owned()))),
                ("remove", Ok(None)),
                ("load again", Ok(None)),
            ]
        );
    }
}
