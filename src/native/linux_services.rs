//! Rust-only Linux session services. Probe on a worker, never while painting.
//! Portals choose the desktop's dialog implementation. No shell or GTK linkage.
#[cfg(feature = "native-desktop")]
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
use zbus::blocking::{Connection, Proxy};
#[cfg(feature = "native-desktop")]
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
pub(super) mod notifications;
#[cfg(feature = "native-desktop")]
pub(super) mod trash;
#[cfg(feature = "native-desktop")]
pub(super) mod tray;
static UI_THREAD: std::sync::OnceLock<std::thread::ThreadId> = std::sync::OnceLock::new();
pub(super) fn on_ui_thread() -> bool {
    UI_THREAD
        .get()
        .is_some_and(|id| *id == std::thread::current().id())
}
static APP_ID: std::sync::OnceLock<String> = std::sync::OnceLock::new();
pub(super) fn set_app_id(id: &str) {
    let _ = APP_ID.set(id.to_owned());
    let _ = UI_THREAD.set(std::thread::current().id());
}
pub(super) fn app_id() -> &'static str {
    APP_ID.get().map_or("dev.volt.application", String::as_str)
}
const PORTAL: &str = "org.freedesktop.portal.Desktop";
const DESKTOP: &str = "/org/freedesktop/portal/desktop";
static WORKERS: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "native-desktop")]
static TOKENS: AtomicUsize = AtomicUsize::new(0);
struct Permit;
impl Drop for Permit {
    fn drop(&mut self) {
        WORKERS.fetch_sub(1, Ordering::AcqRel);
    }
}
/// At most four concurrent explicit operations; portal requests expire after five minutes.
/// The callback runs once on a worker. A startup error never invokes it.
#[allow(deprecated)] // fetch_update is the spelling available at Rust 1.89.
pub fn spawn<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, String> + Send + 'static,
    callback: impl FnOnce(Result<T, String>) + Send + 'static,
) -> Result<(), String> {
    WORKERS
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            (n < 4).then_some(n + 1)
        })
        .map_err(|_| "Linux desktop service queue is busy".to_string())?;
    let permit = Permit;
    std::thread::Builder::new()
        .name("volt-desktop".into())
        .spawn(move || {
            let _permit = permit;
            callback(operation());
        })
        .map(|_| ())
        .map_err(|e| e.to_string())
}
// Bounded method calls, including a missing/unresponsive service. Portal user
// response waiting is separately timed; connection creation must run on a worker.
pub(super) fn bounded_connection() -> Result<Connection, String> {
    zbus::blocking::connection::Builder::session()
        .and_then(|b| b.method_timeout(Duration::from_secs(5)).build())
        .map_err(|e| e.to_string())
}
fn owner(c: &Connection, name: &str) -> bool {
    Proxy::new(
        c,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    )
    .and_then(|p| p.call::<_, _, bool>("NameHasOwner", &(name,)))
    .unwrap_or(false)
}
/// Runtime availability, not permission or proof of presentation. No service is
/// activated and no chooser/notification is opened by this query.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Services {
    pub file_chooser: bool,
    pub open_uri: bool,
    pub file_manager: bool,
    pub notifications: bool,
    pub tray: bool,
}
pub fn probe() -> Result<Services, String> {
    let c = bounded_connection()?;
    let property = |interface: &str| -> bool {
        owner(&c, PORTAL)
            && Proxy::new(&c, PORTAL, DESKTOP, interface)
                .and_then(|p| p.get_property::<u32>("version"))
                .is_ok()
    };
    Ok(Services {
        file_chooser: cfg!(feature = "native-desktop")
            && property("org.freedesktop.portal.FileChooser"),
        open_uri: cfg!(feature = "native-desktop") && property("org.freedesktop.portal.OpenURI"),
        file_manager: cfg!(feature = "native-desktop") && owner(&c, "org.freedesktop.FileManager1"),
        notifications: owner(&c, "org.freedesktop.Notifications"),
        tray: cfg!(feature = "native-desktop") && owner(&c, "org.kde.StatusNotifierWatcher"),
    })
}
#[cfg(feature = "native-desktop")]
pub(super) fn path_uri(path: &Path) -> Result<String, String> {
    use std::os::unix::ffi::OsStrExt;
    if !path.is_absolute() || path.as_os_str().as_bytes().contains(&0) {
        return Err("an absolute path without NUL is required".into());
    }
    let mut uri = String::from("file://");
    for b in path.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~".contains(b) {
            uri.push(*b as char)
        } else {
            uri.push_str(&format!("%{b:02X}"));
        }
    }
    Ok(uri)
}
#[cfg(feature = "native-desktop")]
fn uri_path(uri: &str) -> Result<PathBuf, String> {
    use std::os::unix::ffi::OsStringExt;
    if uri.len() > 65536 {
        return Err("portal path exceeds limit".into());
    }
    let rest = uri
        .strip_prefix("file://localhost/")
        .map(|v| format!("/{v}"))
        .or_else(|| uri.strip_prefix("file:///").map(|v| format!("/{v}")))
        .ok_or("portal returned a non-local file URI")?;
    if rest.contains(['?', '#']) {
        return Err("portal returned a malformed file URI".into());
    }
    let mut bytes = Vec::new();
    let raw = rest.as_bytes();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' {
            let s = raw.get(i + 1..i + 3).ok_or("truncated URI escape")?;
            let s = std::str::from_utf8(s).map_err(|_| "invalid URI escape")?;
            bytes.push(u8::from_str_radix(s, 16).map_err(|_| "invalid URI escape")?);
            i += 3;
        } else {
            bytes.push(raw[i]);
            i += 1;
        }
    }
    if bytes.contains(&0) {
        return Err("NUL in portal path".into());
    }
    Ok(std::ffi::OsString::from_vec(bytes).into())
}
#[cfg(feature = "native-desktop")]
fn request(
    c: &Connection,
    interface: &str,
    method: &str,
    title_or_uri: &str,
    mut options: HashMap<&str, Value<'_>>,
    fd: Option<&std::fs::File>,
) -> Result<HashMap<String, OwnedValue>, String> {
    use futures_lite::StreamExt;
    let token = format!(
        "volt_{}_{}",
        std::process::id(),
        TOKENS.fetch_add(1, Ordering::Relaxed)
    );
    let sender = c
        .unique_name()
        .ok_or("missing D-Bus identity")?
        .as_str()
        .trim_start_matches(':')
        .replace('.', "_");
    let path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
    options.insert("handle_token", Value::from(token));
    futures_lite::future::block_on(async {
        let portal = zbus::Proxy::new(c.inner(), PORTAL, DESKTOP, interface)
            .await
            .map_err(|e| e.to_string())?;
        let request = zbus::Proxy::new(
            c.inner(),
            PORTAL,
            path.as_str(),
            "org.freedesktop.portal.Request",
        )
        .await
        .map_err(|e| e.to_string())?;
        // Subscribe before calling: some portal backends answer immediately.
        let mut replies = request
            .receive_signal("Response")
            .await
            .map_err(|e| e.to_string())?;
        let handle: OwnedObjectPath = if let Some(file) = fd {
            portal
                .call(method, &("", zbus::zvariant::Fd::from(file), &options))
                .await
        } else {
            portal.call(method, &("", title_or_uri, &options)).await
        }
        .map_err(|e| e.to_string())?;
        if handle.as_str() != path {
            let _ = zbus::Proxy::new(
                c.inner(),
                PORTAL,
                handle.as_str(),
                "org.freedesktop.portal.Request",
            )
            .await
            .map_err(|e| e.to_string())?
            .call::<_, _, ()>("Close", &())
            .await;
            return Err("portal returned an unexpected request handle".into());
        }
        let reply = futures_lite::future::or(
            async {
                replies
                    .next()
                    .await
                    .ok_or_else(|| "portal response channel closed".to_string())
            },
            async {
                async_io::Timer::after(Duration::from_secs(300)).await;
                Err("portal request timed out".to_string())
            },
        )
        .await;
        let message = match reply {
            Ok(r) => r,
            Err(e) => {
                let _ = request.call::<_, _, ()>("Close", &()).await;
                return Err(e);
            }
        };
        let (code, values): (u32, HashMap<String, OwnedValue>) =
            message.body().deserialize().map_err(|e| e.to_string())?;
        match code {
            0 => Ok(values),
            1 => Err("cancelled".into()),
            _ => Err("desktop portal rejected the request".into()),
        }
    })
}
#[cfg(feature = "native-desktop")]
pub(super) fn choose(
    title: &str,
    directory: Option<&Path>,
    name: Option<&str>,
    folder: bool,
    save: bool,
) -> Result<Option<PathBuf>, String> {
    let c = bounded_connection()?;
    let mut options = HashMap::new();
    if !save {
        options.insert("multiple", Value::from(false));
        options.insert("directory", Value::from(folder));
    }
    if let Some(name) = name {
        options.insert("current_name", Value::from(name));
    }
    let directory_bytes = directory.map(|p| {
        use std::os::unix::ffi::OsStrExt;
        let mut b = p.as_os_str().as_bytes().to_vec();
        b.push(0);
        b
    });
    if let Some(bytes) = directory_bytes {
        options.insert("current_folder", Value::from(bytes));
    }
    match request(
        &c,
        "org.freedesktop.portal.FileChooser",
        if save { "SaveFile" } else { "OpenFile" },
        title,
        options,
        None,
    ) {
        Ok(mut r) => {
            let value = r.remove("uris").ok_or("portal returned no selected path")?;
            let uris = Vec::<String>::try_from(value).map_err(|e| e.to_string())?;
            if uris.len() != 1 {
                return Err("expected one selected item".into());
            }
            uri_path(&uris[0]).map(Some)
        }
        Err(e) if e == "cancelled" => Ok(None),
        Err(e) => Err(e),
    }
}
/// Dispatch a local file/directory to its default application. Worker-only;
/// success means portal acceptance, not that another app rendered the file.
#[cfg(feature = "native-desktop")]
pub fn open_path(path: &Path) -> Result<(), String> {
    path_uri(path)?;
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    request(
        &bounded_connection()?,
        "org.freedesktop.portal.OpenURI",
        "OpenFile",
        "",
        HashMap::new(),
        Some(&file),
    )
    .map(|_| ())
}
#[cfg(feature = "native-desktop")]
pub(super) fn open_url(url: &str) -> Result<(), String> {
    request(
        &bounded_connection()?,
        "org.freedesktop.portal.OpenURI",
        "OpenURI",
        url,
        HashMap::new(),
        None,
    )
    .map(|_| ())
}
#[cfg(feature = "native-desktop")]
pub(super) fn reveal(path: &Path) -> Result<(), String> {
    let uri = path_uri(path)?;
    let meta = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    let c = bounded_connection()?;
    if owner(&c, "org.freedesktop.FileManager1") {
        return Proxy::new(
            &c,
            "org.freedesktop.FileManager1",
            "/org/freedesktop/FileManager1",
            "org.freedesktop.FileManager1",
        )
        .and_then(|p| p.call::<_, _, ()>("ShowItems", &([uri], "")))
        .map_err(|e| e.to_string());
    }
    if meta.file_type().is_symlink() {
        return Err("portal reveal cannot preserve symbolic-link selection".into());
    }
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    request(
        &c,
        "org.freedesktop.portal.OpenURI",
        "OpenDirectory",
        "",
        HashMap::new(),
        Some(&file),
    )
    .map(|_| ())
}

#[cfg(all(test, feature = "native-desktop"))]
mod tests {
    use super::*;
    #[test]
    fn local_uris_are_byte_preserving_and_reject_remote_or_malformed_paths() {
        for p in ["/tmp/a b#?%", "/tmp/unicode-é"] {
            assert_eq!(
                uri_path(&path_uri(Path::new(p)).unwrap()).unwrap(),
                Path::new(p)
            );
        }
        for u in [
            "https://example.com",
            "file://remote/tmp/file",
            "file:///tmp/%00",
            "file:///tmp/%zz",
            "file:///tmp/%",
        ] {
            assert!(uri_path(u).is_err());
        }
        assert!(path_uri(Path::new("relative")).is_err());
    }
}

#[cfg(all(test, feature = "native-desktop"))]
mod portal_tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader},
        process::{Command, Stdio},
    };
    struct Bus(std::process::Child, String);
    impl Bus {
        fn new() -> Option<Self> {
            let mut child = Command::new("dbus-daemon")
                .args(["--session", "--nofork", "--print-address=1"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .ok()?;
            let mut address = String::new();
            BufReader::new(child.stdout.take()?)
                .read_line(&mut address)
                .ok()?;
            Some(Self(child, address.trim().into()))
        }
        fn builder(&self) -> zbus::blocking::connection::Builder<'_> {
            zbus::blocking::connection::Builder::address(self.1.as_str())
                .unwrap()
                .method_timeout(Duration::from_secs(3))
        }
    }
    impl Drop for Bus {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    struct Chooser {
        code: u32,
    }
    #[zbus::interface(name = "org.freedesktop.portal.FileChooser")]
    impl Chooser {
        async fn open_file(
            &self,
            _parent: &str,
            _title: &str,
            options: HashMap<String, OwnedValue>,
            #[zbus(header)] header: zbus::message::Header<'_>,
            #[zbus(connection)] conn: &zbus::Connection,
        ) -> zbus::fdo::Result<OwnedObjectPath> {
            let token = options
                .get("handle_token")
                .and_then(|v| <&str>::try_from(v).ok())
                .ok_or_else(|| zbus::fdo::Error::InvalidArgs("missing token".into()))?;
            let sender = header.sender().unwrap().as_str();
            let component = sender.trim_start_matches(':').replace('.', "_");
            let path = OwnedObjectPath::try_from(format!(
                "/org/freedesktop/portal/desktop/request/{component}/{token}"
            ))
            .unwrap();
            let mut values = HashMap::<String, Value>::new();
            values.insert(
                "uris".into(),
                Value::from(vec!["file:///tmp/Volt%20fixture.txt"]),
            );
            // Deliberately respond before the method reply to catch subscription races.
            conn.emit_signal(
                Some(sender),
                path.as_str(),
                "org.freedesktop.portal.Request",
                "Response",
                &(self.code, values),
            )
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
            Ok(path)
        }
    }
    #[test]
    fn immediate_portal_replies_and_cancel_are_not_lost() {
        let Some(bus) = Bus::new() else {
            eprintln!("SKIP: dbus-daemon is not installed");
            return;
        };
        for code in [0, 1, 2] {
            let server = bus
                .builder()
                .name(PORTAL)
                .unwrap()
                .serve_at(DESKTOP, Chooser { code })
                .unwrap()
                .build()
                .unwrap();
            let client = bus.builder().build().unwrap();
            let result = request(
                &client,
                "org.freedesktop.portal.FileChooser",
                "OpenFile",
                "Volt private-bus QA",
                HashMap::new(),
                None,
            );
            match code {
                0 => {
                    let values = result.unwrap();
                    let uris =
                        Vec::<String>::try_from(values.get("uris").unwrap().try_clone().unwrap())
                            .unwrap();
                    assert_eq!(
                        uri_path(&uris[0]).unwrap(),
                        Path::new("/tmp/Volt fixture.txt")
                    );
                }
                1 => assert_eq!(result.unwrap_err(), "cancelled"),
                _ => assert!(result.is_err()),
            };
            drop(client);
            drop(server);
        }
    }
}
