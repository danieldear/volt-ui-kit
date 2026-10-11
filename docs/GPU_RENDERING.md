# Native GPU drawing and dependency policy

Source/runtime checked **2026-10-09**, image admission/text hardening **2026-10-10**.
The optional `native-gpu` host now draws
shapes, paths, image/glyph textures, clips, group opacity and shadows with
**Vello 0.6 / wgpu 26**. It no longer software-rasterizes the full window and
uploads that framebuffer for admitted GPU scenes. Detected over-budget frames
deliberately use software fallback; opaque windows use the GPU path too.
See [security/performance hardening](SECURITY_AND_PERFORMANCE.md) for the hard
image registry limit, packing admission, fresh measurements and remaining gaps.

## The boundary

```text
Rust declarations + state + callbacks
                  |
                  v
CPU: validation -> layout -> cosmic-text shaping -> paint commands
                  |                         |
                  |              cached CPU glyph bitmaps (on miss)
                  v                         v
GPU: Vello scene + cached image/glyph resources
     paths / fills / strokes / nested clips / opacity / analytic shadows
                  |
                  v
GPU render texture -> alpha-mode conversion -> wgpu surface -> OS compositor
```

`winit` owns windows/input; it is not a renderer. [Vello](https://docs.rs/vello/0.6.0/vello/)
is a GPU compute 2D renderer. Native drawing sets `use_cpu: false`. CPU work still
includes callbacks, whole-view composition, layout, font shaping, image decoding,
scene encoding and cached glyph bitmap construction. This is not a GPU-only app.

There is one renderer/device per native window, not one per component. The
renderer-independent default toolkit and Volt's existing host adapter do not
create a new GPU context. No Volt checkout/vendor synchronization is performed.

## Run existing apps without changing declarations

```sh
VOLT_UI_RENDERER=gpu cargo run --locked --features native-gpu --example ui_lab
VOLT_UI_RENDERER=gpu cargo run --locked --features native-gpu --example showcase -- --screen studio
VOLT_UI_RENDERER=gpu cargo run --locked --features native-gpu --example desk
VOLT_UI_RENDERER=gpu cargo run --locked --features native-gpu --example northbound

# Compare the same compiled app with software drawing:
VOLT_UI_RENDERER=software cargo run --locked --features native-gpu --example ui_lab

# GPU-capable alpha window; OS blur is a separate platform capability:
VOLT_UI_RENDERER=gpu cargo run --locked --features native-gpu --example ui_lab -- --section window --background blur --window-opacity 0.65 --chrome overlay
```

| `VOLT_UI_RENDERER` | Behavior |
| --- | --- |
| unset / `auto` | GPU if compiled/available; initialization or drawing errors log opaque software fallback |
| `gpu` | Require `native-gpu` and successful GPU drawing; return errors rather than silently use software |
| `software` | Use tiny-skia/softbuffer even in a GPU-enabled binary; no window alpha/blur guarantee |

Startup logs the selected GPU adapter/backend. Resize recreates targets; surface
loss/outdating retries, while other failures follow the policy above. The native
window path never reads the rendered image back to CPU. Idle redraw stays
event-driven; bounded animation may schedule temporary deadlines.

## Feature-pruned dependencies

- Default library: **only `unicode-width`** as a direct dependency, unchanged.
- `native`: software host/font/image/clipboard dependencies, unchanged.
- `native-gpu`: optional Vello/wgpu/pollster, reusing the same native host.
- Vello defaults disabled; only its `wgpu` feature is enabled.
- wgpu defaults disabled; `std`/`wgsl` plus **Metal on macOS, DX12 on Windows,
  Vulkan on other targets**. No GLES/WebGPU backend in this feature policy.

Vello is pinned to `=0.6.0`, wgpu to `=26.0.1` to retain Rust **1.87** compatibility.
The latest Vello release has a newer minimum Rust requirement; see its
[upstream changelog](https://github.com/linebender/vello/blob/main/research/CHANGELOG.md).
This is a deliberate compatibility pin, not a claim to use the newest release.

**Pruned features do not mean fewer macOS crates than the former presenter.**
The measured macOS normal dependency graph has **151 package/version pairs**,
versus **133** for the previous wgpu-30 texture presenter (root included).
Vello's general-purpose drawing adds 18 packages. Before/after Vello pruning both
have 151 on macOS because the unused platforms were not compiled here anyway.
The savings are unwanted features/backend scope; no binary-size/build-time
reduction is claimed without equivalent measurements. Reproduce with `cargo tree
--locked --features native-gpu -e normal --prefix none` and `cargo tree -e features`.

## Correctness and resource boundaries

- Paint batches preserve ordered scopes across batches. Independent corners,
  fills/strokes, text clipping and rounded/circle clips share physical geometry.
- Path clipping honors the public **even-odd** contract with a GPU destination-in
  mask; same-winding holes remain empty. Group opacity composites overlapping
  siblings once, including inside clips.
- Shadow/glow uses analytic GPU Gaussian-like rounded-rect evaluation. Selected
  corners are evaluated by quadrants. This differs from the software three-box
  approximation and is not guaranteed pixel-identical for extreme blur/corners.
- Image fit/position/tile/opacity are supported. GPU image sampling differs from
  software bicubic scaling; antialiasing also differs. Font selection/shaping and
  editor geometry are shared, but not every final pixel is identical.
- Vello's straight-alpha target is adapted to the surface alpha mode on GPU.
  Explicit snapshots convert readback pixels to premultiplied RGBA for tiny-skia.
- Adapter-side image cache: **64 MiB**; glyph cache: **16 MiB / 4096 entries**;
  shared Swash cache: **16 MiB / 4096 entries**. Framebuffer: **64 megapixels**;
  explicit opacity depth: **24**, clip depth: **48**. Invalid streams return errors.
- These are adapter limits, **not a bound on total GPU memory**. Vello's atlas,
  intermediate surfaces, command buffers and OS compositor memory are additional.

`VOLT_UI_GPU` diagnostics report newly constructed cached resource bytes and
`framebuffer_upload_bytes=0` for admitted GPU scenes; native software fallback
reports its actual RGBA upload payload. Neither is an instrumented GPU bandwidth counter:
Vello still transfers commands/assets. `VOLT_UI_TRACE` reports CPU-side
`gpu_encode_submit_present_ms`, not GPU completion time or display scanout.

## Validation / performance

The hardware acceptance test is opt-in so ordinary tests do not require a GPU:

```sh
cargo test --locked --features native-gpu --lib native::gpu_tests::actual_gpu_readback -- --ignored --nocapture

VOLT_UI_RENDERER=gpu cargo run --locked --features native-gpu --example showcase -- --screen studio --snapshot target/gpu-studio.png --size 1280x900 --scale 2
VOLT_UI_RENDERER=gpu cargo run --locked --features native-gpu --example showcase -- --screen studio --bench 60 --bench-target studio-add --size 1280x900 --scale 2
```

On Apple M4 Pro / Metal, actual GPU readback passed shapes/text/images, nested
clips, cross-batch opacity, transparent path holes, cached-resource reuse and
shadow symmetry. The 1x/2x mixed-scene mean absolute RGBA error versus software
was **0.238 / 2.479** on a 0..255 scale. GPU/software Studio Retina snapshots were
visually inspected. This is acceptance for these fixtures, not all possible scenes.

GPU snapshot/benchmark mode uses a persistent renderer and **waits for GPU
completion + CPU readback + alpha conversion**. Its timings include this overhead;
native windows do not pay readback. It excludes OS input queues/presentation, and
is not a measurement of FPS, GPU utilization, energy or input-to-photon latency.
See [resource measurements](RESOURCE_USAGE_AND_DESKTOP.md) for comparisons.

Native UI Lab GPU smoke checks on the unlocked Mac also verified typing/Enter,
button/tab callbacks, slider/popup selection, modal Escape/opener focus restoration,
scrolling, glow, image/vector clipping and a circular clipped click target.
The alpha/blur-request example also launched on Metal and repainted after Increment;
this smoke check does not establish comprehensive desktop-through blur quality.
A 30-second final GPU foreground sample showed no new redraw traces or process CPU
time at `ps` 0.01s precision, with stable 126.53 MiB RSS; see the resource guide.
The earlier GPU delivery baseline all-target suite passed **318 tests**, with this hardware test ignored by default
and then run explicitly (one pass); **22 doctests**, strict Clippy and Rust 1.87
all-target checks passed. Default/declarative/software tests and package verification
also passed. These are scoped checks, not production certification.

Windows/Linux native runtime, device loss under real stress, OS alpha/blur,
GPU memory/energy/utilization, accessibility and mobile remain separate gates.

### Current component/Showcase acceptance (2026-10-09)

Latest source: **391** native-GPU all-target tests passed (four explicit ignores),
**27** rustdoc passes, strict Clippy and Rust 1.87 all-target checks passed. All four
hardware checks (three renderer tests and Northbound's retina-screen layer-budget
test) were run explicitly and passed. Final native Showcase validated jobs/input/menus/popover/wrapped editor and
row selection on Metal. The 20-second idle sample added zero frames, CPU+.02s;
see [the complete receipt and external gates](APP_RUNTIME_VERIFICATION.md).

GPU Showcase defaults to `--background blur --window-opacity 0.82`; `--background
opaque` restores a solid window. As of 2026-10-11 decorated macOS GPU windows request public AppKit material,
not winit's private CGS blur call. Borderless requests retain transparency only.
Shipping/signing and dependency review still gate distribution; no App Store
certification or ChatGPT/Codex appearance parity is claimed.

## 0.3.0 large-frame safety follow-up (2026-10-11)

The pinned Vello coarse shader reserves 64 command words for every 16x16
viewport tile, even empty ones. The old estimate counted two; at 8K, the base
reservation alone nearly fills its 2^23-word buffer. Admission now counts that
reservation and rounded-up 256-word spill allocations per tile, with headroom.
This is conservative: complex large scenes can fall back to software earlier.
The 64 MP cap allows large framebuffers, not all GPU scenes or opacity layers.
CPU group opacity still has a separate 64 MiB aggregate buffer budget and may
reject a large animated scene. Tiled GPU rendering and large-display driver
acceptance remain follow-ups, not delivered guarantees. See
[the review receipt](CLAUDE_REVIEW_2026_10_11.md).
