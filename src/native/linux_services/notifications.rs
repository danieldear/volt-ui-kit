//! Freedesktop notifications: explicit immediate requests, no fake permission grant.
use crate::native::notifications::{LocalNotification, NotificationError};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};
use zbus::{
    blocking::Proxy,
    zvariant::{OwnedValue, Str},
};
static NOTES: OnceLock<Mutex<HashMap<String, (String, u32)>>> = OnceLock::new();
static CONNECTION: OnceLock<Mutex<Option<zbus::blocking::Connection>>> = OnceLock::new();
fn connection() -> Result<zbus::blocking::Connection, String> {
    let mut slot = CONNECTION
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if slot
        .as_ref()
        .is_none_or(zbus::blocking::Connection::is_closed)
    {
        let connection = super::bounded_connection()?;
        notes().lock().unwrap_or_else(|p| p.into_inner()).clear();
        *slot = Some(connection);
    }
    Ok(slot.as_ref().expect("initialized").clone())
}
fn notes() -> &'static Mutex<HashMap<String, (String, u32)>> {
    NOTES.get_or_init(Mutex::default)
}
fn failure(_: impl std::fmt::Display) -> NotificationError {
    NotificationError::NativeFailure
}
pub(in crate::native) fn send(
    note: &LocalNotification,
    callback: impl FnOnce(Result<(), NotificationError>) + Send + 'static,
) -> Result<(), NotificationError> {
    if note.delay().is_some() {
        return Err(NotificationError::Unsupported);
    }
    let note = note.clone();
    super::spawn(
        move || {
            let c = connection()?;
            let bus = Proxy::new(
                &c,
                "org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
            )
            .map_err(|e| e.to_string())?;
            let owner: String = bus
                .call("GetNameOwner", &("org.freedesktop.Notifications",))
                .map_err(|e| e.to_string())?;
            let proxy = Proxy::new(
                &c,
                owner.as_str(),
                "/org/freedesktop/Notifications",
                "org.freedesktop.Notifications",
            )
            .map_err(|e| e.to_string())?;
            let mut notes = notes().lock().unwrap_or_else(|p| p.into_inner());
            if !notes.contains_key(note.id()) && notes.len() >= 64 {
                return Err("notification tracking limit reached".into());
            }
            let previous = notes
                .get(note.id())
                .filter(|(server, _)| server == &owner)
                .map_or(0, |(_, id)| *id);
            let app_id = note.desktop_id().unwrap_or(super::app_id());
            let mut hints = HashMap::<&str, OwnedValue>::new();
            hints.insert("desktop-entry", Str::from(app_id).into());
            hints.insert("suppress-sound", true.into());
            let body = note
                .body()
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            let id: u32 = proxy
                .call(
                    "Notify",
                    &(
                        app_id,
                        previous,
                        app_id,
                        note.title(),
                        body,
                        Vec::<&str>::new(),
                        hints,
                        -1i32,
                    ),
                )
                .map_err(|e| e.to_string())?;
            if id == 0 {
                return Err("notification server returned invalid ID".into());
            }
            notes.insert(note.id().into(), (owner.clone(), id));
            Ok(())
        },
        move |r| callback(r.map_err(failure)),
    )
    .map_err(failure)
}
pub(in crate::native) fn remove(id: &str) -> Result<(), NotificationError> {
    if super::on_ui_thread() {
        return Err(NotificationError::WrongThread);
    }
    let c = connection().map_err(failure)?;
    let mut notes = notes().lock().unwrap_or_else(|p| p.into_inner());
    let Some((owner, number)) = notes.get(id).cloned() else {
        return Ok(());
    };
    // Address the original unique server, not a restarted daemon that may have
    // reused the numeric notification ID for a different application's message.
    let proxy = Proxy::new(
        &c,
        owner.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .map_err(failure)?;
    proxy
        .call::<_, _, ()>("CloseNotification", &(number,))
        .map_err(failure)?;
    notes.remove(id);
    Ok(())
}
