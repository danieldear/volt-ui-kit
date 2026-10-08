# Native responsiveness: measured repair (2026-10-08)

## Reproduction and diagnosis

The first native demo was usable functionally but **not acceptably responsive**
under the documented default `cargo run` command. A button is not responsive just
because its callback test passes.

```text
Mouse/key event received on UI thread
            |
            +--> synchronous Rust callback -> publish bindings -> layout
            |
            +--> paint batches -> CPU raster -> pixel packing -> present
                                 ^
                                 |
                      old dominant bottleneck

While rendering runs, the same thread cannot handle the next queued event.
An event-driven host can still be slow; ControlFlow::Wait only avoids idle work.
```

Local baseline: Apple M4 Pro, arm64, macOS 27.0.1; fixed original demo document,
960x760 logical window, scale 2 (1920x1520 physical framebuffer), 4 warm-up
clicks followed by 30 measured clicks. Each iteration renders the pressed state,
then dispatches release, updates bindings, lays out, paints, rasterizes and packs
pixels. These CPU measurements exclude the OS event queue and native presentation.

| Stage | Original dev p50 / p95 (ms) | Profile-only p50 / p95 (ms) |
|---|---:|---:|
| Release dispatch + projection + layout | 7.261 / 7.537 | 0.248 / 0.346 |
| Paint batches | 0.026 / 0.031 | 0.008 / 0.010 |
| Rasterization | 678.774 / 684.576 | 37.034 / 39.676 |
| Pixel packing | 27.532 / 27.908 | 1.890 / 2.084 |
| Release-to-packed-pixels total | **713.439 / 719.807** | **39.198 / 41.941** |

The profile-only run changes no rendering source or document: it isolates the
large development-build optimization effect. It is not a release build. The
previous unoptimized raster occupied the input thread for roughly 0.68 seconds
per frame; the pressed frame can additionally delay handling the release event.
This supports the reported lag, but is not an exact measurement of the user's
physical click-to-visible-pixel delay.

## Repairs

- Local `profile.dev` optimizes the toolkit at level 1 and dependencies at level 3.
  Debugging symbols, assertions and overflow checks remain enabled. Consumer
  workspaces retain their own profiles; a library cannot override Volt's profile.
  This costs more initial compile time, rather than forcing users to discover
  `--release` to get a tolerable example. [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html).
- Native rounded fills use tested fast interior spans without changing edge AA.
- Native line/stroke batches replace repeated legacy border triangles only in the
  new software adapter. The legacy Painter and widget implementations are unchanged.
- Native surface dimensions are cached, resizing only when needed; this is not
  presented as the main macOS bottleneck (that backend only records dimensions).
- Unchanged layout uses validated cached bindings; changed bindings still validate.
- Font shaping keys include family, weight, italic, physical size and wrap width.
  Drawing and measurement use the same attributes. This is correctness, not a
  claim that font changes alone fix the lag.
- Optional native traces expose stage costs and handled release-to-present return.
  No timer, watcher or periodic idle repaint is added.

## Final measurements and native validation

Same original document, revised native adapter, scale2, N30: total p50/p95
**10.338/11.012 ms**; raster alone **8.252/9.092 ms**. Independent N60 rerun:
**10.368/10.956 ms**. The native default family now resolves to Avenir Next on
this Mac; the isolated profile-only run above retains the original font resolver.
This is around 69x faster CPU release-to-pixels than the original unoptimized run.

| Revised scene/workload | Physical pixels | N | Total p50 / p95 (ms) |
|---|---|---:|---:|
| Fixed original scene, button | 1920x1520 | 60 | 10.368 / 10.956 |
| New components, bound toggle | 1920x1520 | 60 | 9.356 / 9.876 |
| New components, bound checkbox | 960x760 | 60 | 3.015 / 3.235 |

Fast rounded fills retain the original cubic antialiasing in caps/edge strips,
using direct source-over spans only for fully covered interior pixels. Exact-pixel
oracle tests cover 36 combinations of scales1/1.5/2, alpha0/.18/.5/1 and fractional
origin/radius/clipping. This is not a speculative damage/frame cache.

An exploratory macOS native trace recorded20activating mouse releases:
handled-release-to-present-return p50 **16.0 ms**, p95 **18.653 ms**, max **36.05 ms**.
It includes initial changing text and native presentation; it is not click-to-photon
latency or a universal frame-time guarantee. UI checks confirmed10consecutive
button callbacks without loss, typed checkbox/toggle state updates, disabled
no-dispatch, keyboard focus/Space, live font/style F5 edits, invalid style retaining
the last valid UI, and recovery. Temporary reload edits used a scratch document,
not the canonical example. Real window resizing/display-DPI changes and other
platforms still need their separate runtime gates.

## Repeat measurements

```sh
cargo run --locked --features native --example render_benchmark -- --bench 60 --scale 2
cargo run --locked --features native --example components -- --bench 60 --scale 2 --bench-target notification_control
VOLT_UI_TRACE=1 cargo run --locked --features native --example complete_app
```

`render_benchmark` deliberately keeps the original scene for future comparisons;
it is not the updated visual showcase. `components` measures the new controls and
styling scene. Run on an otherwise idle machine, record dimensions/build profile,
and compare p50 and p95—not a single lucky frame. Do not benchmark concurrently
with compilation or other CPU-heavy tests.

Native tracing reports `mouse-release-to-present` beginning when the host handles
release, and ending when presentation returns. It **does not include** time spent
waiting in the OS event queue, compositor scan-out or display refresh. UI automation
round-trip time is also not a native rendering benchmark.

## Remaining boundaries

Software rendering still repaints the full scene. A GPU/damage-aware renderer,
platform accessibility and input editing remain separate work. Slow synchronous
application callbacks will still block input: enqueue expensive work outside the
UI callback, then publish resulting state. No Volt performance/GPU integration,
Linux/Windows native runtime, or frame-rate parity with Iced is asserted here.

## Rust-first composition measurements (2026-10-08)

Current development profile, same Apple M4 Pro/macOS host, scale2,
960x760 logical (1920x1520 physical),4warmups then60actions. Not measured during
compilation. These are the **new Rust scenes**, not the original workbench; they
cannot be used as an apples-to-apples speedup claim over that more complex scene.
Each release dispatches a real callback, calls the Rust body, builds/validates
its tree/closures, atomically installs them, lays out and repaints/packs pixels.

| Rust scene | Dispatch + compose + layout p50 / p95 (ms) | Release-to-packed-pixels p50 / p95 (ms) |
|---|---:|---:|
| Counter / increment | 0.049 / 0.057 | **2.771 / 3.040** |
| Settings / notifications toggle | 0.052 / 0.087 | **5.053 / 5.300** |
| Keyed list / reverse order | 0.031 / 0.038 | **4.336 / 4.595** |
| Settings / conditional details checkbox | 0.055 / 0.087 | **5.411 / 5.884** |

Source-of-state bodies do not run during hover/resize/render/cancelled releases;
unit tests use a body-call counter and dispatch outcomes to verify that contract.
Software repaint remains whole-scene, not GPU/damage-aware or fine-grained reactive
rendering. Text buffers/glyphs are cached in the shared native painter. The serif/
monospace showcase uses installed generic fonts (macOS prefers Georgia/Menlo),
not missing cosmic-text default families silently falling back to sans.

```sh
cargo run --locked --features native --example rust_counter -- --bench 60 --scale 2
cargo run --locked --features native --example rust_settings -- --bench 60 --scale 2 --bench-target notifications
VOLT_UI_TRACE=1 cargo run --locked --features native --example rust_counter
```

CPU figures exclude OS input queue/native presentation/display scan-out. A native
counter launch presented frames, but the Mac was locked during attempted GUI QA;
no activating Rust-host native event-to-present sample set was obtained. Live
Rust pointer/key/conditional-list/recovery validation remains pending after
unlock. Prior JSON-native trace samples above do not validate this new frontend.

## Stacks/placement scene (2026-10-08)

After selector/rounding/placement additions, same M4 Pro/macOS development profile,
1920x1520 physical atscale2;4warmups,60Add actions, no concurrent compilation.
33nodes include nine placement controls, nested stacks, full/numeric rounding,
divider and flexible spacers. This is a different scene, not a speedup/parity test.

| Stage | p50 (ms) | p95 (ms) |
|---|---:|---:|
| Dispatch + compose + layout | 0.124 | 0.180 |
| Paint commands | 0.005 | 0.007 |
| CPU raster | 4.300 | 4.616 |
| Pixel pack | 1.689 | 1.824 |
| Release-to-packed-pixels | **6.140** | **6.526** |

```sh
cargo run --locked --features native --example rust_layout -- --bench 60 --scale 2 --bench-target add
```

These warm CPU timings exclude OS event queue/presentation/scan-out. No allocation,
GPU, live platform-input, or comparison-to-Iced performance claim is made.

## Live Rust layout input sample (2026-10-08)

Local M4 Pro/macOS development build; current Rust `rust_layout`,960x760 logical,
1920x1520 framebuffer. CUA screenshots verified ten consecutive Add clicks changed
visible Rust state from0to10. The trace measures from **host handling mouse
release** through Rust callback/composition/layout and native present-return.

| Sample | Count | p50 (ms) | p95 (ms) | Min / Max (ms) |
|---|---:|---:|---:|---:|
| Initial Add clicks | 10 | 13.829 | 16.789 | 12.389 /16.789 |

Nearest-rank percentiles; this small sample is not a latency guarantee or a
comparison against other frameworks. It excludes time in the OS event queue,
compositor scan-out and click-to-photon latency. No GPU/FPS claim. A3-second
no-action observation showed no new trace lines while the Mac was locked; unlocked
idle behavior still needs its separate test.

The Mac locked before Reset/keyboard/resize/cancellation checks. Both attached
displays currently have2x rendered/logical ratios; a real1x↔2x scale transition
is unverified. This sample supersedes only the earlier missing Rust-host activating
mouse samples; it does not close the remaining native acceptance gates.
