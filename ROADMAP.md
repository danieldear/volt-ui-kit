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
- General-purpose Input, Tabs and selectable List APIs (declarative Checkbox/Toggle and Panel delivered).
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

- Animation with reduced-motion support and explicit scheduling.
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
- Later: explicit container queries, adaptive grids/child wrapping, scrolling and responsive visibility.
- See [implementation gates](docs/IMPLEMENTATION_PLAN.md); legacy Volt APIs stay intact.

Next Rust UI milestones: external state-update wakeup API, measured subtree reconciliation/signals, and richer editing/accessibility. These are future work, not delivered SwiftUI/Compose parity.
