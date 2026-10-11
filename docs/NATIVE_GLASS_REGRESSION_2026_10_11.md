# Native glass parity and Retina fallback — 2026-10-11

## Report and comparison

The latest user clarification is **Material -> Compositor produces square outer
corners**; it supersedes the earlier answer about direct compositor launch. Volt
Terminal demonstrates that winit itself does not require square translucent windows.

Live source comparison (Terminal read-only): both use winit 0.30.13 with decorated,
transparent, full-size content and a hidden title. Both raw blur paths ultimately
call winit `set_blur`, which uses the private macOS CGS blur-radius API. Toolkit
adds an AppKit Unified NSToolbar and uses Vello/wgpu 26; Terminal uses wgpu 29 and
no such toolbar on its custom-titlebar path. These differences are investigation
inputs, **not proof of the corner defect's cause**.

## Native lifecycle changes

```text
Material -> hide (retain attached) NSVisualEffectView
         -> request native frame redraw + invalidate cached window shadow
         -> enable opted-in compositor blur
         -> next native/GPU frame

Compositor -> disable private blur first
           -> show the same retained Material view
```

First tested native frame/shadow invalidation on material teardown; the user
confirmed it **did not resolve the square corners**. The current candidate instead
hides/reuses the retained Material view on backend switches, preserving the native
underlay hierarchy. A hidden view paints no material; it is removed only on actual
window/GPU shutdown. Visibility changes mark native views/shadow dirty. It does not
reparent the winit view, replace the delegate, resize traffic lights, set a custom
corner radius or draw synchronously inside a winit input callback. The stable-underlay candidate still needs desktop-level corner acceptance.
It is not declared fixed merely because unit tests pass.

Separately, initialize the admitted GPU backdrop before first showing the window,
matching Terminal's blur-before-show ordering without enabling the private API for
software fallback. Software initialization/fallback explicitly sets the native
window to opaque. Public Material remains the default; raw Compositor remains
explicit opt-in. The earlier Metal premultiplication correction is unchanged.

## Confirmed and fixed: fallback failing on valid Retina opacity groups

The user's terminal screenshot contains Vello segment-budget overflow, then
`opacity layer depth/memory budget exceeded`, an attempted opaque software fallback
and final fatal error. The old CPU fallback allocated full-window buffers for each
opacity group. A 3028 x 2074 physical-pixel window with two valid nested groups
requires more than the 64 MiB layer budget.

Large valid groups now replay into 512-pixel tiles with a 4-pixel AA overlap. The
output remains the full frame, but temporary group buffers occupy at most 26 MiB
at the existing maximum 24-group depth. Frame dimensions still obey the existing
64-megapixel limit. Invalid opacity, unbalanced streams and excessive nesting are
still rejected; there is no larger allocation cap or silent flattening of groups.
Normal admitted GPU frames do not use this CPU path. An over-budget GPU frame can
upload the CPU result through the existing GPU presenter, retaining native alpha
and blur rather than triggering permanent opaque fallback. CPU replay is slower
and is an overflow fallback, not a GPU-performance claim.

Integer tile translation preserves frame-coordinate rounding of text and image
positions, including half-pixels that become negative locally. Rasterizer AA on a
clipped curved stroke can differ slightly; the regression allows at most 8 byte
levels for that case (observed 5). The independent text/image/rounded/shadow test
remains within 1 byte; uniform nested-alpha pixels match exactly across tile seams.

## Evidence and boundaries

- Red: the 3028 x 2074, two-group regression failed with the same memory-budget
  error as the screenshot, before the fix.
- Green: that regression now passes, preserves root/foreground alpha and restores
  the native viewport. Additional regression covers clips, paths, images, shadows,
  text, negative half-pixel placement and cleanup after malformed input.
- Native baseline QA launched separate Compact and Unified Catalog bundles at
  matching opacity 42%, tint 9%, Green/Dark, compositor enabled. Trace confirms
  Metal/premultiplied alpha, GPU drawing and the expected 9px vs 19px control
  placement. No basis to declare toolbar removal a corner fix.
- Material -> Compositor was driven in the isolated Unified baseline. Window-only
  screenshots retained rounded image bounds, but do not capture/certify the raw
  desktop blur outside the native mask. User's desktop screenshot remains the
  authority for the reported rectangular effect.
- The stable-underlay transition changes require another desktop-level check in
  light/dark mode and repeated Material/Compositor toggles. Do not label this
  specific visual defect resolved based on unit tests or window-only captures.

No dependencies, unsafe code, UI Lab changes, Terminal edits or user `newapp_test/`
edits. Generic in-app GPU backdrop blur remains a separate pending renderer task.

Local validation logs: ignored `work/glass-native-parity-2026-10-11/`.

Latest checks: all-feature/all-target suite 511 passed, 14 ignored; strict Clippy,
Rust 1.87 all-target check, Linux all-target cross-check, default tests and release
Catalog build passed. The actual hardware presentation-shader alpha regression
also passed; it verifies encoded pixels, not native desktop corner clipping.
