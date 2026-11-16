# Sign in with Apple demo

A Volt window with a Sign in with Apple button. After signing in it shows
your Apple ID user identifier, plus your name and email the first time. It
keeps the identifier in the Keychain, and at the next launch it asks Apple
whether you still allow access.

Apple only lets a signed app with a provisioning profile use Sign in with
Apple, so `cargo run` opens the window but the button stays disabled.

## One-time setup in your Apple Developer account

1. **App ID.** In Certificates, Identifiers & Profiles → Identifiers, register
   an App ID with an explicit bundle ID (for example
   `com.yourdomain.volt-signin-demo`). Under Capabilities, tick Sign In with
   Apple.
2. **This Mac as a device.** In Devices, register this Mac with its
   Provisioning UDID (System Information → Hardware shows it, or run
   `system_profiler SPHardwareDataType | grep "Provisioning UDID"`).
3. **Provisioning profile.** In Profiles, add a **macOS App Development**
   profile for that App ID, your Apple Development certificate and this Mac,
   then download it. Keep it outside the repository and outside Downloads
   (macOS may stop tools reading that folder); `.provisionprofile` files are
   ignored by Git.

Apple offers Sign in with Apple to Mac App Store apps and development builds
only. A Developer ID profile can't include it, so `cargo volt release` stops
with a message if you try one.

## Build and run

From the repository root:

```sh
VOLT_BUNDLE_IDENTIFIER=com.yourdomain.volt-signin-demo \
cargo run --features dev-tools --bin cargo-volt -- volt release \
  --identity "Apple Development" \
  --manifest-path crates/volt-ui-kit-security/signin-demo/Cargo.toml \
  --entitlements crates/volt-ui-kit-security/signin-demo/signin.entitlements \
  --provisioning-profile ~/signing/Volt_Sign_in_demo.provisionprofile \
  --no-notarize --no-dmg

open "crates/volt-ui-kit-security/signin-demo/target/release/bundle/Volt Sign in.app"
```

`cargo volt release` checks that the profile is for the same bundle ID and
grants Sign in with Apple, embeds it, and signs the app with your Apple
Development certificate. The result runs only on the Macs in the profile.

## What to try

- Sign in. The first time, Apple asks whether to share or hide your email.
- Quit and open the app again: it still knows you and shows Access: Authorized.
- In System Settings → your name → Sign-In & Security → Sign in with Apple,
  stop using Apple ID with the app. Open the demo again: it signs you out.

A real app sends the identity token to its server, which checks the token's
signature against Apple's public keys, its issuer, audience (the bundle ID),
expiry and nonce before trusting the user identifier.
