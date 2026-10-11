# Independent glass controls — 2026-10-11

## Delivered: desktop background control

Catalog keeps separate controls for **Background opacity**, **Tint strength**,
**Backdrop blur**, and (on macOS) **Material / Compositor**. Opacity is the native
root clear's alpha, not a subtree-opacity animation. The content sheet and text
remain opaque. Tint mixes the selected accent (or a neutral surface for Zinc) into root RGB, not
another alpha layer. Default opacity is 72%, tint 0%; the Catalog opacity control
has a 10% floor to avoid macOS zero-alpha hit-test fall-through. Applications can
request any finite opacity from 0 to 1.

```text
Window content behind app -> selected native blur -> root RGB + opacity
                                                       |
                                                       +-> text/controls on top

Opacity: coverage / see-through strength       10% ... 100% ... 10%
Tint:    accent contribution to background RGB  0% ... 100% (independent)
Blur:    native blur on/off + explicit source   never implied by tint
```

- `WindowBackground::Blur`: existing public AppKit material on macOS, unchanged.
  The material itself adds system tint/opacity; a root alpha of 10% does not mean
  the **combined native material** has 10% opacity. This limitation is labeled.
- `WindowBackground::CompositorBlur`: explicit winit compositor request. Volt
  Terminal uses this path. Pinned winit calls private
  `CGSSetWindowBackgroundBlurRadius` on macOS (radius 80); **not an App Store-safe
  distribution claim**. No radius slider or cross-platform pixel equivalence is
  promised. It has no permanent AppKit material tint underneath the root clear.
- `Transparent`: alpha without blur. `Opaque`: solid root. Software fallback
  remains opaque and disables an opted-in compositor blur after GPU loss/error.
- Raw macOS blur checks Reduce Transparency on each redraw: disable the blur and
  use an opaque clear while enabled. There is no new idle preference observer;
  a subsequent frame/focus/input is needed to apply that preference.

The ordinary GPU Catalog defaults still use public material. To try Terminal-style
behavior, select **Compositor** in Catalog or explicitly launch:

```sh
cargo run --locked --release --features native-gpu,native-desktop --example catalog -- \
  --background compositor --window-opacity 0.72 --tint 0
```

## Rust integration

```rust
Application::new("My app", state, view)
    .background_from_state(|app| WindowBackground::CompositorBlur {
        opacity: app.background_opacity,
    })
    .theme_with(|app, mode| {
        theme(app, mode).with_background_color(app.background_color)
    });
```

`background_from_state` runs after changed/activated input, native command and
background-message updates. Invalid finite/range values are rejected and retain
the previous surface configuration. Initial options are validated before OS
creation. Changing opacity within a translucent mode does not recreate the device;
changing between `Opaque` and translucent modes does recreate the GPU surface.
`background(...)` called afterward clears the state callback (last builder wins).
`UiTheme::with_background_color` overrides the root only: semantic palette colors
remain intact. The host rejects invalid RGBA on rendering. Existing public options
struct fields, `ApplicationRequest` equality, default dependencies, legacy Painter
and Volt's vendored snapshot remain unchanged. `WindowBackground` gained an
additional variant; exhaustive downstream matches must cover it.

## Glass Browser comparison and remaining in-app GPU work

Current Glass Browser source (`crates/glass/src/lib.rs`, `blur.wgsl`,
`composite.wgsl`) uploads its CEF page frame, performs a half-resolution separable
Gaussian blur, then composites saturation, tint, rounded panels and shadows. Its
blurred page texture is cached until page content changes. This is **application
content**, not a capture of the desktop behind the native window.

The user requested both paths. This change delivers the native window path and
independent controls. Generic in-app GPU backdrop blur is **not implemented here**;
the existing landscape preview demonstrates alpha only. The next renderer slice
must add an explicit backdrop source, bounded/cached GPU targets, alpha-correct
blur passes, saturation/tint/opacity, clipping, nested layers and real readback
regressions without per-frame CPU readback. Do not call a tinted panel or native
material a substitute for that renderer.

## Evidence

- Red compile test failed on missing callback / compositor variant; targeted green
  passed after implementation.
- Catalog regression drives opacity 10/72/100/10 through real callbacks and
  recomposition; checks root alpha, foreground-sheet alpha 255, independent tint,
  live source configuration and explicit CLI selection.
- Root-theme override test confirms semantic palette and legacy colors persist.
- Release native QA: End moved background opacity to 100% (solid); Home returned
  it to 10% (translucent), while content sheet/text stayed opaque. Trace confirmed
  Vello / Apple M4 Pro / Metal with zero software-frame fallback and zero CPU
  framebuffer-upload bytes. Other controls later changed during native observation
  and the disposable app quit; no crash/error was recorded. Window-only capture
  does not certify wallpaper blur quality or every user interaction.
- All-feature/all-target suite: 509 passed, 14 ignored; strict Clippy, Rust 1.87,
  Linux cross-check and default tests passed. All-feature doctests: 29 passed,
  1 ignored. Final targeted/lint checks cover the last-builder-wins cleanup.

Local receipts: ignored `work/glass-controls-2026-10-11/`. `newapp_test/` and
Glass Browser / Volt Terminal source were only read, never edited or staged.
