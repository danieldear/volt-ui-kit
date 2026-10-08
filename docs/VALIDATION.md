# Validation and contribution gates

```sh
cargo fmt --all -- --check
cargo test --locked
cargo test --locked --all-features --all-targets
cargo test --locked --all-features --doc
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo +1.87.0 check --locked --all-targets --all-features
cargo package --locked --allow-dirty
cargo run --locked --example gallery
cargo build --locked --all-features --examples
cargo run --locked --features native --example complete_app -- --snapshot target/complete-app.png
```

`--allow-dirty` is only for this pre-commit repository's local verification; it is
not a release approval. Add feature/MSRV checks to CI as features grow. Rustdoc
examples and public error behavior should have tests. Add regression fixtures
before fixing parser/layout issues. Review changes for legacy public struct
literal compatibility and unchanged Painter implementations.

Before authorized Volt integration: snapshot hash inventory, full workspace
suite, unchanged GPU fixtures (or explicitly reviewed expected differences), and
measured terminal/idle behavior. Do not substitute SVG screenshots for GPU proof.
Do not silently bless changed hashes. Keep unrelated host edits intact.

New runtime gates: narrow/long text, fractional DPI, Unicode, clipping, disabled
input, keyboard navigation, capture cancellation, reload failures, duplicate IDs,
IME and accessibility, then platform/runtime performance evidence. Record CPU,
allocations, redraw count, text/layout cache misses and blur submissions under
idle and active workloads. Establish baselines before asserting performance.

## Responsiveness and styling repair gates (2026-10-08)

Passed: default34unit+1doc; declarative56unit+1doc; native65unit+4example+1doc;
strict all-target/all-feature Clippy; formatting; Rust1.87 all-target/all-feature
check; all examples build; package verification58files. All19legacy source hashes
still match the pre-change inventory; default dependency remains unicode-width.

New tests cover style validation/inheritance/precedence, typed boolean state and
pointer/keyboard/cancellation/disabled behavior, shaped-style cache/draw parity,
stroke clip/order, surface resize decisions, and exact fast-fill equivalence.
Native UI and measured CPU results are recorded in [performance](PERFORMANCE.md).
Do not conflate measured software responsiveness with Volt/GPU compatibility proof.

## Rust-first delivery gates (2026-10-08; supersedes counts above)

Passed: default34unit+1doc (1 ignored README fragment); declarative56unit+1doc;
all-features69library+8Rust integration+8example+5doc tests (including3compile-fail;
1ignored README fragment). Formatting, strict all-target/all-feature Clippy,
Rust1.87 all-target/all-feature check, all examples build and local package
verification65files passed. Default dependency tree remains unicode-width only;
all19legacy source hashes still match. Volt/Glass were not modified or synced;
Volt workspace/GPU regression tests were not rerun, so do not claim integration proof.

Tests cover direct closures/functions/owned captures, wrong callback signatures,
conditional/repeated composition, style/key/node/depth validation, exact-kind
focus/reordering/removal/disabling, pending press cancellation, rejected candidate
atomicity, retained handlers, fresh captures after valid composition and view-body
call counts (no calls on redraw/hover/resize/cancelled release). JSON regression
suite remains passing. Generic-font selection now verifies installed faces.

Counter/settings/keyed-list headless raster snapshots were generated and inspected.
A real Rust counter launched and logged presented frames, but live pointer/key
validation of the Rust examples was blocked by the locked Mac in this session.
Prior JSON-example GUI checks do not establish Rust-host GUI completion. These
live checks remain required after unlock, along with resize/DPI and other-platform
runtime checks. CPU-only measurements are in [performance](PERFORMANCE.md).

## Directional spacing gates (2026-10-08)

Passed: seven additional spacing regressions plus a runnable directional-spacing
doctest; all-features69library+15integration+8example+6doc tests (1ignored README
fragment). Default34library+1doc still pass; JSON56library+1spacing+1doc pass.
Strict Clippy, formatting, Rust1.87 all-target/all-feature and examples build pass;
local package verification66files passes. All19legacy hashes/default dependency
tree remain unchanged. No Volt/Glass source or vendor edits/sync were made.

Spacing tests cover top/right/bottom/left and horizontal/vertical precedence,
1/1.5/2x asymmetric content bounds, axis-specific gaps/fallback/no outer gap,
intrinsic height/wrapped width, padding/gap non-hit regions, oversized padding and
strict Rust/JSON numeric/component validation. The pre-change scalar Settings
snapshot and the new runtime scalar Settings snapshot have identical PNG SHA256
at2x. The counter example now uses directional padding/gaps and rendered at2x.
These are headless geometry/raster checks, not a new platform/DPI runtime claim.

## Selector/placement/rounding delivery gates (2026-10-08; current counts)

Passed: all-features69library +23integration (8Rust frontend,8spacing/JSON,
7placement/rounding/spacer) +9example tests;8rustdoc tests including4compile-fail
(1ignored README fragment). Default34library+1doc still pass; declarative-only
56library+2JSON integration+1doc pass. Strict all-target/all-feature Clippy,
formatting, Rust1.87 all-target/all-feature, all native examples build and local
package verification68files and strict warning-free rustdoc generation pass. Default tree remains unicode-width0.2.2 only;
all19legacy widget/Painter source hashes match. No Volt/Glass source, vendor sync,
commit, push or release was performed; consumer/GPU runtime validation remains
separate.

Regressions cover corrected typed `.padding(value, Edge)` / `.gap(value, Axis)`
selectors and gap-edge compile rejection, integer/fractional/full rounding,
resize/DPI-resolved paint/hit radius, local/last-write radii, all9Flow placements
at1/1.5/2x, main/cross placement, unchanged default stretch, explicit stretch,
overflow/padding constraints, hstack/vstack macro/function parity, divider alias,
noninteractive/fixed/proportionally weighted spacers, and additive strict JSON
flow/full-radius/spacer defaults. The interactive `rust_layout` example's actual
Rust callbacks recompose placement and Add/Reset state in headless tests.

The scalar Settings snapshot remains byte-identical at2x after migrating its
Rust methods to selectors/rounded numeric alias. SHA256:
`846a1bfc2a5889435f7324ce02b4ed5246d9c00e1b3689bd0ca71f2eff7763d4`.
The new placement example's2x raster was generated and inspected: fonts, pill /
fractional buttons, dividers, centered preview and spacer-separated footer.
This is headless software geometry/raster evidence, **not native pointer/window
or Volt compatibility acceptance**. Live Rust GUI input/resize/DPI after unlock
and Linux/Windows/platform accessibility checks remain outstanding.

## Live Rust layout QA (2026-10-08; partial, not acceptance completion)

Current native `rust_layout` was built and staged in an ignored local QA bundle.
CUA launched and visually inspected its window at960x760 logical /1920x1520
rendered pixels. Ten consecutive Add clicks produced the visible count0→1→10,
with Reset becoming enabled. Pointer-to-Rust-state-to-render is verified for that
sequence; this supersedes the earlier entirely blocked Rust-host input status.

The Mac then locked: CUA reported `The Mac is locked and automatic unlock could
not unlock it`. Manual unlock was requested. Reset/disabled behavior, Tab /
Shift+Tab /Enter /Space, release-outside/corner hits, all placement switches,
window resizing and close/reopen still require **live** completion after unlock.
The visible AX tree has native window/chrome but no widget semantics; no platform
accessibility acceptance is claimed.

Read-only display inventory currently shows rendered/logical dimensions2x on both
attached displays (Color LCD and DELL). Current demo framebuffer is2x too.
A real1x↔2x monitor transition was **not** performed; no display or security
preferences were changed. Fractional scales are only headless coverage below.

Added one real-font software matrix regression: all9placements ×3logical sizes
(320x240,640x480,960x760) ×5scales (1,1.25,1.5,2,3) =135cases. It validates finite
bounds/clips, visible Add/Reset pointer and keyboard callbacks and focus state.
**Minimum-size limitation found:** at320x240 this dense demo's preview is shorter
than its36px fixed vertical insets, so Add/Reset are clipped away. They correctly
cannot receive keyboard focus. This is not an adaptive/scrolling layout; minimum
size usability remains a limitation, not a passed live resize result. At640x480
and960x760 the controls remain hittable in this headless matrix; that does not
prove label readability or OS-event routing. No runtime semantics were changed.

Current expanded suite: all-features69library+23integration+10example tests and
8rustdoc tests (4compile-fail;1ignored fragment); strict Clippy/MSRV/format/package
checks passed for the new regression.19legacy widget/Painter hashes remain
unchanged. No Volt/Glass edits, vendor sync, commit/push or release.

Native tracing for the initial10Add clicks is recorded in
[performance](PERFORMANCE.md#live-rust-layout-input-sample-2026-10-08).

## Responsive/layered stack slice (2026-10-08)

Implemented: fixed VStack/HStack aliases, adaptive Stack horizontal/vertical
direction, logical viewport breakpoints with named/custom widths, responsive
container padding/gap, independent ordered ZStack layers, and `rust_responsive`.
Rust examples now accept `--size WIDTHxHEIGHT` for native/headless runs.

Passed local gates: all-features70library+36integration+12example tests, plus
10rustdoc tests (5compile-fail;1ignored README fragment). Default34library+1doc,
JSON56library+3integration+1doc and Rust-only feature suites also pass. Formatting,
strict all-target/all-feature Clippy, Rust1.87 all-target/all-feature check and
local package verification73files pass. All19legacy source hashes still match;
default runtime dependency remains unicode-width0.2.2. No Volt/Glass edits,
vendor synchronization or Volt workspace/GPU revalidation were performed.

New regressions cover767/768/769 thresholds, fractional custom widths, scale and
resize-back transitions, highest-threshold/equal-threshold precedence,32-rule
boundary, invalid/inactive rules, directional setter replacement, scalar geometry
parity, nested intrinsic measurement, resolved-axis Flow/spacer allocation,
stateful callback preservation, keyed focus, cancel/outside release, padded /
rounded overlap hits, ordered glyph paint and decorative/disabled pass-through.
Native content tests verify no view-body calls on responsive resize/hover/render;
the normal dispatched-action path still recomposes exactly once.

Real-font example matrix:6width transitions ×5scales1/1.25/1.5/2/3 =30cases at760
logical height, with direction and topmost front dispatch/raster checks. An
additional two-size action test covers back/front callbacks, compact gap updates,
Reset enabling/disabling and focus clearing after disabling Reset. Narrow480x760,
wide960x760 and wide2x snapshots were generated; narrow/wide1x were inspected and
polished so the compact toggle/back-layer label remain visible. Invalid CLI zero
size is rejected before rendering. Snapshot/test evidence is headless only.

**Still unverified live:** new example OS pointer/key routing, actual window
resize across768, scale-factor transitions, close/reopen and other-platform
runtime. Prior Add clicks on rust_layout are not proof for this new example.
No scroll/grid/container-query/absolute-positioning/modal barrier/accessibility
completion claim. Small-height overflow can still shrink/clip controls. Follow
the [interactive checklist](RESPONSIVE_UI.md#try-and-test).
