# Native vertical slice — implementation plan (2026-10-08)

## Scope

One native window with editable documents, Panel/Row/Column/Text/Button,
measured text, resize/DPI, clipping, hover/press/disabled/focus, pointer capture,
Tab/Shift-Tab/Enter/Space, validated Rust callbacks and visible state bindings.
Explicit F5 reload retains the last valid document. Multiple runnable examples
share library infrastructure. No WYSIWYG, scripts, GPU blur, editor/IME or menus.

## Execution sequence

1. Bounded versioned JSON parser and semantic validation; tests.
2. Shared UI tree, basic flex layout, text measurement and interaction; tests.
3. CPU raster adapter (tiny-skia + cosmic-text) and optional winit/softbuffer host.
4. Native window/layout/events/document/complete-app examples and editable files.
5. Automated validation, native runtime proof where available, documentation.

The software renderer is intentional for a first Solid-only slice: it avoids a
second GPU stack and uses the existing Painter component recipes. It is not a
performance claim or a GPU architecture decision. A future Vello/GPU adapter can
reuse the document runtime. The legacy default path receives no host/dependencies.

## Completion gates

- Invalid schema/IDs/properties/dimensions/connections are rejected before install.
- Layout/hit/clipping agree at multiple scales; disabled/cancelled inputs do not activate.
- Rust callbacks update bound visible values; rendering and hit testing share one tree.
- Reload errors preserve prior document, state and connections; no duplicate listeners.
- Native window opens, redraws only on demand, resizes, and closes cleanly.
- Examples build and run from documented commands; default tests/MSRV stay intact.
- Separate source/test evidence from macOS UI proof and untested Linux/Windows.

Volt/Glass source and vendor remain untouched. No commits, remote or release.

## Delivery and evidence (2026-10-08)

All five implementation stages above are implemented for the narrow Solid slice.
Examples: native_window, panels_and_layout, buttons_and_events, declarative_ui,
complete_app; the original SVG gallery remains available.

Automated evidence:
- Default:34 unit +1 doc tests passed; dependency tree only unicode-width.
- Declarative:49 unit +1 doc tests passed.
- All features:53 unit +1 doc tests passed, including shaping/raster and bounded I/O.
- Strict Clippy, formatting, Rust1.87 all-targets/all-features and examples build passed.
- Default package verification passed with --allow-dirty (repository was untracked).
- All native examples generated raster snapshots; complete app also rendered at2x.
- All pre-existing widget/paint/layout source hashes match the pre-change inventory.

macOS native UI evidence:
- All five native examples opened a real window and their test windows closed.
- Complete app: pointer activation changed visible milestone count; Tab/Space reset
  it; buttons example: Tab/Enter changed the visible bound activation count.
- Complete app: a document heading edit appeared after F5. Invalid JSON produced
  a reload-rejected window title while retaining the last valid UI; restoring the
  document and F5 recovered. Temporary test edits were restored.
- Native typography/rasterization was inspected at the actual display scale.

Limits of verification:
- Resize/DPI layout and hit geometry tested headlessly at multiple scales; manual
  resize automation did not establish a distinct native resized viewport. Moving
  between physical displays and platform suspension still require runtime QA.
- Event-driven Wait scheduling is source-verified, not an idle power/CPU benchmark.
- Linux/Windows native runtime, platform accessibility, IME and full Unicode editing
  are not verified/implemented. No GPU backend, Glass blur or benchmark parity claim.
- Volt workspace/GPU integration was not rerun or synced; Volt and Glass untouched.

Review fixes: narrowed row/explicit-width wrapping remeasurement; memoized intrinsic
sizes; one reusable clip mask; shaped-text LRU; bounded regular-file reload; private
unchecked DTO; practical font/text/scale limits and bounded glyph-cache reset policy.

This is a complete first application vertical slice, not a complete desktop framework.

## Post-slice responsiveness/style repair (2026-10-08)

User feedback exposed a major unoptimized CPU render delay absent from the first
functional checks. It is now measured and repaired; see [performance](PERFORMANCE.md).
The newer gates supersede the initial test counts above: native65library+4example+
1doc, declarative56+1doc, default34+1doc. Editable inheritable typography/colors,
local surface/accent/border/radius styles, Checkbox/Toggle/Badge/Separator and the
components example are now implemented. Native mouse/keyboard/live-style reload
checks passed. Volt/Glass remain untouched; accessibility/IME/platform gates remain.

## Rust-first frontend addition (2026-10-08)

Primary authoring is now UI declarations inside Rust. Typed component builders,
layout macros, direct closures and view functions lower directly to the common
tree; JSON remains optional. Counter/settings/keyed-list examples cover direct
state, component functions, conditionals/loops, typography and stable item keys.
The runner composes only initially/after dispatched callbacks and atomically
installs tree/handlers; focus preservation requires exact enabled component kind.
Rejected bodies retain prior UI/callbacks, not prior application state. Rust code
hot reload, signals/subtree reconciliation and external-update wakeups are future work.
See [design](RUST_UI.md), [current validation](VALIDATION.md) and [measurements](PERFORMANCE.md).

This addition is source/test/headless-verified; initial native counter presentation
was logged, but live Rust example input QA was blocked by the locked Mac. Do not
apply the earlier JSON GUI evidence to this new frontend automatically.
