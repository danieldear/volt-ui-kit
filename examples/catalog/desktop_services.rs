//! Developer-owned desktop service policy; callbacks never mutate app state.
use super::{
    pages::{Doc, Section},
    App, Message,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use volt_ui_kit::{
    native::{
        notifications::{self, Authorization, LocalNotification, NotificationError},
        UiSender,
    },
    ui::prelude::*,
};
const ID: &str = "volt.catalog.test";
#[derive(Debug)]
enum Completion {
    Authorization(Result<Authorization, NotificationError>),
    Submitted(Result<(), NotificationError>, Duration),
    Removed(Result<(), NotificationError>),
    Operation(Result<String, String>),
    Selected(Result<Option<std::path::PathBuf>, String>),
}
type Mailbox = Arc<Mutex<Option<(u64, Completion)>>>;
pub struct State {
    pub authorization: Option<Authorization>,
    pub busy: bool,
    pub cancellation_test: bool,
    pub title: String,
    pub body: String,
    pub status: String,
    pub selected: Option<std::path::PathBuf>,
    pub error: Option<String>,
    generation: u64,
    mailbox: Mailbox,
}
impl Default for State {
    fn default() -> Self {
        Self {
            authorization: None,
            busy: false,
            cancellation_test: false,
            title: "Volt desktop test".into(),
            body: "A local notification from the Catalog. No network or personal content.".into(),
            selected: None,
            status: if cfg!(target_os = "linux") {
                "Linux uses immediate desktop notifications; no permission prompt is simulated."
            } else {
                "Permission hasn't been requested yet. macOS remembers your choice for this app."
            }
            .into(),
            error: None,
            generation: 0,
            mailbox: Arc::default(),
        }
    }
}
fn begin(app: &mut App) -> Option<(u64, Mailbox, UiSender<Message>)> {
    if app.services.busy {
        return None;
    }
    let Some(sender) = app.sender.clone() else {
        app.services.error = Some(
            "Open the native Catalog to try this; previews never contact desktop services.".into(),
        );
        return None;
    };
    app.services.generation = app.services.generation.wrapping_add(1);
    app.services.busy = true;
    app.services.error = None;
    app.services.mailbox = Arc::default();
    Some((
        app.services.generation,
        app.services.mailbox.clone(),
        sender,
    ))
}
fn complete(generation: u64, mailbox: Mailbox, sender: UiSender<Message>, completion: Completion) {
    *mailbox.lock().unwrap_or_else(|p| p.into_inner()) = Some((generation, completion));
    // One bounded mailbox. Full means another Catalog update is already queued;
    // every update drains this slot. Never block an OS callback or add idle polling.
    let _ = sender.send(Message::NotificationWake);
}
fn failed(app: &mut App, result: Result<(), NotificationError>) {
    if let Err(error) = result {
        app.services.busy = false;
        app.services.status = "Operation was not started; see the error below.".into();
        app.services.error = Some(error.to_string());
    }
}
pub fn request_authorization(app: &mut App) {
    let Some((generation, mailbox, sender)) = begin(app) else {
        return;
    };
    app.services.status = "Waiting for an OS authorization decision…".into();
    let result = notifications::request_authorization(move |r| {
        complete(generation, mailbox, sender, Completion::Authorization(r))
    });
    failed(app, result);
}
fn notification_plan(app: &App) -> (LocalNotification, Duration) {
    let delay = Duration::from_secs(if app.services.cancellation_test {
        60
    } else {
        8
    });
    let note = LocalNotification::new(ID, &app.services.title, &app.services.body);
    if cfg!(target_os = "linux") {
        (note, Duration::ZERO)
    } else {
        (note.after(delay), delay)
    }
}
pub fn send(app: &mut App) {
    if notifications::capabilities().authorization
        && app.services.authorization != Some(Authorization::Granted)
    {
        app.services.error = Some("Allow notifications first, then send.".into());
        return;
    }
    let Some((generation, mailbox, sender)) = begin(app) else {
        return;
    };
    let (notification, delay) = notification_plan(app);
    app.services.status = format!(
        "Submitting a silent notification for {} seconds from now…",
        delay.as_secs()
    );
    let result = notifications::send(&notification, move |r| {
        complete(generation, mailbox, sender, Completion::Submitted(r, delay))
    });
    failed(app, result);
}
pub fn remove(app: &mut App) {
    if app.services.busy {
        return;
    }
    if app.sender.is_none() {
        app.services.error = Some(
            "Open the native Catalog to try this; previews never contact desktop services.".into(),
        );
        return;
    }
    app.services.error = None;
    let Some((generation, mailbox, sender)) = begin(app) else {
        return;
    };
    let result = notifications::remove_async(ID.into(), move |r| {
        complete(generation, mailbox, sender, Completion::Removed(r))
    });
    failed(app, result);
}
pub fn reset(app: &mut App) {
    if app.services.busy {
        return; // Reset cannot cancel an OS operation or bypass add/removal serialization.
    }
    app.services.error = None;
    // Cancels only this UI witness, not an OS authorization or submitted request.
    app.services.generation = app.services.generation.wrapping_add(1);
    app.services.mailbox = Arc::default();
    app.services.busy = false;
    app.services.status =
        "Status cleared. Your notification permission and any notifications already sent are unchanged.".into();
}
pub fn drain(app: &mut App) {
    let completion = app
        .services
        .mailbox
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take();
    let Some((generation, completion)) = completion else {
        return;
    };
    if generation != app.services.generation || !app.services.busy {
        return;
    }
    app.services.busy = false;
    match completion {
        Completion::Selected(result) => match result {
            Ok(Some(path)) => {
                app.services.status = format!(
                    "Selected {} (no contents opened or modified)",
                    path.display()
                );
                app.services.selected = Some(path);
            }
            Ok(None) => app.services.status = "Chooser cancelled; selection unchanged".into(),
            Err(e) => app.services.error = Some(e),
        },
        Completion::Operation(result) => match result {
            Ok(status) => app.services.status = status,
            Err(e) => app.services.error = Some(e),
        },
        Completion::Removed(result) => match result {
            Ok(()) => {
                app.services.status =
                    "Removal request accepted; check the desktop notification center.".into()
            }
            Err(e) => app.services.error = Some(e.to_string()),
        },
        Completion::Authorization(Ok(value)) => {
            app.services.authorization = Some(value);
            app.services.status = match value {
                Authorization::Granted =>
                    "Notifications allowed. Focus modes or System Settings can still hide them.",
                Authorization::Denied =>
                    "Notifications not allowed, so nothing was sent. You can change this in System Settings.",
            }.into();
        }
        Completion::Submitted(Ok(()), delay) => {
            app.services.status = if cfg!(target_os = "linux") {
                "Linux notification server accepted the immediate request. That doesn't guarantee it will be shown. Presentation depends on desktop settings; no authorization prompt is simulated.".into()
            } else {
                format!("macOS accepted the notification, due in {} seconds. That doesn't guarantee it will be shown. To test cancelling, remove it now, switch to another app, and check Notification Center after the delay. Banners while the Catalog is in front, and notification actions, aren't supported yet.", delay.as_secs())
            };
        }
        Completion::Authorization(Err(e)) | Completion::Submitted(Err(e), _) => {
            app.services.status =
                "The native notification service reported an error; see below.".into();
            app.services.error = Some(e.to_string());
        }
    }
}

pub fn can_send(app: &App) -> bool {
    notifications::capabilities().local_requests
        && !app.services.busy
        && (!notifications::capabilities().authorization
            || app.services.authorization == Some(Authorization::Granted))
}
/// Explain build/platform prerequisites without requesting OS permission.
pub(super) fn notification_setup_help() -> &'static str {
    if !cfg!(target_os = "macos") {
        if cfg!(target_os = "linux") {
            "Linux uses the desktop notification server. No macOS-style permission prompt; immediate delivery only, no delayed scheduling. Missing services return errors."
        } else {
            "Notifications are not available on this platform yet."
        }
    } else if !cfg!(feature = "native-notifications") {
        "This build doesn't include notifications. To try them, run ./scripts/run_catalog.sh from the repository; it builds and opens the full Catalog app."
    } else {
        "Notifications are built in. Launch the Catalog with ./scripts/run_catalog.sh, not cargo run, so macOS can identify the app. Allow notifications, send one, then switch to another app before the delay ends. See “Testing Catalog notifications” in the Application services guide."
    }
}

fn operation(app: &mut App, task: impl FnOnce() -> Result<String, String> + Send + 'static) {
    let Some((generation, mailbox, sender)) = begin(app) else {
        return;
    };
    #[cfg(all(
        target_os = "linux",
        any(feature = "native-desktop", feature = "native-notifications")
    ))]
    {
        if let Err(e) = volt_ui_kit::native::linux_services::spawn(task, move |result| {
            complete(generation, mailbox, sender, Completion::Operation(result))
        }) {
            app.services.busy = false;
            app.services.error = Some(e);
        }
    }
    #[cfg(not(all(
        target_os = "linux",
        any(feature = "native-desktop", feature = "native-notifications")
    )))]
    complete(generation, mailbox, sender, Completion::Operation(task()));
}
fn choose(app: &mut App, kind: u8) {
    let Some((generation, mailbox, sender)) = begin(app) else {
        return;
    };
    let callback = move |result: Result<
        Option<std::path::PathBuf>,
        volt_ui_kit::native::file_dialog::FileDialogError,
    >| {
        complete(
            generation,
            mailbox,
            sender,
            Completion::Selected(result.map_err(|e| e.to_string())),
        )
    };
    let options =
        volt_ui_kit::native::file_dialog::FileDialogOptions::new("Volt Catalog desktop services");
    let result = match kind {
        1 => volt_ui_kit::native::file_dialog::request_open_folder(options, callback),
        2 => volt_ui_kit::native::file_dialog::request_save_file(
            options.suggested_name("catalog-note.txt"),
            callback,
        ),
        _ => volt_ui_kit::native::file_dialog::request_open_file(options, callback),
    };
    if let Err(e) = result {
        app.services.busy = false;
        app.services.error = Some(e.to_string());
    }
}
fn selected_operation(app: &mut App, reveal: bool) {
    let Some(path) = app.services.selected.clone() else {
        return;
    };
    operation(app, move || {
        if reveal {
            volt_ui_kit::native::file_dialog::reveal_file(&path)
        } else {
            volt_ui_kit::native::file_dialog::open_path(&path)
        }
        .map_err(|e| e.to_string())?;
        Ok("Desktop request accepted; rendering in the other application is not confirmed.".into())
    });
}
fn trash_fixture(app: &mut App) {
    operation(app, || {
        use volt_ui_kit::native::file_dialog;
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("volt-catalog-trash-{}-{nonce}", std::process::id()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&directory)
                .map_err(|e| e.to_string())?;
        }
        #[cfg(not(unix))]
        std::fs::create_dir(&directory).map_err(|e| e.to_string())?;
        let path = directory.join("Volt generated test.txt");
        std::fs::write(
            &path,
            "Generated by Volt Catalog for an explicit Move to Trash test. No personal data.",
        )
        .map_err(|e| e.to_string())?;
        file_dialog::prepare_trash_for(&path).map_err(|e| e.to_string())?;
        let receipt = file_dialog::move_to_trash_checked(&path).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_dir(directory);
        Ok(format!(
            "Generated file moved to {}. Trash was not emptied.",
            receipt.trashed_path.display()
        ))
    });
}
pub fn doc(app: &App) -> Doc {
    let caps = notifications::capabilities();
    Doc {sections:vec![Section {
        id:"desktop-notifications",title:"Local notifications",note:"System notifications that appear in Notification Center, unlike in-app toasts. Nothing is requested until you ask.",
        demo:vstack![
            hstack![badge(if caps.local_requests {"Native adapter compiled"} else {"Adapter unavailable"}),
                badge(match app.services.authorization {Some(Authorization::Granted)=>"Last decision: granted",Some(Authorization::Denied)=>"Last decision: denied",None=>if caps.authorization{"Permission not requested"}else{"No authorization prompt on this platform"}})],
            text(notification_setup_help()).key("notification-setup-help").font_size(13.).color(ColorToken::Muted),
            text(ID).font_family("SF Mono").font_size(12.).color(ColorToken::Muted),
            text_field(&app.services.title).key("notification-title").accessible_label("Local notification title").read_only(app.services.busy)
                .on_change(|app:&mut App,v|app.services.title=v),
            text_area(&app.services.body).key("notification-body").accessible_label("Local notification body").height(90.).read_only(app.services.busy)
                .on_change(|app:&mut App,v|app.services.body=v),
            checkbox("Cancellation test: use a 60-second delay (macOS)", app.services.cancellation_test).enabled(!cfg!(target_os="linux"))
                .key("notification-cancellation-test").enabled(!app.services.busy)
                .on_change(|app:&mut App,v|app.services.cancellation_test=v),
            hstack![
                button("Request alert permission").key("notification-authorize").enabled(caps.authorization&&!app.services.busy).on_click(request_authorization),
                button(if cfg!(target_os="linux"){"Send test notification"}else if app.services.cancellation_test { "Send in 60 seconds" } else { "Send in 8 seconds" }).key("notification-send").enabled(can_send(app)).on_click(send),
            ].gap(8.,Axis::Horizontal),
            hstack![button("Remove test notification").key("notification-remove").variant(ButtonVariant::Outline).enabled(caps.removal&&!app.services.busy).on_click(remove),
                button("Clear status").key("notification-reset").variant(ButtonVariant::Ghost).enabled(!app.services.busy).on_click(reset)].gap(8.,Axis::Horizontal),
            text(&app.services.status).font_size(13.).color(ColorToken::Muted),
            text(app.services.error.as_deref().unwrap_or("")).font_size(13.).color(ColorToken::Error),
            text("macOS requires an identified app bundle and explicit alert permission. Linux uses immediate desktop notifications without simulating a permission prompt. Errors stay visible. Sounds, badges and actions are not requested; the OS controls presentation.").font_size(12.).color(ColorToken::Muted),
        ].width(620.).gap(14.,Axis::Vertical).into(),
        code:r#"let caps = notifications::capabilities();
if caps.authorization {
    // macOS: native-notifications + identified .app.
    notifications::request_authorization(move |decision| {
        // Marshal to UiSender. Send from the UI after permission is granted.
    })?;
} else if caps.local_requests {
    // Linux: no simulated permission grant; immediate desktop notification.
    let note = LocalNotification::new("build.ready", "Build ready", "All checks passed.");
    notifications::send(&note, move |accepted| { /* Accepted != displayed */ })?;
}
// Linux: remove_async("build.ready".into(), callback).
// macOS alone supports after(Duration); Linux rejects delayed scheduling."#,

    },Section {
        id:"desktop-files",title:"Files and desktop integration",note:"Explicit OS requests. Choosing a save location writes nothing; opening a path may launch its registered application. The Trash test uses only a generated file.",
        demo:vstack![
          hstack![button("Choose file…").key("service-choose-file").enabled(volt_ui_kit::native::file_dialog::available()&&!app.services.busy).on_click(|app|choose(app,0)),button("Choose folder…").key("service-choose-folder").enabled(volt_ui_kit::native::file_dialog::available()&&!app.services.busy).on_click(|app|choose(app,1)),button("Choose save location…").key("service-save-path").enabled(volt_ui_kit::native::file_dialog::available()&&!app.services.busy).on_click(|app|choose(app,2))].gap(8.,Axis::Horizontal),
          text(app.services.selected.as_ref().map(|p|p.display().to_string()).unwrap_or_else(||"No path selected".into())).font_size(13.).color(ColorToken::Muted),
          hstack![button("Open selected path").key("service-open-path").enabled(app.services.selected.is_some()&&!app.services.busy).on_click(|app|selected_operation(app,false)),button("Reveal selected item").key("service-reveal-path").enabled(app.services.selected.is_some()&&!app.services.busy).on_click(|app|selected_operation(app,true))].gap(8.,Axis::Horizontal),
          button("Open example.com in browser").key("service-open-url").enabled(volt_ui_kit::native::links::available()&&!app.services.busy).on_click(|app|operation(app,||{volt_ui_kit::native::links::open_url(&volt_ui_kit::ui::ExternalUrl::parse("https://example.com").map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;Ok("Browser request accepted".into())})),
          button("Move a generated test file to Trash").key("service-trash-fixture").enabled(volt_ui_kit::native::file_dialog::available()&&!app.services.busy).on_click(trash_fixture),
          text("No personal files are moved by the Trash test, and Trash is never emptied. Native-only operations never run in browser previews.").font_size(12.).color(ColorToken::Muted),
          button("Check Linux service availability").key("service-probe").enabled(cfg!(target_os="linux")&&!app.services.busy).on_click(|app|operation(app,||{
            #[cfg(all(target_os="linux",any(feature="native-desktop",feature="native-notifications")))]
            {let p=volt_ui_kit::native::linux_services::probe()?;Ok(format!("Runtime services: chooser={}, open={}, file manager={}, notifications={}, tray={}",p.file_chooser,p.open_uri,p.file_manager,p.notifications,p.tray))}
            #[cfg(not(all(target_os="linux",any(feature="native-desktop",feature="native-notifications"))))]
            Err("Linux service probe is not compiled in this build".into())
          })),
          text(&app.services.status).font_size(12.).color(ColorToken::Muted),text(app.services.error.as_deref().unwrap_or("")).font_size(12.).color(ColorToken::Error),
        ].gap(12.,Axis::Vertical).width(680.).into(),
        code:r#"// File panels: nonblocking on Linux; macOS starts its panel on the main thread.
file_dialog::request_open_file(FileDialogOptions::new("Open"), move |result| {
    // Result<Option<PathBuf>, FileDialogError>; None means Cancel.
    // Marshal the result into UiSender before changing application state.
})?;
// On a worker, after an explicit user action:
file_dialog::open_path(&path)?;
file_dialog::reveal_file(&path)?;
// Only after confirmation and application-specific protected-path checks:
file_dialog::prepare_trash_for(&path)?;
let receipt = file_dialog::move_to_trash_checked(&path)?;"#,
    },Section {
        id:"desktop-tray",title:"One command registry, multiple surfaces",note:"The optional native-desktop tray (macOS and supported Linux desktops) reuses these command IDs. Menu/toolbar/palette/tray commands all pass current enablement and modal guards.",
        demo:vstack![text(format!("Command executions: {}",app.command_probe)).font_size(24.).font_weight(700),
            button("Count command execution").key("desktop-command-probe").on_click(|app:&mut App|super::lifecycle::request(app,volt_ui_kit::native::ApplicationRequest::Command("command-probe".into()))),
            text("The Catalog puts a Volt icon in the macOS menu bar or a supported Linux StatusNotifier tray. Linux desktops without a tray host report it unavailable. Its “Count command execution” item adds to this same counter. It doesn't reopen the window, keep running in the background or manage several windows.").font_size(13.).color(ColorToken::Muted),
        ].width(620.).gap(14.,Axis::Vertical).into(),
        code:r#"DesktopConfig::new("My app")
    .tray(TrayConfig { tooltip: "My app".into(), rgba, width: 18, height: 18,
        template: true, commands: vec![
            DesktopCommand::new("command-probe", "Count command execution"),
        ] })
// Register the same ID in CommandRegistry; don't duplicate callbacks."#,
    }],api:vec![
        ("notifications::capabilities()","Adapter availability plus explicit unsupported actions/settings/foreground capabilities."),
        ("request_authorization(callback)","macOS alert-only decision; Linux reports authorization=false, with no fake prompt or grant."),
        ("LocalNotification::new(id,title,body)","Bounded stable ID and UTF-8 content; no truncation or arbitrary attachments."),
        ("notification.after(Duration)","macOS-only one-shot delay; Linux accepts immediate notifications and rejects delayed scheduling."),
        ("notifications::send / remove","Accepted-not-displayed callback; removal has no completion receipt."),
    ]}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_plan_is_explicit_and_keeps_the_same_removable_id() {
        let mut app = App::new();
        assert!(!app.services.cancellation_test);
        let (normal, delay) = notification_plan(&app);
        assert_eq!(
            delay,
            if cfg!(target_os = "linux") {
                Duration::ZERO
            } else {
                Duration::from_secs(8)
            }
        );
        assert_eq!(normal.id(), ID);
        assert!(normal.validate().is_ok());
        app.services.cancellation_test = true;
        let (pending, delay) = notification_plan(&app);
        assert_eq!(
            delay,
            if cfg!(target_os = "linux") {
                Duration::ZERO
            } else {
                Duration::from_secs(60)
            }
        );
        assert_eq!(pending.id(), ID);
        assert!(pending.validate().is_ok());
        app.services.title.clear();
        assert!(notification_plan(&app).0.validate().is_err());
    }
    #[test]
    fn submission_receipt_retains_its_delay_even_if_controls_change() {
        let mut app = App::new();
        app.services.busy = true;
        *app.services.mailbox.lock().unwrap() =
            Some((0, Completion::Submitted(Ok(()), Duration::from_secs(60))));
        drain(&mut app);
        if cfg!(target_os = "linux") {
            assert!(app.services.status.contains("immediate"));
            assert!(!app.services.status.contains("60 seconds"));
        } else {
            assert!(app.services.status.contains("60 seconds"));
        }
        assert!(app
            .services
            .status
            .contains("doesn't guarantee it will be shown"));
        assert!(!app.services.busy);
    }
    #[test]
    fn late_results_and_reset_never_change_newer_permission_witness() {
        let mut app = App::new();
        app.services.busy = true;
        app.services.generation = 2;
        *app.services.mailbox.lock().unwrap() =
            Some((1, Completion::Authorization(Ok(Authorization::Granted))));
        drain(&mut app);
        assert_eq!(app.services.authorization, None);
        assert!(app.services.busy);
        reset(&mut app);
        assert!(app.services.busy);
        assert_eq!(app.services.generation, 2);
        *app.services.mailbox.lock().unwrap() =
            Some((2, Completion::Authorization(Ok(Authorization::Denied))));
        drain(&mut app);
        assert_eq!(app.services.authorization, Some(Authorization::Denied));
        assert!(!app.services.busy);
        reset(&mut app);
        assert_eq!(app.services.authorization, Some(Authorization::Denied));
    }
    #[test]
    fn shared_tray_commands_recheck_permission_and_busy_state() {
        let mut app = App::new();
        let registry = super::super::lifecycle::commands();
        let enabled = |app: &App, id: &str| {
            registry
                .entries(app)
                .into_iter()
                .find(|c| c.id == id)
                .unwrap()
                .enabled
        };
        assert_eq!(
            enabled(&app, "notification-send"),
            notifications::capabilities().local_requests
                && !notifications::capabilities().authorization
        );
        app.services.authorization = Some(Authorization::Denied);
        assert_eq!(
            enabled(&app, "notification-send"),
            notifications::capabilities().local_requests
                && !notifications::capabilities().authorization
        );
        app.services.authorization = Some(Authorization::Granted);
        assert_eq!(
            enabled(&app, "notification-send"),
            notifications::capabilities().local_requests
        );
        app.services.busy = true;
        assert!(!enabled(&app, "notification-send"));
        assert!(!enabled(&app, "notification-remove"));
        reset(&mut app);
        assert!(app.services.busy); // Cannot bypass add/removal serialization.
    }
    #[test]
    fn native_failure_is_visible_and_never_reports_delivery() {
        let mut app = App::new();
        app.services.busy = true;
        *app.services.mailbox.lock().unwrap() = Some((
            0,
            Completion::Submitted(
                Err(NotificationError::NativeFailure),
                Duration::from_secs(8),
            ),
        ));
        drain(&mut app);
        assert!(app.services.error.is_some());
        assert!(!app.services.busy);
        app.services.busy = true;
        *app.services.mailbox.lock().unwrap() =
            Some((0, Completion::Submitted(Ok(()), Duration::from_secs(8))));
        drain(&mut app);
        assert!(app
            .services
            .status
            .contains("doesn't guarantee it will be shown"));
        let mut headless = App::new();
        request_authorization(&mut headless);
        assert!(headless
            .services
            .error
            .as_deref()
            .unwrap()
            .contains("Open the native Catalog"));
        remove(&mut headless);
        assert!(headless
            .services
            .error
            .as_deref()
            .unwrap()
            .contains("Open the native Catalog"));
        // No sender: pure headless errors, no OS service calls.
    }
}

#[cfg(all(test, target_os = "linux", feature = "native-desktop"))]
mod linux_live_tests {
    use super::*;
    #[test]
    #[ignore = "explicit Linux desktop QA: one generic local notification and removal"]
    fn notification_round_trip() {
        let services = volt_ui_kit::native::linux_services::probe().unwrap();
        assert!(services.notifications, "no running notification service");
        let (tx, rx) = std::sync::mpsc::channel();
        notifications::send(
            &LocalNotification::new(
                "volt.catalog.linux.qa",
                "Volt Linux desktop test",
                "A generic local Catalog test. No network or personal content.",
            )
            .application_id("dev.volt.catalog"),
            move |r| {
                let _ = tx.send(r);
            },
        )
        .unwrap();
        rx.recv_timeout(Duration::from_secs(15)).unwrap().unwrap();
        println!("Notification accepted by the Linux server");
        // Leave a short observation window before the app explicitly removes its own message.
        std::thread::sleep(Duration::from_secs(10));
        let (tx, rx) = std::sync::mpsc::channel();
        notifications::remove_async("volt.catalog.linux.qa".into(), move |r| {
            let _ = tx.send(r);
        })
        .unwrap();
        rx.recv_timeout(Duration::from_secs(15)).unwrap().unwrap();
        println!("Notification removal accepted");
    }
    #[test]
    #[ignore = "explicit Linux desktop QA: only a generated recoverable Trash file"]
    fn trash_round_trip() {
        use volt_ui_kit::native::file_dialog;
        let root = std::env::var_os("VOLT_DESKTOP_QA_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        assert!(root.is_absolute() && root.is_dir());
        let folder = root.join(format!("volt-desktop-qa-{}", std::process::id()));
        std::fs::create_dir(&folder).unwrap();
        let path = folder.join("Generated Trash test.txt");
        std::fs::write(&path, "Generated Volt QA data only").unwrap();
        file_dialog::prepare_trash_for(&path).unwrap();
        let receipt = file_dialog::move_to_trash_checked(&path).unwrap();
        assert!(!path.exists());
        assert_eq!(
            std::fs::read_to_string(&receipt.trashed_path).unwrap(),
            "Generated Volt QA data only"
        );
        println!(
            "Generated file recoverably moved to {}",
            receipt.trashed_path.display()
        );
        std::fs::remove_dir(folder).unwrap();
    }
}
