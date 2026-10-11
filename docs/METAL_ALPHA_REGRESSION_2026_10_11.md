# Metal translucent-window regression — 2026-10-11

## Finding and bounded fix

The user's real desktop screenshot showed a white Catalog sidebar at 10% tint,
missing/subdued badge and selection backgrounds and incorrect-looking type.
A disposable Catalog baseline reproduced the white sidebar on Apple M4 Pro /
Metal, with GPU rendering active and `reduced_transparency=false`. Lowering tint
was not a fix. A material layer-backing experiment did not resolve that capture;
it was reverted, together with an unvalidated active-state experiment.

Our pinned `wgpu-hal 26.0.6` advertises `PostMultiplied`, but its Metal surface
implementation only sets `CAMetalLayer` nonopaque. Core Animation consumes
premultiplied pixels. The old presentation shader followed the advertised mode
and sent straight RGB. Upstream confirms the mismatch in
[wgpu #9896](https://github.com/gfx-rs/wgpu/issues/9896), corrected in
[wgpu #9922](https://github.com/gfx-rs/wgpu/pull/9922) (merged 2026-08-18).
That upstream change is not in our pinned rendering stack.

```text
Vello straight RGBA          Presentation shader       macOS compositor
white at 10%: (1,1,1,0.1) -> old: (1,1,1,0.1)       -> washed-out white
                          -> fixed: (.1,.1,.1,.1)   -> correct alpha blending
```

`present_straight_alpha` now separates supported surface configuration from
pixel encoding. Metal receives premultiplied RGB while retaining its advertised
configuration. Genuine straight-alpha modes on other backends are unchanged;
already opaque pixels are unchanged. Both Vello frames and software-texture
fallback frames use this same presentation shader. Revisit the workaround when
changing the wgpu pin; do not select an unsupported surface mode to fix encoding.

There is no API, dependency, legacy Painter, Volt vendor, tint-default, native
material/state/view hierarchy or typography redesign in this repair. Blur versus
transparency remains application-controlled. No private blur API is restored.

## Typography check

Local runtime diagnostics resolve `SF` to **SF Pro Text / SF Pro Display**, and
`SF Mono` to **SF Mono**. Regression coverage checks the actual shaped face ID
against the requested family's weight query at 1x/2x, sizes 13/44 and weights
400/500/600/700. Missing SF installations still use the existing portable fallback;
we do not download, register or bundle Apple fonts. This verifies local selection,
not the user's font availability or subjective acceptance of every text style.

`VOLT_UI_TRACE=1` emits family names in `VOLT_UI_FONTS` and the selected surface
mode/encoding in `VOLT_UI_SURFACE`. It does not log application text. On this pin,
Metal transparency should report `alpha_mode=PostMultiplied` alongside
`pixel_encoding=premultiplied`—that combination is intentional.

## Validation and remaining acceptance

- Red: targeted Metal encoding assertion failed against the old policy.
- Green: backend matrix passes; genuine postmultiplied other-backend policy stays
  straight, premultiplied and opaque policies stay premultiplied.
- Real GPU presentation-shader readback passes: 10%-alpha white, colored
  half-alpha and opaque samples retain expected RGB/alpha (one-byte tolerance).
  This tests the exact presentation shader, not just offscreen Vello output.
- Same native light-mode/10%-tint comparison: after the shader-only repair the
  white sidebar becomes the native material surface; filter border, selected-row
  fill and badge backgrounds are visible again. Native logs report Metal, native
  AppKit material, and zero software/framebuffer-upload fallback.
- SF family/weight selection test passes locally.
- Native appearance toggle responds and dark-mode/10%-tint rendering retains
  visible material, selection and badge backgrounds. All disposable comparison
  apps were closed after validation.
- `cargo test --locked --all-features --all-targets`: 505 passed, 14 ignored
  (hardware/manual tests remain opt-in). The new ignored presentation-shader test
  was explicitly run and passed on Apple M4 Pro / Metal.
- Strict all-feature/all-target Clippy, Rust 1.87 all-feature/all-target check,
  Linux x86_64 all-feature/all-target check, default tests and formatting pass.
- All-feature documentation tests: 29 passed, 1 ignored.

Window-only automation captures do **not** certify the appearance of wallpaper
or other-window textures through the native material. Foreground/background,
desktop light/dark contrast and Reduce Transparency remain native
acceptance checks. Do not label a flat capture proof that blur exists or is absent.
Material follows the native active-window state, unchanged by this fix.

Retest the normal launch (no new flags required):

```sh
cargo run --locked --release --features native-gpu,native-desktop --example catalog
```

For diagnosis, set `VOLT_UI_TRACE=1 VOLT_UI_RENDERER=gpu` and optionally
`-- --theme light --window-opacity 0.10`. Put a colorful window behind the sidebar;
verify it is softened rather than sharp transparency, then toggle appearance and
adjust tint. Software fallback remains opaque by design.

Local receipts and disposable bundles are under ignored
`work/blur-regression-2026-10-11/`. A pre-existing unused `mut` in Northbound's
frame-test closure was removed to permit strict all-target linting; no app behavior
was changed. `newapp_test/` was not edited or staged.
