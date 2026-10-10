# Resource usage and a later desktop-style experiment

Measured/source-checked **2026-10-09**, Apple M4 Pro (14 logical CPUs),
24 GiB RAM, arm64, macOS 27.0.1. These results are local observations, not a
cross-framework benchmark or a promise for another device.

## What runs now

```text
Rust declarations / callbacks / layout / font shaping       CPU
                          |
                  ordered paint commands
                          |
             +------------+-------------+
             |                          |
       native software            native-gpu (auto/gpu)
       tiny-skia RGBA             Vello GPU scene drawing
       RGB + softbuffer           paths/textures/clips/effects
             |                          |
             +------------+-------------+
                          |
                  OS window compositor
```

The default embedded toolkit allocates no GPU device/window/event loop and adds
no idle timer. Its **host chooses the renderer and scheduling**. Volt's adapter
remains separate; these native showcase numbers do not measure Volt.

`native-gpu` now uses Vello 0.6/wgpu 26 for **opaque and alpha windows**; the earlier
CPU-framebuffer texture presenter is replaced. CPU still handles composition,
layout/shaping, scene encoding and cached glyph bitmap misses, but not full-window
rasterization. Native GPU presentation never performs CPU readback. Use
`VOLT_UI_RENDERER=software` for the software comparison or `gpu` to require GPU
success. See [GPU drawing and dependency policy](GPU_RENDERING.md).

Source anchors: `src/native/window.rs` (selection/fallback/scheduling),
`src/native/gpu_scene.rs` (GPU scene/resources), `src/native/gpu.rs` (GPU rendering/
surface/explicit readback), `examples/support/rust_ui.rs` (benchmark stages).

## GPU versus software: fresh headless comparison (2026-10-09)

Same GPU-feature binary, scene/action/size and optimized development profile;
4 warmups, 60 measured release actions. GPU mode **waits for completion and copies
pixels back to CPU**, converts alpha and packs RGB. Software renders/packs locally.
These boundaries are comparable for producing readable pixels, **not native FPS**.

| Scene / physical size | Renderer | Draw + readback p50 / p95 ms | Release-to-pixels total p50 / p95 ms |
| --- | --- | ---: | ---: |
| Studio, 2560x1800, 2x | Vello GPU | 10.541 / 15.729 | **14.357 / 19.650** |
| Studio, 2560x1800, 2x | Software | 19.872 / 20.191 (raster only) | **23.413 / 23.762** |
| Desk, 1360x900, 1x | Vello GPU | 8.269 / 11.750 | **10.175 / 13.915** |
| Desk, 1360x900, 1x | Software | 3.015 / 3.073 (raster only) | **4.457 / 4.587** |

Studio benefits in this experiment; the smaller Desk scene is **slower** with
blocking GPU readback. Native windows omit readback, but their real frame latency,
GPU execution/utilization and energy still require separate measurement. Do not
claim universal speedups, reduced CPU/GPU percentages or guaranteed 60 FPS.
Receipts: `work/vello-bench-{studio,desk}-{gpu,software}.log`.

### GPU foreground idle sample (2026-10-09)

Final GPU binary, UI Lab Controls/Light at **1920x1520 physical / 2x**, after
editing/submission/theme changes: over **30.008 seconds**, process CPU time stayed
at 0.44s (**0.00s increase at 0.01s `ps` precision**), no new trace frames/lines,
and RSS stayed **126.53 MiB**. The intended window was inspected before/after.
This supports event-driven idle for this sample; it does not measure total GPU
memory, energy/utilization or universal zero CPU. Receipt: `work/vello-native-idle.json`.

## Earlier software active-frame baseline (2026-10-09)

Optimized **development** profile: local code opt-level 1, dependencies opt-level 3.
Each run has 4 warm-up actions and 60 measured actions. A pressed frame is also
rendered; the reported total below is the **release** callback through packed
pixels, not the sum of both frames. The counter/history changes exercise actual
recomposition and text shaping. No native OS event queue or presentation is timed.

| Scene / physical framebuffer | Compose + layout p50 / p95 ms | Raster p50 / p95 ms | Total p50 / p95 ms |
| --- | ---: | ---: | ---: |
| Studio, 1280×900, 1x | 1.074 / 1.249 | 7.253 / 7.678 | **9.267 / 9.685** |
| Studio, 2560×1800, 2x | 1.147 / 1.401 | 25.399 / 26.159 | **29.739 / 30.574** |
| Desk, 1360×900, 1x | 1.021 / 1.151 | 3.774 / 3.986 | **5.714 / 5.935** |

Studio uses 1280×900 **logical** pixels in both runs; 2x draws four times as many
physical pixels. The raster stage dominates. Studio at Retina scale still exceeds a
60 Hz frame budget (16.67 ms); 1x CPU-only costs are lower in this snapshot. This
does not measure real FPS or input-to-photon latency. It is **not defensible to
promise universal 60 FPS** merely because callbacks are fast. Source/render changes
and device conditions differ from the October8 baseline, so do not attribute the
improvement to this component batch alone. Receipts: `work/full-components-perf-*.log`.

These are frame costs, **not CPU utilization percentages**. Utilization depends
on how often frames are requested, active animation, input rate, window visibility,
scene size and the renderer. One software-rendering thread can saturate one core
while a machine-wide CPU percentage appears small. At rest, the native event loop
uses `Wait`; bounded animations temporarily schedule `WaitUntil` deadlines. Source
inspection supports “no continuous idle repaint,” not a measured zero-CPU claim.

### Earlier software foreground idle measurement (2026-10-09)

After unlocking, UI Lab Controls was visibly foreground at **1920×1520 physical
pixels, 2x scale**. Following input/popup/modal activity and settling, a **30.012s**
process sample found **0.00s CPU-time increase at `ps` 0.01s reporting precision**;
RSS stayed **122.83 MiB**. This is a single process/scene sample, not a promise of
zero CPU on every machine, and RSS is not total OS/compositor/GPU memory.
Screenshots verified the intended window before and after the interval. Raw receipt:
`work/full-components-idle.json`.

Native checks now verify keyboard typing/submit, clipboard Unicode paste, atomic
emoji deletion and undo, slider/popup input, modal Escape/focus restoration and
blocked background wheel input. Long text areas scroll their clipped inner
viewport without moving the page; Desk sends actual edited text by click and
Cmd+Enter (Ctrl+Enter on other hosts is covered by runtime tests).
Before the Vello change, the GPU-presented transparent example also launched
and repainted after Increment;
its process map contains Metal and the AGX Metal driver resource. This is smoke
validation, **not** GPU utilization, energy, input-to-photon latency or a complete
macOS/Linux/Windows alpha/blur acceptance test. Those measurements remain open.

Former presenter upload arithmetic, **not measured throughput**: a 2560×1800 RGBA texture is
18,432,000 bytes (~17.58 MiB). Uploading one every frame at a hypothetical 60 Hz
would move ~1.106 GB/s of raw pixel data before other work. This is not a claim
that this scene reaches 60 Hz or that the actual device consumes that bandwidth.
This full-frame CPU upload is removed from the new native GPU path.
Group opacity still needs full-frame software layers in the software adapter;
scene caching and bounded compositing are important before rich always-visible UI.

## Reproduce and complete the measurement

```sh
cargo run --locked --features native --example showcase -- --screen studio --bench 60 --bench-target studio-add --size 1280x900 --scale 1
cargo run --locked --features native --example showcase -- --screen studio --bench 60 --bench-target studio-add --size 1280x900 --scale 2
cargo run --locked --features native --example desk -- --bench 60 --bench-target desk-send --size 1360x900 --scale 1
VOLT_UI_TRACE=1 cargo run --locked --features native --example ui_lab
```

On an unlocked target machine, measure separate scenarios:

1. Foreground idle after animations settle (30–60 seconds): process CPU time delta,
   resident memory, frame count and energy; record display scale/window size.
2. Repeated clicks and wheel scrolling: handler-to-present traces, native event
   delay, frame p50/p95/p99, process CPU and memory growth.
3. Active glow/fade/opacity versus reduced motion, and small versus Retina windows.
4. Software versus Vello GPU drawing: GPU execution/transfer timings and OS GPU
   utilization using supported platform tools. Do not infer GPU% from CPU traces.
5. Equivalent scene/build/backend/device settings before comparing Iced/libcosmic.

This evidence should gate performance claims, not just compilation or snapshots.

## What to optimize before a lightweight shell

1. Profile the dominant raster operations with a reproducible rich-screen workload.
2. Add validated damage/cached-surface rendering so unchanged regions are not
   rebuilt/rasterized/uploaded. Keep clip, overlap, effects and hit maps correct.
3. Bound opacity/effect intermediate surfaces to the required region; retain cache
   budgets and translucent overlap correctness.
4. Profile the delivered **Vello GPU drawing adapter**, especially layer/clip-heavy
   scenes and glyph/image atlas memory. Reuse host devices when adding embedded
   adapters; native measurements do not measure Volt's existing renderer.
5. Keep event-driven idle behavior; constrain animations and virtualize large lists.

## COSMIC comparison and deferred experiment

COSMIC separates its application/widget toolkit from desktop window management:
[libcosmic](https://github.com/pop-os/libcosmic) is based on Iced;
[cosmic-comp's manifest](https://github.com/pop-os/cosmic-comp/blob/master/Cargo.toml)
uses Smithay for its compositor. Current primary sources were checked for this
distinction; it is not a claim that we match COSMIC's maturity or performance.

We can later build an original **desktop-style demo** using this toolkit:

```text
+-------------------------------------------------------------+
| Volt desktop preview       Workspaces       Status / Clock  |
+-------------------------------------------------------------+
|                                                             |
|     +------------------------+   +---------------------+    |
|     | Launcher               |   | Appearance          |    |
|     | Local demo applications|   | Light / Dark / Accent|    |
|     | Filter / select        |   | Density / Motion    |    |
|     +------------------------+   +---------------------+    |
|                                                             |
|         +----------------------------------------+          |
|         | Dock / pinned demo apps / notifications|          |
|         +----------------------------------------+          |
+-------------------------------------------------------------+
```

Start as one normal window, optionally borderless, with shared theme tokens,
launcher, panel, dock, notifications and a settings pane. Keep effects optional;
launching real applications/settings writes must be explicit later integrations.
It can demonstrate coherent OS-like look/feel without taking over the user's OS.

```text
Widget design system      -> already the foundation; keep improving
Desktop-style demo        -> feasible later, confined to its own window
Linux desktop integration -> separate platform-specific adapters and tests
Real Wayland compositor  -> separate large project, not a demo toggle
```

A fullscreen client app cannot manage other applications' windows, replace the
desktop session, implement Wayland protocols or provide secure screen locking.
Those require compositor/window-manager, input, display, shell/session and
security infrastructure. The simple experiment is **explicitly deferred**, as
requested; no shell/compositor code or OS configuration changes are made here.

Mobile ports also remain deferred. Keeping the declaration/state layer portable
helps, but touch input, platform lifecycle, text/IME, accessibility, keyboard and
distribution are real work, not supplied by `TabPlacement::Bottom`.

## GPU support: exact answer

**Yes: optional real GPU drawing now, not only texture presentation.**
See [GPU rendering](GPU_RENDERING.md) for the full pipeline, feature-pruned
Metal/DX12/Vulkan dependency policy, Rust 1.87 pin and hardware readback tests.
The macOS GPU normal graph adds 18 packages versus the previous presenter;
pruning avoids unused features, not a claimed reduction in macOS crate count.

```sh
VOLT_UI_RENDERER=gpu cargo run --locked --features native-gpu --example ui_lab
VOLT_UI_RENDERER=software cargo run --locked --features native-gpu --example ui_lab
```

Native widgets share the renderer rather than allocating separate GPU contexts.
Event-driven idle behavior is retained. Scene/damage retention, GPU memory/energy
profiling and broad platform acceptance remain open. The desktop-style demo,
mobile host and real compositor remain explicitly deferred.

## Showcase Activity / component milestone (2026-10-09)

New component visual acceptance now lives in Showcase, not Lab. On Apple M4 Pro /
Metal, release build, 1280x900 logical, 60 counter releases after four warmups:

| Mode | scale | release-to-pixels p50 / p95 | dispatch + compose + layout p50 / p95 |
|---|---|---|---|
| Software | 1x | 3.144 / 3.406 ms | 0.487 / 0.559 ms |
| GPU + readback | 1x | 8.328 / 9.373 ms | 0.476 / 0.535 ms |
| Software | 2x | 9.962 / 10.400 ms | 0.493 / 0.548 ms |
| GPU + readback | 2x | 11.819 / 14.365 ms | 0.514 / 0.581 ms |

GPU is **not faster in this flat Activity readback fixture**. Native presentation
does not read back pixels, so these are not native latency/FPS/utilization or
energy comparisons. The native trace's submit/present timing does not measure
completed GPU work or input-to-photon latency. Resource receipts must not become
a blanket claim that GPU is faster or more energy-efficient.

```sh
VOLT_UI_RENDERER=software cargo run --release --locked --features native-gpu --example showcase -- --screen activity --background opaque --bench 60 --bench-target activity-counter --scale 2
VOLT_UI_RENDERER=gpu cargo run --release --locked --features native-gpu --example showcase -- --screen activity --background opaque --bench 60 --bench-target activity-counter --scale 2
```

A final 20-second foreground-idle native Explorer sample (no worker/spinner/editor
caret) added **zero render frames**, process CPU time 2.85 -> 2.87 seconds
(`ps` 0.01s resolution; about 0.1% of one CPU core over the interval). RSS was
125.75 -> 121.64 MiB. This is one process sample, not a GPU allocation/energy
measurement or a guarantee for background desktop/compositor processes.
All 236 native frames logged by that idle receipt reported Metal, no CPU frame and zero framebuffer
upload. Background messages intentionally wake/compose while work is active.

Final hardware stress passed 24 alternating resize/virtual-window frames:
p50 4.788ms/p95 9.052ms **including readback**; max newly encoded CPU asset bytes
50,584, which is not GPU memory. Full receipt and limitations:
[application/runtime verification](APP_RUNTIME_VERIFICATION.md).
