# Application services

## Background work

Callbacks run on the UI thread, so do slow work elsewhere and send the result
back as a typed message:

```rust,no_run
use volt_ui_kit::{native::{Application, UiSender}, ui::prelude::*};

enum Message {
    Loaded(String),
}

#[derive(Default)]
struct App {
    sender: Option<UiSender<Message>>,
    result: String,
}

fn view(app: &App) -> Element<App> {
    vstack![
        text(&app.result),
        button("Load").key("load").on_click(|app: &mut App| {
            let sender = app.sender.clone().expect("installed before the first view");
            std::thread::spawn(move || {
                let data = "loaded on a worker thread".to_string(); // I/O goes here
                let _ = sender.send(Message::Loaded(data));
            });
        }),
    ]
    .into()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Application::new("Worker", App::default(), view)
        .messages(
            |app, sender| app.sender = Some(sender),
            |app, message| match message {
                Message::Loaded(data) => app.result = data,
            },
        )
        .run()
}
```

- Messages need `Send`; your state does not.
- `send` never blocks. The queue holds 256 messages; when it is full or the
  window has closed, `send` drops the message and returns `UiSendError::Full`
  or `Closed`. Decide whether to retry, drop or merge.
- The UI handles up to 64 messages per wake-up and calls `view` once per batch.
- Use any executor (threads, tokio, async-std); the kit starts none of its own.
- Tag long jobs with an ID so you can ignore results from cancelled work.

## Commands and shortcuts

A command registry gives actions an ID, a label, an optional shortcut and a
rule for when they are enabled. The same command can then run from a keyboard
shortcut, the macOS menu bar, a tray menu, your own buttons or a command
palette built from `registry.entries(&app)`.

```rust,no_run
use volt_ui_kit::{
    native::{
        Application, ApplicationRequest, CloseDecision, CloseReason, Command, CommandRegistry,
        Shortcut, UiSender,
    },
    ui::prelude::*,
};

#[derive(Default)]
struct App {
    draft: String,
    dirty: bool,
    closing: Option<CloseReason>,
    requests: Option<UiSender<ApplicationRequest>>,
}

fn commands() -> CommandRegistry<App> {
    CommandRegistry::new()
        .command(
            Command::new("save", "Save", |app: &mut App| app.dirty = false)
                .shortcut(Shortcut::primary('s'))
                .enabled_when(|app| app.dirty),
        )
        .command(
            Command::new("close", "Close window", |_: &mut App| {})
                .shortcut(Shortcut::primary('w'))
                .request_close(CloseReason::Window),
        )
}

fn view(app: &App) -> Element<App> {
    let editor = text_area(&app.draft).key("draft").on_change(|app: &mut App, text| {
        app.draft = text;
        app.dirty = true;
    });
    let Some(reason) = app.closing else {
        return editor.into();
    };
    zstack![
        editor,
        dialog(hstack![
            button("Cancel").key("cancel").on_click(|app: &mut App| app.closing = None),
            button("Discard").key("discard").on_click(move |app: &mut App| {
                app.dirty = false;
                if let Some(requests) = &app.requests {
                    let _ = requests.send(ApplicationRequest::Close(reason));
                }
            }),
        ].gap(8., Axis::Horizontal))
        .key("confirm")
        .title("Discard unsaved changes?")
        .open(true)
        .on_dismiss(|app: &mut App| app.closing = None),
    ]
    .into()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Application::new("Editor", App::default(), view)
        .commands(commands())
        .requests(|app, sender| app.requests = Some(sender))
        .on_close_request(|app, reason| {
            if app.dirty {
                app.closing = Some(reason);
                CloseDecision::KeepOpen
            } else {
                CloseDecision::Close
            }
        })
        .run()
}
```

- `Shortcut::primary('s')` is Cmd+S on macOS and Ctrl+S elsewhere; add
  `.shift(true)` or `.alt(true)`. Shortcuts are ignored while typing with an IME
  or while a modal is open, and clipboard and undo keys stay with text fields.
- Disabled commands never run, whichever way they are triggered.
- `ApplicationRequest::Command(id)` runs a command from your own UI;
  `ApplicationRequest::Close(reason)` asks to close, and the close guard is
  checked again.

### Guarding close

`on_close_request` decides whether the window may close. Return `KeepOpen`,
show your own Save / Discard / Cancel dialog, and request the close again when
the user decides. Save asynchronously with a background message and close when
the save succeeds.

The guard covers the window's close button, Cmd+W and, with `native-desktop`,
the owned Quit menu item. It cannot stop Force Quit, system shutdown or a crash.

## Files and dialogs

```rust,no_run
use volt_ui_kit::native::{
    file_dialog::{self, FileDialogOptions},
    files::{read_text_file, save_text_file, SavePolicy},
};
// Panels run on the main thread (inside a callback); `Ok(None)` means cancelled.
if let Some(path) = file_dialog::open_file(&FileDialogOptions::new("Open notes"))? {
    // Do the I/O on a worker thread, then report back with a message.
    let original = read_text_file(&path)?;
    save_text_file(&path, "Updated", SavePolicy::MatchContents(&original))?;
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

- `open_file` and `save_file` (with `.suggested_name(..)`) show the native macOS
  panels and need `native-desktop`. Other platforms return `Unsupported` for now.
- `save_text_file` writes to a temporary file and moves it into place.
  `CreateNew` never overwrites; `MatchContents(original)` refuses to save if the
  file changed on disk since you read it; `ApprovedReplacement` overwrites.
- These helpers are for small UTF-8 text files (up to 8 KiB). Use `std::fs`
  directly for anything larger.

## Opening web links

Composition never launches another application. A link's callback can capture,
confirm or reject the target, or explicitly call the native adapter:

```rust,no_run
use volt_ui_kit::{native::links, ui::prelude::*};
#[derive(Default)]
struct App { status: String }
let ui: Element<App> = link("Project website ↗", "https://example.com")
    .key("website")
    .on_open(|app: &mut App, url| {
        app.status = match links::open_url(&url) {
            Ok(()) => "Browser request accepted".into(),
            Err(error) => error.to_string(),
        };
    }).into();
```

`native::links::available()` reports compiled support: currently macOS with
`native-desktop`. Dispatch requires the main thread and uses public
`NSWorkspace`/`NSURL` APIs, not a shell or subprocess. Other builds return
`OpenUrlError::Unsupported`. Success means the OS accepted the request, not
that a browser loaded the page. Volt itself makes no network request. Custom
schemes, email actions, app deep-link registration and embedded browsing are not
part of this HTTP(S)-only adapter.

## Menus and tray

With `native-desktop` on macOS, add an application menu and a tray item. Their
items share IDs with your registered commands, so they enable, disable and run
exactly like shortcuts:

```rust,no_run
use volt_ui_kit::native::system::{DesktopCommand, DesktopConfig, DesktopMenu};
let desktop = DesktopConfig::new("My app").menu(DesktopMenu::new(
    "File",
    [DesktopCommand::new("save", "Save"), DesktopCommand::new("close", "Close window")],
));
// Application::new(..).commands(..).desktop(desktop, |_app, _unregistered_id| {})
```

The menu includes the standard About, Services, Hide and Quit items. A
`TrayConfig` adds a menu-bar icon (RGBA, up to 128×128) with its own commands.

`native::file_dialog::reveal_file(&absolute_path)` asks Finder to select an
existing file or folder without opening its contents. It requires
`native-desktop` on macOS and the UI thread; other platforms return
`Unsupported`. Success reports dispatch, not an observed Finder window.

### Tray-resident window visibility

Use `.window_visibility_from_state(|app| !app.hidden)` with a tray command that
clears `hidden`. In `.on_close_request`, set `hidden = true` and return
`CloseDecision::KeepOpen` for `CloseReason::Window`; allow `Quit` to close.
Hiding changes the native window's visibility without shutting down the worker
queues. A false-to-true transition restores and focuses the window. Visibility
is not focus-stealing on every message: it is applied only on a transition.

This opt-in does not install a tray icon, login item or daemon automatically.
Always provide an explicit Open and Quit path. Existing applications retain
normal close-to-exit behavior unless they opt into a close guard.

## Notifications

With `native-notifications` on macOS:

```rust,no_run
use std::time::Duration;
use volt_ui_kit::native::notifications::{self, LocalNotification};
// Ask from a deliberate user action; the callback runs on an OS thread.
notifications::request_authorization(|decision| { let _ = decision; })?;
let note = LocalNotification::new("build.ready", "Build ready", "All checks passed.")
    .after(Duration::from_secs(5));
notifications::send(&note, |accepted| { let _ = accepted; })?;
notifications::remove("build.ready")?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Notifications work only from a bundled `.app` (see [Packaging and releasing](#packaging-and-releasing)).
"Accepted" means the OS took the request; Focus modes and user settings decide
whether it is shown. Forward the callback results to your UI with a
background message.

### Testing Catalog notifications

Use the full Catalog launcher from the repository root:

```sh
./scripts/run_catalog.sh
```

It builds every Catalog capability (GPU, desktop integrations and local
notifications), creates and ad-hoc signs a temporary identified macOS `.app`,
registers it with Launch Services, and launches it. There is no separate feature or packaging setup. The launcher
waits for the app to quit and removes its temporary bundle afterward; it does
not duplicate repository history. Python 3, Cargo and the macOS command-line
tools are required. `./scripts/run_catalog.sh --check` builds and verifies
without opening the app or requesting permission.

Open **Desktop services**, click **Request alert permission** and allow alerts.
Then click **Send in 8 seconds** and switch to another app before the timer
expires: this adapter does not force foreground presentation. Check
Notification Center if no banner appears, and use **Remove test notification**
to clean up. Focus modes and macOS notification settings can suppress banners.
No permission prompt or notification is sent automatically. The stable Catalog
bundle identifier is `com.volt.ui.catalog`.

Direct `cargo run --features native-desktop --example catalog` still works for
ordinary component testing, including context menus, but intentionally omits
GPU rendering and the separately opt-in notification adapter. Application
developers retain independent library feature flags. Non-macOS launches report
that local notifications are unavailable rather than pretending to deliver.

## The development loop

`cargo volt dev` rebuilds your project when a source or manifest changes and
restarts the app after a successful build. To keep your place across restarts,
save and restore state:

```rust,no_run
use volt_ui_kit::{native::Application, ui::prelude::*};
#[derive(Default)]
struct App { page: usize }
fn view(app: &App) -> Element<App> { text(format!("Page {}", app.page)).into() }
Application::new("Dev", App::default(), view)
    .dev_state(
        |app| app.page.to_string(),
        |app, saved| app.page = saved.parse().unwrap_or(0),
    )
    .theme_file("theme.json")
    .run()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

- `dev_state` runs only under `cargo volt dev`; the window keeps its size and
  position too. Use any format for the saved string.
- `theme_file` reloads the palette whenever the file changes, without a rebuild.
- The dev loop runs your project's build scripts; use it on code you trust.

### Explicit macOS Trash

`native::file_dialog::move_to_trash(&absolute_path)` moves one item to macOS
Trash through Foundation, without permanently deleting it or emptying Trash.
It requires `native-desktop` on macOS; other builds report `Unsupported`.
The caller must own the confirmation UI, protected-location policy and
concurrent-change validation. Run preflight and the filesystem operation on a
worker, not in the rendering callback. The helper never falls back to deletion.
Moving synced content can propagate to other devices; returning success does
not guarantee immediate free-space recovery. The OS API is documented by
[Apple](https://developer.apple.com/documentation/foundation/filemanager/trashitem(at:resultingitemurl:)).

## Packaging and releasing

Describe the app once in `Cargo.toml`, then let `cargo volt` build it:

```toml
[package.metadata.volt.bundle]
name = "My App"
identifier = "com.example.myapp"
icon = "assets/AppIcon.icns"          # relative to Cargo.toml
minimum-system-version = "13.0"
category = "public.app-category.productivity"
copyright = "© 2026 Example"
notary-profile = "myapp"              # optional; see below
aliases = ["myapp"]                   # optional; see below

# Files the app opens. Content types name system UTIs; extensions claim
# types macOS has no UTI for. Give one or the other per entry.
[[package.metadata.volt.bundle.document-types]]
name = "Markdown Document"
role = "Editor"                       # Editor, Viewer, Shell or None
rank = "Default"                      # Owner, Default, Alternate or None
content-types = ["net.daringfireball.markdown"]

[[package.metadata.volt.bundle.document-types]]
name = "Markdown Document"
role = "Editor"
extensions = ["mdown", "mkd"]
```

`imported-types` declares UTIs for formats no app on the system defines,
and `resources`, `usage` (privacy descriptions) and `build-number` work as
the manual flags below.

`aliases` adds other names for the executable, as links beside it in
`Contents/MacOS`. Use one to ship a command-line tool inside the app: link
`/usr/local/bin/myapp` to `My App.app/Contents/MacOS/myapp`, and have
`main` check the name it was run as (`std::env::args().next()`) to run the
command line instead of opening a window. The links are signed and
notarized with the app.

App extensions, such as a Quick Look preview, are other binaries of the
package. Each goes into `Contents/PlugIns/NAME.appex` with its own
`Info.plist` and is signed in the app sandbox before the app:

```toml
[[package.metadata.volt.bundle.extensions]]
name = "My App Preview"
identifier = "com.example.myapp.preview"   # extends the app's identifier
binary = "myapp-preview"                   # a [[bin]] of this package
point = "com.apple.quicklook.preview"      # NSExtensionPointIdentifier
principal-class = "MyAppPreviewProvider"   # the Objective-C class it defines
[package.metadata.volt.bundle.extensions.attributes]
QLIsDataBasedPreview = true
QLSupportedContentTypes = ["net.daringfireball.markdown"]
```

The binary defines the principal class and hands control to
`NSExtensionMain`; the attributes become `NSExtensionAttributes`.

`cargo volt bundle` strips the executable's symbol table, about a fifth of a
Rust release binary (`strip = false` keeps it, for readable backtraces). For
a smaller app still, give the package a size-tuned release profile:

```toml
[profile.release]
opt-level = "s"        # optimise for size
lto = true             # link-time optimisation across crates
codegen-units = 1
panic = "abort"        # no unwinding tables or landing pads
strip = true
```

Together these took one app from 20.6 MB to 9.8 MB per architecture, for
rendering about 3% slower and release builds a minute or two longer.

```sh
cargo volt bundle                 # target/release/bundle/My App.app, signed ad hoc
cargo volt bundle --universal     # one binary for Apple silicon and Intel
```

`cargo volt release` makes a build you can hand to other Macs: it signs with
your Developer ID and the hardened runtime, notarizes the app with Apple and
staples the ticket, then builds, signs, notarizes and staples a disk image.

```sh
# Once: store App Store Connect credentials in the keychain.
xcrun notarytool store-credentials myapp --apple-id you@example.com --team-id TEAMID

cargo volt release --universal --dry-run    # print every step
cargo volt release --universal              # target/release/bundle/My-App-1.0.0.dmg
```

The identity defaults to the only "Developer ID Application" certificate in
your keychain (`--identity` picks another). The notary profile comes from
`--notary-profile`, `VOLT_NOTARY_PROFILE` or `notary-profile` in the
metadata. `--no-notarize` and `--no-dmg` stop early, and `--entitlements`
adds an entitlements file. Steps run programs directly, never a shell, and
only direct outputs under `target/release/bundle` are replaced. The metadata
`name` is a single filename: directory separators, `.` and `..` are rejected.
Inputs and bundle metadata are validated after the build and before replacing
an existing app; cleanup cannot follow intermediate directory symlinks. The
prebuilt flags form below keeps its explicit output path independent of the
app's display name.

To bundle a binary built some other way, pass everything as flags:

```sh
cargo volt bundle \
  --binary target/release/my-app \
  --output MyApp.app \
  --name "My App" \
  --identifier com.example.myapp \
  --version 0.1.0 \
  --minimum-system-version 11.0 \
  --icon assets/AppIcon.icns
```

Every bundle gets an `Info.plist`, checks that the binary links only system
libraries and declares a deployment target no newer than the minimum, and
never overwrites a path it did not create. Run `cargo volt bundle --help` for
every option.

To receive the files Finder opens with your app (double-click, Open With,
the Dock), install an AppKit handler in `Application::before_launch`, which
runs once the application object exists and before launch events arrive.
