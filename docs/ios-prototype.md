# Experimental reusable iOS host

**An integration prototype, not production mobile support.** Build screens,
state and callbacks in Rust with Volt UI Kit. The toolkit supplies the reusable
UIKit/Metal host, native single-line editor and Xcode build integration. No
per-app Swift application code, SwiftUI, WebView or second widget framework is
needed. Android has a shared Rust foundation, **not a working Android host**.

## Create a Rust iOS project

Requires macOS, full Xcode with an installed iPhone Simulator, Python 3 and
Cargo/Rust 1.89+. From a current toolkit checkout:

```sh
cargo install --locked --path . --features dev-tools --bin cargo-volt
rustup target add aarch64-apple-ios-sim aarch64-apple-ios
cargo volt new my-ios-app --platform ios --kit-path /path/to/volt-ui-kit
cd my-ios-app
open ios/VoltApp.xcodeproj
```

Select **VoltApp**, choose an installed iPhone Simulator, then Run. Xcode builds
and links the Rust static library automatically. Intel Macs need
`x86_64-apple-ios` instead of `aarch64-apple-ios-sim`. Deployment target is iOS 17;
that does not establish runtime coverage of all iOS 17 devices.

The template includes navigation, a scrollable list, an editable name form,
validation and a Rust callback counter. Edit **`src/lib.rs`**. Session state is
not persisted. No permission, network, telemetry or cleanup action is requested.
`--template ios` is an alternative to `--platform ios`; minimal/navigation with
`--platform ios` select the same starter. Desktop templates/defaults are unchanged.

```text
my-ios-app/
  Cargo.toml               staticlib/rlib + reusable host dependency
  src/lib.rs               your Rust state, views, callbacks, factory
  ios/
    VoltApp.xcodeproj/      ready to open; shared host source references
    App/Info.plist         bundle/scene configuration
    build-rust.sh          metadata launcher for the canonical build helper
    Tests/SmokeTests.swift generated native workflow test (not screen code)
```

The Xcode project **references**, rather than copies, the canonical Swift host
and C header in the local `volt-ui-kit-mobile` dependency. No XcodeGen, Swift
package manager, per-app Swift entry point or extra UI dependency is needed to
use a generated project. Maintain the local toolkit checkout; if it moves,
update both Cargo and Xcode references or regenerate into a fresh directory.
Paths with spaces work; toolkit paths with `$` or control characters are
rejected because Xcode expands build-setting variables. Existing destinations
are never overwritten. The host crate is experimental/local and not published.

The first Xcode build resolves `Cargo.lock`; commit it. Subsequent builds use
`--locked`. A normal `cargo build` produces a library, not a desktop executable;
`cargo volt dev` is currently desktop-only.

## Rust application contract

```rust,ignore
use volt_ui_kit_mobile::{Application, HostContext};
use volt_ui_kit_mobile::ui_kit::ui::prelude::*;

pub struct App { count: usize }
fn view(app: &App) -> Element<App> {
    vstack![
        text(format!("Count: {}", app.count)).font_size(28.),
        button("Add").key("add")
            .on_click(|app: &mut App| app.count += 1),
    ].padding(20., Edge::All).into()
}
pub fn application(host: HostContext) -> Result<Application<App>, String> {
    Application::new(host, App { count: 0 }, view)
}
volt_ui_kit_mobile::export_ios_app!(application);
```

Export one factory per static-library application. The macro supplies the
registration symbol; developers need no raw pointers or handwritten UIKit glue.
The root toolkit retains `forbid(unsafe_code)`. The separate host crate contains
the narrow, documented unsafe iOS surface/FFI boundary. Installed system SF is
read locally, not bundled; an explicit app root font overrides the host fallback.

`Application::theme_with` supplies an app-owned `UiTheme` function;
`on_appearance` optionally updates app state for literal light/dark styles.
The default is Zinc light/dark. Native editor/background colors come from the
Rust theme, not app-specific hardcoded Swift colors or field IDs.

```text
Your Rust app: state + view + typed callbacks
                        |
                        v
volt-ui-kit-mobile: Application + PlatformEvent + semantic snapshot
                        |
                        v
volt-ui-kit: MobileView -> existing Runtime -> layout / text / drawing
                        |
          +-------------+-------------------+
          |                                 |
          v                                 v
  iOS adapter (implemented)          Android adapter (future)
  UIKit lifecycle/safe area         Activity + native window lifecycle
  native keyboard / AX proxies     native keyboard / TalkBack / back
  CAMetalLayer + Metal surface     Vulkan surface + Gradle/NDK packaging
```

The typed platform event layer handles touch, focus/text, scroll, appearance,
resize and suspension without UIKit/JNI types. The iOS wire decoder bounds
messages, rejects unknown fields/operations and invalid geometry. The header
and Rust host check ABI version 1 before creating a surface. This is an
**experimental internal host ABI**, not a stable third-party ABI guarantee.
UIKit owns the Metal layer; all host calls and destruction stay on its main
thread. Checked numeric handles replace exported object pointers. The surface
is dropped before UIKit releases its layer. Host failures stay visible.

## Run the original Field Notes example

The existing three-screen prototype now uses this **same reusable host**:

```sh
open examples/ios/VoltIOS.xcodeproj
cargo test --locked --manifest-path examples/ios/rust/Cargo.toml \
  --target-dir target/ios-prototype
cargo test --locked --manifest-path crates/volt-ui-kit-mobile/Cargo.toml \
  --target-dir target/ios-prototype
```

Choose **VoltIOS** and an installed iPhone Simulator. Maintainers need XcodeGen
only when regenerating the checked-in example project after editing
`examples/ios/project.yml`; developers using the template do not need it.

- [Field Notes screens/state](../examples/ios/rust/src/lib.rs)
- [Safe mobile view](../src/mobile.rs)
- [Reusable application and semantics](../crates/volt-ui-kit-mobile/src/application.rs)
- [Bounded platform events](../crates/volt-ui-kit-mobile/src/protocol.rs)
- [iOS surface/FFI adapter](../crates/volt-ui-kit-mobile/src/ios.rs)
- [Shared UIKit host](../crates/volt-ui-kit-mobile/apple/VoltUIHost.swift)
- [Project generator](../src/bin/volt/ios.rs)
- [Field Notes UI tests](../examples/ios/Tests/PrototypeTests.swift)

## Test a generated project

From the generated app directory, after initial dependency resolution:

```sh
cargo test --locked
xcodebuild -project ios/VoltApp.xcodeproj -scheme VoltApp \
  -destination 'platform=iOS Simulator,name=iPhone 18 Pro' test
```

Use an installed Simulator name. The generated UI test checks the Rust counter,
navigation, native text editing, Save and scrolling to the last row. Shared
headless tests cover callbacks, Unicode values, suspension, font recomposition,
app-controlled theming, invalid resize atomicity, clipped semantics and secret
withholding. Field Notes additionally checks Cancel, validation, rotation,
keyboard layout, background/resume and idle redraw stability.

The generated app disables code signing. For a physical device, enable Code
Signing Allowed, select your Development Team and a unique bundle ID in Xcode.
Unsigned device compilation/linking is **not** physical-device acceptance.
Signing, provisioning and App Store distribution remain developer-owned.

## GPU drawing versus presentation

**A Metal surface does not prove GPU rasterization.** The host queries Vello's
compute and indirect-execution requirements. Compatible adapters draw on the
GPU. The tested Simulator lacks indirect execution, so the starter explicitly
calls `.allow_software_fallback(true)`: software drawing with Metal presentation.
`Application` defaults to GPU-only; fallback is an app choice, not a silent
change to desktop GPU defaults. Inspect the `render-proof` diagnostic for the
actual backend, drawing mode and presented-frame count. `VOLT_UI_TRACE=1` adds
renderer diagnostics. Rendering is event-driven, with at most three surface
retries and no perpetual display link. Mobile animations are currently settled.

See Apple's [Simulator Metal restrictions](https://developer.apple.com/documentation/metal/developing-metal-apps-that-run-in-simulator).
Physical iPhone/iPad GPU, memory/energy and frame-time acceptance are pending.

## Android direction and compatibility

Android was considered in the extraction, not bolted onto UIKit. The common
Rust application/event/snapshot layer has no Apple-specific public types. The
iOS adapter is target-gated; wgpu selects Vulkan on Android. Cross-compiling
this Rust layer does **not** provide an Activity, working keyboard or APK.
`cargo volt new --platform android` currently returns an explicit unsupported
message rather than generating a misleading empty project.

A future Android vertical slice needs:

1. Activity and native-window lifecycle, surface loss/recreation and insets.
2. Vulkan adapter/device validation, pause/resume and Android back behavior.
3. Native IME/composition/selection and accessibility (TalkBack), not key-event
   simulation as a substitute for editable forms.
4. Gradle/NDK/JNI packaging, signing and a reusable Rust-first template.
5. Emulator **and physical-device** interaction, GPU/energy and compatibility QA.

The Android [GameActivity text-input guide](https://developer.android.com/games/agdk/game-activity/use-text-input)
and [native-window API](https://developer.android.com/ndk/reference/group/a-native-window)
are reference points, not a committed dependency choice. No new Android OS
library, Kotlin UI framework or runtime dependency was added here.

- Default dependencies and legacy Painter/Viewport APIs remain unchanged.
- `native` / `native-gpu` desktop re-exports retain their paths and types.
- `rendering` shares drawing without a desktop host; `rendering-gpu` adds GPU.
- `mobile` opts into the safe touch view. Use it alone on iOS, not desktop
  services or `--all-features` for an iOS target.
- The reusable host is a separate crate, not a new dependency of desktop apps.
- Volt's live vendored snapshot is not modified; compatibility uses a copy.

## Remaining mobile limits

This is not yet a production iOS SDK. Pending: physical-device GPU/energy QA;
kinetic scrolling/bounce, gesture arbitration/multitouch and native back swipe;
full IME/dictation/font fallback and multiline input; Dynamic Type, VoiceOver
and Switch Control acceptance; persistence/restoration/multi-window and mobile
services; signing/release coverage and distributable SDK packaging. Secure
fields deliberately remain read-only in the native adapter because secret
values are withheld from snapshots; existing values cannot be cleared by a
blank editor. The active plain single-line field alone uses a native editor.
