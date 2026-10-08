# Changelog

## 0.1.0 (unreleased)

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
