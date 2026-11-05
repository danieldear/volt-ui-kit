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
| `Borderless` | No system decorations; you supply the window controls. |

Mark your header with `.drag_region(true)` so the window can be dragged by it.
Buttons and other controls inside the header still receive clicks.

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
