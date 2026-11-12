# Windows and rendering

## Configuring the window

`Application` holds the window settings. Everything is validated before the
window opens.

```rust,no_run
use volt_ui_kit::{
    native::{Application, RendererPreference, WindowBackground, WindowChrome},
    ui::prelude::*,
    ThemeMode,
};

#[derive(Default)]
struct App;

fn view(_: &App) -> Element<App> {
    vstack![
        hstack![text("My app").font_weight(600)]
            .height(60.)
            .padding(100., Edge::Left)
            .align(Align::Center)
            .drag_region(true),
        text("Content"),
    ]
    .into()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Application::new("My app", App, view)
        .size(1100, 760)
        .theme(ThemeMode::System)
        .chrome(WindowChrome::Overlay)
        .background(WindowBackground::Blur { opacity: 0.8 })
        .renderer(RendererPreference::Auto)
        .run()
}
```

## Title bar and chrome

| `WindowChrome` | Result |
| --- | --- |
| `Native` (default) | The platform's standard title bar. |
| `Overlay` | macOS: content fills the window under a transparent, unified title bar; the traffic lights stay native. Reserve about 60 pt of header and 100 pt on the left for them. Other platforms keep a native title bar. |
| `CompactOverlay` | The same with the shorter, older macOS title bar geometry. |
| `Borderless` | No visible title or system controls. macOS keeps an invisible native frame for rounded corners, shadow, resizing and public material blur. Linux/Windows are undecorated; you own the complete window UI. |
| `Custom` | Content extends into the existing app header; no extra title strip. macOS retains native traffic lights, rounded frame, resizing and material blur. Linux/Windows use inline `window_controls` and toolkit-managed edge resizing. |

Mark your header with `.drag_region(true)` so the window can be dragged by it.
Buttons and other controls inside the header still receive clicks.

### Custom controls and live mode switching

The native Catalog has a **Window settings** button in the existing header at
all supported widths. It opens the mode selector, app-drawn control style and
actual host diagnostics. The sidebar footer/drawer and wide-header selectors
remain available: Native, Custom, Borderless and Unified (Overlay on Linux).
Changing chrome is live: it does not replace the window, restart workers or
clear application state. `--chrome native|custom|borderless|overlay` chooses the
initial mode; `--control-style symbols|traffic-lights` chooses the app-drawn style.
macOS retains native traffic lights; the style picker changes the simulated
preview there, never draws duplicate buttons over AppKit.

### Reuse the existing header

`window_header(content, controls)` composes one existing row with a draggable
background and interactive children. It does not insert a global title bar or
change the native decoration policy. Its default height (56 logical pixels),
gap and padding are ordinary container properties you can customize.

```rust,ignore
window_header(
    text("Workspace").font_weight(600),
    window_controls_with_style(
        WindowControlsState { maximized: app.window.maximized,
            can_maximize: app.window.resizable, ..Default::default() },
        WindowControlStyle::TrafficLights, // or Symbols (the legacy default)
        |app: &mut App, action| {
            if let Some(sender) = &app.actions { let _ = sender.send(action); }
        },
    ),
).key("workspace-header").padding(16., Edge::Horizontal)
```

Connect `Application::window_actions` and `on_window_state` as below. Close still
uses the application's close guard. On macOS, omit application-drawn controls
and reserve the native control safe area. Borderless intentionally hides controls;
simple legacy examples retain native chrome unless their layouts opt in.

Custom mode embeds controls in the existing layout, not a new bar. The Catalog
uses `navigation_split_view(...).integrated_header(true)` to put native traffic
lights and the brand in the existing sidebar header. Its right panel retains
exactly the same opaque toolbar/content, 8pt top/right/bottom inset, rounded
corners and floating-sheet shadow as Unified and Borderless. Chrome selection
does not change the panel's material styling. The sidebar still reveals the
window backdrop; text and controls stay opaque.

`integrated_header` controls layout only and is opt-in, preserving the old
split-view defaults. `.window_controls_inset(100.)` sets the logical horizontal
safe area (0–480, finite), without repositioning OS buttons.

Toolbar glass is a separate developer choice:
`navigation_split_view(...).material(true).toolbar_translucent(true)` reveals
the window backdrop through the existing toolbar while keeping page content
opaque. It works with or without an integrated header and defaults to **false**.
For that explicit glass toolbar the sheet shadow is omitted so its interior
cannot darken the transparent header. The panel keeps its normal top inset.
The host's `WindowBackground` independently selects blur/transparency; this
builder never enables native blur or changes foreground opacity. Apps may also
paint their own toolbar background. Catalog does not opt into toolbar glass.

```rust,no_run
use volt_ui_kit::{native::{Application, WindowChrome, WindowState, UiSender}, ui::prelude::*};
#[derive(Default)]
struct State {
    chrome: WindowChrome,
    window: WindowState,
    actions: Option<UiSender<WindowAction>>,
    sidebar_open: bool,
}
fn view(app: &State) -> Element<State> {
    let mut items: Vec<Element<State>> = vec![text("Search or workspace actions").into(), spacer().into()];
    if !cfg!(target_os = "macos") {
        items.push(window_controls(app.window.maximized, |app: &mut State, action| {
            if let Some(sender) = &app.actions { let _ = sender.send(action); }
        }).into());
    }
    navigation_split_view(|_| text("Inbox"), text("Your content"))
        .key("workspace")
        .sidebar_header(text("My workspace"))
        .toolbar(hstack(items).align(Align::Center).drag_region(true))
        .integrated_header(true)
        .titlebar_inset(cfg!(target_os = "macos") && !app.window.fullscreen)
        .window_controls_inset(100.)
        .material(true)
        .drawer_open(app.sidebar_open)
        .on_drawer_change(|app: &mut State, open| app.sidebar_open = open)
        .into()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = State { chrome: WindowChrome::Custom, ..Default::default() };
    Application::new("Workspace", state, view)
        .chrome_from_state(|app| app.chrome)
        .background(volt_ui_kit::native::WindowBackground::Blur { opacity: 0.72 })
        .custom_resize_border(6.)
        .window_actions(|app, sender| app.actions = Some(sender))
        .on_window_state(|app, state| app.window = state)
        .run()
}
```

- `window_controls` draws accessible vector buttons; it emits `WindowAction`
  intents, not OS calls. Its returned row accepts ordinary gap/padding/style
  modifiers, and can be replaced entirely with application-owned controls. Pass
  `WindowControlsState` instead of a boolean to disable individual actions (for
  example maximize when the window is not resizable or is fullscreen).
- `window_actions` uses a separate bounded queue, leaving existing application
  command IDs and `ApplicationRequest` unchanged. Close always follows
  `on_close_request`, including Save/Discard/Cancel policies.
- `on_window_state` reports native maximize/fullscreen/focus/resizable state;
  its `chrome` field is the host's applied decoration policy, not compositor
  acknowledgement. It runs only on the UI thread, with no idle polling timer.
- Linux/Windows Custom edges use a logical resize border (default 6 px,
  configurable 0–24). Maximized/fullscreen/non-resizable windows have no edge
  hits. Reserve that border around interactive content. Header buttons never
  start a window drag. Double-click a blank draggable header to maximize or
  restore. Custom-header dragging waits for 4 logical pixels of movement before
  asking the OS to move the window, so the first click does not swallow a second
  click. Pending drags are cancelled on release, focus loss, resize, suspension,
  maximize and chrome changes.
- **Header system menu:** right-click blank Custom drag regions on Wayland or
  Windows to request the native window menu through winit. Buttons, fields and
  other interactive children keep their application context-menu behavior.
  X11/macOS retain application-defined context menus (the Catalog includes a
  fallback); this does not add a separate title bar.
- **macOS:** Custom keeps the full-size-content window and native traffic
  lights, so AppKit retains outer rounded corners, shadow, edge resizing,
  fullscreen and public material blur. Borderless is visually undecorated:
  it hides the title/buttons but retains the same invisible frame and material.
  This is not a raw `NSWindowStyleMask::Borderless` window. Mode switches restore
  the full-size-content style, button visibility and native shadow. No delegate,
  content view, native button sizing or Metal layer ownership is replaced.
  Private compositor blur is never enabled silently.
- **Linux GPU client frames:** Custom and Borderless clip the *outer window*
  to antialiased 12-logical-pixel corners, including its background clear and
  all content. A translucent swapchain is used even for an opaque background;
  only the corners become transparent, not text or controls. The Wayland blur
  region follows the rounded shape, resize and scale changes. Maximized and
  fullscreen windows become rectangular; restoring them brings the corners back.
  Software fallback remains opaque/rectangular. Native/Overlay mode still uses
  the desktop's decorations, whose corner shape and buttons are compositor-owned.
- Chrome changes requested while fullscreen are deferred until fullscreen ends;
  the Catalog disables its selector there. Compositor shadows, native frame
  shaping, snapping affordances and decoration policy are still platform-owned—custom
  painting does not promise identical OS behavior everywhere. GNOME Wayland
  generally provides no compositor shadow for undecorated surfaces. Custom
  Windows 11 controls do not provide the native maximize-button Snap Layouts
  flyout; use Native chrome when that affordance is required.
- Native remains the host default. Northbound and Catalog default to Custom
  on Linux, placing minimize/maximize/close in their existing headers; use
  `--chrome native` to request desktop decorations instead. Catalog also has
  a live Window selector. Borderless deliberately has no window buttons.
  `WindowChrome` gains `Custom`; downstream exhaustive
  matches on that enum need to handle the new variant. Existing option structs,
  core painters, application request variants and rendering defaults are unchanged.

### Linux desktop acceptance checklist

Cross-compiling and frame-pixel tests do not prove compositor behavior. Test the
native GPU Catalog on the actual KDE/GNOME/COSMIC session at 100% and 125% scale:

```sh
cargo run --release --locked --features native-gpu,native-desktop \
  --example catalog -- --chrome custom
```

- Drag blank existing header space; double-click to maximize and restore.
- Resize from all four edges and all four corners.
- Minimize, maximize/restore (including icon change), close and test close guards.
- Test Super+Up/Down and switch light/dark appearance.
- Verify rounded floating corners, rectangular maximized/fullscreen corners and
  rounded corners after restoring; verify blur if supported by the compositor.
- Right-click blank header space for the native menu on Wayland; on X11 verify
  the Catalog fallback. Controls and fields must not initiate dragging.
- Switch Native / Custom / Borderless without recreating the window; Borderless
  intentionally has no system buttons. Native decorations remain desktop-owned.

## Transparency and blur

| `WindowBackground` | Result |
| --- | --- |
| `Opaque` (default) | A solid window. |
| `Transparent { opacity }` | The window's background is see-through; content stays opaque. |
| `Blur { opacity }` | macOS: public system material (`NSVisualEffectView`), requiring a decorated window. Linux Wayland: negotiated compositor blur. Both require `native-gpu`. |
| `CompositorBlur { opacity }` | A raw compositor blur request (Wayland uses the same negotiated adapter). On macOS this uses a private API, so it is not App Store safe. |

- `opacity` applies only to the window background. Text and controls keep their
  own opacity; give your root a translucent or no background to let the blur show.
- `.background_from_state(|app| WindowBackground::..)` changes the background
  from your state, and `UiTheme::with_background_color(..)` tints it.
- Blur is a request to the OS. The current Windows and X11 adapters do not request it, and
  Reduce Transparency switches the macOS blur to an opaque background.
- The window blur shows what is behind the window. Blurring the app's own
  content behind a panel is not supported yet.

## Linux desktop blur

On native Wayland windows, the Linux adapter prefers
`ext-background-effect-v1` when advertised by the compositor,
then falls back to the older `org_kde_kwin_blur_manager` protocol. It discovers
capabilities on the existing window connection and updates the request after
resize, scale changes, capability changes and user toggles. At 125% scale the
blur region uses **logical surface coordinates**, not framebuffer pixels.

winit 0.30.13 is still the latest stable release at the time of this change.
The newer 0.31 beta has the modern protocol but is not API-compatible with the
current AccessKit adapter. A Linux-only compatibility bridge preserves the
stable winit/desktop interfaces instead of requiring that migration.

```rust,ignore
Application::new("Glass", app, view)
    .background(WindowBackground::Blur { opacity: 0.72 })
    .on_backdrop_status(|app, status| {
        app.backdrop_status = status.description().to_owned();
    })
    .run()?;
```

`BackdropStatus` distinguishes `Disabled`, `Discovering`, `Requested(protocol)`,
`Unsupported(reason)` and `Failed`. **Requested is not proof of visible blur**:
the compositor still controls its blur algorithm, enabled effects and policy.
Catalog shows this report instead of macOS material instructions on Linux.
X11, missing protocols, a declined blur capability and opaque software drawing
are reported explicitly. No desktop capture, compositor configuration changes
or automatic fallback to a different drawing backend is performed by this adapter.

The dispatcher blocks on socket/command readiness when idle; there is no periodic
redraw timer. It never attaches buffers or commits the renderer's `wl_surface`.
After a blur-region change, it requests a normal host redraw so the renderer's
next surface commit applies the double-buffered effect state.

Existing winit consumers can reuse `native::linux_backdrop::Controller` on Linux,
passing their `Arc<Window>` and matching `event_loop.owned_display_handle()`.
Retain the controller, call `update` on size/scale/blur changes and route its
worker-thread status callback through an event-loop proxy. Do not also enable
winit's own blur object for that window. Keep only one controller per window;
duplicate controller creation is rejected before sending protocol requests.
The native Volt host manages this automatically. An older standalone terminal
binary using only `Window::set_blur` does **not** inherit this fix automatically.

The native host logs `VOLT_UI_BACKDROP status=Requested(WaylandBackgroundEffect)`
or the actual unsupported/failure status. Wire-level mock-compositor tests cover
modern-only/legacy-only servers, missing protocols, capability withdrawal and
restoration, global removal/rebinding, resize, disable/re-enable, idle behavior,
disconnect and teardown. These tests and cross-compilation are not a substitute
for native KDE/COSMIC/NVIDIA visual acceptance on the user's machine.

### Testing on Linux

Run from a terminal in your Wayland desktop session, not from a virtual console
started with `startx` (which starts X11). Use `native-gpu`; `native-desktop` alone
does not enable translucent GPU presentation.

```bash
printf 'Desktop=%s Session=%s Wayland=%s\n' \
  "$XDG_CURRENT_DESKTOP" "$XDG_SESSION_TYPE" "$WAYLAND_DISPLAY"

VOLT_UI_RENDERER=gpu cargo run --release --locked \
  --features native-gpu --example catalog -- \
  --page theming --theme dark --background blur \
  --window-opacity 0.45 --tint 0
```

Place a window with text behind the Catalog sidebar. Toggle **Backdrop blur**:
the text behind the sidebar should soften when enabled, while Catalog text stays
sharp. Move **Background opacity** between low and high values, resize the window,
and repeat at 100% and 125% display scale. Repeat with `--theme light`.

Look for `VOLT_UI_BACKDROP status=Requested(WaylandBackgroundEffect)` (modern)
or `Requested(WaylandKde)` (legacy) in the terminal and the protocol description
in the sidebar. A request with no visible blur still needs compositor-side
investigation; an explicit unsupported/failure status identifies a different gate.

## GPU and software drawing

With `native-gpu`, each window draws through **Vello** on wgpu (Metal on macOS,
DX12 on Windows, Vulkan on Linux). Without it, or when the GPU cannot be used,
the host draws with **tiny-skia** on the CPU. Both produce the same layout,
text and hit areas.

| `RendererPreference` | Behaviour |
| --- | --- |
| `Auto` (default) | GPU when compiled and available; falls back to software on errors. |
| `GpuRequired` | Returns an error instead of falling back. |
| `Software` | Always draws on the CPU. |

The `VOLT_UI_RENDERER=auto|gpu|software` environment variable does the same for
apps that use `run_ui` instead of `Application`.

Why Vello? Metal, DX12 and Vulkan are the low-level graphics APIs; Vello is a
2D renderer that runs on top of them through wgpu. It turns paths, text,
gradients, clips and blends into GPU compute work, so the kit has one drawing
path on every platform instead of three hand-written ones.

What still runs on the CPU: your callbacks and `view`, layout, text shaping and
encoding the scene. Layout reuses the measurements of unchanged nodes, and text
shaping is cached.

### Large scenes

Vello works within fixed-size GPU buffers. The kit estimates each frame's cost
before drawing it, splits very large frames into up to 16 horizontal strips, and
draws a frame in software only when it cannot fit. Images are admitted up to
96 MiB of unique pixels per frame.

## Performance and diagnostics

- A settled window draws nothing and uses no timers. Animations request frames
  only while they run.
- `VOLT_UI_TRACE=1` prints per-frame timings (layout, paint, GPU encode and
  present, text cache misses), GPU scene statistics (shapes, glyphs, layers,
  estimated buffer use) and the font and surface setup. It never logs your
  application's text.
- Many examples accept `--bench N` and `--snapshot out.png --size 1200x800 --scale 2`
  for repeatable timing and headless screenshots.

```sh
VOLT_UI_TRACE=1 cargo run --release --features native-gpu --example catalog -- --page dashboard
```

Trace timings start when the host handles an event and end when the frame is
handed to the OS; they do not include display scan-out.

### Headless scene-budget checks

With `native-gpu`, `native::estimate_gpu_scene(&mut painter, &batches)` uses
the GPU scene encoder to report estimated lines, segments and any exceeded
encoder budget **without creating a graphics adapter**. This suits CI and
diagnostics on machines without GPU drivers. Estimates do not prove device
residency, GPU execution or presentation.

The Catalog's editor-page budget regression runs this way on every platform.
Actual GPU readback remains a separate, explicit acceptance test:

```sh
cargo test --locked --all-features --example catalog \
  actual_gpu_editor_heavy_pages -- --ignored --nocapture
```

That command requires a working GPU adapter and fails rather than silently
skipping when one is unavailable.

## Resource limits

| Resource | Limit |
| --- | --- |
| Window framebuffer | 64 megapixels |
| GPU image cache | 64 MiB |
| GPU glyph cache | 16 MiB / 4,096 glyphs |
| Opacity layer depth / clip depth | 24 / 48 |
| Shadow work per frame (software) | 16 megapixels; extra shadows are skipped and counted |

These bound the kit's own caches. GPU drivers, Vello's internal buffers and the
OS compositor use additional memory.

## Linux identity, launchers and backend diagnostics

winit remains the window/input host. On a Wayland session it creates a normal
Wayland toplevel, not a dialog wrapper; on X11 it uses the X11 backend. Volt's
blur adapter attaches to the same Wayland surface. No compositor fork or second
window is involved.

Set a stable reverse-DNS ID matching your desktop-entry filename:

```rust,ignore
Application::new("My app", app, view)
    .app_id("com.example.my_app") // Wayland app_id and X11 WM_CLASS
    .on_window_diagnostics(|app, state| app.diagnostics = Some(state))
    .run()?;
```

`WindowDiagnostics` reports the actual backend, renderer, chrome, scale and
backdrop request state. The host also emits an event-driven `VOLT_UI_WINDOW`
line on changes (not per frame). An unset ID gets a deterministic development
fallback hash derived from the initial title (not its plaintext); explicitly choose
a unique ID for distribution. This is a development ID, not a privacy/security hash.
This property does not replace a macOS bundle ID or Windows package identity.

The default is automatic backend selection. To constrain Linux testing:

```sh
VOLT_UI_BACKEND=wayland cargo run --release --locked --features native-gpu,native-desktop --example catalog -- --chrome custom --background blur --control-style traffic-lights
VOLT_UI_BACKEND=x11 cargo run --release --locked --features native-gpu --example northbound -- --chrome custom
```

The builder equivalent is `.linux_backend(LinuxBackend::Wayland)` (or `X11` /
`Auto`), overriding the environment. Unsupported or unavailable explicit choices
fail clearly; they never silently select a different backend. Non-Linux accepts
only `Auto`. GPU/Vulkan is a rendering backend, not a window-system backend.

Current native blur requires Wayland and a compositor supporting one of Volt's
negotiated protocols. X11 blur is unsupported; software is opaque. Linux GPU
Custom/Borderless windows use the existing rounded mask; maximized/fullscreen
windows are rectangular. Compositor shadows/effects remain desktop policy and
need live KDE/COSMIC acceptance, not just successful compilation.

`cargo volt new` generates wired Custom controls on Linux, native integrated
controls on macOS and native decorations on Windows. All templates use the
shared header/control helpers; you can change the style or chrome policy in Rust.
Creation options `--chrome`, `--control-style` and `--app-id` configure those
policies directly; mobile templates reject these desktop-only options.
The generated `packaging/linux/<app_id>.desktop` and editable SVG icon match the
source identity. Neither `new` nor `run` installs them. Without a registered
launcher/icon, KDE may show the generic Wayland icon even with a correct app ID;
that does not mean the app is a dialog or running inside a Wayland wrapper.

After building a generated app, explicitly register it with the current user:

```sh
cargo build --release
cargo volt install-linux --binary target/release/my-app \
  --app-id dev.volt.my_app --name "My App" \
  --icon packaging/linux/dev.volt.my_app.svg
```

This writes a matching desktop entry in `$XDG_DATA_HOME/applications` and copies
its icon into `$XDG_DATA_HOME/icons/hicolor` (default `~/.local/share`). The
installed desktop entry uses the explicit icon path, avoiding Plasma/Qt theme
lookup or cache differences while retaining the matching themed icon asset. The
launcher points directly to the canonical executable path, with no shell or PATH
lookup. Executable paths containing `%` or `=` are rejected for desktop/GIO
compatibility; build at another path before registering. **Keep that binary at its
registered path**. Rebuild in place, then restart
the app. Re-run registration with `--replace` to update a managed launcher/icon;
unmanaged entries, destination symlinks and ordinary overwrite attempts are
refused. Keep the same SVG/PNG format on updates. SVG or a 256×256 PNG is supported;
icons are size/header checked, not fully decoded. No startup service, sudo,
permission request, compositor setting or dependency is installed. No app is
launched by registration. `--data-dir /absolute/path` isolates packaging tests.

For the checkout's examples:

```sh
cargo build --release --locked --features native-gpu,native-desktop,dev-tools \
  --example catalog --example northbound --bin cargo-volt
./target/release/cargo-volt install-linux --binary target/release/examples/catalog \
  --app-id dev.volt.catalog --name "Volt Catalog" \
  --icon examples/linux/dev.volt.catalog.svg
./target/release/cargo-volt install-linux --binary target/release/examples/northbound \
  --app-id dev.volt.northbound --name "Northbound" \
  --icon examples/linux/dev.volt.northbound.svg
```

Restart running apps after registering; desktop menus/icon caches may refresh
asynchronously. This is local registration, not a portable Linux package or a
system-wide installer. To unregister, remove only the matching managed `.desktop`
and icon paths printed by the command; the executable is never copied/deleted.
Update the source app ID, desktop filename and icon name together if rebranding.
