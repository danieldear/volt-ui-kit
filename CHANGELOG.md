# Changelog

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
