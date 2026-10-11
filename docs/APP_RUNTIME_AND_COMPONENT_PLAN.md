# Application runtime and component milestone

Date: 2026-10-09. Status: scoped code delivered; final verification receipt and external acceptance gates below. User redirected new visual testing from UI Lab to the polished Showcase during refinement.

## Delivery plan

| Stage | Contract | Acceptance |
|---|---|---|
| 1 Runtime | Typed bounded worker messages; coalesced winit wakeups; UI-thread state updates; bounded batch; no polling | Queue race/order/full/close tests; visible cancellable native job |
| 2 Interaction | Nested focus auto-reveal; draggable visible scrollbars; opt-in multiline soft wrap | Offscreen Tab order, pointer capture, removal/cancel, 1x/2x caret/hit/IME tests |
| 3 Components | Typed action/context menus; hover/focus tooltips; controlled popovers; progress/loading; virtual lists; tables; trees; split panes | Input/disabled/Escape/focus lifecycle tests; bounded 10,000-record composition; pointer+keyboard resize |
| 4 Showcase | Activity and Explorer screens share Atelier colors/SF/SF Mono/corners; existing Lab stays unchanged as a fixture | 1x/2x and compact/wide tests; actual native GPU background job + live callback; transparent root/surface tests |
| 5 Compatibility | Existing feature builds and default renderer API; real Volt consumer check without editing its checkout | Rust 1.87, tests/Clippy/format/package; isolated Volt compile; CPU/RSS stress evidence |

```text
worker thread -- UiSender<Message> --> bounded queue
                                      |
                              one coalesced wake
                                      v
                           winit UI event thread
                           update(&mut App, msg)
                           compose once per batch
                                      |
                         shared runtime and scene
                           /                  \
                     software                Vello GPU
```

## Boundaries

- No Tokio or new dependency is necessary for worker messages. This does not implement an async executor; apps may use their own executor and send results.
- Application state stays on the UI thread; messages are `Send`. Callback work itself must remain short. Queue capacity and batch bounds do not make arbitrary callbacks nonblocking.
- Fixed-row virtualization mounts a controlled visible range plus overscan, not the entire dataset. Variable-height rows, arbitrary jump-to-unmounted focus and database fetching remain future API work.
- Tables/trees are declarative compositions, not OS-native controls or a full spreadsheet. Their data sorting/filtering is application-owned.
- Soft wrap is opt-in. Actual foreign-language input-method acceptance, bidi selection semantics and OS accessibility bridges are separate gates, not implied by unit tests.
- GPU/software share declarations. Native GPU utilization/energy, Linux/Windows/device runtime and VoiceOver cannot be certified by local compilation or screenshots. Showcase requests OS alpha/blur; since 0.3.0 decorated macOS windows request public AppKit material. App Store-safe packaging/dependency review, borderless/fullscreen and comprehensive backdrop acceptance remain release gates.
- Mobile and a desktop-shell/compositor prototype remain deferred. This milestone is the application toolkit foundation.
- Preserve existing dirty work and Northbound. No commit, push, memory changes or OS preference changes are part of this request.

## Verification receipt

- Component/runtime source and bounded input contracts are implemented. No new dependency for this batch; default remains `unicode-width` only.
- Real Apple M4 Pro / Metal hardware fixtures passed (three explicit ignored tests), including 24 alternating resize/virtual-window frames, p50 4.788ms/p95 9.052ms including readback. No framebuffer upload; CPU asset diagnostics are not GPU-memory measurements.
- Actual public Volt 0.1.12 workspace compiled offline against an isolated synchronized toolkit snapshot. Original Volt checkout was not changed; this is source compatibility, not native terminal release acceptance.
- Native Showcase Activity visibly reached 10% while a live callback incremented its counter; completion stopped worker animation. Native menu disabled-item skipping/action, popover Escape/opener restoration, wrapped SF paste/auto-reveal and Explorer row selection were also verified. GPU logs show Vello/Metal, no CPU frame or framebuffer upload.
- New Showcase tests check callbacks, overlays, wrapped editor, stale cancelled messages, virtual remount/selection, sort/tree/split, transparent root, opaque text and fill-width geometry at compact/Retina sizes.
- Full feature/test/lint/MSRV/package results are recorded in [the verification receipt](APP_RUNTIME_VERIFICATION.md).

Remaining external gates: physical pointer/trackpad/drag acceptance (native automation lost window focus), foreign-language OS IME and visual-order bidi editing, accessibility bridge/VoiceOver, Windows/Linux native runtime, GPU memory/utilization/energy, public macOS materials and actual terminal visual/performance acceptance. Do not describe these as completed.
