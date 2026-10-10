# Security and performance hardening — 2026-10-10

Scope: canonical Volt UI Kit, after reviewing Claude's changes at `c54e421`.
This is a source/dependency review and regression-hardening batch, not a security
certification, cross-framework benchmark or completed platform accessibility audit.
No Volt checkout, UI Lab design, OS permissions or font distribution was changed.

## Plan and delivered changes

1. Capture a clean baseline, current dependency advisories and reproducible defects.
2. Add failing tests before fixing action validation, rendering and GPU admission.
3. Reduce bounded resource/staging work without changing legacy APIs or defaults.
4. Validate default/native/GPU builds, real Metal rendering, Showcase and Rust 1.87.
5. Record measured results and explicitly retain the acceptance gaps below.

```text
Assistive request
  -> current tree / bounded payload / role / enabled / modal scope checks
  -> focus, selection and callback (invalid requests are no-ops)

Native paint commands
  -> explicit image admission: unique handles + bytes + compatible atlas packing
       | reject                       | accept
       v                              v
  software fallback           protect needed cache hits
                              -> upload misses -> encode -> buffer-budget gate
                                   |                         |
                                   | safe                    | detected overflow
                                   v                         v
                               GPU drawing              software fallback
```

### Security and resource hardening

- AccessKit rejects foreign tree IDs, text over 8192 bytes and non-finite or
  unrepresentable numeric payloads before cloning/converting them.
- Runtime validates the current target's role, enabled/mounted state and modal
  scope **before** changing focus, editor selection or invoking a callback.
  Text input rejects read-only fields, oversized sanitized text (including tab
  expansion), and control-only replacements. Slider actions cannot target editors
  or buttons. Existing password withholding remains intact.
- Explicit GPU image sources are admitted before any registry upload. Unique
  per-frame source bytes must fit **96 MiB**, dimensions must fit the device and
  Vello's 8192 atlas side, and images must actually pack. Pixel area alone could
  admit incompatible full-width/full-height strips and silently lose one image.
- Admission uses the same pinned `guillotiere 0.6.2` allocator already present
  transitively through Vello. It adds **zero new locked packages**; the default
  toolkit still depends only on `unicode-width`. CPU-native does not enable it.
- Registry eviction now has a hard limit, not temporary over-allocation. All
  needed existing textures are protected up front to avoid eviction/re-upload
  churn when the next required image occurs later in draw order.
- A rejected frame has fresh fallback statistics, uploads no new image textures,
  and renders in software. Subsequent admitted frames can use the GPU normally.

These are raw registered image byte limits, **not total VRAM limits**. Framebuffer,
Vello atlas/buffers, driver allocations, fonts and application-owned CPU assets
are separate. This library does not sandbox application callbacks or filesystem
access. Assets are explicitly loaded by the host; existing PNG/JPEG decoding caps
remain 16 MiB encoded, 4096 per dimension and 128 MiB decoder allocation.

### Rendering correctness and performance

- Incremental repaint and GPU opacity-layer bounds include the default stroke's
  maximum miter reach plus anti-aliasing margin, for paths and batched lines.
  An acute 12px join no longer leaves 14 stale pixels after a colour change;
  regression tests retain partial repaint and compare it with a full repaint.
- GPU glyph opacity uses `brush_alpha` with an opaque foreground brush: bitmap
  emoji fades correctly and outline text is not attenuated twice.
- GPU clip elision requires actual outline-ink bounds, not advance width alone.
  Italic overhang is clipped; bitmap/colour faces without outline-only proof keep
  their clips. Normalized ink metadata is cached at at most 4096 face/glyph keys,
  avoiding repeated font-table parsing without rasterizing ordinary vector text.
- `ButtonVariant::Primary` now applies its paired `@primary-content` label colour,
  like `.primary(true)`, so inherited text colour cannot erase contrast.
- Opaque image upload staging borrows the existing pixel buffer instead of
  cloning it. Translucent image conversion remains byte-for-byte equivalent.
  In a local release helper measurement, a 2048x2048 opaque image (16 MiB),
  30 iterations, took **2.279ms -> 1.109ms** on average; the temporary staging copy
  fell from **16 MiB -> zero**. This measures CPU staging, not GPU upload time or
  whole-app latency. A unit test asserts borrowed buffer identity and alpha parity.

## Fresh dependency audit

`cargo audit --json`, advisory database commit
`7eebec69c352c7191b1f13eb95dd510eeca5d1de`, updated 2026-10-09:
**zero known vulnerabilities** in the lockfile. Five informational maintenance
warnings cover three package families:

| Package | Dependency route | Official advisory |
| --- | --- | --- |
| `paste 1.0.15` | Metal -> wgpu-hal, optional GPU/macOS | [RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436.html) |
| `rustybuzz 0.14.1` | cosmic-text, optional native | [RUSTSEC-2026-0206](https://rustsec.org/advisories/RUSTSEC-2026-0206.html) |
| `ttf-parser 0.20.0/0.21.1/0.25.1` | native font stack | [RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192.html) |

These advisories are **unmaintained**, not reported exploitable vulnerabilities;
they have no patched versions. They are not suppressed. Replacing/forking the
upstream native font/Metal stack needs a separate compatibility/MSRV/visual plan,
not a blind lockfile update. Audit results are time-sensitive and do not prove
unknown flaws absent; no fuzzing, penetration test or supply-chain attestation is
claimed by this batch.

## Measurement and verification receipts

Raw logs live in `work/security-performance-2026-10-10/`. The delivered review
copies the relevant logs alongside this report. Showcase remains the primary
polished acceptance app; no new primary examples or UI Lab features were added.

Benchmark: Apple M4 Pro / Metal, release build, Showcase Activity, 1280x900 logical
pixels, opaque background, `activity-counter`, 4 warmups and 60 measured actions.
Three serial before/after runs at each renderer/scale compare the archived
`c54e421` source to this batch. GPU measurements include blocking completion and
CPU readback; OS event queues/presentation are excluded. First overlapping runs
are diagnostic only and are not used for conclusions.

Median of each run's p50/p95, milliseconds (three runs per cell):

| Renderer / scale | Before p50 / p95 | After p50 / p95 |
| --- | ---: | ---: |
| software / 1x | 2.729 / 2.784 | 2.736 / 2.830 |
| software / 2x | 8.739 / 9.072 | 8.783 / 9.018 |
| gpu / 1x | 3.622 / 5.031 | 3.749 / 3.890 |
| gpu / 2x | 8.471 / 8.792 | 8.541 / 8.996 |

Software changes are small; GPU 1x p50 is **0.127ms higher** in this sample,
GPU 2x **0.070ms higher**. This safety/metadata work is not a demonstrated
whole-app speedup. Three local runs are not statistical or cross-device proof.

Final validation:

| Gate | Result |
| --- | --- |
| Default / declarative (including doctest) | 40 / 65 passed, one ignored doctest each |
| Native software all targets | 399 passed, one timing test ignored |
| Native GPU all targets | 408 passed, ten explicit acceptance/timing tests ignored |
| GPU doctests | 28 passed, one ignored |
| Explicit GPU-library acceptance + staging timing | 8 passed (7 hardware, 1 CPU timing) |
| Explicit Northbound GPU + incremental timing | 2 passed |
| Strict all-target Clippy, fmt/diff, Rust 1.87 all-feature/all-target check | Passed |
| Warning-free rustdoc, all-feature package verification | Passed (161 packaged files) |
| Isolated public Volt 0.1.12 workspace compile + snapshot check | Passed; real checkout untouched |
| Final adversarial probe rerun | Miter: zero pixel differences; emoji alpha: 51; italic outside pixels: zero; primary variants: matching contrast; all image centres present |

Counts overlap between feature configurations and include existing example
tests; they must not be summed into a count of unique tests. Package metadata
(homepage/repository) and non-root profile notices remain non-fatal warnings.
Do not equate these frame costs with CPU/GPU utilization, native FPS, energy,
thermal load or comparisons against Iced/COSMIC.

Reproduce:

```sh
cargo audit
cargo test --locked --features native-gpu --all-targets
cargo test --locked --features native-gpu --lib -- --ignored --nocapture
cargo clippy --locked --features native-gpu --all-targets -- -D warnings
cargo +1.87.0 check --locked --all-features --all-targets
VOLT_UI_RENDERER=gpu cargo run --release --locked --features native-gpu --example showcase -- --screen activity --background opaque --bench 60 --bench-target activity-counter --scale 2
```

## Still pending / outside this batch

- Accessibility scrolling/focus reveal and composite semantic names/selected
  states from the Claude review are **not** completed by action hardening.
  Live VoiceOver/Narrator/Orca acceptance remains separate.
- Explicit source-image packing is preflighted. Vello's late-bound colour-glyph
  image generation is not completely packing-preflighted; dense mixed emoji/image
  scenes and approximation-based fixed-buffer estimates remain GPU stress risks.
- Real window input-to-present latency, CPU/GPU utilization, energy, memory under
  sustained image churn, device-loss recovery and Linux/Windows runtime acceptance
  remain profiling/platform gates. Headless Metal tests do not satisfy those gates.
- The isolated public Volt compile is a compatibility check, **not** an actual
  vendor sync, visual/performance test or release approval.

Recommended next batch: complete AX semantics/scrolling, then profile Showcase
native idle/interaction/animation/scroll/image churn and validate the remaining
GPU/platform risks before making production performance claims.
