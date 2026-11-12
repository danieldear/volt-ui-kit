# Examples

From a repository checkout, launch the complete Catalog with `./scripts/run_catalog.sh`.
Run individual examples with `cargo run --release --locked --features <features> --example <name>`.
`native-gpu` draws with Vello on the GPU; `native` draws on the CPU.

## Start here

| Example | Shows |
| --- | --- |
| `catalog` | Every component with live previews, source and API tables; themes, the Theme editor and a dashboard |
| `samples` | Eight runnable Button, Select and Table recipes, using the same builders as the Catalog |
| `rust_counter` | The smallest app: state, a view and callbacks |
| `rust_settings` | Reusable view functions, conditionals and typed toggles |
| `rust_keyed_list` | Lists, stable keys and callbacks that capture IDs |
| `rust_layout` | Stacks, spacing, placement, rounding, dividers and spacers |
| `rust_responsive` | Breakpoints, responsive direction and spacing, overlapping layers |
| `rust_surfaces` | Theme colours, images, vector drawing and clipping |
| `rust_scroll` | Fixed headers and nested scroll views |
| `rust_effects` | Shadows, hover and press states, animation, fades and custom window chrome |
| `rust_transparent` | A see-through window (needs `native-gpu`) |

```sh
./scripts/run_catalog.sh
cargo run --locked --features native --example rust_counter
```

## Catalog launch modes

| Mode | Command / purpose |
| --- | --- |
| Full Catalog (recommended) | `./scripts/run_catalog.sh`: GPU, desktop services and local notifications; builds, signs and registers an identified macOS `.app` before opening it. |
| Build/verify only | `./scripts/run_catalog.sh --check`: no UI launch, permission prompt or notification. |
| Targeted component work | Direct `cargo run` below: accepts Catalog flags, but does not package the notification demo. |
| Software component work | Replace the features below with `native`; no GPU drawing, native menus/tray or notification adapter. |

```sh
cargo run --release --locked --features native-gpu,native-desktop --example catalog -- --page dashboard --theme dark --accent violet
```

Window settings is always available in the Catalog's existing header, including
compact windows. Choose Native / Custom / Borderless and Symbols / Traffic lights;
macOS retains native traffic lights. For direct Linux testing:

```sh
VOLT_UI_BACKEND=wayland cargo run --release --locked --features native-gpu,native-desktop --example catalog -- --chrome custom --background blur --control-style traffic-lights
VOLT_UI_BACKEND=wayland cargo run --release --locked --features native-gpu --example northbound -- --chrome custom --control-style traffic-lights
```

The `VOLT_UI_WINDOW` log identifies the actual backend, application ID, renderer,
scale and blur request state. `VOLT_UI_BACKEND=x11` explicitly tests X11 (no native
blur support). Without the variable, normal winit auto-selection remains in use.
Other small recipes keep their existing native frames; Custom is not an automatic
injected title bar. See [chrome integration](../docs/windows-and-rendering.md).

For desktop-menu/taskbar integration, optional launchers are in `examples/linux/`.
After building the release binaries, you may install the matching binary and
launcher yourself (this is not required to run with Cargo):

```sh
install -Dm755 target/release/examples/catalog "$HOME/.local/bin/volt-catalog"
install -Dm644 examples/linux/dev.volt.catalog.desktop "$HOME/.local/share/applications/dev.volt.catalog.desktop"
```

Ensure `~/.local/bin` is on the graphical session's PATH, or edit `Exec` and
`TryExec` to your absolute installed binary path before using the launcher.
Northbound has an equivalent `dev.volt.northbound.desktop` launcher and expects
`volt-northbound` on PATH. These development launchers use a generic system icon;
projects can supply their own. They do not install tray/startup services or
alter window-manager rules. A missing launcher affects identity/icon matching,
not whether the window is a modal dialog.

On macOS, local notifications need both `native-notifications` **and** a running
identified app bundle; use the full launcher rather than adding a flag to
`cargo run`. Permission remains an explicit button click. See
[notification testing](../docs/app-services.md#testing-catalog-notifications).
Other platforms report the unavailable notification adapter explicitly.
The launcher takes only `--check` or `--help`; it does not forward the direct
example flags above. It requires Cargo and Python 3 (plus macOS command-line
tools for bundling), and exists only in the repository, not the published crate.

## Runnable component recipes

Button, Select and Table Code panels contain complete native Rust programs,
including their state, imports, view and `main`. Other panels remain labeled
Catalog excerpts. Run the recipes independently from this checkout:

```sh
cargo run --locked --features native --example samples -- --list
cargo run --locked --features native --example samples -- button-states
cargo run --locked --features native --example samples -- select-vs-combobox
cargo run --locked --features native --example samples -- table-square
cargo run --locked --features native --example samples -- --source table
```

The eight recipe names are `button-variants`, `button-sizes`, `button-states`,
`select`, `select-disabled`, `select-vs-combobox`, `table` and `table-square`.
`--list` and `--source` do not open windows. Replace `native` with `native-gpu`
to render on the GPU.

Six source files in [`catalog/samples`](catalog/samples) implement the eight
recipes. Select/comparison and rounded/square Table reuse their implementation.
Within the native Catalog, the two Table previews still share sorting and row
selection; separate browser previews have independent state.

To copy a Code panel into a new project's `src/main.rs`, add this dependency:

```toml
volt-ui-kit = { path = "/path/to/volt-ui-kit", features = ["native"] }
```

Replace the path with this checkout (the crate is not yet published). The
Select program also accepts `--compare`; the Table program accepts `--square`.
These choose the comparison and square variants when running a copied program.

## Linux window controls and corners

Northbound and Catalog default to **Custom** on Linux: minimize, maximize/restore and close
are integrated into their existing headers, without another title bar. Use
`--chrome native` for desktop-owned decorations. Borderless intentionally omits
window buttons. Northbound centers its client controls 26 logical pixels below
its top edge, independently of the title/search row; the header stays the same
height. Resetting the sample itinerary preserves its host connection.

For rounded outer corners in Custom/Borderless, use GPU rendering. The software
fallback is opaque/rectangular. Maximizing removes the corners until restored;
Native mode's frame and buttons remain the desktop compositor's responsibility.

```sh
cargo run --release --locked --features native-gpu --example northbound
cargo run --release --locked --features native-gpu --example catalog -- --chrome custom
```

## Apps

| Example | What it is |
| --- | --- |
| `relay` | A coding-agent client: thread sidebar, streaming transcript, composer menus and an inspector. Replies come from a local script, so it runs offline. |
| `northbound` | A trip planner using most components, with generated photos and two themes. |
| `showcase` | Volt Atelier: welcome, workspace and component-studio screens. |
| `desk` | A three-pane conversation workbench with an editable composer. |
| `maps` | Experimental Rust-rendered maps, camera rotation/tilt, search and routes; MapKit snapshots on macOS. Not a reusable toolkit map component yet. |

### Maps prototype boundaries

`cargo run --release --locked --features native-gpu --example maps` uses live
network services. Mapbox/Google need developer-provided keys; Apple snapshots
need macOS. The website uses the free vector provider, without provider keys.

Network vector tiles are limited to 16 MiB before and after decompression,
128 layers, 100,000 features and 1,000,000 points, with a separate logical
allocation budget. Text responses are capped at 8 MiB and desktop raster tiles
at 32 MiB. Browser downloads use streaming limits, six concurrent requests and
a 20-second abort deadline. Invalid/over-budget responses become errors; these
limits do not imply a bounded long-session tile cache (eviction is pending).
Managed Mapbox tokens are attached only to the exact HTTPS API authority.

## Experimental iOS app

[`ios/VoltIOS.xcodeproj`](ios/VoltIOS.xcodeproj) runs **Field Notes**, a Rust-owned
notes/list/detail/profile prototype with a UIKit keyboard and Metal host. It
uses a separate unpublished static-library crate, not `cargo run --example`.
The example uses the reusable mobile host. For your own Rust-only app source,
use `cargo volt new my-ios-app --platform ios --kit-path /path/to/volt-ui-kit`.
See the [iOS guide](../docs/ios-prototype.md) for setup, tests, Android direction
and limitations; no per-app Swift implementation is required.

## JSON frontend

`complete_app` and `components` load their interface from `examples/ui/*.json`.
Edit the file while the example runs and press F5 to reload it. See
[JSON interfaces](../docs/json-ui.md).

## Other

| Example | Purpose |
| --- | --- |
| `specimen` | Every component at its default style, for design review |
| `ui_lab` | Older interactive test sections (scrolling, windows, data, runtime) |
| `gallery` | Renders the drawing recipes to SVG files in `target/gallery/`; needs no features |

## Common flags

Most examples accept:

| Flag | Effect |
| --- | --- |
| `--theme light\|dark\|system` | Colour mode |
| `--size 1200x800`, `--scale 2` | Window or snapshot size and pixel density |
| `--snapshot out.png` | Render one frame to a PNG without opening a window |
| `--bench 60` | Time repeated updates and print p50/p95 |

The Catalog adds `--page <name>` (the sidebar title in lowercase with hyphens,
such as `theme-editor` or `form-inputs`), `--accent zinc|blue|violet|rose|green|orange`,
`--background opaque|transparent|blur|compositor`, `--window-opacity 0.8` and
`--tint 20` (percent).

Snapshots and benchmarks use the same layout and drawing code as the window,
but they don't exercise real input or the OS compositor.

## Experimental Android starter

Use `cargo volt new my-android-app --platform android --kit-path /path/to/volt-ui-kit`.
The generated Rust app references one shared native OS adapter; no per-app
Java/Kotlin UI code. See the [Android guide](../docs/android-prototype.md) for
SDK/NDK setup, APK commands, the smoke test and remaining hardware gates.

## WebAssembly live demo

`examples/web` uses the separate Rust browser host for live buttons, tabs, forms and scrolling. See the [web guide](../docs/web-prototype.md) for tool versions, build steps, GPU/software behavior and remaining limitations.

### Linux application icons

A correct Wayland app ID alone does not install a menu entry or icon. To avoid
KDE's generic Wayland icon, build the examples and explicitly register their
matching icons/launchers with `cargo volt install-linux`. See the exact
[Catalog and Northbound commands](../docs/windows-and-rendering.md#linux-identity-launchers-and-backend-diagnostics).
Registration never launches an app or changes the compositor; plain `cargo run`
stays installation-free. Restart existing windows after registration.
