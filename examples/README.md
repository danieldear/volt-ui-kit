# Examples

From a repository checkout, launch the complete Catalog with `./scripts/run_catalog.sh`.
Run individual examples with `cargo run --release --locked --features <features> --example <name>`.
`native-gpu` draws with Vello on the GPU; `native` draws on the CPU.

## Start here

| Example | Shows |
| --- | --- |
| `catalog` | Every component with live previews, source and API tables; themes, the Theme editor and a dashboard |
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

On macOS, local notifications need both `native-notifications` **and** a running
identified app bundle; use the full launcher rather than adding a flag to
`cargo run`. Permission remains an explicit button click. See
[notification testing](../docs/app-services.md#testing-catalog-notifications).
Other platforms report the unavailable notification adapter explicitly.
The launcher takes only `--check` or `--help`; it does not forward the direct
example flags above. It requires Cargo and Python 3 (plus macOS command-line
tools for bundling), and exists only in the repository, not the published crate.

## Apps

| Example | What it is |
| --- | --- |
| `relay` | A coding-agent client: thread sidebar, streaming transcript, composer menus and an inspector. Replies come from a local script, so it runs offline. |
| `northbound` | A trip planner using most components, with generated photos and two themes. |
| `showcase` | Volt Atelier: welcome, workspace and component-studio screens. |
| `desk` | A three-pane conversation workbench with an editable composer. |

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
