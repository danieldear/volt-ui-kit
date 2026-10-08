# Phased delivery

| Phase | Scope | Completion gate | Status |
|---|---|---|---|
| 0 | Source audit and architectural docs | Evidence, compatibility rules, explicit gaps | Initial pass complete |
| 1a | Opt-in Rust callback registry | Typed signatures, invalid dispatch, duplicates, borrowed closures tested | Implemented |
| 1b | Versioned document schema and parser | Valid/invalid fixtures, bounded parsing, diagnostics | Implemented |
| 1c | Shared tree and native vertical examples | Same Button/Panel recipes; one host/runtime | Implemented |
| 1d | Primary Rust-first authoring | Typed builders/direct closures, action-only body calls, atomic tree/handler replacement, keyed examples | Implemented; current validation in RUST_UI.md/PERFORMANCE.md |
| 1e | Responsive and layered stacks | Logical viewport thresholds, scalar parity, retained callbacks, ordered overlap hits | Implemented; headless gates in RESPONSIVE_UI.md/VALIDATION.md; native acceptance pending |
| 1f | Selective styles, effects, keyed motion and chrome | Geometry/pixel/state/cache/input regression gates; native acceptance | Implemented bounded first slice; EFFECTS_AND_WINDOWING.md; native OS validation pending |
| 2 | Layout, measured text, focus, capture, semantics | Nested constraints, keyboard/pointer agreement, cache tests | Basic layout/text/button input done; broader semantics pending |
| 3 | Toggle/Tabs/List/Menu/Input | State matrices, clipping, Unicode and editing coverage | Checkbox/Toggle/Badge/Separator done; Tabs/List/Menu/Input pending |
| 4 | Native gallery and platform adapters | IME/clipboard/accessibility, event-driven idle behavior | Solid host done; editing/platform gates pending |
| 5 | Optional GPU Glass | Correct compositing, backdrop caching, reduced transparency | Pending |
| 6 | Authorized Volt integration and release | Snapshot/test/GPU/performance/platform gates | Pending |

Do not claim declarative UI works end-to-end until phase 1c. Do not claim a desktop
framework until runtime/platform requirements are met. No release/version bump,
remote creation or publication without request.

The next target was expanded to the [native vertical slice](VERTICAL_SLICE.md).
Windowing, basic layout, measured text, button input/focus, Rust callbacks,
bindings and explicit reload are now implemented. This does not complete phase2
accessibility or phase4 editing/IME/platform validation. See examples/README.md.

Primary authoring now uses Rust declarations, not JSON. Follow [Rust UI](RUST_UI.md) for the delivered API; optional JSON reload remains compatible. Fine-grained state observation, external update notification and editing/platform layers are not yet implemented.
