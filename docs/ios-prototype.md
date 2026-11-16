# Experimental iOS apps

**Experimental, not production iOS support.** Write your screens, state and
callbacks in Rust with Volt UI Kit. iOS supplies what people notice first
when it is wrong: the navigation bar and back swipe, scrolling, switches,
sliders, text editing and the keyboard. Volt draws each page's content, in
iOS's colours and system font by default or in your app's own look. No
per-app Swift, SwiftUI or web view is needed.

The [Android prototype](android-prototype.md) uses the same Rust application
layer and project template; there, Volt draws the navigation bar as well.

## How an iOS app is put together

```text
Your Rust app: state + view + typed callbacks
                        |
volt-ui-kit-mobile: Application, platform events, page snapshot
                        |
volt-ui-kit: layout, text, drawing (Vello on Metal)
                        |
VoltUIHost.swift, the kit's thin UIKit shell:
  UINavigationController   bar, large title, back button and swipe
  UIScrollView             momentum, bounce, indicators, tap to top
  UISwitch, UISlider       drawn by iOS where Volt lays them out
  UITextField, UITextView  typing, selection, autocorrect, dictation
  CAMetalLayer             Volt's pages, presented in step with UIKit
```

| Part | Who draws it | Notes |
|---|---|---|
| Navigation bar, large title, back | iOS | From `navigation_stack` page modifiers |
| Back swipe, from the edge or anywhere (iOS 26) | iOS | Paused while a finger drags a slider or carousel |
| Scrolling physics and indicators | iOS | Volt draws the content at UIKit's position |
| Switches and sliders | iOS | Volt lays them out and keeps your state |
| Text fields and text areas while editing | iOS | Volt draws them at rest |
| Everything else on the page | Volt | Lists, buttons, cards, grids, charts, canvases |

The Swift shell is kit code. Generated Xcode projects reference it; your app
does not contain or edit Swift.

## Create a project

Requires macOS, full Xcode, Python 3 and Rust 1.89 or newer. From a current
toolkit checkout:

```sh
cargo install --locked --path . --features dev-tools --bin cargo-volt
rustup target add aarch64-apple-ios-sim aarch64-apple-ios
cargo volt new my-ios-app --platform ios --kit-path /path/to/volt-ui-kit
cd my-ios-app
open ios/VoltApp.xcodeproj
```

Select **VoltApp** and an iPhone Simulator, then Run. Xcode builds and links
the Rust static library. Intel Macs need `x86_64-apple-ios` instead of
`aarch64-apple-ios-sim`. The deployment target is iOS 17.

The starter has a large-title home page with a counter and a list, and a
Settings page (opened from the bar) with a text field, a switch and Save with
validation. Edit **`src/lib.rs`**.

```text
my-ios-app/
  Cargo.toml               staticlib/rlib + the reusable host dependency
  src/lib.rs               your state, pages, callbacks and factory
  ios/
    VoltApp.xcodeproj/     ready to open; references the kit's Swift shell
    App/Info.plist         bundle, scene and 120 Hz configuration
    build-rust.sh          builds the Rust library during the Xcode build
    Tests/SmokeTests.swift a UI test of the starter
```

The first Xcode build resolves `Cargo.lock`; commit it. Later builds use
`--locked`. Keep the toolkit checkout where the project expects it; if it
moves, regenerate the project. Existing destinations are never overwritten.

## Run on your iPhone

Generated projects have code signing off, so the Simulator works without an
Apple account. For your phone:

1. On the iPhone, turn on **Settings → Privacy & Security → Developer Mode**
   and restart when asked. Connect it by cable and choose **Trust**.
2. In Xcode, select the **VoltApp** target → **Signing & Capabilities**, turn
   on **Automatically manage signing**, pick your team and set a bundle ID of
   your own, such as `com.yourdomain.myapp`.
3. Choose your iPhone as the destination and Run. Keep the phone unlocked
   while Xcode prepares it the first time.

From the command line instead:

```sh
xcodebuild -project ios/VoltApp.xcodeproj -scheme VoltApp \
  -destination 'id=YOUR-DEVICE-ID' -allowProvisioningUpdates \
  CODE_SIGNING_ALLOWED=YES CODE_SIGN_STYLE=Automatic \
  DEVELOPMENT_TEAM=YOUR-TEAM-ID PRODUCT_BUNDLE_IDENTIFIER=com.yourdomain.myapp build
xcrun devicectl device install app --device YOUR-DEVICE-ID \
  build/Build/Products/Debug-iphoneos/VoltApp.app
```

`xcrun devicectl list devices` shows your device ID. Signing, provisioning and
App Store distribution are yours; Volt does not store certificates or IDs.

## Pages and navigation

```rust,ignore
use volt_ui_kit_mobile::{Application, HostContext};
use volt_ui_kit_mobile::ui_kit::ui::{prelude::*, Container};

#[derive(Clone, Copy, PartialEq)]
enum Page { Home, Settings }
pub struct App { history: NavigationHistory<Page>, count: usize }

fn home(app: &App) -> Container<App> {
    vstack![button(format!("Count: {}", app.count)).key("add")
        .on_click(|a: &mut App| a.count += 1)]
    .key("home").flex(1.).scroll(Scroll::Vertical)
}
fn view(app: &App) -> Element<App> {
    navigation_stack(&app.history, |page| match page {
        Page::Home => home(app)
            .navigation_title("My app")
            .large_title(true)
            .bar_button("Settings", |a: &mut App| { a.history.push(Page::Settings); })
            .into(),
        Page::Settings => vstack![text("Settings")].navigation_title("Settings").into(),
    })
    .key("app")
    .on_back(|a: &mut App| { a.history.back(); })
    .into()
}
pub fn application(host: HostContext) -> Result<Application<App>, String> {
    Application::new(host, App { history: NavigationHistory::new(Page::Home), count: 0 }, view)
}
volt_ui_kit_mobile::export_ios_app!(application);
```

- `navigation_title`, `large_title` and `bar_button` describe a page's bar.
  `on_back` runs for the back button and the back swipe. Your
  `NavigationHistory` decides the pages; iOS animates the push and pop.
- Give each page's scroll view a key. It then runs under the bars with iOS's
  insets, its large title shrinks as it scrolls, and going back returns to the
  same scroll position.
- On desktop, the web and Android, the same code draws the bar with Volt.

## Colours, type and motion

- **Colours.** Without a theme, pages use iOS's own colours for the current
  appearance and contrast setting: grouped backgrounds, label colours,
  hairline separators and the app tint. `Application::theme_with` replaces them
  with your theme. `Application::theme_from_system` starts from iOS's colours
  and adjusts them, for example a brand accent on some pages only.
- **Type.** The iPhone's system font is read from the device at runtime and
  never bundled. Weights such as `font_weight(700)` come from its variable
  `wght` axis. An explicit `font_family` on your root overrides it.
- **Motion.** Press feedback and transitions run from a display link that
  stops when nothing moves. The Reduce Motion setting is followed. The
  generated `Info.plist` allows 120 Hz on ProMotion iPhones.

## The iPhone gallery

[`examples/ios-gallery`](../examples/ios-gallery) shows every part on a phone:
navigation, buttons, lists, switches and sliders, text input, typography,
badges and cards, and **Pantry**, a branded grocery shop (product grid,
carousel, product page, cart and checkout). Each page ends with what to check
on a device.

```sh
cargo test --locked --manifest-path examples/ios-gallery/rust/Cargo.toml \
  --target-dir target/ios-gallery
open examples/ios-gallery/VoltGallery.xcodeproj
```

For your phone, override the bundle prefix and team on the command line; the
UI test runner gets `VOLT_BUNDLE_ID` plus `.tests`:

```sh
cd examples/ios-gallery
xcodebuild test -project VoltGallery.xcodeproj -scheme VoltGallery \
  -destination 'id=YOUR-DEVICE-ID' -allowProvisioningUpdates \
  CODE_SIGNING_ALLOWED=YES CODE_SIGN_STYLE=Automatic \
  DEVELOPMENT_TEAM=YOUR-TEAM-ID VOLT_BUNDLE_ID=com.yourdomain.gallery
```

The UI tests drive the phone: every page through the real bar, the bar
button, momentum and the collapsing large title, the keyboard and text area,
returning to a page without moving it, and a shop order by touch. Leave the
phone alone while they run.

- `VOLT_GALLERY_PAGE=shop` (or `controls`, `text`, `lists`, ...) opens a page
  at launch, for screenshots.
- `VOLT_ANIMATION_SPEED=0.1` plays UIKit animations ten times slower, to look
  at transitions frame by frame. It works in any Volt iOS app.

Regenerate the checked-in Xcode project with XcodeGen after editing
`examples/ios-gallery/project.yml`.

The older [Field Notes](../examples/ios) example (`examples/ios`) still runs
on the same shell and keeps its own UI tests.

## Test a generated project

```sh
cargo test --locked
xcodebuild -project ios/VoltApp.xcodeproj -scheme VoltApp \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro' test
```

Use an installed Simulator name. The starter's UI test checks the counter, the
Settings bar button, native text editing, the switch, Save and scrolling to
the last row.

## Drawing and presentation

Compatible GPUs draw with Vello. The Simulator lacks some features Vello
needs, so the starter calls `.allow_software_fallback(true)`: software drawing
with Metal presentation there. The `render-proof` accessibility element
reports the backend, drawing mode and presented-frame count. Frames are
presented with Core Animation transactions, so UIKit parts on top of Volt's
content move in the same frame. Rendering stays event-driven: no frames are
drawn while nothing changes. See Apple's
[Simulator Metal restrictions](https://developer.apple.com/documentation/metal/developing-metal-apps-that-run-in-simulator).

## Host boundary

The kit keeps `forbid(unsafe_code)`; the host crate holds the narrow iOS
surface boundary. The Swift shell and Rust exchange small, bounded JSON
messages (touches, scroll positions, bar actions, colours) and a page
snapshot. This is plumbing between the shell and Rust, not a way to describe
UI. The wire decoder rejects unknown operations and fields, non-finite
geometry and out-of-range values. All calls stay on UIKit's main thread. It
is an internal ABI, not a stable third-party one.

## Remaining limits

Tested on a physical iPhone 17 Pro with the gallery's UI tests. Still missing
or partial:

- **Fallback fonts.** Emoji and scripts the system font lacks (Chinese,
  Japanese, Korean, Arabic and others) do not render yet.
- **Network images**, **bottom sheets with detents**, a **tab bar**, native
  **segmented controls**, **menus and alerts**, **date pickers** and **maps**
  on iOS.
- **Password fields** stay read-only in the native editor; secret values never
  enter snapshots.
- **Dynamic Type** and full **VoiceOver** acceptance; persistence and state
  restoration; multi-window; push notifications and other services.
- Energy and frame-time profiling, App Store packaging and a distributable
  SDK.
