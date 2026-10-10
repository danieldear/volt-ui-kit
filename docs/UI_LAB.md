# Volt UI Lab — one app for feature testing

Start here rather than running a different app for each capability:

```sh
cargo run --features native --example ui_lab
```

The lab is Rust declarations and real Rust callbacks, not JSON, screenshots or
mock controls. App state is shared across sections and remains in memory until
you reset or exit. It performs no network requests or preference writes.

```text
+----------------------------------------------------------------+
| Volt UI Lab                                      action count  |
| Light / Dark / System                          Accent selector |
| Controls | Layout | Text | Effects | Surfaces | Scroll | Window |
+----------------------------------------------------------------+
|                                                                |
| Selected section                                               |
| Live specimens and controls                                    |
| Scrollable content (fixed navigation and footer)                |
|                                                                |
+----------------------------------------------------------------+
| Previous | Next | Reset                  last callback result   |
+----------------------------------------------------------------+
```

Navigation scrolls horizontally on narrow windows. Previous/Next always provides
a visible way to reach every section. The page body scrolls independently; the
header and footer stay fixed. Reset keeps your current section, appearance and
window launch settings, and resets demo values. There is no persistence to disk.

## Sections and what to test

| Section | Live coverage |
| --- | --- |
| Controls | Editable single/multi-line fields, password/read-only inputs, stepped slider, typed dropdown, focus-trapped review dialog, Add/clear count, enabled/disabled buttons, checkbox/toggle state, joined toolbar actions, typed density selection, active-page tabs with top/bottom placement, keyed list reverse/remove/restore, callback activity |
| Layout | Responsive Stack, fixed VStack/HStack, ZStack layering, nine placements, spacers/dividers, multiple padding edges/gap axes, default/numeric/full/selected corners |
| Text & colors | Installed system/sans/serif/monospace families, weights 300–900, sizes, italic, Unicode, literal color override, paired semantic color swatches |
| Effects | All-sided glow, shadow, hover/pressed/focus animation, centered keyed fade notice, whole-group opacity with interactive controls |
| Surfaces | Image cover/contain/stretch/tile and position, image-only opacity, rectangles/circles/ellipses/triangles/curves/transforms, rounded/circle/ellipse/diamond clipping with matching input masks, background-only alpha |
| Scrolling | 60-row vertical list, horizontal cards, both-axis canvas, nested boundary bubbling, indicator hide/show while scrolling remains enabled |
| Window | Current startup requests, translucent panels, drag region, native transparency/blur/chrome commands, honest platform acceptance checklist |
| Showcase | Volt Atelier: Welcome, Workspace and Component Studio using the same App state, with original art, SF typography, themes and working controls |

Appearance controls at the top update every section. Choose Light, Dark or System;
cycle blue/violet/coral accents. Semantic color roles have paired foregrounds.
Missing font families or weights fall back to installed fonts. `--font` selects a
family name, not an arbitrary TTF/OTF file.

## Launch options

For the new component family, scroll Controls to **Toolbars, joined actions and
typed selection** and **Tabs / active page only**. Test Save/Reset, density changes,
Overview/Activity, the disabled Later tab and bottom placement. With a tab or
segment focused, Left/Right and Home/End select; selecting the current value
emits no callback. Inactive pages must not accept clicks or retain installed
handlers. See [the component API and delivery phases](COMPONENTS.md).

```sh
# Same app, different starting section/appearance
cargo run --features native --example ui_lab -- --section surfaces --theme light --accent violet
cargo run --features native --example ui_lab -- --section typography --font "Helvetica"
cargo run --features native --example ui_lab -- --section effects --reduced-motion

# Native compositor requests; restart to change window creation settings
cargo run --features native-gpu --example ui_lab -- --section window --background transparent --window-opacity 0.55
cargo run --features native-gpu --example ui_lab -- --section window --background blur --window-opacity 0.65 --chrome overlay

# All flags
cargo run --features native --example ui_lab -- --help
```

`--section` accepts `controls`, `layout`, `typography`, `effects`, `surfaces`,
`scrolling`, `window`, or `showcase`. `--screen welcome|workspace|studio` opens
Showcase directly; `--palette iris|tide|ember` sets its palette. See
[the app-scale showcase guide](SHOWCASE_APPS.md) for all screens and Volt Desk. `--chrome` accepts `native`, `overlay`, or `borderless`.
Overlay keeps native traffic lights on macOS; other platforms may retain native
decorations. Borderless removes system decorations and has no in-app window-control
components yet. Use your OS app/window controls to close it.

The default background is opaque. Alpha/blur requests need `native-gpu` and a
supporting GPU/OS/compositor. Initialization failures log an explicit opaque
software fallback. Native blur is an OS capability, not a portable radius effect.
The lab deliberately avoids an opaque root fill for alpha/blur requests, but that
alone is not proof of desktop compositing. Window creation settings are startup
options, not nonfunctional live toggles.

## Scrolling without a visible scrollbar

```rust
use volt_ui_kit::ui::prelude::*;
let ui: Element<()> = hstack![text("A"), spacer().width(800.), text("B")]
    .height(100.)
    .scroll(Scroll::Horizontal)
    .scrollbars(Scrollbars::Hidden)
    .into();
ui.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

Hidden indicators do not disable scrolling or overflow clipping. There is no
reserved gutter, so hiding indicators does not change layout. `Scrollbars::Auto`
is the default: an indicator is painted only for an overflowing enabled axis.
The Scrolling section's toggle applies this policy to its inner demo views;
navigation/page-body indicators remain visible to aid discovery.

## Reproducible headless checks

```sh
cargo test --features native --example ui_lab --locked
cargo test --features native --test surface_composition --locked

cargo run --features native --example ui_lab -- --section surfaces --theme light --size 600x760 --scale 2 --snapshot /tmp/ui-lab-surfaces.png
cargo run --features native --example ui_lab -- --section controls --bench 20 --bench-target increment
```

Tests cover every registered section at 320/600/1120 logical widths and 1x/2x DPI,
section navigation/state retention, compact navigation, keyed callbacks, theme
selection and CLI validation. Scroll tests reach the final row and verify hidden
indicators do not break input. Snapshots show the settled visual state; benchmarks
exclude OS queues/compositor presentation and are not input-to-photon latency.

Mac smoke checks now cover actual typing/submit/Unicode paste/undo, slider keys,
selection popup and modal Escape/focus restoration. Broader acceptance still needs
a target device: wheel/trackpad feel, native blur,
desktop-through transparency, OS theme-change events and window controls. Thin
indicators are not draggable yet; keyboard focus auto-reveal, touch-drag and smooth
scrolling remain pending. OS accessibility and actual non-Latin input-method
acceptance remain pending; text inputs, sliders, select and dialogs are implemented.

### Historical verification receipt (2026-10-08)

The current all-feature target suite passed 200 tests; rustdoc passed 18 with one
existing ignored example (218 passing total). The lab contributes ten behavioral
tests, including exact adaptive-stack geometry. Separate runtime tests cover nested
scroll sizing and fractional badge widths; the surface suite includes a 1x/2x
hidden-indicator pixel/input regression. Strict all-target Clippy, Rust 1.87 checks,
default-feature and Rust-only tests and
all-feature package compilation also passed. Light/dark snapshots of all seven
sections, compact 2x and a taller surfaces view were generated and representative
views inspected.

The default Controls page at 960x760, 20 warm CPU samples after four warmups,
measured 0.336 ms p95 for dispatch/composition/layout and 8.501 ms p95 from release
handling to packed pixels. These are local CPU observations, not OS input latency,
compositor timings, portable performance guarantees or native device acceptance.

### Current verification receipt (2026-10-09)

The Lab now has **20 behavioral tests**, including live controlled editors,
undo/redo, submission, slider changes, typed select and modal focus restoration
at compact/desktop sizes and 1x/2x scale. The full `native-gpu` all-target suite
passed **318 tests** (one opt-in hardware test ignored, then run explicitly), plus
**22 rustdoc tests** (one existing ignored example);
shared Lab tests occur through multiple example entrypoints. Strict all-target
Clippy, Rust 1.87 checks, default/declarative-only checks and packaging passed.

Native Mac checks verified typing, Unicode clipboard paste, whole-emoji deletion,
undo/redo, slider and popup input, modal focus trap/dismissal and blocked background
scrolling. A 24-line text-area paste follows the caret; wheel scrolling reveals
earlier lines without moving the surrounding page. Desk also sends the actual
edited draft by click or Cmd+Enter. Foreign-language OS IME UX and accessibility
acceptance remain separate from these tests.

One earlier **software** Controls idle sample at 2x scale showed no CPU-time increase at
`ps` reporting precision over 30 seconds and stable **122.83 MiB RSS**. This is
not a universal zero-CPU or GPU-memory claim. Current rich-screen CPU frame costs,
GPU drawing/readback boundaries and remaining measurements are documented in
[resource usage](RESOURCE_USAGE_AND_DESKTOP.md).

## Keep extending this app

- Entry point: `examples/ui_lab.rs`.
- State, route registry, theme and app-specific CLI: `examples/ui_lab/state.rs`.
- Fixed shell/shared specimen helpers: `examples/ui_lab/views.rs`.
- Feature specimens: `examples/ui_lab/views/*.rs`.
- Behavior and viewport matrix: `examples/ui_lab/tests.rs`.
- Common native/snapshot/benchmark host: `examples/support/rust_ui.rs`.

Add future features to an existing section when possible. For a new section, add
it to `Section::ALL`, slug/title and the view match; the registry drives navigation,
CLI selection and the whole-section test matrix. Use owned captures and stable
keys; asset creation/loading belongs outside view functions. Add actual callbacks,
a visible result and a regression test, not a static imitation. Keep the focused
examples for minimal repros rather than introducing more manual-test launchers.

## Real GPU drawing

`VOLT_UI_RENDERER=gpu cargo run --locked --features native-gpu --example ui_lab`
uses Vello for opaque windows too. `VOLT_UI_RENDERER=software` selects the
comparison backend without changing declarations. GPU snapshots/benchmarks use
explicit blocking readback; native windows do not. See
[GPU dependency policy and validation](GPU_RENDERING.md). Earlier idle/frame
measurements above are software/presenter baselines, not GPU utilization proof.

## Runtime and Data sections (2026-10-09)

- `--section runtime`: cancellable Rust worker messages, clickable counter during work, loading/progress, typed action/context menus, nonblocking tooltip, anchored inspector and soft-wrapped notes.
- `--section data`: 10,000 fixed-height records with bounded mounted window and stable selection; actual sortable table, expandable tree and pointer/keyboard-resizable split.
- Native worker channels also initialize from direct `showcase` launch; headless snapshots/benchmarks do not start threads.
- GPU/software share the declarations. Use `VOLT_UI_RENDERER=software` to compare without changing the view.
- See [runtime contract](APP_RUNTIME.md), [extended components](EXTENDED_COMPONENTS.md), and [milestone plan/receipt](APP_RUNTIME_AND_COMPONENT_PLAN.md).
