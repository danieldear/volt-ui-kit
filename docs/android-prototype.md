# Experimental Android host and Rust template

A Rust-first Android vertical slice, **not production Android support**. The
same Volt elements, layout/runtime, application state and typed callbacks drive
the screen. A single reusable Java OS adapter supplies SurfaceView/Vulkan,
lifecycle, insets, the native single-line editor and basic accessibility proxies.
No per-app Java/Kotlin screen code, Compose, WebView or second UI framework is
needed. App design is deliberately left to the application developer.

## Create and build

Requires Cargo/Rust 1.89+, JDK 17, Python 3, Android SDK platform 36, SDK build
tools 35.0.0+ and NDK **29.0.13599879**. The template pins Android Gradle Plugin
8.13.2 and Gradle 8.13, rather than selecting dynamic versions. The checked-in
Gradle wrapper verifies the distribution SHA-256. See the official
[AGP compatibility table](https://developer.android.com/build/releases/agp-8-13-0-release-notes).
This is a tested SDK 36 baseline, not a claim to target the newest Android SDK.

From a current toolkit checkout:

```sh
cargo install --locked --path . --features dev-tools --bin cargo-volt
rustup target add aarch64-linux-android
cargo volt new my-android-app --platform android --kit-path /path/to/volt-ui-kit
cd my-android-app/android
# Set JAVA_HOME to JDK 17 and ANDROID_HOME to your Android SDK first.
./gradlew assembleDebug
adb -s YOUR_DEVICE_SERIAL install -r build/outputs/apk/debug/VoltApp-debug.apk
adb -s YOUR_DEVICE_SERIAL shell am start \
  -n dev.volt.apps.my_android_app/dev.volt.host.VoltActivity
```

On Windows use `gradlew.bat` and, if needed, `-PvoltPython=python`. SDK components
and Rust targets must be installed by the developer; the host does not silently
accept SDK licenses or install tools. Android Studio can install the components.
Build dependencies are downloaded normally by Cargo/Gradle; the app itself adds
no network permission, telemetry or network requests.

The starter targets **API 30+**, compile/target SDK 36, with arm64-v8a by default.
For an x86_64 emulator, install `x86_64-linux-android` and pass
`-PvoltAbi=x86_64`. Only one ABI is built per invocation. The NDK helper requests
16 KiB linker alignment; this is not device/page-size acceptance coverage.
Debug signing is for local testing only. Release signing and Play submission
remain developer-owned. Moving the toolkit requires updating both Cargo and
Gradle source references.

## What you edit

```text
my-android-app/
  Cargo.toml             Rust cdylib/rlib, shared mobile host dependency
  src/lib.rs             your Rust state, views, callbacks and factory
  android/
    AndroidManifest.xml  application identity and OS configuration
    build.gradle         references the canonical Java host; calls Rust helper
    settings.gradle
    gradlew / gradlew.bat pinned, checksum-verified Gradle launcher
    gradle/wrapper/
```

There is **no per-app Java/Kotlin application source**. `cargo volt new` refuses
existing destinations and unsupported desktop/mobile combinations. The first
build resolves `Cargo.lock`; commit it. Later builds are locked. `cargo volt dev`
is still desktop-only; Android uses the generated Gradle build.

The template reuses the iOS starter's ordinary Rust navigation/list/form/counter
composition, but exports `export_android_app!(application)` from a **cdylib**.
Its factory returns `Application<State>`. Use `.back_with(can_go_back, action)`
to handle platform Back in Rust; without it, Back follows Activity behavior.
Theme and font fallback remain app-controlled; Android registers installed
Roboto locally instead of redistributing a system font.

```text
Rust app -> Application + PlatformEvent -> MobileView -> existing Runtime
                         ^                                  |
                         |                                  v
 Android main Looper -> bounded JNI events           shared drawing batches
    Activity lifecycle / SurfaceView                        |
    native EditText / basic virtual AX nodes                v
    system bars + IME insets                         Vulkan presentation
                                                    /                 \
                                     Vello on compatible GPU      explicit software
                                                                  drawing fallback
```

## Rendering and lifecycle boundaries

- JNI entry points check the main Looper, validate handle IDs and bound events
  and font data. The common strict decoder rejects unknown operations/fields.
- `ANativeWindow_fromSurface` supplies a retained reference. Its Vulkan renderer
  is dropped **before** the reference is released on detach/destroy.
- Surface detach/recreation preserves Rust application state in the same
  Activity. Pause suppresses rendering/input; resume recreates the surface as
  needed. Orientation changes are handled without recreating the Activity.
  Process death resets session-only state; restoration is not implemented.
- System/keyboard insets resize the Rust viewport, rather than covering the
  editable form. The active single-line field uses native EditText and keeps
  composition native until commit. Secure fields remain read-only, because
  snapshots intentionally withhold their values. Multiline/complete IME support
  is not claimed.
- Rendering is event-driven, with bounded surface retries, no permanent timer
  and no display loop. Android's redraw-needed callback supplies initial pixels.
- Android SurfaceView can advertise only `Inherit` alpha. The native window is
  explicitly opaque, and the renderer selects an advertised alpha mode instead
  of assuming `Opaque` is supported. Desktop opaque mode remains preferred.

The starter explicitly permits software fallback. This covers adapters without
Vello compute/indirect support **and CPU adapters such as SwiftShader**; it does
not silently change the desktop GPU constructor. The diagnostic reports the
actual adapter and software drawing versus Vello drawing. A Vulkan surface,
SwiftShader compute execution or successful APK build does **not** establish
physical hardware GPU rendering. The emulator's software Vulkan driver exhibited
a native crash during Vello buffer uploads, so running compute rasterization on
that CPU adapter is deliberately avoided under the opt-in fallback policy.

The platform boundaries follow Android's [SurfaceView lifetime contract](https://developer.android.com/reference/android/view/SurfaceView)
and [native-window API](https://developer.android.com/ndk/reference/group/a-native-window).
The shared host is experimental, not a stable JNI ABI or complete Android SDK.

## Validation and repeatable smoke test

The generated SDK 36 arm64 APK was built and run on the API 36 emulator:
Rust callbacks, navigation, native editing/Save, Back, background/resume, last-row
scrolling and idle redraw checks passed. A separate live check passed rotation,
surface recreation, dark-mode changes and Rust state preservation. Rendering used software drawing with
Vulkan presentation on SwiftShader; physical GPU acceptance remains pending.

The generated APK is built from Rust source and the canonical Java host. For an
already-installed **fresh starter**, a bounded test uses an explicitly selected
emulator/device, checks Rust callbacks, navigation, native editing/Save, Back,
background/resume, scrolling to the last row and idle frame stability:

```sh
python3 /path/to/volt-ui-kit/crates/volt-ui-kit-mobile/android/smoke-test.py \
  --serial YOUR_DEVICE_SERIAL --package dev.volt.apps.my_android_app \
  --output /path/to/qa-output
```

It does not install/uninstall apps, request permissions, delete personal files
or reset a device. It uses only the generated starter and removes its own
`/sdcard/volt-starter-smoke.xml` UI dump. Screenshots and the last semantic dump
are retained in the selected output directory. Run on a disposable starter,
not an app with personal data. Start with count zero and the default name.

Shared Rust tests cover events, secure value withholding, theme/font behavior,
back navigation, suspension and clipping. Root tests retain desktop coverage;
compatibility is checked against a copied Volt consumer, never by editing the
live vendor. Android-only JNI/NDK dependencies do not enter desktop/default
builds. See the [iOS guide](ios-prototype.md) for the other adapter.

## Still pending before Android support

Physical-device Vulkan rendering and recovery, memory/energy/frame-time profiling;
full IME/selection/dictation and accessibility/TalkBack acceptance; native
predictive Back, multi-touch/kinetic gestures; state restoration and mobile
services; multi-ABI release bundles, signing and store acceptance; and broader
SDK/device/16 KiB-page coverage. This first template/build helper does not make
those production guarantees.
