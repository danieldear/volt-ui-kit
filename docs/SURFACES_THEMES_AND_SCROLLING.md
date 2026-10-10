# Themes, surfaces, clipping and scrolling

All examples below are Rust declarations, not JSON. The core remains renderer-independent;
`native` supplies image decoding and software rendering; `native-gpu` adds an optional
alpha-capable window renderer. Legacy `Theme`, `NativeOptions`, `WindowOptions` fields
and existing `Painter` implementations remain source-compatible.

## Try the examples

For everyday testing, use the single [Volt UI Lab app](UI_LAB.md):
`cargo run --features native --example ui_lab`. The focused examples below are
retained for learning and regression isolation.

```sh
cargo run --features native --example rust_surfaces
cargo run --features native --example rust_surfaces -- --theme light
cargo run --features native --example rust_scroll
cargo run --features native --example rust_effects -- --theme light
cargo run --features native-gpu --example rust_transparent
```

The first two new examples support `--snapshot /absolute/path.png`, `--size 600x760`,
`--scale 2`, and `--theme light|dark|system`. Headless `system` uses a dark fallback;
it does not pretend to discover the desktop appearance. Snapshot rendering is not
proof of native input, window alpha or backdrop blur.

## Semantic colors and global accents

```rust
use volt_ui_kit::ui::prelude::*;
let ui: Element<()> = panel![
    text("Profile").color(ColorToken::BaseContent),
    text("Saved").color(ColorToken::Success),
].background(ColorToken::Base200).padding(24., Edge::All).into();
ui.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

Roles include Base100/200/300, BaseContent, Primary/PrimaryContent,
Secondary/SecondaryContent, Accent/AccentContent, Neutral/NeutralContent,
Info, Success, Warning, Error and their matching `*Content` foregrounds,
Border and Muted. Rust tokens lower to validated `@role` strings; literal hex
colors still work and remain explicit overrides. Text color tokens inherit;
backgrounds and local accents do not. Tokens resolve on paint, not at composition.

For a native application:

```rust,no_run
use volt_ui_kit::{native::{NativeOptions, run_ui_with_options}, ui::prelude::*};
let options = NativeOptions::default()
    .with_theme(ThemeMode::System)
    .with_accent([0.38, 0.42, 0.92, 1.]);
run_ui_with_options(options, (), |_| {
    vstack![text("Hello"), button("Save").primary(true).on_click(|_: &mut ()| {})]
        .background(ColorToken::Base100).color(ColorToken::BaseContent).into()
})?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`with_theme` also accepts Light and Dark. `with_palette(Palette)` supplies a named,
fully custom set of semantic colors. `Palette::dark()/light().with_accent(rgba)` is
an easy starting point; `with_accent` updates both the primary/accent colors and
chooses contrasting black/white foregrounds for opaque fills. Translucent fills
still need a foreground appropriate to their eventual backdrop.

`run_ui_with_theme(options, state, view, |state, system_mode| UiTheme)` lets Rust
application state select the palette. See the Light / Dark button in
`rust_surfaces`. The native host observes winit theme changes where supported.
On platforms without usable system-theme reporting, System falls back to Dark;
it is not a universal desktop-settings integration. A state-derived palette updates
native chrome for explicit Light/Dark configuration; System keeps OS-managed chrome.

Legacy `runtime.paint(painter, &Theme)` keeps its default recipe. Hosts using new
semantic palettes call `runtime.paint_ui(painter, &UiTheme)`.

## Image backgrounds on panels and stacks

Load once outside `view`, then clone the shared handle:

```rust,no_run
use volt_ui_kit::{native::load_image, ui::prelude::*};
let image = load_image("assets/panel.jpg")?;
let ui: Element<()> = vstack![text("Welcome")]
    .background(ColorToken::Base200)
    .background_image(image.background()
        .fit(ImageFit::Cover)
        .position(ImagePosition::Center)
        .opacity(0.35))
    .rounded(16.)
    .clip_contents(true)
    .into();
ui.build()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Fits: Cover (crop to fill), Contain (whole image), Stretch, Tile. Position options
include Center, the four sides and the four corners. Tiles use intrinsic logical
size, scaled for DPI. Images respect the surface's rounded silhouette and inset
border. Clones share premultiplied pixels; paint does not read files or decode.
No implicit URLs, network downloads, SVG scripts or external-resource loading.
PNG/JPEG loaders cap encoded input at 16 MiB, dimensions at 4096 per side and
decoder allocation at 128 MiB. `Image::from_rgba` accepts host-supplied RGBA with
a 16 MP per-image limit and premultiplies it once.

### Three different kinds of opacity

```text
image.opacity(0.35)         -> background image only
.background_opacity(0.35)   -> color and image; text/children unchanged
.opacity(0.35)              -> entire subtree, including effects and children
```

Whole-subtree opacity does not disable input; use enabled state or remove controls
when that is intended. Image opacity multiplies background opacity. An opaque
background color remains visible behind a faded image.

## Vector canvas and clipping

```rust
use volt_ui_kit::ui::prelude::*;
let ui: Element<()> = canvas(|c| {
    c.fill(VectorPath::circle(60., 60., 40.), ColorToken::Accent);
    c.stroke(VectorPath::new().move_to(10., 100.)
        .cubic_to([35., 10.], [90., 10.], [110., 100.]), "#FFFFFF", 2.);
}).width(120.).height(120.).clip(Clip::Circle);
ui.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

The Rust closure runs when composing the view, not on every animation frame.
Commands are retained: rectangles, circles, ellipses, moves/lines, quadratic/cubic
curves, fill and stroke. `path.transformed([a,b,c,d,tx,ty])` supplies affine
translation/scale/rotation. Coordinates are local logical pixels; runtime maps
paths to physical pixels. Fills and path clips use even-odd winding. This is a
vector drawing API, not an SVG-file parser.

`clip_contents(true)` uses the surface's selective rounded corners; false removes
that explicit clip. `clip(Clip::Bounds|Rounded|Circle|Path(path))` selects a mask.
Circle is the centered circle inscribed in the bounds, not an ellipse. Path clips
use local logical coordinates. Input follows the same ancestor clip geometry;
curve input testing uses bounded flattening, while painting has antialiased edges.

```text
subtree opacity group (optional)
  outer shadow / glow                 <-- overflow through ordinary layout parents
  content mask (explicit, optional)
    surface color + image             <-- image also preserves rounded border
    text / vectors / child elements
  scrollbar overlay (scroll views)
end opacity group
```

The surface's own outer glow/shadow is not cut off by its content mask. Descendant
effects obey explicit clip/scroll ancestors and the window boundary. Custom path
content masks do not change the underlying rounded-surface shadow silhouette.
This fixes the prior three-sided clipping caused by tight hstacks. Existing
rectangular layout/hit bounds and padding behavior otherwise remain unchanged.
Native clip masks are cached with a bounded 8 MiB / 32-entry budget.

New `DocumentPainter` image/path/clip hooks have default no-op implementations,
so old adapters compile unchanged. An embedding adapter must implement these
hooks to render the new capabilities; only the supplied native adapter is
verified for them. Do not assume a legacy adapter provides clipping or images.

## Overflow scrolling

```rust
use volt_ui_kit::ui::prelude::*;
let rows = (0..100).map(|i| text(format!("Row {i}")).height(32.).into());
let ui: Element<()> = vstack![
    text("Fixed header").height(40.),
    vstack(rows).key("list").flex(1.).scroll(Scroll::Auto),
].into();
ui.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

Modes: Vertical, Horizontal, Both, Auto. Auto retains natural wrapping in the
cross axis, but permits explicitly wider/taller children to overflow; it enables
scrolling only on axes with excess content. Give nested scroll views a fixed
height/width or a flex allocation so they have a meaningful viewport.

Wheel/trackpad events scroll the deepest view under the pointer. At its boundary,
only the unconsumed delta bubbles to scrollable ancestors. Shift-wheel supports
horizontal scrolling. Offsets use logical pixels, survive matching keyed
recomposition, and clamp after content/viewport changes. Offscreen items do not
receive pointer events. Scroll views automatically clip overflow and draw thin
position indicators. `.scrollbars(Scrollbars::Hidden)` hides only those indicators,
while keeping wheel/trackpad scrolling and clipping enabled. Visible indicators are draggable and keyboard focus reveals mounted offscreen controls, including nested scroll views. Hidden indicators never disable wheel scrolling. Touch/drag content scrolling and application-driven smooth scrolling remain follow-up work.

Embedding hosts call `runtime.scroll_at(physical_x, physical_y, logical_delta)`
and then layout/redraw if it returns true. Positive deltas move toward the end.
Ordinary scrolling does not invoke handlers or rebuild the Rust view. Controlled virtual lists dispatch `WindowChanged`; native hosts use `scroll_outcome` and recompose the bounded row window when needed. Scrolling is opt-in:
we do not silently change existing container sizing or JSON behavior.

## Real window transparency and native blur

```rust,no_run
use volt_ui_kit::{native::{NativeOptions, WindowBackground}, ui::prelude::*};
let options = NativeOptions::default().with_theme(ThemeMode::System)
    .with_background(WindowBackground::Blur { opacity: 0.65 });
```

`WindowBackground::Transparent { opacity }` requests alpha without blur;
Opaque is the backward-compatible default. The opacity tints the root clear;
text and controls remain opaque unless their own modifiers request otherwise.
**Do not paint an opaque root background** if the desktop should remain visible.

`native-gpu` now draws Vello GPU scenes for opaque/alpha windows, adapting the
output to the advertised PreMultiplied/PostMultiplied compositor mode. It does not
make the default widget library GPU-dependent. Without this feature, or when
initialization is unavailable in `auto` mode, the host logs opaque software
fallback. `VOLT_UI_RENDERER=gpu` instead requires successful GPU drawing. See
[GPU renderer/dependency policy](GPU_RENDERING.md).
It never labels an opaque tint/shadow as backdrop blur.

winit blur is an OS request: macOS and KDE Wayland support vary; Windows/X11 and
other unsupported platforms do not gain native blur from this API. There is no
portable backdrop blur-radius control. Pixel tests verify premultiplied rendering;
desktop-through transparency, blur, OS appearance changes and trackpad feel require device/platform acceptance testing. The current pinned winit 0.30.13 macOS blur uses private `CGSSetWindowBackgroundBlurRadius`; this is not an App Store-safe `NSVisualEffectView` material integration. Winit 0.31 beta provides public materials, but is not adopted in this stable/MSRV-preserving milestone.

Primary references: [semantic colors](https://daisyui.com/docs/colors/),
[winit window capabilities](https://docs.rs/winit/0.30.13/winit/window/struct.Window.html),
[softbuffer RGB format](https://docs.rs/softbuffer/0.4.8/softbuffer/struct.Buffer.html).
The image decoder is pinned to 0.25.8 for Rust 1.87 compatibility. The optional
renderer uses Vello 0.6.0/wgpu 26.0.1 with pruned backend features; the complete
GPU all-target build is checked with Rust 1.87. Newer Vello needs newer Rust.

## Verification receipt (2026-10-08)

- All-feature targets: 187 tests passed; rustdoc: 17 passed, 1 existing ignored.
- Default and Rust-only feature tests passed independently.
- Strict all-feature Clippy, formatting, and Rust 1.87 all-target checks passed.
- The all-feature Cargo package was assembled and verified by compiling it.
- Light/dark and 1x/2x headless snapshots were inspected; tests cover nested
  scrolling, reachable callbacks, scroll clamping, matching paint/input clipping,
  image fit/opacity, semantic colors and glow on all four sides.

These checks do not replace the native device acceptance listed above. Warm CPU
benchmarking of the 960x900 surfaces example measured a 14.3 ms p95 from release
handling to packed pixels (20 samples after 4 warmups); it excludes OS event queues,
compositor presentation and input-to-photon latency, and is not a frame-rate promise.

### Wheel scrolling through an empty overlay

Layout-only layers in an overlapping composition can opt out of being the initial
wheel target. This is explicit, not a change to default hit routing:

```rust
use volt_ui_kit::ui::prelude::*;
let ui: Element<()> = zstack![
    vstack![text("Scrollable content").height(420.)].scroll(Scroll::Vertical),
    spacer().scroll_passthrough(true),
].height(180.).align(Align::Stretch).into();
ui.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

Apply it to an empty overlay's container **and** any spacer covering the scroll
view. Descendants remain eligible wheel targets; buttons/keyboard input and
painting are unchanged. Do not apply it to opaque overlays that should block
scrolling behind them. Volt Desk uses it for its empty pane-placement layer.
