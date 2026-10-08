# Source audit — 2026-10-07

Scope: canonical toolkit source, Cargo manifest, gallery and CI. Not a Volt or
Glass runtime audit. Repository source was already untracked before this work.

## Retain

| Evidence | Finding | Decision |
|---|---|---|
| `src/lib.rs` | Unsafe code forbidden; renderer-independent exports | Preserve |
| `src/paint.rs`, `Painter` | Static dispatch; host-selected buffers | No required methods or signature changes |
| `src/button.rs`, `Button` | Public fields and direct paint/hit API | Preserve struct-literal compatibility |
| `src/geometry.rs`, `HitMap` | Paint-order routing, rounded bounds and clip, blockers | Reuse for pointer routing; not yet focus/capture |
| `src/panel.rs`, `src/surface.rs` | Opaque fallback, host blur request, brightness hysteresis | Material stays optional |
| `scripts/sync_into_volt.py` | Hash inventory and guarded canonical-to-vendor sync | Never edit vendor separately |
| `Cargo.toml` | Rust 1.87, unicode-width only | Keep default dependency footprint |

## Gaps at the initial audit (before native slice)

1. `Painter::approx_text_width` estimates character count, not shaped advances.
   Button/form/search/card recipes use it. Preserve legacy behavior; add a new
   measured-text service for the new runtime rather than silently changing Volt.
2. `task_form`, `task_strip`, `workspace_card`, and settings retain application
   vocabulary. Introduce generic tree components separately; keep compatibility APIs.
3. Component-specific layout structs are not a nested constraint layout engine.
4. `HitMap` does not provide keyboard focus, capture, semantic events or accessibility.
5. Input paint recipes are not a Unicode editor with IME, clipboard and undo.
6. SVG gallery is static and cannot prove native shaping or GPU backdrop blur.
7. CI covers macOS/Linux, not Windows or assistive technology runtime behavior.
8. No existing UI document parser, bindings, shared tree, or hot-reload mechanism.

## First change

An opt-in `declarative` feature now exposes a dependency-free typed callback
registry in `src/declarative.rs`. Existing paint/layout implementation files are
unchanged. This establishes event signature validation; it does not connect
native input or parse/render UI documents.

## Subsequent delivery

The 2026-10-08 native vertical slice adds bounded documents, basic layout, measured
text, button input/focus, explicit bindings/reload and a Solid software host. See
[delivery evidence](VERTICAL_SLICE.md) for current results and remaining limitations.
