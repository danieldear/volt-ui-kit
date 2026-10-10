# Component/runtime/Showcase verification

Verified 2026-10-09 on macOS, Apple M4 Pro / Metal. These are scoped source,
automated and native keyboard/visual checks—not production/platform certification.

## Delivered code

- `UiSender<M>` and additive native message launchers: queue 256, at most 64
  updates per native turn, coalesced wake, UI-thread state, close/full handling.
  Reentrant producers cannot starve macOS redraw by repeatedly rearming inside
  winit's `try_iter`; rearm happens in `AboutToWait`.
- Mounted/nested focus reveal, captured visible scrollbar dragging and opt-in
  multiline soft wrap. Drawing/caret/hit/selection use shared visual rows.
- Typed in-window action/context menus, tooltip/popover, progress/loading,
  fixed-row virtual list, sortable table, expandable tree and split pane.
- Showcase Activity/Explorer own new presentation and tests. Existing Lab is a
  legacy diagnostic fixture. SF/SF Mono and Atelier semantic colors/corners stay
  consistent; `.fill_width()` follows parent allocation, including virtual rows.
  Menu modifiers now reach the actual trigger, not just its wrapper.
- GPU Showcase defaults to a Blur request with 0.82 root smoke tint. Surface
  opacity does not fade text/controls. Default toolkit/Volt behavior remains opaque.

## Automated gates

| Check | Current result |
|---|---|
| Default unit/integration tests | 39 passed |
| Declarative-only unit/integration | 64 passed |
| Rust UI unit/integration | 230 passed |
| Native software, all targets | 385 passed, no ignored tests |
| Native GPU, all targets | 391 passed, 4 explicit ignores |
| Native GPU rustdoc | 27 passed, 1 existing ignored |
| Explicit Metal hardware tests | 4 passed |
| Strict GPU all-target and declarative-only Clippy | Passed, warnings denied |
| Rust 1.87 GPU all-target check | Passed |
| Format / diff whitespace | Passed |
| All-feature Cargo package verification | Assembled and compiled |
| Default runtime dependency tree | Only unicode-width 0.2.2 |

The four normal-suite ignores are opt-in GPU hardware checks: three renderer
checks and Northbound's retina-screen layer-budget check. All four were explicitly
run and passed on Metal. Showcase's launch
harness contains 27 passing tests, including the four new Showcase regressions;
the shared source is also exercised by the Lab harness.

```sh
cargo test --locked --features native-gpu --all-targets
cargo test --locked --features native-gpu --doc
cargo test --locked --features native-gpu --lib native::gpu_tests -- --ignored --nocapture
cargo test --locked --features native-gpu --example northbound -- --ignored --nocapture
cargo clippy --locked --features native-gpu --all-targets -- -D warnings
cargo +1.87.0 check --locked --features native-gpu --all-targets
cargo fmt --all -- --check
```

## Actual native Showcase checks

Using the built native GPU app on the unlocked Mac, not just snapshots:

- Activity displayed worker progress at 10% while the live callback incremented
  its counter. Worker completion displayed 100% and removed the spinner.
- Opened the action menu during progress. Down skipped the disabled service
  action; Enter cleared the counter and closed the menu.
- Opened the anchored inspector, Escape restored opener focus, then Tab revealed
  the offscreen editor. Cmd+A/paste edited and soft-wrapped an SF paragraph.
- Switched to Explorer and selected Study 00000; the detail surface reflected the
  selection. Final contrast-tuned glass preview was visually inspected.
- Both Midnight and Paper themes were visually inspected; Paper kept dark text
  and paired accent contrast. The app was returned to Midnight.
- All 236 native frames logged by the idle receipt used Vello/Metal, `software_frame=false`,
  `framebuffer_upload_bytes=0`. Native windows do not perform verification readback.

Pointer automation lost window focus, so physical right-click/trackpad/thumb
drag/divider drag and arbitrary window-resize feel are **not** certified here.
Their deterministic input/geometry/capture/removal tests pass separately.

## Resource evidence, with boundaries

Foreground Explorer idle, 20 seconds, no worker/spinner/editor caret:
zero new redraw traces; process CPU time 2.85 -> 2.87 seconds (`ps` 0.01s
resolution), about 0.1% of one core over that interval. RSS 125.75 -> 121.64 MiB.
This is process-resident memory, not total GPU allocation or compositor energy.

Release Activity, 1280x900 logical, 60 releases after four warmups:

| Rendering / scale | Release-to-pixels p50 | p95 |
|---|---:|---:|
| Software / 1x | 3.144 ms | 3.406 ms |
| GPU including readback / 1x | 8.328 ms | 9.373 ms |
| Software / 2x | 9.962 ms | 10.400 ms |
| GPU including readback / 2x | 11.819 ms | 14.365 ms |

GPU is not faster in this flat readback fixture. These timings include synthetic
release/whole-view composition/drawing/pixel packing, and GPU mode waits for
completion/readback. They exclude OS event queue/presentation and are not native
FPS, input-to-photon, utilization or energy. Dispatch+compose+layout p50 was
0.476–0.514ms. See [resource methods](RESOURCE_USAGE_AND_DESKTOP.md).

Final hardware stress: 24 alternating resize/virtual-window frames passed,
p50 4.788ms/p95 9.052ms including readback, maximum newly encoded CPU asset bytes
50,584 (not GPU memory). Mixed scene software/GPU RGBA MAE was 0.238 / 2.479 at
1x/2x. Deep nested scenes beyond Vello's spill capacity exercise a correctness
software fallback; do not describe every possible scene as GPU-only.

## Volt compatibility

An isolated copy of the actual public Volt 0.1.12 workspace was synchronized
using `scripts/sync_into_volt.py` and `cargo check --workspace --offline` passed.
Assets and embedded shell-integration scripts were included in that fixture.
The real Volt checkout/vendor snapshot was not modified. This is source/build
compatibility, not release terminal appearance/mouse/TUI/performance acceptance.

## Explicit remaining gates

- **Native materials / distribution:** pinned winit 0.30.13 macOS blur calls
  private `CGSSetWindowBackgroundBlurRadius`. The preview requests real window
  blur, but is not public `NSVisualEffectView` materials or App Store-safe native
  integration. Winit 0.31 beta adds the public path; this stable/MSRV-preserving
  milestone does not migrate to that beta. [Upstream release notes](https://github.com/rust-windowing/winit/releases).
- Backdrop alpha/blur contrast across arbitrary desktops, reduced-transparency
  settings and physical pointer/trackpad/drag acceptance.
- Foreign-language OS IME/candidate behavior, full UAX14/bidi visual-order editing.
- Platform accessibility semantic bridge and VoiceOver (painted controls are not
  yet exposed as complete native accessibility controls).
- Linux/Windows native runtime, device-loss stress, GPU memory/utilization/energy.
- Variable-height virtualization, direct unmounted-row focus jumps, richer
  spreadsheet semantics, subtree reconciliation and multi-window acceptance.
- Mobile and a desktop-shell/compositor prototype remain deferred.

No commit, push, original Volt checkout changes, memory updates or OS preference
changes were made in this milestone.

The local audit logs are under the ignored repository `work/` directory:
`showcase-final-tests.log`, `showcase-final-clippy3.log`, `showcase-final-msrv2.log`,
`showcase-final-doctests.log`, `showcase-final-hardware.log`,
`showcase-northbound-hardware.log`, `showcase-native-qa.log`,
`showcase-idle-receipt.json`, `showcase-activity-*-bench.log` and the isolated Volt
compatibility/package receipts. Fresh source/tests are authoritative.
