# Handoff note for Codex (from Claude, 2026-10-10)

Hi Codex — we have both been editing this tree. To protect both sessions'
work I committed everything on a branch before changing anything:

- Branch: `claude/gpu-a11y-damage` (from `main` at `e8d1ed5`)
- `70d148e` **Checkpoint** — your uncommitted work and mine, unchanged.
- Then six commits of mine, described below. Nothing is pushed.

If you have edits in flight, please rebase them onto this branch (or tell the
user which files you touched) rather than editing `main` directly.

## What changed and why

| Commit | Area | Summary |
|---|---|---|
| `96aa20e` | GPU | Vello **vector text** (zero-copy font handles shared with cosmic-text; bitmaps only as fallback), **GPU-resident images** (`TextureImages`, LRU 96 MiB, unpremultiplied upload, `register_texture`), **all Vello fixed buffers estimated** with per-frame software fallback, **device-loss / uncaptured-error recovery** (`PresentStatus::Lost`), benchmarks settle animations, Windows in CI, rustdoc link fix |
| `2542ec5` | Software | **Damage tracking**: `RasterPainter::rasterize_incremental` diffs batches (common prefix/suffix), repaints only the damage through the clip masks, culls off-damage shapes; window presents only damaged rows when `buffer.age() == 1` |
| `99a68d9` | A11y | **AccessKit**: toolkit-neutral `Runtime::accessibility_nodes()` + actions in `declarative/runtime/accessibility.rs`; native bridge in `native/accessibility.rs`; adapter created before the window is shown |
| `5ce360a` | Limits | `MAX_NODES` 512 → **4096**, `MAX_DEPTH` 24 → **48**; limit tests derive from the constants |
| `9c16b33` | Components | `ButtonVariant`, `ControlSize`, `.disclosure()`, `.label_align()`, token opacity, primary-label contrast, refined defaults, table/tree polish, `specimen` example |

## Contracts to preserve

1. **Never present a stale GPU frame.** Vello 0.6 silently drops a frame that
   overflows any fixed buffer (blend spill, lines, segments, tiles, tile
   commands, binning, image atlas 8192²) and the previous image stays on
   screen. `GpuSceneEncoder::finish_budget` estimates each one and sets
   `SceneStats::over_budget`; hosts draw such frames in software. Any new
   `scene.*` drawing or `push_layer` must go through `self.layer()` /
   `self.note()` so the estimates stay conservative.
2. **Layers are expensive in Vello.** Deep nesting over large areas spills.
   Batch clips are skipped when redundant, text clips only when ink
   overflows, images are brush fills (no layer), opacity layers are sized to
   their content (`opacity_layer_bounds`). Keep new effects layer-light.
3. **Incremental software frames must equal full repaints, pixel for pixel**
   (`examples/northbound/tests.rs::incremental_software_frames_match_full_repaints`).
   Every write path must respect the clip mask or be clamped to
   `RasterPainter::damage`; `shape_extent` must cover blur/stroke extents.
   Structural changes (layers/clips) deliberately force a full repaint.
4. **Accessibility never exposes passwords** (value is `None`; `editor_text`
   already withholds them). While a modal is open only its scope is exposed,
   mirroring pointer/keyboard reachability (`scope_allows_id`).
5. **Rust 1.87 minimum**: `uuid` is held at 1.26.1 in `Cargo.lock` (AccessKit
   pulls it). All 386 locked crates were checked for `rust-version` ≤ 1.87.
6. **Primary labels**: `Button::primary(true)` sets `@primary-content`
   explicitly so inherited text colours can't override it.

## Behaviour changes others may notice

- Rust `panel(...)` now has 12px padding (its doc always said "padded
  surface"). Explicit `.padding(...)` overrides it. Two layout tests were
  updated accordingly.
- Rust buttons/bars default to 6px corners (JSON documents unchanged).
- Joined bars / segmented controls use a 1px gap seam.
- Tables and trees have natural height unless `.height(...)` is set.

## Verified

- 401 tests across all targets + 28 doctests, strict clippy, fmt, Rust 1.87
  all-features check, `cargo doc -D warnings`, Linux target `cargo check`.
- GPU hardware tests on Apple M4 Pro/Metal (`--ignored`): readback vs
  software (MAE 0.37 / 2.55 at 1x/2x), nesting depth 3–8 never stale,
  fake-italic slant matches software, Northbound screens stay on GPU.
- Retina hover frame (software): ~12.6 ms full → ~2.1 ms incremental.

## Not verified (needs a person or another platform)

- **Live VoiceOver/Narrator/Orca**: this terminal lacks macOS assistive
  access, and granting it is the user's call. Tree/action logic is unit- and
  integration-tested up to the AccessKit boundary.
- Windows (DX12) and Linux (Vulkan, AT-SPI) **runtime**; CI now builds/tests
  Windows headlessly.
- Real device loss under stress.

## Catalog example (after your 0.2.0 commit)

`examples/catalog.rs` + `examples/catalog/` is a shadcn-style documentation
site covering every component (including menus, context menu, popover,
tooltip, progress, table, tree, virtual list, split pane and worker messages),
with Preview/Code tabs, API tables, a search palette and six themes. It adds
files, an `[[example]]` entry, a README row, and specific table validation
errors in `src/ui/data.rs` (columns are sortable by default, which the old
combined message hid). Tests: every page builds in both modes, at two widths,
with every code view and the overlays open.

## Suggested next steps

1. Icons inside `ui::button` (leading/trailing slots) — currently apps layer a
   canvas over a button (see Northbound's `kit::icon_button`).
2. Theme *metrics* (radius scale, control heights, type scale) alongside the
   colour palette, so variants/sizes can be re-themed globally.
3. Checkbox indeterminate state; text-field prefix/suffix/error/help text.
4. GPU damage: Vello always redraws the full scene; a scene-diff that skips
   unchanged frames entirely would cut idle-hover cost further.
5. Fine-grained recomposition (whole tree is rebuilt per action; ~25 ms at
   4096 nodes).

— Claude
