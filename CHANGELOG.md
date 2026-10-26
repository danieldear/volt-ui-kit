# Changelog

## Unreleased

- Menus follow one design: every row's label starts on one edge (past a
  glyph column only when some row has a check or an icon), rows are 12.5
  regular, hover and keyboard focus fill with the accent at low strength
  instead of a ring, and the card always casts a shadow.
- Tooltips cast a shadow, so they read as floating over content.
- Add `on_drag(|app, [dx, dy]|)` for containers: `[0, 0]` at the press,
  then the offset from it as the pointer moves, until release. For
  resize grips. Tests: `tests/form_components.rs`.
- The document budget is 16,384 nodes (was 4,096), depth 48 unchanged, so
  long documents and transcripts render whole.
- The code view's scrollbar can be held and dragged, and pressing its track
  brings the thumb under the pointer. The held thumb widens and darkens,
  the pointer shows the arrow over the scrollbar, and no caret press reaches
  the application. Tests: `tests/code_view.rs`.
- Add `text_scale(f)`: multiplies every font size beneath, explicit ones
  included, for an interface text-size setting.
- `combobox` takes `.max_results(n)` (up to 128, scrolling past nine rows),
  `.surface(..)`, `.height(..)` and `.font_size(..)`, so it can replace a
  select you type into.
- Hover, pressed, focus and disabled styles, and shadows, accept tokens and
  `"light|dark"` pairs like static styles instead of failing validation.
- Add `code_view(line_count)`, a controlled, virtualized code surface for
  editors (built for Volt Editor). The application keeps the document and
  supplies only the lines around the visible window (`.lines(first, ..)`)
  with coloured byte spans (`.palette(..)`), plus every selection.
  - **Drawing:** the view draws the gutter and line numbers, the
    current-line band, selections, several carets, input-method
    composition at the caret, and a thin scrollbar. Only visible lines are
    shaped, so per-frame work doesn't grow with the file.
  - **Input:** every key (with its modifiers), typed or committed text,
    paste, press (with click count), drag, Copy and Cut reaches one
    `on_input` handler as a `CodeInput`. Copy and Cut return the selected
    text for the clipboard. Read-only views select, scroll and copy.
  - **Scrolling:** wheel and trackpad scrolling, drag auto-scroll,
    `.reveal(request)` (keep the primary caret on screen) and
    `.scroll_to(line, request)`. `CodeInput::Visible` asks for lines only
    when the view leaves the supplied window or its height changes.
  - **Focus and theming:** `.focus_when(..)` takes focus as it turns on.
    Palette entries may be `"light|dark"` pairs, picked by the theme in
    effect. Presses and scrolling pass through transparent layers above
    the view, such as the row holding a find bar.
  - **Additive:** new files `runtime/code_view.rs`, `ui/code_view.rs` and
    `native/window/code_input.rs`, plus small hooks in focus, scrolling,
    painting, IME and the native key and pointer routing. Existing
    components are unchanged. Tests: `tests/code_view.rs`.
- Add `.caret_band(height)` to `code_view`. The view reports
  `CodeInput::CaretBand(Some([x, width]))` (whole window points) while the
  primary caret is on screen within its bottom `height` points, and `None`
  once it leaves. Reports are sent only when that changes, so a surface
  floating over the bottom of an editor can keep clear of the caret.
  Additive; covered in `tests/code_view.rs`.
- Add `.on_hover(|state, inside|)` to containers. The container learns when
  the pointer enters or leaves its bounds, with descendants counting as
  inside, so a floating panel can stay open while it's in use. Hosts call
  the new `Runtime::dispatch_hover_regions(pointer, ..)` after pointer
  motion; the native window does this on cursor moves and when the cursor
  leaves the window. Additive: new file `runtime/hover.rs`, plus one field on
  elements, nodes and layout boxes. Tests: `tests/hover_regions.rs`.
- Add `.on_focus_within(|state, inside|)` and `.on_escape(|state|)` to
  containers, for non-modal floating surfaces:
  - **Focus within:** the container learns when keyboard focus moves onto
    it or a descendant, and when focus leaves, so it can stay open while
    it's used from the keyboard. The native window reports this after each
    event through `Runtime::dispatch_focus_regions`.
  - **Escape:** an Escape that no editor, menu or dialog used goes to the
    last `on_escape` container in the document, through
    `Runtime::dispatch_escape`.
  - Additive, in `runtime/hover.rs`. Tests: `tests/hover_regions.rs`.
- Add `.decorations(..)` to `code_view`: marked ranges as `CodeDecoration`s,
  each with a `CodeMark` and a colour (a token, hex or `"light|dark"`).
  - **Marks:** `Fill` tints behind the text and under selections (search
    matches); `Underline` (diagnostics) and `Outline` (the current match)
    draw above the text; `Lines` tints whole lines across the view with a
    bar at the gutter's edge (changed lines), and an empty `Lines` range
    marks the boundary where lines were removed.
  - **Bounded:** like lines, decorations are supplied near the visible
    window, at most 10,000 per view; only those touching drawn lines
    are resolved and painted.
  - Additive: one field and builder; selections share the new range
    geometry and look the same. Tests: `tests/code_view.rs`.
- Add `code_view` looks for editors that follow a design closely:
  `.italic_palette(..)` draws chosen palette entries in italic (comments),
  `.indent_guides(unit)` draws a guide at each indentation level (a blank
  line takes the depth of the code after it), and `.scroll_marks(..)`
  marks lines along the scrollbar's track (search matches, changes).
  The overlay scrollbar is now a 7pt rounded thumb in a track inset 6pt.
  Additive apart from that look. Tests: `tests/code_view.rs`.
- Element style colours (text, background, border, accent) may be
  `"light|dark"` pairs of hex values or tokens, as `code_view` palettes
  already could. `UiTheme::resolve` picks the half for the theme in
  effect (light text means a dark theme), so System appearance needs no
  recomposition. Additive: one shared check in style and canvas validation.
  Tests: `tests/theme_pairs.rs`.
- Add finite damped-spring timing, configurable `Spring`, additive stagger delays
  and stable-key `transition_revision` replay, without adding Easing variants or
  dependencies. Add polished, action-driven Catalog motion workflows and guides.
- Fix delay-aware animation scheduling, early pruning of overshooting curves,
  bounded opacity/color interpolation, reverse offset animation to zero, and
  paint-only sliding exits. Resolve themed colors consistently across offset
  and final paint passes to prevent continuous animation restarts, and present
  completed endpoints before sleeping through a stagger gap.
  Reduced motion settles everything immediately;
  the macOS native host also samples the system preference at startup.

- Add optional typed `NavigationHistory`, validated serializable checkpoints,
  `navigation_link` and `navigation_stack`. Existing navigation controls and
  public accessibility enums/structs retain their shapes and behavior.
- Add standalone themed `link` controls with keyboard/accessibility activation,
  conservative HTTP(S) targets and explicit public-API macOS browser dispatch
  (`native-desktop`); unsupported builds report capability/errors. No new
  dependencies or automatic browser/network activity.
- Add interactive navigation/link Catalog workflows, responsive preview coverage,
  lifecycle/security regression tests and matching component/service guides.

- Add opt-in content padding/gap to forms and disclosure composites, dialogs,
  popovers and tooltips. Defaults remain unchanged. Add OTP `digit_gap` and
  combobox `option_gap` where generic container gaps would not affect the
  intended inner layout.
- Add `Tabs::tab_gap` and `tab_padding`, independent of strip-to-page spacing;
  preserve the current 2px tab-bar default and explicit joined-bar gaps.
- Make menu popup padding, item gap/height and height cap configurable; retain
  the current 6/2/34/480 defaults and `surface`, with `popup_background` alias.
- Add table row/header height and horizontal cell padding, tree row height and
  indentation, and fixed panel padding/gap without changing existing defaults.
  Regression coverage includes explicit zero, responsive overrides, both DPI
  scales, input callbacks and content-card versus overlay spacing.

- Add `canvas_frames(frames, frame_ms, revision)`: a canvas tween computed
  while composing and played at paint time (no recomposition or layout per
  frame). It restarts only when `revision` changes for that element and
  shows the last frame under reduced motion.
- `sunburst_chart(..).animate_from(previous, revision)`: arcs tween to new
  data (d3's cubic in-out over ~360 ms; new arcs grow from their start) and
  labels cross-fade. A different root draws without a tween.
- Menus and context menus take `.surface(color)` for the popup card (default
  `Base200`); a caller-set surface also floats on a soft shadow, for apps whose
  panels are translucent. The popup's height cap rises from 360 to 480 so a
  dozen-row context menu isn't cut off.
- Add `Application::appearance_from_state(..)`: an in-app Light / Dark /
  System choice. The window (and so its native material) and the mode passed
  to `theme_with` follow it after each update; `System` follows the OS again.
  Not covered by headless tests (it drives the native window).
- A keyed segmented control or tab bar gives its options stable ids
  (`{key}-{index}`, unless an item has its own key), so their hover, press
  and fade state can't move to another element when siblings before the
  control come and go.
- Add `sunburst_chart`, a zoomable sunburst after d3's (rings around a root,
  a rainbow hue per top-level branch, a centre that can go back up), with
  `sunburst_layout` and d3's `rainbow` palette. Arcs are sector-clipped
  buttons sharing the hierarchy charts' ids, labels, selection, review
  markers, dashed context targets, hover links, menus and keyboard roving.
- Add `.on_height(...)`, the height counterpart of `.on_width(...)`, reported
  through the same `dispatch_widths` pass, and `selectable_list(..)
  .flexible_height()` so a list can take a column's remaining height and
  scroll inside it.
- Joined segmented controls and tab bars keep a caller-set `.gap(...)`
  instead of always forcing the 1px seam (still the default).
- Keyboard-opened context menus (Shift-F10 / Context Menu) fall back to the
  focused item's nearest menu owner when the item's box centre lies outside
  a clipped, non-rectangular target such as a ring segment.

- Add opt-in paint-only `.hover_link(key)` peers and hierarchy
  `.hover_linked(true)`: matching mounted components share hover styling without
  callbacks, recomposition, selection, focus, scrolling or peer tooltips. Legacy
  chart behavior and public input event variants remain unchanged.

- Add opt-in hierarchy multi-selection, passive review markers, custom node
  palettes, hidden icicle root bands, distinct context targets and per-node
  context callbacks. Keyboard navigation uses one roving Tab stop, arrow keys,
  and native Shift-F10 / Context Menu requests. Existing chart defaults, public
  input event variants and the feature-free Volt core remain unchanged.

- Add opt-in macOS `native::file_dialog::move_to_trash` for one caller-confirmed
  item, with unsupported-platform reporting and no permanent-delete fallback.
  Confirmation, path protection and revalidation remain application policy.

- Add Rust-native `icicle_chart` and squarified `treemap_chart`, inclusive
  hierarchy weights, bounded layout helpers, accessible tile labels, tooltips
  and typed mouse/keyboard selection. Demonstrate both on the Catalog Charts page.
- Add macOS `native::file_dialog::reveal_file` for a toolkit-owned Finder reveal
  request without opening or executing file contents.
- Add opt-in `Application::window_visibility_from_state` for tray-resident
  applications; hide/show keeps existing workers alive and restores focus only
  on a visibility transition. Existing window-close behavior is unchanged.

## 0.5.0 (2026-10-17; not published to crates.io)

- Add `./scripts/run_catalog.sh`: one-command, fully featured GPU Catalog,
  automatically bundled and ad-hoc signed on macOS. `--check` verifies without
  launching; notification permission and sending remain explicit UI actions.
- Fix context menus blocked by passive toast layers, fixed-sidebar detail
  overflow, generated navigation key collisions, and Zinc info/success
  foreground contrast. Add regressions for scale, placement and modal guards.
- Carry forward the redesigned documentation landing page, add visible
  high-contrast accent labels and a pause-motion control, and refresh the
  native component previews and application-service instructions.

- Add `Palette::zinc_light()` and `Palette::zinc_dark()`, the Catalog's
  neutral Zinc palette, as kit presets; `.with_accent(..)` gives them a
  coloured primary. The Catalog's themes are built on them (renders
  unchanged), and every `cargo volt new` template now generates an app in
  Zinc that follows the system's light or dark mode. The kit's default
  palette is unchanged.

- Add `navigation_split_view`, the Catalog's app shell as a component (like
  SwiftUI's `NavigationSplitView`): a sidebar beside a detail column with a
  draggable toolbar, room for the macOS window controls, an optional resizable
  divider, a drawer behind a menu button below `Lg`, and `.material(true)` for
  a frosted sidebar with a floating sheet. The Catalog's shell is built on it
  (renders unchanged), and the `navigation`, `mail`, `settings` and `agent`
  templates now generate apps on it, with a sidebar that resizes like the
  Catalog's.

- Draw `tab_bar` and `tabs` as line tabs: muted text over a hairline, with the
  selected tab underlined in the primary colour, instead of filled buttons.
  Add `TabVariant::Contained` for a raised chip in a quiet track. Segmented
  controls keep their joined look. Buttons now honour `border_sides` and
  dashed/dotted borders.

- Draw `navigation` with the Catalog's sidebar rows in every placement. Add
  `navigation_item(..).section(..)` headings and `.badge(..)`, and let
  `.selected(None)` highlight nothing. The Catalog's sidebar and page drawer
  are now built on `navigation`.

- Include every glyph sharing a cluster endpoint when placing the final caret,
  fixing multi-glyph emoji fallback on Windows while preserving indexed lookup,
  unsorted/repeated offsets and first visual-direction priority.

- Make Catalog scene-budget regressions adapter-independent using the same
  encoder as GPU drawing. Keep actual editor-page readback as an explicit GPU
  acceptance test; headless CI no longer requires graphics drivers for budget
  assertions. Expose CPU-only scene estimates under `native-gpu`.

- Keep plain and rich text in the requested font family when an exact weight
  is unavailable, rather than falling back to an unrelated face. Cover sparse
  font inventories, inherited run weights and macOS without optional SF font
  installs. Include Catalog test metadata in Cargo source packages.

- Expand the landing page into a static documentation portal: 50 Catalog pages,
  115 toolkit-rendered example previews, Rust usage excerpts/API notes and eight
  HTML guides. Add an explicit offline Catalog exporter, repeatable site
  generation and manifest/coverage/link regression tests.

- Redesign the documentation landing page with self-hosted OFL display
  typography, a native Catalog composition and keyboard-accessible CSS-only
  palette, corner and shadow studies. Keep the site script-free and update
  static-asset packaging and license notices for the local font.

- Add a reusable, preview-first Git identity correction tool. It rewrites only
  selected branch commit metadata, reuses all trees/blobs and keeps zero-copy
  backup refs. Local rewrite, lease-protected push and local restore are
  separate explicit operations; no clone, stash, GC or global identity change
  is performed. Include regression tests and a maintainer guide.

- Prepare for public collaboration: static documentation landing page,
  repository hygiene/link checks, pinned security workflows, Dependabot,
  contribution templates and a reproducible main-protection policy. Document
  publication gates without changing repository visibility or claiming remote
  CI/runtime acceptance.
- Make `cargo volt dev` state handoffs fresh, atomically created directories
  (owner-only on Unix), refuse pre-existing paths/symlinks, and remove the
  directory on normal exit after stopping the child. No public API or runtime
  dependency changes.


- Harden four reviewed paths without changing public APIs or dependencies:
  metadata-driven bundles reject path-component names and validate inputs
  before replacing a direct output; cleanup cannot traverse parent directories
  or intermediate symlinks. Read-only selection/copy respects the active modal
  and is cleared on scope changes. Lazy lists validate counts/estimates before
  allocation or row construction and bound estimate iterators. Long editor
  lines use bounded wrapping work and indexed native caret advances, retaining
  shaped cluster, Unicode, alignment and password behavior. Add regression
  tests, including the 4 MiB editor boundary.

- `cargo volt bundle` reads the app from `[package.metadata.volt.bundle]`
  in Cargo.toml and builds it (`--universal` for Apple silicon and Intel).
  Bundles can declare document types (by UTI or extension), imported UTIs,
  a category and a copyright. New `cargo volt release`: Developer ID
  signing with the hardened runtime, notarization and stapling, and a
  signed, notarized disk image; `--dry-run` prints each step.
- Bundles can give the executable other names (`aliases = ["md"]`, or
  `--alias md`): links in `Contents/MacOS` that ship a command-line tool
  inside the app, signed and notarized with it.
- `cargo volt bundle` and `release` strip the bundled executable's symbol
  table, about a fifth of a Rust release binary (`strip = false` or
  `--strip none` keeps it). The packaging guide shows a size-tuned release
  profile.
- Shortcuts may use unshifted punctuation keys as well as letters and
  digits (`,` `.` `;` `'` `/` `[` `]` `\` `-` `=` `` ` ``), so apps can bind
  ⌘, for Settings, ⌘=/⌘− for zoom and ⌘[/⌘] for history.
- Native menus: `DesktopCommand::separator()` divides a menu into groups,
  and `DesktopConfig::app_menu([..])` adds commands (such as Settings…)
  to the application menu below About.
- Text fields: `.autofocus(true)` takes keyboard focus when the field first
  appears (a find bar opening, say) without stealing it back on later
  rebuilds, and `.on_cancel(f)` runs when Escape is pressed in the field.
  Single-line fields no longer clip descenders of typed or placeholder text.
- `native::font_families()` lists installed font families for font
  pickers; `file_dialog::open_folder` chooses a folder and
  `file_dialog::open_file_or_folder` either.
- Shortcuts match the key pressed without modifiers, so Shift+Cmd+] is
  `primary(']').shift(true)` and Option shortcuts work on every layout.
- Tooltips fit their label instead of always being 240 px wide (long
  labels still wrap at 240) and centre on their target; menus and
  popovers keep aligning with the target's leading edge.
- `cargo volt bundle` and `release` embed app extensions declared under
  `[[package.metadata.volt.bundle.extensions]]` (a Quick Look preview, for
  example): another binary of the package in `Contents/PlugIns/NAME.appex`
  with its own `Info.plist`, stripped, and signed in the app sandbox before
  the app, ad hoc or with Developer ID.
- Fixed: emoji in bold (or other non-regular) text could show as a
  black-and-white outline. Font fallback prefers any installed font with a
  matching weight that has the code point, such as a bold Nerd Font, over
  the regular-only colour emoji font. Emoji graphemes now always use the
  platform's colour emoji font (Apple Color Emoji, Segoe UI Emoji or Noto
  Color Emoji) when it is installed.
- Fixed: after a double-click selected a word in selectable text, the
  slightest pointer movement before release shrank the selection to the
  pointer. Double- and triple-click drags now extend by whole words or
  paragraphs from the first one, in either direction, as macOS does.
- Containers take `.on_width(|state, width| ..)`: called after layout with
  the container's width in logical points whenever it changes, so content
  can adapt to the room it really has (a drawing scaled to fit, tighter
  margins in a narrow pane). Hosts recompose once after a report.
- Fixed: switching a window between opaque and translucent (for example
  with `background_from_state`) left the previous frame showing through
  translucent areas. Each switch made a new GPU surface, and on macOS every
  surface adds a Metal layer to the window that stays behind, frozen. The
  surface is now reconfigured in place, and a lost device reuses it.
- Drop zones treat `.height(..)` as a minimum and grow to fit their icon,
  title, hint and button instead of squeezing them together.
- Icons: `Sidebar`, `FileText`, `Type` and `Columns`.
- Text areas hold whole documents: multiline editors take up to 4 MiB
  (single-line fields stay at 8 KiB), reflow only the lines that changed,
  and open with the caret at the top. `native::files` reads and saves up
  to the same limit.
- Selectable text: `.selectable(true)` on a container lets its text be
  selected with the pointer across paragraphs (double-click a word,
  triple-click a paragraph, Shift-click to extend), with Cmd/Ctrl+A to
  select all and Cmd/Ctrl+C to copy; `.selectable(false)` opts a subtree
  out. `.on_select_text(|app, text| ..)` reports the selection. Selections
  are anchored to keyed text boxes. New `TextMetrics::text_offset_at` hit
  test; hosts call `Runtime::dispatch_text_selection` after input.
  `LayoutBox::text()` exposes a box's text.
- `Application::before_launch(|app| ..)` runs once the native event loop
  and application object exist, before the run loop starts, for platform
  integration that launch events need (receiving Finder's open requests).
- Editor hooks for writing tools: `.on_selection(|app, sel| ..)` reports
  the selection with the caret's and the editor's rectangles after layout
  (hosts call `Runtime::dispatch_editor_selections`); `.on_keys([..], f)`
  hands chosen keys (arrows, Enter) to the app instead of editing;
  `.select(range, request)` places the selection; `.tab_indents(true)`
  makes Tab insert four spaces. Selections also report the first visible
  line (`top`), `.scroll_to_offset(byte, request)` scrolls an editor, and
  `.highlights(palette, ranges)` colours byte ranges (syntax highlighting)
  without changing layout.
- `lazy_list(count, |row| ..)`: a scrolling column of variable-height rows
  that builds only the rows near the viewport, so documents and feeds of
  any length stay within the element budget. Rows are measured as they
  mount (estimates stand in for the rest), the visible row stays put when
  rows above it are measured, and `.scroll_to_row(row, request)` jumps to
  rows that are not built yet. Windows are reported through
  `on_window_change`, as for `virtual_list`.
- GPU text on macOS now uses font-smoothing-style stem darkening (outline
  dilation matching CoreText) so it no longer reads thin and light next to
  native apps, and is hinted on displays below 1.5x.
  `VOLT_UI_TEXT_DARKENING=0` turns the darkening off.
- `native::clipboard::{set_text, text}` for plain-text copy and paste.

- Rich text: `rich_text([span(..), ..])` lays out one paragraph of styled
  runs (weight, italic, font family, colour, rounded background, underline,
  strikethrough; `.code()` for inline code). It wraps, aligns and truncates
  as a single text, in both renderers. Font changes go through cosmic-text
  rich shaping; colours are applied per glyph when drawing, so colour-only
  runs share the plain text's shaped layout. New painter hook
  `DocumentPainter::ui_text_range_rects` returns per-line rectangles for a
  byte range (backgrounds and decorations use it). Catalog has a Rich text
  page.
- Rich text links and sizes: `span(..).link(target)` with
  `Text::on_link(|app, target| ..)` (pointer cursor, hover underline, click
  reported through the new `Event::Link`), and `span(..).size(scale)` for
  relative font size on the paragraph's line spacing; `.code()` now uses
  0.88×. The range-rectangle hook moved to `TextMetrics::text_range_rects`
  so layout can compute link targets.

- GPU stack upgraded from Vello 0.6 / wgpu 26 to **Vello 0.11 / wgpu 30**
  (guillotiere 0.7, matching Vello's atlas allocator). The minimum Rust
  version rises from 1.87 to **1.89**, which Vello 0.11 requires; generated
  `cargo volt new` projects follow. Rendering is unchanged on 51 of 53
  Catalog views (within 1/255). Bitmap images drawn with
  `ImageQuality::High` now get true bicubic sampling on the GPU (Vello 0.6
  fell back to bilinear), which brings them 2–3× closer to the software
  renderer. Vello's fixed buffer capacities are unchanged, so frame
  admission and strip splitting are kept as they were. The Metal
  premultiplied-alpha workaround is still required on wgpu 30.
- Fixed: charts with many data points could exceed the canvas limit of 256
  paths, and the view was then rejected. Selecting a candle in the Catalog
  Dashboard's 6M range did nothing for this reason. Candlestick, volume,
  stacked-bar and bar charts now batch their shapes by colour into a few
  paths, so their path count no longer grows with the data (the output is
  pixel-identical). `line_chart` draws point markers only while they don't
  overlap. The limits are public as `graphics::MAX_CANVAS_PATHS` and
  `graphics::MAX_PATH_COMMANDS`.

- Documentation rewritten for a public release. The 21 Markdown guides,
  which had grown into session logs, are now eight user guides under
  `docs/`, plus a new README, `CONTRIBUTING.md` and `SECURITY.md` and a
  shorter roadmap. The guides are also the `guide` module (with `native`),
  so all 31 of their Rust examples are compiled as doctests. The crate root
  docs no longer include the README.
- Removed the `native_window`, `panels_and_layout`, `buttons_and_events`,
  `declarative_ui` and `render_benchmark` examples; `complete_app`,
  `components` and the Catalog's `--bench` cover the same ground.

- Interactive charts. `area_chart`, `candlestick_chart`, `donut_chart` and
  `stacked_bar_chart` take `.key(..)`: every data point gets an invisible,
  focusable hit target with a value tooltip (hover or keyboard focus; no
  recomposition) and a spoken label. `.selected(..)` / `.on_select(..)` add
  click and Enter selection, drawn as a highlight (candle band, outline and
  close level; area rule and dots; popped-out donut slice; emphasised bar
  column). Donut targets are clipped to their ring sectors and legend rows
  select slices. The chart builders are now generic over the app state type.
  The Catalog Dashboard reacts to selections and the Charts page has an
  interactive example.

- State-preserving dev restarts: `Application::dev_state(save, load)`.
  Under `cargo volt dev`, a successful rebuild asks the running app to
  save its state and window frame and exit; the new build restores both
  before its first view (apps without `dev_state` restart as before).
  `Application::theme_file(path)` reloads a theme JSON live when the file
  changes. The Catalog keeps its page, theme, accent, sidebar width and
  window across dev restarts.
- crates.io readiness: repository, keywords, categories and docs.rs
  configuration; `cargo publish --dry-run` passes (752 KiB package).

- Faster interactions on large views. Layout reuses measurements of
  subtrees that are unchanged since the previous recomposition (same id
  and position, equal node fields, equal resolved styles for the whole
  subtree, same bindings and viewport width); a test checks that reused
  layouts equal fresh layouts on every Catalog page, across state changes
  and a resize. Internal caches use a fast non-cryptographic hasher and
  key styles by node identity instead of hashing id strings. Per click at
  2× (rebuild + install + layout + paint): Catalog landing 4.75 → 2.56 ms,
  Dashboard 2.84 → 1.67 ms; layout alone 3.05 → 0.88 ms.
  `Runtime::reused_measurements()` reports reuse for diagnostics.
- Fix building with only the `declarative` feature (two `rust-ui` fields
  were initialised unconditionally). The core (`rust-ui`) also builds for
  `wasm32-unknown-unknown`; CI now checks each feature alone and wasm32.

- Data charts with axes (`charts` module): `area_chart` (overlaid or
  stacked), `candlestick_chart` (OHLC with optional volume strip),
  `donut_chart` (centre label, legend, percentages) and `stacked_bar_chart`.
  Shared "nice" ticks (`nice_ticks`), compact labels (`short_number`), Y tick
  labels on gridlines, length-aware X-label thinning, and theme-following
  series colours (series 1 is `primary`). Invalid data is dropped quietly.
- Catalog Dashboard pattern page: a dense analytics screen (KPIs with
  sparklines, candlesticks with volume and range switch, traffic donut,
  stacked revenue area, regional stacked bars, sortable orders table) built
  only from Volt components; stays under 9% of the GPU segment budget.

- Fix GPU frames falling back to software on pages with multi-line editors
  (Catalog Theme editor: 109% of Vello's segment budget). Editor rows are
  shaped unbounded, and an edge-touching text run was clipped to that
  1,000,000 px box; Vello sizes clip layers by area. Text clip layers are now
  clamped to the frame (output unchanged). Editor-heavy pages drop to under
  10% of the budget; the heaviest Catalog page is 12%.
  `VOLT_UI_TRACE` frames now report `lines_est` / `segments_est`, and
  `GpuOffscreenRenderer::last_path_estimate()` exposes them.

- Themes are fully user-defined: `ColorToken::ALL`, `Palette::set(role,
  colour)`, `Palette::to_json()` / `Palette::from_json(text, &base)` (partial
  files override only the listed roles; dependency-free parser),
  `contrast_ratio`, `hex` and `parse_hex`, and `ColorToken::content_pair()`.
  The Catalog gains a Theme editor page: every role for light and dark, a
  colour picker, WCAG contrast grading, a component preview and JSON
  export/import; preset accents clear a custom theme.
- Custom SVG icons: `SvgIcon::parse` / `SvgIcon::load` read real SVG files
  (`path`, `circle`, `ellipse`, `rect` with radii, `line`, `polyline`,
  `polygon`, nested `g` with inherited fill/stroke/stroke-width/`style` and
  `translate`/`scale`/`matrix` transforms; `viewBox` fitting). `svg_icon(&svg)`
  takes the same `.size` / `.color` / `.stroke` options as built-in icons, plus
  `.keep_colors(true)` for multi-colour art. `IconSet::from_dir` loads a whole
  icon-set folder by file name. The Catalog Icons page accepts dropped or
  pasted SVGs.

## 0.4.0 (2026-10-12; not published to crates.io)

- Minor-version milestone for the expanded Rust component set, application
  foundation, native desktop integrations and Catalog. Existing default
  dependencies and opt-in native/GPU feature boundaries are unchanged.
  Private-compositor corner switching, generic in-app GPU backdrop blur,
  full composite keyboard/accessibility acceptance, multi-window lifecycle
  and signed distribution remain separate gates; see `docs/VALIDATION.md`.

- Harden the component-parity delivery after review: validate inactive
  responsive rules and disabled offsets, bound date/time and pagination
  arithmetic, keep number-input callbacks finite, revalidate file-drop scope
  and enablement, and retain private file permissions on linkless volumes.
  Align editable ink, caret/selection and IME geometry consistently; form
  labels name controls unless an explicit accessible name was supplied.
  Registered fonts now share immutable bytes across windows and have a
  256 MiB aggregate limit. Add review regressions and an acceptance receipt.

- `select` is styled as a field, matching text fields and `combobox`: a
  light filled trigger with a thin outline and an inner chevron (hover
  darkens the outline), and a card popup with quiet rows and a check mark on
  the chosen option instead of a primary fill. Behaviour, keys and
  accessibility are unchanged; `.background(..)` / `.border_color(..)` still
  override. The catalog Select page compares select and combobox.
- Catalog chrome uses the new components: `command_palette` for ⌘K search
  (grouped pages, fuzzy keywords), a `kbd` hint on the search button, an
  icon filter field with a clear action, a clickable `breadcrumb`, `toaster`
  confirmations, checked accent menu items, `form_field` sign-in with a
  password reveal, and a `command_palette` for application commands.

- Pixel-sized split panes: `split(a, b).first_size(px).max_first(px)` keeps
  the first pane at a fixed width as the window resizes and reports pixels
  from `on_resize`; `.handle_color(..)` styles the divider. A responsively
  hidden first pane collapses the split. `.fill_height()` joins
  `.fill_width()`. `.handle_style(SplitHandle::Reveal)` hides the divider
  until hover, drag or keyboard focus, then shows a rounded pill; the resize
  cursor holds for the whole drag. Divider drags patch the installed
  document and lay out once per frame: `on_resize` still fires on every move,
  but moves return `changed` (layout) rather than `activated` (recompose), and
  the host recomposes once on release. Layout is cheaper when widths change:
  text that fits unwrapped reuses its natural-width measurement (and draws
  from it when start-aligned), and the measure memo, style map and shaped-text
  cache are hashed rather than ordered by id/text strings (catalog landing:
  24.7 → 5.9 ms per drag frame at 2×; other pages 1–1.6 ms). The catalog sidebar is now resizable (200–420 px), and
  collapses into a drawer below 1024 px; the landing hero has compact,
  medium and full sizes; doc pages stop at a 1040 px column.

- Tailwind / shadcn parity (see `docs/COMPONENTS.md`). All new APIs are
  Rust-only; the schema-1 JSON format is unchanged.
  - Layout and style utilities: `grid(..).columns(n)` / `.min_column_width`
    with `.column_span`, `.wrap(true)`, `.justify(Justify::…)`, min/max
    sizes, `.width_fraction`, `.aspect_ratio`, responsive `.visible` /
    width / font size, `.text_align`, `.line_height`, `.max_lines` /
    `.truncate`, `.uppercase`, `.underline`, `.strikethrough`, single-side
    and dashed/dotted borders, linear/radial gradients, `.offset`,
    `.cursor(Cursor::…)`, `.scroll_to`, design `tokens`, `Ambient::pulse`,
    slide transitions, `Easing::CubicBezier`, animation delays, disabled and
    focus-visible styles, `native::register_font`, `VectorPath::from_svg`.
  - Icons: `icon(Icon::…)`, 65 original theme-aware glyphs (`ALL_ICONS`).
  - Date and time: `CalendarDate`, `calendar`, `date_picker`,
    `date_range_picker`, `time_picker`.
  - Form inputs: `radio_group`, `number_input`, `combobox`, `tag_input`,
    `range_slider`, `color_picker` (`Hsl`), `otp_input`, `rating`,
    `form_field`, text-field adornments (`.leading_icon`,
    `.trailing_icon`, `.trailing_action`) and `drop_zone` with native file
    drag and drop.
  - Overlays and disclosure: `collapsible`, `accordion`, `drawer`,
    `toaster` (`Toast`), `command_palette`, `hover_card`; menu rows gain
    `.checked`, `.shortcut`, `.icon`, and `menu_submenu` drill-downs.
  - Display: `avatar` / `avatar_group`, `alert` (`Severity`), `skeleton`,
    `kbd`, `breadcrumb`, `pagination`, `steps`, `timeline`, `empty_state`,
    `stat`, `sparkline`, `bar_chart`, `line_chart`, `carousel`, `image_view`.
  - Runtime: two-thumb sliders, `Event::FilesDropped` /
    `Handlers::on_files_dropped` with `Runtime::file_hover` / `drop_files`,
    `Popover::keep_anchor_interactive`, `Button::label_inset`, and keyboard
    menu navigation over decorated (icon/shortcut) rows.
  - Catalog: Utilities, Icons, Date & time, Form inputs, Overlays, Data
    display and Charts pages.
  - Upgrade notes: `Event` and `EventKind` gain a `FilesDropped` variant
    (exhaustive matches need an arm). The prelude now exports `avatar`,
    `stat`, `steps` and other component names; a local function with the same
    name imported by glob becomes ambiguous — import it explicitly.

- Animation frames are paced by the display. GPU windows (FIFO presentation)
  request the next frame after each present and let vsync set the rate —
  120 Hz on ProMotion, measured 601 frames in 5 s — instead of a fixed
  16.7 ms timer re-armed after each frame, which drifted to ~48 fps and beat
  against vsync (judder). Software windows tick at the monitor's refresh
  interval on a drift-free cadence. Refresh is rechecked each second while
  animating (windows can move between displays).

- `Ambient` paint-time motion: `.ambient(Ambient::float(x, y))` drifts a
  subtree on a slow ellipse and `.ambient(Ambient::spin())` rotates a canvas,
  with `period_ms` and `phase`. Applied at paint only: no recomposition,
  layout or hit-target movement; frames continue only while an ambient node
  is visible, and reduced motion stops them.
- `.letter_spacing(em)` tracking for text (Rust-only style; schema 1 unchanged).
- Catalog landing hero: typography-led on the page's own background (light or
  dark): a custom vector VOLT wordmark, a sans + italic-serif headline, square
  letter-spaced mono buttons, two hairline orbits with one slow accent arc,
  and three small floating cards built from real components. Letter-spaced
  mono navigation with an inverted active chip; a swaying capability ticker.
- Ambient drift is clipped like a paint-only transform (by clipping ancestors,
  not the parent's padding box), so drifting content never loses an edge.

- Font fallback lists: `.font_family("New York Extra Large, Charter, serif")`
  picks the first installed family, like CSS; aliases (`SF`, `SF Mono`) and
  generic names in a list always resolve. Installed families are indexed once.
- Catalog typography: New York display serif with italic accents for the
  landing hero, section headings, numerals and page titles; numbered SF Mono
  eyebrows; JetBrains Mono for code. Each has fallbacks for other machines.

- Smooth scrolling: a scroll moves boxes without resizing them, so the
  runtime now shifts the scrolled subtree in place and recomputes its clips,
  instead of a full layout per wheel event (catalog landing at 2x: 4.3 ms →
  0.19 ms per scroll frame). Open overlays and virtual lists still take the
  full layout. Resolved styles are also cached across layouts until the
  document or viewport width changes. Tests prove the shifted result equals a
  full layout, including nested scrollers and real catalog pages.
- Catalog landing: hero and product shot no longer use rounded clip layers,
  which nested five deep and spilled Vello's blend stack on most scroll
  positions (up to 3 GPU strips per frame); now no spill.
- Shared Rust example harness: `--bench-scroll PX` times scroll frames.

- Shaped-text cache raised from 256 to 2048 layouts. A page whose labels
  (measured at several widths) exceeded 256 made the LRU miss on every
  lookup, reshaping all text every frame: the catalog landing page took
  ~38 ms of layout per click and 89k misses in 87 frames; now ~4.8 ms.

- GPU strip rendering: frames too large for Vello 0.6's fixed per-pass
  buffers (5K/6K/8K Retina windows exceed the tile-command buffer on its
  per-tile reservation alone) render in up to 16 horizontal strips on the GPU
  instead of every frame falling back to software (6K: ~280 ms software →
  ~27 ms GPU including readback). Budgets are tracked per tile row; frames
  whose strips still overflow, or that exceed scene-wide buffers, keep the
  explicit software fallback. `VOLT_UI_TRACE` reports `bands=`.
- New-file saves fall back to an exclusive create where hard links are
  unsupported (exFAT, many network shares); never overwrites either way.
- `cargo volt bundle` names the executable after the app (ASCII-safe,
  `VoltApp` fallback) instead of always `VoltApp`.
- Catalog landing page redesigned as an editorial tour: aurora hero, a live
  miniature app, numbers, features, code with a live counter, live theming,
  the component index and a closing call to action.

- Keep Retina opacity-group overflow fallback bounded with tiled replay; preserve alpha instead of failing again in software. Reuse a hidden native material underlay on blur-backend switches, refresh native frame/shadow, and prepare backdrops before window visibility. Desktop-level Material-to-Compositor corner validation remains pending.

- Separate Catalog background opacity from accent tint; add live blur toggle and
  public-material/explicit compositor selection. Add state-derived native
  backgrounds and root-only theme color overrides. Raw compositor blur uses a
  private macOS API, remains opt-in and checks Reduce Transparency on redraw.
  No dependency or legacy default changes; in-app GPU backdrop blur is pending.

- Fix native Metal window presentation with pinned wgpu 26: compensate for its
  misadvertised straight-alpha mode, restoring translucent sidebar fills,
  badge/selection backgrounds and antialiased text/borders. No tint, material,
  font-family, public default or dependency changes. Add backend-policy and
  real-GPU presentation-pixel regressions, SF face/weight checks and opt-in
  surface/font diagnostics; see `docs/GPU_RENDERING.md`.

- Generated apps now use Catalog-style unified macOS chrome instead of a separate
  native title strip. All five templates retain native traffic lights, include an
  editable 60pt drag header with safe button clearance, SF and semantic colors.
  Other platforms keep native chrome. Existing application defaults/projects,
  blur policy and dependencies are unchanged.

- Add independent optional `native-notifications` with feature-pruned safe
  macOS UserNotifications bindings: explicit alert-only permission, bounded silent
  local requests, one-shot delay, stable IDs and removal. Main-thread/.app checks
  and accepted-not-displayed semantics are explicit; no automatic permission.
  Settings/action/foreground delegates and other OS adapters remain unsupported.
- Catalog Desktop services adds notification/status/error witnesses and guarded
  shared tray commands. Native permission, actual display (human screenshot),
  counted tray delivery (human click + CUA witness) and delivered removal (human
  disappearance confirmation) accepted on the test Mac; pending-only cancellation
  and native tray enablement/modal acceptance remain separate.
  Add a default-off 60-second cancellation-test option; retain captured-delay
  receipts and busy/modal regression coverage without new host API or crates.
  One new pinned optional framework crate; Rust 1.87/default/Volt contracts
  retained. See `docs/DESKTOP_NOTIFICATIONS.md`.

- Extend optional `cargo volt` tooling with macOS `.app` packaging for prebuilt
  executables: explicit metadata, bounded flat resources/ICNS/privacy descriptions,
  no overwrite, Mach-O/system-library/deployment checks and local ad-hoc signing.
  No new crates; Developer ID/notarization, installers and nested code are not
  supplied or claimed. See `docs/MACOS_PACKAGING.md`.

- Add optional public macOS Open/Save panels and explicit unsupported/main-thread
  errors, using existing native-desktop AppKit features (no new registry crate).
- Add bounded UTF-8 file helpers: private sibling saves, no-clobber new targets,
  explicit replacement and ordinary external-edit checks. This is not a file
  sandbox, cross-process transaction or full power-loss durability guarantee.
- Catalog Documents adds real async Open/Save/Save As, failed/cancelled-operation
  draft retention and save-before-close. Current lightweight editor limit is
  8 KiB; no silent truncation. The memory-only lifecycle demo stays separate.
- Catalog adds a counted command witness; host repeat events are tested as inert.
  Discrete native shortcut/menu/button executions and modal suppression were
  accepted; real held-key AppKit repeat/IME/VoiceOver remain separate gates.

- Add bounded state-aware `CommandRegistry`, logical Cmd/Ctrl shortcuts and
  metadata for developer-owned palettes/buttons; unknown/disabled IDs are inert.
- Add `Application::on_close_request` and a separate bounded application-request
  queue. Dirty documents can remain open for Save/Discard/Cancel; confirmed
  requests recheck the guard. No forced OS/Dock quit protection or persistence
  is claimed; the standalone host is still single-window.
- Owned macOS menus/tray reflect registered command enablement; native logical
  accelerators and guarded Quit/Cmd+Q use the same dispatcher/close policy.
- Catalog adds an Application lifecycle page and in-memory draft/command palette.
  UI Lab/older Showcase and Volt's vendored copy are unchanged.
- See `docs/APPLICATION_COMMANDS.md` for APIs, limits and platform boundaries.

## 0.3.0 (2026-10-11; not published to crates.io)

- Application foundation: opt-in `Application`, independent sidebar/rail/bottom
  navigation, local project templates and rebuild/restart tooling, plus optional
  macOS application/tray menus. The full lifecycle and desktop-service roadmap
  remains incomplete; see `docs/APPLICATION_DEVELOPMENT.md`.
- macOS GPU blur now requests a public `NSVisualEffectView` behind the winit
  content view, without changing native controls, the content view parent or
  delegates. Decorated windows on macOS 10.14+ only; unavailable/borderless
  requests explicitly retain transparency without blur. AppKit owns appearance
  and material accessibility behavior. No App Store certification is claimed.
- Correct Vello 0.6 GPU admission: reserve **64**, not 2, command words per
  viewport tile and include 256-word spill allocation chunks. Oversized scenes
  fall back before GPU submission. Native diagnostics now report framebuffer
  uploads for software fallback instead of incorrectly reporting zero.
- Surface retries use bounded event-loop backoff (16–256ms), not immediate
  redraw loops. This does not claim to eliminate upstream driver deadlocks.
- Relay retains at most 16 threads/16 messages per thread; prompts/replies,
  steps/files and rich-text blocks are bounded. Oversized prompts keep their
  draft, stale/cancelled runs cannot mutate newer runs, and file counts saturate.
- Catalog adds a live follow-end log and Enter/Shift+Enter composer demo;
  UI Lab and older showcase designs are unchanged.
- New `relay` example: a coding-agent desktop client with a frosted thread
  sidebar, streaming transcript (collapsible work steps, file-change cards,
  code blocks), composer with permission/model/effort menus, inspector card,
  thread actions popover and settings. A local scripted worker streams replies
  through bounded messages, with stop, retry and stale-run rejection.
- Catalog: frosted full-height sidebar with the content on a solid sheet that
  floats inside the material (inset 8px, rounded 12px), so it never meets the
  window's rounded corners. Blur is the default with `native-gpu`;
  `--background opaque` restores a solid window and software drawing stays
  opaque. The window clears fully transparent and the view paints the tint, so
  a sidebar slider (Catalog) or Settings slider (Relay) adjusts it live, 10–100%.
- Checked switch knobs and checkbox ticks contrast with the accent fill they
  sit on; a white accent (zinc, dark) previously hid the knob.
- Window framebuffers up to 64 MP are accepted (was 16 MP, below a 6K Retina
  display's ~20 MP, which made maximised windows exit). Shadows over the
  software blur budget are skipped (`effects_skipped`) instead of failing the
  frame.
- `.anchor_end(true)` keeps a vertical scroll view at its end while the reader
  is there (chat transcripts, logs); scrolling up releases it.
- `text_area(...).submit_on_enter(true)`: Enter submits and Shift+Enter inserts
  a newline, as chat composers do. Cmd/Ctrl+Enter still submits either way.
- GPU: axis-aligned lines (borders, dividers) are filled as exact rectangles.
  Stroked, a long horizontal border on the surface's top edge filled its whole
  first 16-pixel tile row, hiding a bordered sheet's background there.


## 0.2.0 (unreleased)

- macOS overlay chrome uses public AppKit unified toolbar geometry, giving native
  traffic lights more top/leading clearance. Catalog keeps them over its sidebar's
  drag strip. New `CompactOverlay` preserves the earlier short
  titlebar; standard Native/default chrome is unchanged. Native controls are not
  resized or repainted when switching appearance.

- Catalog data views: working rounded (12px default) and square table previews,
  continuous rounded frames, matching header/last-row focus contours without
  an internal gutter, dividers, column alignment and two-axis overflow;
  tables expose cell/header ancestry, indices and sort direction. Trees use vector
  chevrons, inset selection rows and logical accessible child relationships.

- Catalog is now the primary component demo: named controls, semantic selection,
  mounted accessibility focus/scroll reveal, scoped scroll actions, opt-in window
  alpha/blur, reliable worker completion and launch/state regression tests.
  Rust-only `.accessible_label(...)` and composite roles/states preserve schema1
  and legacy Painter/default dependencies. See `docs/CATALOG.md` for limits.

- Security/performance hardening: validate accessibility targets and values
  before side effects; enforce a hard 96 MiB GPU image registry budget and
  packing admission before uploads; borrow opaque image staging pixels and
  cache outline-ink bounds. Fix miter damage, GPU colour-glyph alpha/italic
  clipping and primary-variant contrast. See `docs/SECURITY_AND_PERFORMANCE.md`
  for measured results, advisory warnings and remaining limitations.

- Components: `ButtonVariant`/`ControlSize` presets, `.disclosure(open)` and
  `.label_align`, token opacity (`ColorToken::alpha`, `"@role/NN"`), readable
  primary labels under inherited colours, 6px default corners, padded panels,
  seamed joined bars, cleaner tables/trees and specific tree errors. New
  `specimen` example shows every default.
- Accessibility: AccessKit screen-reader tree and actions for native windows
  (roles, labels, values, ranges, states, focus; modal scoping; passwords never
  exposed), built on a toolkit-neutral runtime snapshot.
- Software renderer damage tracking: only changed regions are repainted and
  presented; incremental frames match full repaints pixel for pixel.
- GPU: Vello vector text, GPU-resident images, estimates for every Vello fixed
  buffer with per-frame software fallback for detected overflows, device-loss
  recovery. Benchmarks time settled frames. Windows added to CI.
- Document limits raised to 4096 nodes and depth 48.

- Bounded typed native worker messages with coalesced winit wakeups, batched
  UI-thread state updates, reentrant-producer fairness and close/cancel contracts.
- Nested focus reveal, captured draggable scrollbars and opt-in multiline soft
  wrap with shared paint/caret/selection metrics. Physical IME/bidi and OS
  accessibility acceptance remain separate gates.
- Typed in-window menus/context actions, tooltip/popover, progress/loading,
  fixed-row virtualization, sortable tables, expandable trees and split panes.
- Showcase Activity/Explorer are the new polished component acceptance surface;
  legacy Lab routes remain fixtures. GPU Showcase requests a smoke-tinted blurred
  window by default; opaque override, default Volt APIs/dependencies unchanged.
  Document the pinned winit macOS private-blur-API release limitation explicitly.
- Additive `.fill_width()` clears fixed component defaults for parent allocation;
  virtual rows stretch with their list and menu styling reaches the actual trigger.

- Replace the optional native GPU texture presenter with Vello 0.6/wgpu 26 GPU
  scene drawing for opaque/alpha windows: paths, glyph/image textures, nested
  clips, opacity and analytic shadows. Explicit auto/gpu/software policy,
  cached resources, diagnostic boundaries and actual hardware readback tests.
  Prune unused backend features while retaining Rust 1.87 and default embedded
  dependencies. No full CPU framebuffer raster/upload on the native GPU path.

- Rust-only reusable ButtonBar/Toolbar, typed SegmentedControl/TabBar and Tabs
  composites, with controlled selection, active-page-only callbacks, joined
  Full/selected outer corners and disabled-item validation. Additive group-scoped
  Left/Right/Home/End selection preserves ordinary button/legacy JSON behavior.
  UI Lab covers these controls; Atelier navigation now uses the typed tab bar.
- Controlled TextField/TextArea with grapheme editing, bounded undo/redo, selection,
  clipped viewport and IME; lazy native text clipboard, read-only/password protection.
  Typed Slider/Select and controlled modal/popup portals with input isolation and
  focus trapping/restoration. UI Lab exercises them together; Desk has real editing.
  Preserve Northbound contributor app. OS accessibility and mobile remain pending.

- Canonical `ui_lab` app with persistent demo state, section navigation, appearance
  controls, text/color specimens, ellipse/path clipping, horizontal/both-axis/nested
  scroll views, launch flags and headless section snapshots. Future feature demos
  extend this one workbench; focused examples remain regression fixtures.
- Additive `.scrollbars(Scrollbars::Hidden)` indicator policy. Hiding indicators
  leaves scroll input, offsets, clipping and callbacks intact.
- Fix natural scroll sizing through nested automatic containers while preserving
  bounded flex/spacers inside fixed-size panels, and round natural badge widths
  outward to prevent fractional-width last-glyph wrapping/clipping.

Extracted Volt presentation models, layouts, icon paths and paint recipes.

- Glass Browser-inspired optional surface tokens, adaptive tint, generic Panel and rounded/clipped layered pointer routing. Existing Volt visuals remain unchanged.

- Optional schema1 JSON documents, bounded validation, nested constrained layout,
  text/boolean bindings, button focus/capture and transactional explicit reload.
- Optional winit/softbuffer native Solid host with tiny-skia/cosmic-text adapter.
- Progressive native examples plus optional editable JSON files and usage documentation.
- Existing widget/Painter implementations and default dependencies preserved.

- Additive declarative styles: inheritable font family/size/weight/italic and text
  color; independent surface/border colors, widths and radii.
- Stateful Checkbox/Toggle, Badge and Separator; styled workbench/components demos.
- Optimized local dev CPU rendering, native border strokes, cached surface dimensions,
  opt-in stage/input tracing and reproducible CPU benchmark example.

- Primary `rust-ui` authoring: typed component builders/layout macros, direct owned
  Rust callbacks/functions, ordinary if/loops/reusable components, no JSON bridge.
- `native::run_ui`: action-only whole-tree composition and atomic tree/handler
  installation; exact-kind keyed focus, cancelled transient input and rejected-view
  recovery. Existing JSON run and input bool APIs retained.
- Counter/settings/keyed-list Rust examples, design docs and callback compile-fail
  tests. Native generic fonts choose installed sans/serif/monospace families.

- Additive container directional padding (horizontal/vertical/top/right/bottom/left)
  and horizontal/vertical inter-child gaps, with matching optional JSON fields,
  shared asymmetric layout/measurement/hit clipping and bounded validation.

- Correct in-development Rust UI spacing to `.padding(value, Edge)` / `.gap(value, Axis)`;
  unchanged Volt legacy widgets/Painter and JSON scalar fields.
- Numeric/full `.rounded` with layout-resolved radius and shared paint/hit geometry;
  nine `.flow(Flow)` content placements, divider alias and flexible/fixed spacers.
- HStack/VStack function/macro aliases preserve Row/Column behavior; new `rust_layout`
  interactive example and placement/rounding/spacer/compile-fail regressions.

- Typed responsive minimum-width values for Stack direction and container spacing,
  using logical viewport units with named/custom thresholds and bounded validation.
- Stack/ZStack functions/macros: adaptive orientation versus independent ordered
  layers; resize keeps nodes/handlers rather than recomposing view bodies.
- `rust_responsive` example, size-selectable Rust snapshot/benchmark/window CLI,
  threshold/DPI/focus/overlap regressions and responsive design documentation.

- Rust padding/gap accept enum/array/slice/vector selections; independent corner
  radius side/corner groups preserve uniform rounded APIs and shared hit geometry.
- New Rust buttons use 2 logical-pixel corners; legacy Button/JSON stay unchanged.
- Opt-in shadow/glow, subtree group opacity and hover/press/focus appearance;
  keyed interruptible color/border/opacity animation and paint-only enter/exit fade.
- Bounded cached native approximate-Gaussian masks and reusable opacity layers;
  exit ancestor/descendant opacity, sibling order and reinsertion continuity.
- Additive Native/Overlay/Borderless window options and safe header drag regions;
  active-only timers, focus/occlusion settling and explicit reduced-motion policy.
- rust_effects example and native pixel/latency/drag/compatibility regressions;
  no required Painter methods, new default dependencies or Volt vendor edits.

### Semantic surfaces and scrolling

- Fixed glow/shadow overflow through tight ordinary layout parents.
- Added semantic palettes, Light/Dark/System mode, custom palettes/global accents,
  and state-derived theme callbacks without changing legacy options structs.
- Added shared bounded PNG/JPEG image handles, fit/position/opacity, retained vector
  canvas commands, and explicit rounded/circle/path clips with input routing.
- Added opt-in Auto/Vertical/Horizontal/Both scrolling, keyed offsets, wheel/trackpad
  routing, nested edge bubbling, resize clamps and overlay position indicators.
- Added optional `native-gpu` alpha presenter and native blur request with logged
  opaque fallback. Real compositor/device acceptance remains separate.
- Added surface/scroll/transparent examples and regression tests.

### Original app-scale showcases

- Added Volt Atelier Welcome/Workspace/Component Studio screens to the shared UI
  Lab state, with a thin `showcase` launcher, responsive columns, real callbacks,
  live semantic palettes, typography, vectors, clips and horizontal gallery.
- Added original Volt Desk three-pane desktop composition with keyed local
  conversations, fixed sample composer, independently scrolling panes and compact
  launch mode. Still not an AI client; the later input batch adds an editable composer.
- Added installed-font SF/SF Pro and SF Mono aliases in the optional native painter;
  preserve old system/generic font behavior and never bundle Apple fonts.
- Added opt-in wheel-target passthrough for empty decorative overlay layout boxes;
  descendants still receive input, and existing routing defaults remain unchanged.
- Added font/glyph identity, geometry, callback, scroll, DPI and bounded-state
  regressions plus app-scale design and manual acceptance documentation.
- Fixed Atelier's Full corner selector: retain `Rounding::Full` rather than an
  invalid 999px sentinel, so selecting it no longer rejects later UI updates.
  Added all-preset rendering and pointer/keyboard/recomposition regressions.

### Application development first slice (unreleased)

- Additive standalone Application API with explicit renderer/theme/window policy,
  existing typed messages, and GPU-required recovery behavior.
- Independent typed destination navigation in Catalog; no mandatory shell.
- Opt-in cargo-volt local project templates and rebuild/restart development runner.
- Optional macOS application menu/tray menus with validated command delivery;
  default core/native dependency graphs preserved. Modern notifications and
  web/maps/AI/intents remain pending, not unsupported-operation success stubs.
