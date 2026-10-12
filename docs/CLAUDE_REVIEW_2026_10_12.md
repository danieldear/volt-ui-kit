# Component-parity delivery: review and push preparation

Reviewed on 2026-10-12 against `4bafad7` on `claude/gpu-a11y-damage`.
This covers the current tracked diff **and** the new component/Catalog files,
not just the older 2026-10-10 `HANDOFF_FOR_CODEX.md`. Claude's implementation
and design were retained; the corrections below were added during review.

## Assessment

The delivery is a substantial, coherent expansion of the Rust-first toolkit:
layout/style utilities, icons, dates/times, forms, disclosure/overlays, display
components/charts, and seven new Catalog pages. Runtime changes address scroll
and split-drag cost; GPU bands keep large admitted scenes on the GPU. The
implementation uses existing primitives rather than adding dependencies.

```text
Rust application state + typed callbacks
                 |
     composed Elements -> validated Document
                 |
       layout / input / accessibility
                 |
       retained shapes + shaped text
                 |
     +-----------+------------+
     |                        |
 Vello GPU, bounded       software raster,
 banded rendering        incremental damage
     |                        |
     +------- native host ----+
                 |
 Catalog: live examples, code, API tables
```

## Confirmed findings corrected before preparation

| Severity | Finding | Correction / evidence |
|---|---|---|
| P1 | Centered editable text was shaped using a 1,000,000px width, pushing ink/caret outside the field. | `src/declarative/runtime/editing.rs`: prepare start-aligned rows, then apply the field's alignment to both ink and caret/selection/IME geometry. Honor configured line height. Raster regression verifies visible centered ink and bounded caret; 24 existing editor tests pass. |
| P2 | A cached file-drag target could bypass a newly opened modal; binding-disabled targets could still receive files. | `src/declarative/runtime/drops.rs`: revalidate mounted, non-departing, enabled and active-scope boxes before dispatch; clear old hover state. Integration/modal and binding-disabled unit regressions. |
| P2 | Exclusive-create fallback for linkless filesystems lost the staged file's private Unix permissions. | `src/native/files.rs::commit_new`: use mode `0600`, preserving no-clobber creation. Regression checks contents, existing-target refusal and permission bits. Helpers remain explicitly non-sandbox/nonatomic across multiple processes. |
| P2 | New responsive width/font/visibility rules bypassed validation, including inactive rules, bad thresholds and rule-count limits. | `src/declarative/extras.rs`: apply existing `Responsive::validate` to every stored rule. `disabled_style` offsets now receive the same validation as other states. |
| P2 | Public date/time offsets and pagination's next-page target could overflow; number-input rounding could emit infinity. | `src/ui/dates.rs`, `display.rs`, `forms.rs`: bounded/checked or widened arithmetic, saturating pagination, finite numeric bounds and overflow-safe decimal rounding. Extreme-value regressions. |
| P2 | Form-field labels were visual only: unnamed fields could announce their current value instead of their purpose. | `src/ui/forms.rs`: use the form label as the control's accessible name when none was explicitly supplied; preserve explicit names. Accessibility snapshot regression. |
| P2 | Registered fonts could retain up to 4 GiB, then copy those bytes for each painter/window. | `src/native/raster.rs`: 256 MiB aggregate/64-file limit and shared immutable `Arc` bytes; parser and budget tests. Font registration remains a pre-window operation. |
| P3 | Static landing cards looked like actual frame-time/backend telemetry. | `examples/catalog/landing.rs`: label the time as sample data and the status as a native-rendering demo; clarify that software damage tracking, not all GPU rendering, limits repaint work. |

`tests/review_regressions.rs` contains eight additional integration/raster
regressions. Drop binding and font/file budget checks also live with their modules.

## Fresh validation

Receipts are local/ignored under `work/claude-review-2026-10-12/`.

| Gate | Result |
|---|---|
| `cargo test --locked --all-features --all-targets` | **592 passed, 0 failed, 16 ignored**, across 54 suites |
| All-feature doctests | **29 passed, 0 failed, 4 ignored** |
| Default test command | **43 passed**, including the default doctest; no failures |
| Strict all-feature/all-target Clippy, formatting, diff whitespace | Passed |
| Rust 1.87 all-feature/all-target check | Passed |
| Linux `x86_64-unknown-linux-gnu` all-feature/all-target check | Passed; compilation, not Linux runtime acceptance |
| Rustdoc with warnings denied | Passed |
| Real Apple M4 Pro/Metal library GPU suite | **11 passed**: alpha presentation, ink/clips/italic, images/admission, opacity/nesting, resize, top-edge borders, and 6K GPU strips. One of these is a timing probe, not a performance threshold. |
| Catalog explicit GPU/border tests and drag timing probe | **3 passed**; all pages remain admitted at 1x/2x |
| Existing Northbound GPU/incremental-hover checks | **2 passed**; retained as regression fixtures, not new demonstration work |
| Release Catalog build | Passed |
| Headless release Catalog Form inputs render | Inspected; centered numbers and component fills are visible. GPU readback snapshot, not native input/window blur proof. |
| Local `.app` bundling | Passed, including ad-hoc signature verification; not notarization |
| Cargo package build/verification | Passed with `--allow-dirty`; 222 files, packaged crate compilation verified, not a publishing action |
| Current RustSec audit | **0 vulnerability advisories**; informational unmaintained-dependency warnings below |

Native CUA interaction witnesses on the disposable Catalog app: calendar day
selection updated the due date; date-picker selection closed its popup and
updated the date; the number stepper changed 3 to 4; typed combobox search
filtered to Iced. These are targeted interaction/accessibility witnesses, not a
complete manual acceptance pass. External UI activity interrupted the attempted
native Enter check. Window-only screenshots did not consistently match the
bound accessibility tree, so they are not used to certify native glass/typography.

The Catalog drag probe measured approximately 7ms for Introduction and
0.8–1.8ms for other sampled pages on this Mac. This is a local layout/drag probe,
**not** total CPU/GPU utilization, presentation latency or an application-wide
performance guarantee. CPU layout/text shaping still exists; Vello generally
redraws the admitted scene, even when software rendering uses damage tracking.

## Compatibility and remaining acceptance

- `Cargo.toml`/`Cargo.lock`, legacy `Painter`, default features and the sole
  default runtime dependency (`unicode-width`) are unchanged. The crate still
  forbids unsafe code. New Rust extensions are skipped in schema-1 JSON.
- `Event`/`EventKind::FilesDropped` and additional prelude names can require
  changes to exhaustive matches/glob imports; upgrade notes are in the changelog.
- This is toolkit validation, **not** a new Volt vendored synchronization or
  terminal workspace/runtime acceptance run. No consumer checkout was modified.
- RustSec reports unmaintained **paste**, **rustybuzz** and **ttf-parser** via
  the existing Metal/font stacks (`RUSTSEC-2024-0436`, `RUSTSEC-2026-0206`,
  `RUSTSEC-2026-0192`). These are informational, not zero-risk assurances.
  Plan a dependency-stack upgrade separately while retaining the MSRV/renderer
  contracts; do not apply an untested broad update to this delivery.
- Public macOS Material remains the default. Private compositor blur is opt-in;
  the reported **Material -> Compositor external square-corner regression is
  not certified fixed** by this review or the headless GPU tests.
- Live VoiceOver, Windows/Linux runtime, older macOS deployment, real device
  loss and native filesystem/drop-provider behaviour need separate acceptance.
- Range sliders still need independently keyboard/screen-reader-addressable
  thumbs; combobox/palette dedicated Up/Down result navigation is incomplete.
  Toast timers, cascading submenus and interactive chart axes/tooltips are
  explicitly deferred in `PARITY_PLAN.md`.

## Push boundary

Prepare only reviewed toolkit source, examples, tests and documentation.
Do not include local QA bundles, logs, snapshots or unrelated generated apps.
The package remains **0.3.0 / Unreleased**; this review is not a version bump,
crates.io publication or notarized distribution. Passing these gates supports
committing the delivery with the limitations recorded, not declaring every
production/native acceptance item complete.
