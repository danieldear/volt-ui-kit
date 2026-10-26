# Styling and themes

## Modifiers and inheritance

Every element takes style modifiers. Typography and text colour inherit into
children; surfaces, borders, corners and accents stay on the element that sets
them.

| Area | Modifiers |
| --- | --- |
| Text | `font_family`, `font_size`, `text_scale`, `font_weight`, `italic`, `color`, `line_height`, `letter_spacing`, `text_align`, `uppercase`, `underline`, `strikethrough`, `max_lines` (ellipsis), `no_wrap` |
| Surface | `background`, `background_gradient`, `border_color`, `border_width`, `border_style`, `border_sides`, `rounded`, `rounded_on` |
| Effects | `shadow`, `glow`, `opacity`, `hover_style`, `pressed_style`, `focus_style`, `disabled_style` |
| Interaction | `accent_color`, `cursor`, `enabled` |

Colours are `#RRGGBB`, `#RRGGBBAA` (alpha last) or theme roles, below.
Use `rounded(Rounding::Full)` for pills and circles, and
`rounded_on(12., Corners::Left)` or a list of `Corner`s for selected corners.

## Theme colours

Use semantic roles instead of fixed colours and the whole interface follows
the active theme, light or dark:

```rust
use volt_ui_kit::ui::prelude::*;
let card: Element<()> = vstack![
    text("Profile").color(ColorToken::BaseContent),
    text("Saved").color(ColorToken::Success),
    text("Muted note").color(ColorToken::Muted),
]
.background(ColorToken::Base200)
.border_color(ColorToken::Border)
.border_width(1.)
.padding(16., Edge::All)
.into();
card.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

The roles are `Base100`, `Base200`, `Base300` and `BaseContent` (surfaces and
their text); `Primary`, `Secondary`, `Accent` and `Neutral`; the status colours
`Info`, `Success`, `Warning` and `Error`; each with a matching `*Content` role
for text drawn on it; plus `Border` and `Muted`. `ColorToken::Primary.alpha(15)`
is the role at 15% opacity, which keeps tints correct in both modes.

## Built-in palettes

`Palette::light()` and `Palette::dark()` are the kit's default, a warm palette
with an amber accent, which `Application` uses unless you choose another.
`Palette::zinc_light()` and `Palette::zinc_dark()` are the Catalog's neutral
Zinc palette: white or near-black surfaces in zinc greys, with the text colour
as the primary. Apps made with `cargo volt new` start with Zinc.
`.with_accent(rgba)` gives Zinc a coloured primary on the same neutral
surfaces, which is how the Catalog's Blue, Violet, Rose, Green and Orange
themes are made.

`.palette(..)` sets one palette for both appearances. To follow the system's
light or dark mode, choose the palette in `.theme_with(..)`:

```rust,no_run
use volt_ui_kit::{native::Application, ui::prelude::*};

fn view(_: &()) -> Element<()> {
    text("Hello").color(ColorToken::BaseContent).into()
}

fn theme(_: &(), system: ThemeMode) -> UiTheme {
    let palette = if system == ThemeMode::Dark {
        Palette::zinc_dark()
    } else {
        Palette::zinc_light()
    };
    UiTheme::from_palette(palette.with_accent([0.49, 0.23, 0.93, 1.]))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Application::new("Zinc", (), view).theme_with(theme).run()
}
```

## Custom themes

A `Palette` assigns a colour to every role, and you can define all of them:

```rust,no_run
use volt_ui_kit::{native::Application, ui::prelude::*, ColorToken, Palette, ThemeMode};

#[derive(Default)]
struct App;

fn view(_: &App) -> Element<App> {
    text("Hello").into()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Start from a built-in palette and change any role.
    let mut palette = Palette::dark().with_accent([0.55, 0.36, 0.96, 1.]);
    palette.set(ColorToken::Base100, [0.07, 0.07, 0.09, 1.]);
    palette.set(ColorToken::Success, [0.20, 0.78, 0.45, 1.]);

    Application::new("Themed", App, view)
        .theme(ThemeMode::Dark)
        .palette(palette)
        .run()
}
```

- `with_accent(rgba)` sets the brand roles and picks readable text for them.
- `Palette::to_json()` and `Palette::from_json(text, &base)` save and load a
  palette. Theme files list roles by name (`"base-100"`, `"primary-content"`,
  …) with `#RRGGBB` values; roles a file leaves out keep the base palette.
- `.theme_file("theme.json")` loads a theme file at start and reloads it while
  the app runs whenever the file changes, which suits design iteration.
- `.theme_with(|app, mode| UiTheme::from_palette(..))` derives the theme from
  your own state, for in-app theme switching.
- `contrast_ratio(a, b)` gives the WCAG contrast of two colours; the Catalog
  uses it to grade every text/background pair.

The Catalog's **Theme editor** page (`--page theme-editor`) edits every role
live, checks contrast, and imports and exports theme JSON.

## Typography and fonts

`font_family("system")` resolves to the platform UI font; `"monospace"`,
`"serif"` and `"sans-serif"` pick suitable installed families; any other name
selects an installed font. On macOS, `"SF"` and `"SF Mono"` resolve to the
system San Francisco faces (no Apple fonts are bundled).

To ship your own font, register it before opening a window:

```rust,no_run
let families = volt_ui_kit::native::register_font(std::fs::read("assets/Inter.ttf")?)?;
println!("registered {families:?}");
# Ok::<(), Box<dyn std::error::Error>>(())
```

Up to 64 font files (256 MiB in total) can be registered.


`text_scale(f)` multiplies every font size beneath it, explicit sizes
included, and nested scales multiply (clamped to 0.5–2). Put it on the root
to offer an interface text-size setting; code views keep their own size.

## Effects and interaction states

```rust
use volt_ui_kit::ui::prelude::*;
let save: Element<usize> = button("Save").key("save")
    .border_color("#475569").border_width(1.)
    .shadow(Shadow::new().color("#00000040").blur(8.).offset(0., 2.))
    .hover_style(|s| s.background("#326481").glow(Glow::new().color("#60a5fa80").blur(12.)))
    .pressed_style(|s| s.opacity(0.7))
    .focus_style(|s| s.border_color("#B3E6FF"))
    .animate_on(Interaction::Hover, Animation::ease_out().duration_ms(120))
    .on_click(|count: &mut usize| *count += 1)
    .into();
save.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

- Shadows and glows draw outside the element without changing its layout or
  click area.
- `.opacity(..)` fades a whole subtree as one layer. It does not disable input.
- State styles apply in the order focus, hover, pressed; disabled controls use
  `disabled_style` instead. Focus rings appear only after keyboard navigation.


State styles accept the same colour values as static ones: `#RRGGBB[AA]`, a
token such as `@primary/80`, or a `"light|dark"` pair, so a hover follows the
appearance in effect. Shadows take pairs too.

## Animation and transitions

Use motion to explain a state change, not as a permanent decoration. These
animations use the host's monotonic clock, never call `view` or your callbacks,
and return to idle after finishing. All values are logical pixels.

```rust
use volt_ui_kit::ui::prelude::*;
let task: Element<()> = panel![text("Ready for review")].key("task")
    .offset(64., 0.)
    .animation(Animation::spring().duration_ms(520))
    .into();
task.build().unwrap();
```

`.animation(..)` interpolates appearance and offset changes for a stable key.
`animate_on(Interaction::Hover, ..)` (also Pressed or Focus) supplies timing for
interaction patches. Interruptions start from the current painted value. Only
position overshoots; opacity, colors and border widths stay bounded. Existing
linear/ease-in/ease-out/ease-in-out and `Easing::CubicBezier(..)` curves remain
available. Explicit `.easing(..)` replaces any spring timing on that animation.

`Animation::spring_with(Spring::new().damping_ratio(0.7))` tunes bounce. The valid
damping range is 0.4..=2, default 0.8; 1 is critically damped. This is a finite,
normalized damped response: duration controls settling time, and interruptions
preserve position **not physical velocity**. It is not a layout/sizing animation
or a physics simulation. Small offsets suit controls; larger movement is best
kept to decorative or noninteractive content because hit targets do not move.

### Entrances, exits and stagger

```rust
use volt_ui_kit::ui::prelude::*;
let replay_revision = 0u64; // owned by the application
let rows: Element<()> = vstack((0..3).map(|index| -> Element<()> {
    text(format!("Update {}", index + 1)).key(format!("update-{index}"))
        .transition(Transition::slide(0., 10.)
            .animation(Animation::ease_out().duration_ms(320))
            .stagger(index, 70))
        .transition_revision(replay_revision)
        .into()
})).into();
rows.build().unwrap();
```

- `Transition::fade()` fades a keyed subtree in/out; `slide(dx, dy)` adds an
  offset. `.animation(Animation)` shares timing configuration, including springs.
- `.delay_ms(..)` waits before starting. `.stagger(index, interval_ms)` adds
  `index * interval` to that delay on either `Animation` or `Transition`.
  A transition uses this timing on both entrance and exit. Invalid or overflowing
  configurations are rejected during build; values never wrap to a short delay.
- Animation durations are at most 10 seconds and delays at most 60 seconds.
  Scheduling covers the entire delay plus duration, including zero-duration
  delayed changes. Overshooting curves do not end a track prematurely.
- `.transition_revision(revision)` replays an entrance when the revision changes
  without changing identity, editor selection or focus. The same revision stays
  stable across ordinary recompositions. Increment it in an explicit action;
  never generate it from the clock inside `view`.
- Departing subtrees are paint-only snapshots. Removed controls cannot activate
  through pointer, keyboard or accessibility, even while their exit is visible.
- `.offset`, slide and ambient effects move pixels, **not layout or hit targets**.
  No scale/rotation transforms, layout tweening or automatic FLIP are implied.

### Reduced motion and idle

`Application::reduced_motion(true)`, `VOLT_UI_REDUCED_MOTION=1`, or
`Runtime::set_reduced_motion(true)` settles finite transitions immediately and
stops decorative ambient motion. Delayed-only scenes sleep until the first
useful update rather than render unchanged frames; custom hosts can use
`Runtime::animation_wait()` (None for idle, zero for frame pacing, or a positive
delay). The macOS native host also samples the system
Reduce Motion preference at startup. It does not monitor live system preference
changes yet. Unfocused, occluded or minimized native windows already settle
animations rather than requesting invisible frames.

`Ambient::pulse(0.4)`, `float(..)` and canvas-only `spin()` are intentionally
continuous while visible; opt into them only where useful and remove loading
indicators when finished. They stop under reduced motion. Shadows and glows
remain optional; geometry stays fixed during alpha/color animation so native
blur masks can be reused.

The Catalog's **Effects & motion** page has Smooth/Spring/Instant comparison,
reversible task motion, a replayable three-step sequence, and hover/press/focus
feedback. Instant previews are local to those demos; the launch/runtime reduced
motion controls apply to the entire app.

## Images

```rust,no_run
use volt_ui_kit::{native::load_image, ui::prelude::*};
let image = load_image("assets/cover.jpg")?; // load once, outside `view`
let hero: Element<()> = vstack![text("Welcome")]
    .background(ColorToken::Base200)
    .background_image(image.background().fit(ImageFit::Cover).opacity(0.35))
    .rounded(16.)
    .clip_contents(true)
    .into();
hero.build()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Fits are `Cover`, `Contain`, `Stretch` and `Tile`, placed by `ImagePosition`.
Image handles share their pixels, so cloning one into `view` is cheap. PNG and
JPEG files are limited to 16 MiB and 4096 px per side; nothing is ever fetched
from a URL.

## Vector drawing and clipping

```rust
use volt_ui_kit::ui::prelude::*;
let badge: Element<()> = canvas(|c| {
    c.fill(VectorPath::circle(60., 60., 40.), ColorToken::Accent);
    c.stroke(
        VectorPath::new().move_to(10., 100.).cubic_to([35., 10.], [90., 10.], [110., 100.]),
        "#FFFFFF",
        2.,
    );
})
.width(120.)
.height(120.)
.clip(Clip::Circle);
badge.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

- `canvas` records rectangles, circles, ellipses, lines and curves with fills
  and strokes in local logical coordinates. `VectorPath::from_svg(d)` imports
  SVG path data.
- `.clip(Clip::Rounded | Circle | Path(path))` masks an element's contents, and
  clicks follow the same shape. `.clip_contents(true)` clips to the rounded
  corners.
