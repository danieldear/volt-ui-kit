# Getting started

Volt UI Kit builds desktop interfaces in plain Rust. You write a function from
your application state to a tree of elements; the kit lays it out, draws it on
the GPU (or the CPU as a fallback), routes input and accessibility, and calls
your Rust closures when something happens.

## Add the dependency

The crate is not on crates.io yet. Use a Git or path dependency:

```toml
[dependencies]
volt-ui-kit = { git = "https://github.com/danieldear/volt-ui-kit", features = ["native-gpu"] }
# or, from a local checkout:
# volt-ui-kit = { path = "../volt-ui-kit", features = ["native-gpu"] }
```

Rust 1.89 or newer is required. Pin a reviewed Git revision using `rev` and
commit your app's `Cargo.lock` for reproducibility; a moving default branch is
not a versioned release.

### Features

Nothing is enabled by default; pick what your application needs.

| Feature | Adds |
| --- | --- |
| `native` | A window host: winit, cosmic-text, software drawing (tiny-skia), images, clipboard, IME and AccessKit accessibility. Includes `rust-ui`. |
| `native-gpu` | GPU drawing with Vello on wgpu (Metal, DX12 or Vulkan), with automatic software fallback. Includes `native`. |
| `native-desktop` | macOS application menus, a tray menu, and native open/save panels. |
| `native-notifications` | macOS local notifications. |
| `rust-ui` | The Rust builders (`ui` module) without a window, for custom hosts. |
| `declarative` | The validated document tree, runtime and optional JSON frontend. |
| `dev-tools` | The `cargo volt` command: project templates, a rebuild-and-restart dev loop, and macOS app bundles. |

Most applications want `native-gpu`. Add `native-desktop` for menus and file
panels.

## Your first window

```rust,no_run
use volt_ui_kit::{native::Application, ui::prelude::*};

#[derive(Default)]
struct App {
    count: u32,
}

fn view(app: &App) -> Element<App> {
    vstack![
        text(format!("Clicked {} times", app.count))
            .font_size(24.)
            .font_weight(600),
        button("Click me")
            .key("add")
            .variant(ButtonVariant::Primary)
            .on_click(|app: &mut App| app.count += 1),
    ]
    .padding(24., Edge::All)
    .gap(12., Axis::Vertical)
    .into()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Application::new("Counter", App::default(), view)
        .size(480, 320)
        .run()
}
```

- `App` is your state. The kit never owns or copies it.
- `view` describes the screen for the current state. It runs once at start and
  again after every callback, so keep it free of I/O and side effects.
- Callbacks receive `&mut App`. Change state there; the next `view` shows it.
- `.key("add")` gives the button a stable identity, so focus and animation
  survive recomposition. Key every interactive element in a dynamic list.

`Application` defaults to the system light/dark theme, GPU drawing when
compiled in, and an ordinary native title bar. See
[Windows and rendering](windows-and-rendering.md) to change that.

## Start from a template

`cargo volt` creates a project and runs it with automatic rebuilds:

```sh
# From a checkout of this repository:
cargo install --locked --path . --features dev-tools --bin cargo-volt

cargo volt new my-app --kit-path /path/to/volt-ui-kit --template navigation
cargo volt dev my-app
```

Templates: `minimal`, `navigation`, `mail`, `settings` and `agent`. Add
`--desktop` for macOS menus and a tray item. `new` never overwrites an existing
directory. All but `minimal` start from `navigation_split_view`, the Catalog's
own sidebar-and-detail shell, with a sidebar you resize by dragging its
divider. Every template also uses the Catalog's colours:
its `theme()` function picks `Palette::zinc_light()` or `Palette::zinc_dark()`
to match the system appearance (see
[Built-in palettes](styling-and-themes.md#built-in-palettes)).

`cargo volt dev` watches your sources, rebuilds on save and restarts the app
after a successful build; a failed build leaves the running app alone. Apps that
call [`Application::dev_state`](app-services.md#the-development-loop) keep their
state and window position across restarts.

## Explore the components

The Catalog example shows every component with live previews, source code,
API tables, themes and a working dashboard:

```sh
cargo run --release --locked --features native-gpu,native-desktop --example catalog
```

Useful flags: `--page dashboard`, `--theme light|dark`, `--accent violet`.
Page names are the sidebar titles in lowercase with hyphens, for example
`--page theme-editor` or `--page form-inputs`.

## Platform support

| Platform | Status |
| --- | --- |
| macOS 11+ | Primary platform; tested on Apple silicon with Metal. |
| Windows | Cross-compilation checked; headless CI configured (DX12). Native runtime not yet validated. |
| Linux | Cross-compilation checked; headless CI configured (Vulkan, X11/Wayland). Native runtime not yet validated. |
| WebAssembly | The core (`rust-ui`) compiles for `wasm32-unknown-unknown`; there is no web host yet. |

## Next

- [Building interfaces](building-uis.md): state, callbacks, keys, layout and scrolling.
- [Components](components.md): everything from buttons to tables and charts.
- [Styling and themes](styling-and-themes.md): colours, custom themes, icons, effects and motion.
- [Application services](app-services.md): background work, commands, files, menus and packaging.
