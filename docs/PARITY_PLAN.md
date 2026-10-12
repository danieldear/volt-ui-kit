# Tailwind / component-library parity plan

Goal: close the gaps found when comparing Volt UI with Tailwind CSS utilities
and the component sets built on it (shadcn/ui, daisyUI, Tailwind UI).
Foundations come first because most components depend on them (a calendar
needs a grid, a date field needs truncation, a picker needs min/max sizes).

Status legend: ☐ planned · ◐ in progress · ☑ done · ✗ deferred (with reason)

## Phase 1 — Foundations (layout, text, style, motion, platform)

| Item | API sketch | Status |
|---|---|---|
| Cursor styles | `.cursor(Cursor::Pointer)`; buttons/links → pointer, editors → I-beam, split handles → resize | ☑ |
| Text alignment | `.text_align(TextAlign::Center)` | ☑ |
| Line height | `.line_height(1.5)` (multiple of font size) | ☑ |
| Truncation | `.max_lines(1)` → ellipsis; `.no_wrap(true)` | ☑ |
| Case / decoration | `.uppercase()`, `.underline(true)`, `.strikethrough(true)` | ☑ |
| Min/max sizes | `.min_width() .max_width() .min_height() .max_height()` | ☑ |
| Fractional sizes | `.width_fraction(0.5)` (share of the parent's space) | ☑ |
| Aspect ratio | `.aspect_ratio(16. / 9.)` | ☑ |
| Justify | `.justify(Justify::SpaceBetween)` (start/center/end/between/around/evenly) | ☑ |
| Wrapping rows | `.wrap(true)` on a row | ☑ |
| Grid | `grid(children).columns(3)` or `.min_column_width(220.)`; child `.column_span(2)` | ☑ |
| Single-side borders | `.border_sides(Sides::BOTTOM)` | ☑ |
| Dashed / dotted borders | `.border_style(BorderStyle::Dashed)` | ☑ |
| Gradients | `.background_gradient(Gradient::linear(angle, stops))`, radial | ☑ |
| Translate / hover lift | `.offset(x, y)`, `.hover_offset(0., -2.)` | ☑ |
| Pulse (skeletons) | `Ambient::pulse()` opacity animation | ☑ |
| Slide / scale transitions | `Transition::slide(dx, dy)` alongside fade | ☑ |
| Custom easing / delay | `Easing::CubicBezier(..)`, `Animation::delay_ms` | ☑ |
| Disabled / focus-visible styles | `.disabled_style(..)`; focus rings only after keyboard focus | ☑ |
| Responsive visibility / width / font size | `.visible(responsive(false).at(Md, true))`, responsive width / font size | ☑ |
| Programmatic scroll | `.scroll_to(key, request)` on a scroll view | ☑ |
| Bundled fonts | `native::register_font(bytes)` | ☑ |
| SVG path import | `VectorPath::from_svg(d)` | ☑ |
| Design tokens | `tokens::space::*`, `tokens::radius::*`, `tokens::text::*` scales | ☑ |

## Phase 2 — Icon set

A built-in, theme-aware vector icon set (`icon(Icon::Search)`), 65 glyphs,
original drawings on a 24-unit grid. ☑

## Phase 3 — Date and time

`CalendarDate` (no external date crate), `calendar`, `date_picker`,
`date_range_picker`, `time_picker`. ☑

## Phase 4 — Form inputs

| Item | API sketch | Status |
|---|---|---|
| Radio group | `radio_group([(v, "Label")]).selected(v).on_select(..)`, descriptions, horizontal | ☑ |
| Number input | `number_input(v).range(0., 10.).step(1.).decimals(1).suffix(" kg")` | ☑ |
| Combobox | `combobox(items, &state).state(..).on_select(..)`; type to filter, Enter picks | ☑ |
| Tag input | `tag_input(&tags, &state).on_change(..)`; Enter adds, × removes | ☑ |
| Range slider | `range_slider(lo, hi).on_change(\|app, lo, hi\| ..)`; nearer thumb, no crossing | ☑ |
| Colour picker | `color_picker("#3B82F6")`: swatches, H/S/L sliders, hex field (`Hsl`) | ☑ |
| Drop zone | `drop_zone().accept(["png"]).on_drop(..)`; native drag and drop via winit | ☑ |
| One-time code | `otp_input(code, 6).on_complete(..)` | ☑ |
| Rating | `rating(n).max(5).on_change(..)`; read-only without a handler | ☑ |
| Adornments | `text_field(..).leading_icon(Icon::Search).trailing_action(..)` | ☑ |
| Form field | `form_field("Email", control).description(..).error(..).required(true)` | ☑ |

Runtime additions: two-thumb sliders (`SliderProps.lower`), `Event::FilesDropped`
with `Runtime::file_hover` / `drop_files`, and `Popover::keep_anchor_interactive`
so a combobox field keeps focus and typing while its list is open.

## Phase 5 — Overlays and disclosure

`accordion`, `collapsible`, `drawer`, `toaster`, `command_palette`, menu
check/radio items + submenus + shortcut hints, `hover_card`. ☑

Submenus open in place of the parent list with a back row (drill-down)
rather than as cascading side panels; both are controlled by app state.

## Phase 6 — Display

`avatar` / `avatar_group`, `alert`, `skeleton`, `kbd`, `breadcrumb`,
`pagination`, `steps`, `timeline`, `empty_state`, `stat`, charts
(`sparkline`, `bar_chart`, `line_chart`), `carousel`, `image` with fallback
(`image_view`). ☑

## Phase 7 — Catalog, docs, tests

Catalog pages for every new component, API docs, changelog, and tests for
each feature (layout equivalence, validation, accessibility roles). ☑

Seven catalog pages (Utilities, Icons, Date & time, Form inputs, Overlays,
Data display, Charts), each with live demos, code and an API table, checked
by the catalog's every-page build test and by rendered snapshots.

## Progress notes

Phase 1 complete: every foundation item above is implemented and tested
(544 tests, all features). Offsets, hover lifts and slide transitions animate
through the motion system at paint time; layout and hit targets stay put.

Phases 2–4 complete: icons, date/time pickers and form inputs, each with
integration tests (`tests/date_components.rs`, `tests/form_components.rs`).

Phases 5–7 complete: overlays and display components
(`tests/overlay_display_components.rs`), catalog pages and changelog.

## Deferred (and why)

Implemented APIs are not the same as complete native acceptance or exact
Tailwind/shadcn behavioural parity. See [the current review receipt](CLAUDE_REVIEW_2026_10_12.md).

Recorded here as work proceeds, with the reason, rather than silently skipped.

- Toast auto-dismiss timers: the toolkit has no timer API, so toasts are
  removed by the app (e.g. a worker thread sleeping, then a `UiSender`
  message). A built-in timer service would be a separate feature.
- Cascading submenus: implemented as drill-down instead (see Phase 5);
  nested side-by-side popups need multi-popup focus rules in the runtime.
- Interactive charts (hover tooltips, axes with tick labels): charts are
  static vector drawings; hover read-outs would need per-point hit regions.
- Composite keyboard/accessibility polish: range sliders currently expose
  one slider value and arrow keys move the last grabbed thumb; separate
  keyboard/screen-reader thumb targets remain to be added. Combobox Enter
  selects the best match and command palettes support Tab; dedicated
  up/down result navigation remains to be added.
