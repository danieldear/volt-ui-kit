# Reusable Rust UI components

## Delivered Rust-first component family (2026-10-09)

These are real reusable builders under `rust-ui`, not mock controls or JSON:

| Component | Contract |
| --- | --- |
| `button_bar![]` / `button_bar(items)` | Horizontal actions; independent Rust callbacks; optional joined outer corners |
| `toolbar![]` / `toolbar(items)` | Horizontal application toolbar; accepts text, spacers, actions and other elements |
| `segmented_control([(value, label), ...])` | Joined, controlled typed selection |
| `tab_bar![tab_item(value, label), ...]` | Controlled typed selection with per-item keys and disabled options |
| `tabs![tab(value, label, content), ...]` | Tab strip plus **only the selected page**, above or below the page |
| `text_field(value)` / `text_area(value)` | Controlled plain-text editing, selection, bounded undo/redo, IME and native text clipboard |
| `slider(value)` | Controlled finite `f32` range/step, captured drag and keyboard input |
| `select([select_item(value, label), ...])` | Typed selection with controlled, anchored popup and disabled options |
| `dialog(content)` | Controlled in-window modal, input isolation, trapped/restored focus, dismissal |

Bars lower to existing Rows, Columns and Buttons; editors/sliders/overlays carry
Rust-only metadata through that document. No separate window, GPU device or renderer
is allocated per component. Unicode segmentation is optional under `rust-ui`; text
clipboard support is native-only. Default embedded dependencies remain unchanged. These are custom-painted UI
components, **not** OS-native toolbars/menus or an accessibility implementation.

```text
Application state: Page enum, density, counters
                   |
                   v
Rust view() -> toolbar / button_bar / segmented_control / tabs
                   |
                   v
Existing document + typed callback registry
                   |
                   v
Shared layout / pointer / focus / painting
                   |
           selection event
                   |
                   +--> Rust on_select -> update App -> recompose
```

## Variants, sizes and refined defaults (2026-10-10)

Review every default together with `cargo run --features native --example specimen`
(add `--theme light`, or `--snapshot PNG --size 1400x1000` for headless review).

```rust
use volt_ui_kit::ui::prelude::*;
let actions: Element<()> = hstack![
    button("Save").key("save").variant(ButtonVariant::Primary).on_click(|_: &mut ()| {}),
    button("Preview").key("preview").variant(ButtonVariant::Soft).on_click(|_: &mut ()| {}),
    button("Cancel").key("cancel").variant(ButtonVariant::Ghost).on_click(|_: &mut ()| {}),
    button("Delete").key("delete").variant(ButtonVariant::Danger).size(ControlSize::Small)
        .on_click(|_: &mut ()| {}),
    button("Sort by").key("sort").disclosure(false).label_align(Align::Start)
        .width(140.).on_click(|_: &mut ()| {}),
]
.gap(8., Axis::Horizontal)
.background(ColorToken::Primary.alpha(8))
.into();
actions.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

- `ButtonVariant` (Neutral, Primary, Soft, Outline, Ghost, Danger) and
  `ControlSize` (Small 28px/12pt, Medium 36/14, Large 44/16) are theme-token
  presets; later `.background`/`.color`/`.hover_style` calls still override them.
- `.disclosure(open)` draws a trailing vector chevron (up while open) and
  left-aligns the label; select and menu triggers use it. `.label_align(Align)`
  places any button label at the start, centre or end.
- Theme tokens accept an opacity: `ColorToken::Primary.alpha(15)` (`"@primary/15"`)
  resolves from the active palette, so tints follow light/dark and accents.
- Primary buttons pair their label with `PrimaryContent` explicitly, so an
  inherited text colour can no longer make them unreadable; built-in palettes
  derive `on_primary` by contrast with the primary fill.
- Defaults: buttons and bars use 6px corners (JSON documents keep their
  legacy recipe); `panel(...)` is a padded surface (12px, override with
  `.padding`); joined bars and segmented controls show 1px seams.
- Tables: sortable headers are left-aligned ghost buttons and fixed columns are
  plain labels (no disabled look); rows are ghost targets with a soft selected
  tint; cells are inset to align with headers; height is natural unless set.
  Trees use left-aligned ghost rows with a soft selection and muted expanders,
  and report the specific validation problem.

## A complete declaration

```rust
use volt_ui_kit::ui::prelude::*;

#[derive(Clone, Copy, Default, PartialEq)]
enum Page { #[default] Overview, Activity }
#[derive(Default)]
struct App { page: Page, count: usize }

fn view(app: &App) -> Element<App> {
    vstack![
        toolbar![
            text("My workspace").font_weight(600), spacer(),
            button_bar![
                button("Add").key("add")
                    .on_click(|app: &mut App| app.count += 1),
                button("Clear").key("clear").enabled(app.count > 0)
                    .on_click(|app: &mut App| app.count = 0),
            ].joined(true).rounded(8.),
        ],
        segmented_control([
            (Page::Overview, "Overview"), (Page::Activity, "Activity"),
        ]).key("page-picker").selected(app.page).rounded(Rounding::Full)
            .on_select(|app: &mut App, page| app.page = page),
        tabs![
            tab(Page::Overview, "Overview",
                text(format!("Count: {}", app.count)).font_size(24.))
                .key("overview-tab"),
            tab(Page::Activity, "Activity", text("A second, mounted-on-demand page"))
                .key("activity-tab"),
        ].key("pages").selected(app.page).placement(TabPlacement::Bottom)
            .on_select(|app: &mut App, page| app.page = page)
            .gap(16., Axis::Vertical),
    ].padding(20.5, [Edge::Horizontal, Edge::Top])
        .gap(18., Axis::Vertical).font_family("sans-serif").into()
}

let app = App::default();
view(&app).build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

The duplicated selector here demonstrates sharing a typed enum between widgets;
normally choose either a segmented picker or tab strip for the same navigation.
Use `native::run_ui` to drive a view, or supply your own document/input host.

### Item keys and disabled choices

```rust
use volt_ui_kit::ui::prelude::*;
let ui: Element<usize> = tab_bar![
    tab_item(0, "Editor").key("editor-tab"),
    tab_item(1, "Preview").key("preview-tab"),
    tab_item(2, "Later").key("later-tab").enabled(false),
].key("navigation").selected(0)
 .on_select(|page: &mut usize, next| *page = next)
 .height(44.).scroll(Scroll::Horizontal).into();
ui.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

Use explicit item keys for reordered/dynamic options. The tuple-based segmented
factory uses positional automatic keys; use `tab_bar(items).joined(true)` when
you need keyed segmented items. Values need `Clone + PartialEq + 'static`; they
can be enums, integers or application types. Labels are display text, not IDs.

## Behavior and boundaries

- Selection is **controlled**: `.selected(app.value)` and `.on_select(...)`.
  The host must rebuild/install the view after activation, as `run_ui` does.
- Missing callbacks, duplicate values, empty selections, a selected value not in
  the options, duplicate keys and invalid styles fail `build()` before installation.
  If `.selected(...)` is omitted, the first option is selected.
- Disabled items do not activate; `.enabled(false)` disables all items in a
  selection control. A disabled item may remain the app's selected value.
- Selecting the already-selected option emits **no** callback and requests no
  recomposition. Keyboard selection wraps and skips disabled/unlaid-out options.
- With an item focused, native Left/Right and Home/End select within its group.
  Embedded hosts use `Runtime::selection_key`. Enter/Space and Tab/Shift-Tab retain
  the existing button contract. This is not yet ARIA/platform roving-tab semantics.
- `tabs` installs callbacks and layout only for the active page. Keep persistent
  page data in App; inactive pages do not retain a hidden widget/cursor subtree.
  Rust expressions passed to `tab(...)` are still evaluated eagerly during view
  construction. This is active-only **mounting**, not lazy evaluation of arbitrary
  page-producing functions. Build expensive content conditionally in your view.
- `TabPlacement::Bottom` moves the strip; it does not add an iOS/Android host.
- Container styling/spacing/font/theme/clip/scroll modifiers work on the group.
  Selected options explicitly use `PrimaryContent` for the primary surface so
  a general inherited foreground cannot make pale accent buttons unreadable.
  Individual button styling remains available in button bars/toolbars. This first
  selection API does not provide arbitrary per-option view templates.
- `.joined(true)` removes horizontal gaps and squares internal corners. `.rounded`
  and `.rounded_on` choose the **outer** corners; Full stays symbolic. Joined child
  corners intentionally replace each child's independent radius.
- Toolbars are horizontal, not automatically wrapping or overflowing into menus.
  Use responsive stacks/scrolling around large groups. Keyboard focus auto-reveal,
  touch navigation and screen-reader semantics remain separate follow-ups.
- Legacy widgets/Painter and the schema1 JSON format remain unchanged. Group
  keyboard behavior is a Rust-only opt-in; ordinary buttons ignore selection keys.

## Test in one app

```sh
cargo run --locked --features native --example ui_lab -- --section controls
cargo run --locked --features native --example showcase
cargo test --locked --features rust-ui --test component_bars
cargo test --locked --features native --example ui_lab
```

Controls includes joined Save/Reset, density selection, active-page tabs,
disabled options, top/bottom placement and real callback activity. Atelier's
screen navigation now uses the reusable typed tab bar and its header is a toolbar.
Automated tests cover pointer/keyboard callbacks, Full/selected corners, disabled
items, active-only handlers, 1x/2x DPI and compact lab scrolling. Native Mac smoke checks now include typing/submit, Unicode paste/deletion/undo,
slider keys, popup selection, modal Escape/focus restoration and background wheel isolation.
Actual non-Latin OS IME and Linux/Windows acceptance still need target-platform testing.

## Editable fields, value controls and modal declarations

```rust
use volt_ui_kit::ui::prelude::*;
#[derive(Clone, Copy, Default, PartialEq)]
enum Order { #[default] Recent, Name }
#[derive(Default)]
struct App { draft: String, notes: String, level: f32, order: Order, picker: bool, review: bool }
fn view(app: &App) -> Element<App> {
    zstack![
        vstack![
            text_field(&app.draft).key("draft").placeholder("Draft title")
                .on_change(|app: &mut App, text| app.draft = text)
                .on_submit(|app: &mut App| app.review = true),
            text_area(&app.notes).key("notes").height(120.).font_family("SF Mono")
                .on_change(|app: &mut App, text| app.notes = text),
            slider(app.level).key("level").range(0.0..=100.0).step(5.)
                .on_change(|app: &mut App, value| app.level = value),
            select([
                select_item(Order::Recent, "Recent").key("recent"),
                select_item(Order::Name, "Name").key("name"),
            ]).key("sort").selected(app.order).open(app.picker)
                .on_open_change(|app: &mut App, open| app.picker = open)
                .on_select(|app: &mut App, value| app.order = value),
            button("Review").key("review").on_click(|app: &mut App| app.review = true),
        ].gap(12., Axis::Vertical).padding(20., Edge::All),
        dialog(vstack![
            text(format!("Review: {}", app.draft)),
            button("Done").key("done").on_click(|app: &mut App| app.review = false),
        ].gap(12., Axis::Vertical))
            .key("review-dialog").title("Review draft").open(app.review)
            .on_dismiss(|app: &mut App| app.review = false),
    ].font_family("SF").into()
}
view(&App::default()).build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

```text
Rust state: draft / notes / level / order / open flags
                         |
                         v
view -> controlled fields + slider + select + dialog
                         |
            +------------+-------------+
            |                          |
runtime: caret/selection/IME     portal: topmost input scope
         undo/redo/clipping             focus + dismissal
            |                          |
            +---------- typed callbacks +--> App -> recompose
```

### Exact contracts and limits

- Mutable fields require `on_change`; `.read_only(true)` permits a field without
  it. `.enabled(false)` disables input. Single-line Enter calls `on_submit`;
  multi-line Enter inserts a newline; **Cmd/Ctrl+Enter** calls its submit callback.
  Tab changes focus, not document indentation.
- Stable keys retain caret, selection and history through controlled recomposition.
  An externally replaced value resets undo history. Limits: **8192 UTF-8 bytes per
  editor and 64 undo snapshots**. Oversized inserts are rejected, not byte-truncated.
- Arrows/Shift-arrows, Home/End, select-all and Cmd/Ctrl undo/redo are supported.
  Native clipboard initialization is lazy and retained; a failed copy must not cause
  cut deletion. OS IME preedit is paint-only until commit, with a candidate rectangle.
  There is deliberately **no idle caret-blink repaint timer**.
- `.password(true)` masks painted text and blocks copy/cut and editor diagnostic
  text. **App state remains ordinary plaintext in memory**, not an encrypted vault.
- `text_area` is plain text, not rich text, syntax highlighting, spellchecking or a
  production code editor. Lines follow explicit newlines; wheel scrolling and caret-following use a clipped
  internal viewport. Automatic wrapping and
  fully correct complex bidi visual navigation/selection are not claimed.
- Sliders validate finite values, ascending bounded range, representable positive
  step and in-range initial value. Pointer capture survives controlled recomposition;
  equal values emit no callback. Arrows/Home/End/PageUp/PageDown change value.
- Select requires a stable `.key(...)`, **1–128 unique typed options**, `on_select`
  and `on_open_change`. Up/Down/Home/End reveal offscreen choices; Enter commits.
  Closed/disabled popups unmount their options. The popup flips above its anchor
  near the bottom edge, escapes ancestor clips, and scrolls long fixed-height lists.
- Dialog styles/width target the card, not its full-window backdrop. Place it after
  covered content in a root `zstack`. Escape and backdrop dismissal can be disabled;
  close callbacks must update the controlled open flag. Nested scopes isolate input,
  trap Tab, restore surviving opener focus and cancel stale press captures.
- These are **custom-painted in-window controls**, not OS dialogs, native menus,
  accessibility roles or an automatic focus-reveal implementation for arbitrary
  scroll containers. Keyboard reveal is guaranteed for Select's own option list.
- Schema1 JSON/legacy widgets remain unchanged; these richer builders are Rust-only.
  Optional declarative `Event`/`EventKind` gain a numeric event; downstream exhaustive
  matches using that optional API may require a new arm. This does not change the
  default embedded API or synchronize Volt's separate vendored copy automatically.

Custom hosts should route `pointer_down_outcome` / `pointer_move_outcome` (with
handlers/context), `pointer_up_outcome`, `slider_key(ValueKey)`, `overlay_key`,
`dismiss_overlay`, editor keys/text and IME methods. The legacy bool pointer helpers
cannot dispatch continuous slider changes. The native host supplies this routing.

### Remaining priorities

OS accessibility, broader native IME/device acceptance, Linux/Windows clipboard and
interaction testing, automatic text wrapping/bidi completeness, arbitrary scroll
focus reveal, asynchronous state-update wakeups and rendering performance remain.
Mobile hosting and a desktop/compositor experiment are deferred. The scoped control
batch is delivered; the entire toolkit is **not** declared production-complete.
See [resource usage and GPU boundaries](RESOURCE_USAGE_AND_DESKTOP.md).

## Verification receipt (2026-10-09)

- `native-gpu` all-target suite: **318 tests passed**, one opt-in GPU hardware test
  ignored by default and run separately (one pass). Shared Lab tests occur through
  multiple example entrypoints; this is not a count of unique cases.
- Native doctests: **22 passed**, one pre-existing ignored native-host example.
  Dedicated suites include **17 editable-input**, **19 value-control** and
  **15 modal-dialog** integration tests. Claude's Northbound example and its
  **five tests** are preserved.
- Default and declarative-only tests and strict all-target Clippy passed; default
  direct dependencies still contain only `unicode-width`. GPU all-target strict
  Clippy, formatting, diff whitespace checks and **Rust 1.87** GPU all-target checks
  passed. Package assembly/default-build verification passed. Package metadata
  still warns about missing repository/homepage/docs links.
- On the unlocked Mac, actual native typing/Enter submission, Unicode clipboard
  paste, whole-emoji deletion, undo/redo, multiline editing and internal wheel
  scrolling were inspected. Slider and popup input, modal Tab trapping,
  Escape/opener focus restoration and blocked background scrolling were checked.
  Desk sends actual edited text by click and **Cmd+Enter**; no service is contacted.
- Optional native GPU drawing now uses Vello scenes rather than uploading a CPU
  framebuffer. Actual Apple M4 Pro/Metal readback passed 1x/2x fixtures, clipped
  opacity/path holes, cached resources and shadow symmetry. On the unlocked Mac,
  GPU UI Lab typing/Enter, button/tab callbacks, slider/popup selection, modal
  Escape/focus restoration, clipped images/vectors and circular hit targets were
  visually checked. This is not measured GPU utilization or full OS alpha/blur
  acceptance. See [GPU drawing](GPU_RENDERING.md) and the resource guide.
- Foreign-language OS IME UX, VoiceOver, Linux/Windows native interactions,
  GPU utilization/energy and input-to-photon latency still need separate acceptance.
- No commit/push, memory write, Volt checkout edit or OS desktop setting change.
