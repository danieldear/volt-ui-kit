# Roadmap

## Delivered: toolkit extraction

- Terminal-independent presentation/layout models and vector paint recipes.
- Generic button shared with Volt; application-specific widget compatibility.
- Existing-host renderer adapter and static SVG backend.
- Component gallery, unit tests and reproducible vendoring.
- No additional GPU device, event loop or periodic redraw timer in Volt.
- Glass-inspired generic Panel, smoked/frosted tokens, host-fed tint hysteresis.
- Rounded/clipped topmost hit map with explicit background blockers.

## Next: polish and generalization (high priority)

- Replace compatibility-specific labels/action IDs with application-supplied metadata.
- General-purpose Input and selectable List APIs (declarative Checkbox/Toggle,
  Panel and typed Tabs delivered; editing/selection semantics remain required).
- Measured typography, clipping, long text, narrow-window and DPI improvements.
- Shared keyboard focus model, pointer capture and predictable disabled/hover states.
- Validate each change in the gallery and Volt; retain screenshot and performance gates.

## Before calling this a complete framework (required)

- Native standalone winit/wgpu host with text shaping, IME, clipboard and undo/redo.
- Accessibility semantic tree, actions and platform adapters.
- Nested layout, constraints, scroll containers and virtualization.
- Rich Unicode/multiline text editing and input method composition.
- Platform-specific menus/dialogs/window lifecycle and multi-window validation.
- Stable API, documentation, packaging, published crate and compatibility policy.
- macOS/Linux/Windows runtime and assistive-technology validation.

## Later possibilities

- Broader motion: layout/transforms, springs/keyframes and system reduced-motion discovery (bounded appearance animation delivered below).
- Optional GPU glass compositor: cached downsampled/separable blur, host-supplied backdrop, consistent color space, reduced-transparency fallback; no blur work for opaque widgets.
- Additional renderer backends without changing widget APIs.
- Rich tables, tree views and inspector-oriented controls.
- Theme editor as a generic color/palette component.

No claim of automatic accessibility, full cross-platform framework readiness,
or terminal benchmark parity is made by the initial extraction.

## Declarative UI track

- Delivered: primary Rust-first UI builders/layout macros, direct callbacks, action-only whole-tree composition, conditional/keyed examples.
- Delivered: responsive Stack direction/padding/gap and layered ZStack; fixed VStack/HStack aliases retained.
- Delivered: optional typed Rust callback registry (default dependency footprint unchanged).
- Delivered: schema1 JSON, shared layout/input runtime, text/boolean bindings and explicit validated reload.
- Delivered: optional software native host, progressive examples, editable typography/colors and typed Checkbox/Toggle controls.
- Later: two-way editing bindings, richer focus/input/semantics and custom components.
- Later: explicit container queries, adaptive grids/child wrapping and responsive visibility
  (keyed scrolling, mounted focus auto-reveal and fixed-row virtualization are delivered).
- See [implementation gates](docs/IMPLEMENTATION_PLAN.md); legacy Volt APIs stay intact.

Next Rust UI milestones: measured subtree reconciliation/signals and richer editing/accessibility. The bounded external state-update wakeup API is delivered. These are future work, not delivered SwiftUI/Compose parity.

### Component delivery (2026-10-08)

Delivered: reusable action/tool bars, typed segmented controls and tab bars,
active-page-only tabs, group-scoped selection keys, and joined outer corners.
See [the exact API, tests and unfinished phases](docs/COMPONENTS.md).

Delivered 2026-10-09: controlled text/IME/clipboard foundation, bounded slider/select
controls and modal focus/dismissal; UI Lab and Desk now exercise real editing.
Next: platform accessibility/IME/device acceptance, unmounted focus navigation,
wrapping/bidi completeness and renderer optimization. Mounted focus reveal, opt-in soft wrap and bounded background messages are delivered.
Software rasterization dominated the earlier rich-screen baseline; optional
Vello GPU drawing is now delivered. See [GPU rendering](docs/GPU_RENDERING.md)
for the feature policy, measurements and remaining platform gates.
Do not label the whole component set or toolkit complete yet.

Later, explicitly deferred: an original desktop-style panel/dock/launcher demo
using these widgets, not a replacement OS session/window manager. A real Wayland
compositor is a separate Linux project. See [resource usage and experiment scope](docs/RESOURCE_USAGE_AND_DESKTOP.md).

### Delivered bounded motion/chrome slice

Single/multiple spacing/corner selection, shadows/glow/group opacity, keyed
appearance animation/fade and integrated macOS window chrome are implemented.
Native OS acceptance remains pending. Future: automatic OS reduced-motion
discovery, display-synchronized scheduling, layout transforms/springs/keyframes,
window-control components and cross-platform custom-decoration adapters.

## Surface vertical slice (2026-10-08)

The original diagnostic workbench is `ui_lab`. Showcase was the primary polished
acceptance app from 2026-10-09. As of the user's 2026-10-10 direction, **Catalog
is the canonical component demonstration/acceptance app**. New work extends
Catalog's existing design; Showcase, Lab and focused examples remain regression
fixtures. See [Catalog](docs/CATALOG.md).

Implemented: semantic UI palettes, root/system/state theme selection, global accent,
background images/opacity, vector paths/canvas, explicit content clips, effect
overflow fix, keyed nested scrolling and optional alpha-capable window presenter.

Remaining acceptance: OS desktop transparency/blur and appearance changes, Windows
and Linux runtime verification, trackpad feel. Delivered follow-ups: draggable visible scrollbars, mounted keyboard focus auto-reveal and bounded fixed-row virtualization. Remaining: touch/drag scrolling, smooth scrolling, SVG asset parsing, variable-height virtualization and general accessibility/IME. Default legacy
renderers and terminal/vendor integration remain unchanged.

### Native GPU drawing (2026-10-09)

Delivered: feature-pruned Vello/wgpu scene drawing, opaque/alpha native windows,
explicit auto/gpu/software selection, software fallback, cached glyph/image
resources, GPU clips/opacity/analytic shadows and hardware readback acceptance.
Rust 1.87 and the default embedded footprint are retained.
Next: cross-platform runtime, GPU profiling/memory, damage/cached scenes and
real device-loss stress. Mobile and compositor work remain deferred.

### Application runtime and richer components (2026-10-09)

Delivered: std-only bounded typed messages, one batch per native turn with
coalesced wake/fairness; controlled menus/context/popovers/tooltips; progress and
reduced-motion loading; 10,000-record fixed-row viewport; sortable tables,
expandable trees, captured/keyboard split resizing; opt-in wrapped editing.
Showcase Activity/Explorer own new acceptance and GPU Showcase requests
translucent smoke/blur by default. Default embedded/Volt behavior stays opaque.

0.3.0 supplies a public AppKit material bridge for decorated macOS GPU windows.
Borderless/fullscreen/preference-change acceptance and application/dependency
review still gate App Store-safe distribution claims.
Physical IME, VoiceOver, cross-platform runtime, pointer/device acceptance and
GPU energy/utilization remain distinct from automated toolkit checks.
See [the plan and measured receipt](docs/APP_RUNTIME_AND_COMPONENT_PLAN.md).

### Application-first track (2026-10-10)

First slice: additive Application configuration, standalone controlled navigation,
GPU-first local templates/dev runner, optional macOS application-menu/tray adapters.
Follow-up: public decorated macOS GPU window materials, state-aware commands/
shortcuts, close veto and bounded native notes-file workflow are implemented.
Remaining: full notification settings/action lifecycle, full OS termination, state restoration/multi-window,
shipping bundles, deeper navigation, debugging and richer motion.
WebKit/maps/Foundation Models/App Intents follow desktop service acceptance.
Theme editor is explicitly deferred; templates never prescribe the developer's UI.
See [status, APIs and gates](docs/APPLICATION_DEVELOPMENT.md).

Unreleased 2026-10-11: native panels + async 8 KiB UTF-8 file workflow and counted
command witness in Catalog only. See [scope, safety limits and receipt](docs/DOCUMENT_FILES.md).

### Local macOS packaging follow-up (2026-10-11; Unreleased)

- `cargo volt bundle` packages a developer-selected prebuilt executable, identity,
  flat resources/icon and privacy descriptions with native validation and local
  ad-hoc signing. Catalog launch/native resource reading accepted on the test Mac.
- No new crates or default/vendored Volt changes. Developer ID/notarization,
  installers, nested code, universal assembly and other OS packaging remain
  separate release work. See [macOS packaging](docs/MACOS_PACKAGING.md).
- Next runtime services remain notifications/tray acceptance, richer navigation
  and multi-window; existing large-display GPU and IME/VoiceOver gates remain.


### Bounded notifications/tray follow-up (2026-10-11; Unreleased)

- Independent safe `native-notifications`: alert permission, silent local
  stable-ID requests and removal; precise unsupported settings/action/foreground
  capabilities. Rust 1.87 and default/Volt compatibility retained.
- Catalog only: Desktop services, bounded completion witness and shared guarded
  tray commands. Native acceptance currently pending because the QA Mac was locked.
- Other OS adapters, delegates, multi-window and the original GPU/IME/VoiceOver
  gates remain separate. See [notifications](docs/DESKTOP_NOTIFICATIONS.md).
