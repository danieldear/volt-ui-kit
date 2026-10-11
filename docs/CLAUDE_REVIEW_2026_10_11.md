# Claude changes: review and v0.3.0 delivery receipt

Reviewed 2026-10-11 against `419725f` on `claude/gpu-a11y-damage`.
The handoff was useful context, not validation evidence. Source, targeted
regressions, fresh dependency advisories and native runs were checked again.
This is a toolkit milestone, **not completion of the application roadmap**.

## Changes retained

- Relay: an offline, scripted coding-agent UI with threads, streaming transcript,
  cancellation/retry, composer menus and inspector. It does not run commands,
  edit files or connect to a real model; permission/model controls are mock UI.
- Catalog's frosted full-height sidebar and independent solid documentation
  sheet, live tint controls and opaque alternative. No mandatory app layout or
  blur placement is introduced. New component demonstrations stay in Catalog;
  UI Lab and older Showcase code are unchanged.
- Additive Rust-only follow-end scrolling and Enter-to-submit text areas;
  Shift+Enter inserts a newline. JSON schema 1 remains unchanged.
- Accent-aware checked-control contrast and exact axis-aligned GPU line fills.
- Larger bounded framebuffers and nonfatal omission of oversized decorative
  software shadows, counted in `effects_skipped`.

## Severity-ranked findings and fixes

| Severity | Finding | Resolution / source |
|---|---|---|
| P1, fixed | GPU command-buffer admission reserved only two words per viewport tile; pinned Vello 0.6 reserves 64 even for empty tiles. Raising the framebuffer cap exposed this undercount. | `src/native/gpu_scene.rs:83-92,272-305`: account for fixed reservation and per-tile 256-word spill allocation chunks, retaining conservative headroom. An old-code 8K admission regression failed, then passed with the fix. Over-budget scenes explicitly fall back. |
| P1, fixed | Unbounded Relay history could exceed the validated document's node limit and terminate the host after repeated use. | `examples/relay/mod.rs:148-167,288-300,332-367,518-543`: bound history, prompts, replies and worker metadata; preserve oversized drafts, explicitly label demo history trimming, and saturate counts. `examples/relay/view.rs:831-904` bounds rich-text expansion. Long-session and UTF-8 regressions pass. |
| P2, fixed | Native fallback uploaded a full RGBA framebuffer while GPU diagnostics reported zero upload bytes. | `src/native/gpu.rs:485-505,563`: report actual fallback payload; admitted vector-rendered frames still report zero framebuffer upload. Offscreen CPU-return paths do not falsely report GPU uploads. |
| P2, fixed | Repeated transient surface errors scheduled immediate redraws. | `src/native/window.rs:580-604,1101-1104`: event-loop retry deadlines with 16–256ms exponential backoff; successful presentation resets the timer. Deterministic retry/idle test passes. This cannot prevent a blocking driver call. |
| P2, improved | macOS blur depended on winit's private blur request. | `src/native/macos_material.rs`: public, retained `NSVisualEffectView` below the unchanged winit content view, using a safe shared-ancestor lookup. macOS host never requests winit blur. No unsafe code, view reparenting, delegate replacement or native-button resizing. |
| P2, open runtime gate | Earlier large-display animated zoom stalled in `Surface::configure`/Metal wait, followed by a queue-drop timeout. | Current smaller native zoom/restore passed; repeated 6K snapshots passed. The original larger-display/driver case and stress device loss are **not certified fixed**. Keep a separate reproduction/acceptance gate before claiming production GPU resize reliability. |
| P3, dependency maintenance | Refreshed RustSec audit reports five unmaintained-package warnings, but no vulnerability errors. | `paste 1.0.15` ([RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436)); `rustybuzz 0.14.1` ([RUSTSEC-2026-0206](https://rustsec.org/advisories/RUSTSEC-2026-0206)); three `ttf-parser` versions ([RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192)). Plan dependency migration with font fidelity and Rust 1.87 gates; do not hide warnings or upgrade the rendering stack blindly. |

## Validation receipts

All commands below passed on the test Mac. Logs, screenshots and disposable QA
bundles are local under ignored `work/claude-review-2026-10-11/`, not packaged
release assets. Counts include repeated coverage across targets, not a claim of
460 distinct user interactions.

| Gate | Result |
|---|---|
| `cargo test --locked --all-features --all-targets` | 460 passed, 13 intentionally ignored |
| `cargo test --locked --all-features --doc` | 28 passed, 1 ignored |
| `cargo test --locked` | 40 passed, 1 ignored |
| Strict all-feature/all-target Clippy; native-only library/tests Clippy | Passed with `-D warnings` |
| `cargo +1.87.0 check --locked --all-features --all-targets`; formatting | Passed |
| `cargo package --locked --allow-dirty` | Local package verification passed; not published. Existing missing registry-metadata warning remains. |
| Fresh `cargo-volt new ... --template navigation --desktop`, Rust 1.87 check | Passed against the local toolkit; fresh resolver-3 dependency resolution |
| Default normal dependency tree | Only `unicode-width`; no new locked packages apart from root version update |
| Real Metal hardware tests | Four `actual_gpu` tests and axis-aligned edge-border regression passed |
| Catalog Metal page coverage | All 37 pages at 1x/2x passed the ignored GPU-admission test |
| Large snapshots | Four consecutive 6K Relay and two 6K Catalog snapshots completed; 8K Relay explicitly used software fallback |
| Native Catalog | Version 0.3 visible; new log text submitted via Enter/Rust callback. Public material installed; light/dark and tint changes, zoom/restore at 4112×2430, rounded/square table sorting and shared selection exercised during review |
| Native Relay | Enter submission, streamed scripted completion, Stop → “Stopped.”/Idle verified with real input on the release binary |
| `cargo audit` | Refreshed advisory database; exit 0, five maintenance warnings, no vulnerability errors |

Snapshot/readback tests are not compositor, accessibility or device-loss
acceptance. Frame timing logs are not CPU/GPU utilization or power benchmarks.
Live VoiceOver/Narrator/Orca, Windows/Linux runtime and every macOS material
transition remain separate gates. No Volt vendored copy was edited or synced.

## Material and performance limits

The public AppKit adapter currently needs a decorated window and macOS 10.14+.
Borderless/unsupported hierarchies report transparent fallback rather than
pretending blur exists. AppKit owns material appearance; reduced-transparency
status is logged at installation, but a live OS-preference-toggle acceptance run
is still pending. The native button frames were 14×14 points in both modes;
automation capture obscures the controls, so the painted-circle silhouette on
all macOS versions is not certified. No App Store certification is implied:
transitive dependency symbols, signing and distribution still need review.

64 MP is the framebuffer cap, **not a promise that every 64 MP scene fits GPU
buffers or software opacity layers**. CPU opacity layers retain a separate
64 MiB aggregate budget; large complex fallback scenes may reject instead of
rendering partially. Conservative GPU admission can use software at smaller
sizes. Large decorative shadows may be omitted, not silently treated as drawn.

## Remaining delivery plan

```text
Reviewed source + bounded tests + native smoke runs
                        |
                    v0.3.0
                        |
        Application production-readiness gates
        +-- Larger-display GPU resize / device-loss stress
        +-- Multi-window lifecycle and command routing
        +-- Notifications, identity and bundle tooling
        +-- Navigation / motion and component-slot polish
        +-- Debug / reload diagnostics and performance profiles
        `-- Later: web / maps / system AI / visual theme tooling
```

| Priority | Remaining work | Acceptance needed |
|---|---|---|
| First | Original larger-display GPU resize failure; large opacity/fallback budgets; measured utilization and power | Repeated animated zoom/fullscreen and device-loss runs, explicit budget outcomes, CPU/GPU/idle/power profiles |
| First | Application lifecycle, multi-window state and command dispatch | Menus/shortcuts routed to the correct window, open/close/quit and restoration tests |
| Next | Native notifications and project identity/bundling | Real permission, delivery/click callbacks and explicit unsupported-platform capabilities; signed bundle tests |
| Next | Menu/tray runtime acceptance and platform adapters | Real OS menu/tray behavior, cleanup, shortcuts and permission/failure handling; Windows/Linux acceptance |
| Next | Navigation and animation polish; leading/trailing control slots and richer input states | Catalog-only demos, keyboard/AT coverage, reduced-motion and clipping tests |
| Next | Developer diagnostics, trusted rebuild/restart, optional state-preserving reload | Actionable errors, recovery without corrupting the installed UI; no claim that current restart preserves state |
| Later | Embedded web/maps, on-device AI/App Intents | Actual native adapters and capability/permission reporting, not mock success |
| Deferred | Visual theme tooling | Explicitly deferred by the user |

The existing scaffolds are useful UI starting points for editor/mail/settings/
agent layouts, **not** editor engines, mail protocols or agent security runtimes.
See [application delivery plan](APPLICATION_DEVELOPMENT.md) for the complete
boundary and APIs.
