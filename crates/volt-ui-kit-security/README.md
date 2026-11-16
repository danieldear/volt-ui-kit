# Volt UI Kit Security

Opt-in secure storage, device-owner confirmation and Sign in with Apple for
Volt UI Kit apps.
It's a separate crate so `volt-ui-kit` keeps forbidding unsafe code and apps
that don't need these services don't download Apple's bindings.

| Module | What it does | macOS | Linux, Windows |
|---|---|---|---|
| `secrets` | Store, read and remove small secrets (API keys, tokens, passwords) | Login Keychain | `Unsupported` for now |
| `local_auth` | Ask the person to confirm with Touch ID or their account password | LocalAuthentication | `Unsupported` for now |
| `apple_id` | Sign in with Apple and check whether the person still allows access | AuthenticationServices | `Unsupported` |

Planned: Secret Service on Linux, Credential Manager and Windows Hello on
Windows, then passkeys and saved passwords.

## Use

```toml
[dependencies]
volt-ui-kit-security = { path = "crates/volt-ui-kit-security" }
```

```rust,no_run
use volt_ui_kit_security::{
    local_auth::{self, Policy},
    secrets::{self, Secret, SecretKey},
};

let key = SecretKey::new("com.example.maps", "mapbox")?;
secrets::store(&key, Secret::new("pk.your-key".to_owned()), |saved| {
    // Background thread: send `saved` to the UI with a UiSender message.
    let _ = saved;
})?;
local_auth::confirm("show your saved keys", Policy::BiometricsOrPassword, |confirmed| {
    let _ = confirmed; // Ok(()) when the person confirmed
})?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

- Callbacks run on a background thread. Forward results to the UI with a
  message; don't edit UI state from them.
- Secret operations run in order on one worker, so a read after a save sees
  the saved value, and a Keychain prompt never freezes the window.
- `Secret` zeroes its buffer when dropped and prints as `Secret(<redacted>)`.
- Secrets are 1 to 2,560 bytes, the smallest limit among the platform stores.
- `local_auth::check` tells you, without showing anything, whether a policy
  can be used and whether the Mac has Touch ID.

## How macOS behaves

- Items are generic passwords in the login Keychain, kept on this Mac and not
  synced. Keychain Access lists each item under its service name.
- macOS remembers which app saved an item. A rebuilt development binary counts
  as a different app, so reading an item it saved before shows an "allow
  access" prompt; choose Always Allow. Signed releases keep their identity
  and don't prompt.
- The confirmation sheet reads "<App> is trying to <reason>." Pass a short,
  lowercase reason that finishes that sentence.
- A confirmation is a gate in your app, not encryption. Keep the data itself
  in `secrets`.

## Sign in with Apple

Apple allows it only for a signed app that has the Sign in with Apple
entitlement and an embedded provisioning profile. `apple_id::entitled()` tells
you at runtime; `sign_in` returns `NotEntitled` otherwise, so `cargo run`
builds can't sign in. Package with:

```sh
cargo volt release --entitlements app.entitlements \
  --provisioning-profile ~/Downloads/MyApp.provisionprofile
```

- On macOS Apple grants it to Mac App Store apps and development builds, not
  to Developer ID apps; those use Apple's web sign-in flow instead.
- `sign_in` must start on the main thread, from a deliberate action such as a
  button. The sheet appears over the app's key window.
- The credential's `user` is the stable account key. Email and name arrive
  only on the first sign-in.
- The identity token is a claim until your server verifies it: Apple's
  signature, issuer, audience (your bundle ID), expiry and nonce.
- Call `credential_state` at launch and sign the person out when Apple says
  `Revoked`.

The [signin-demo](signin-demo/README.md) walks through the developer account
setup and runs the whole flow.

## Demo

```sh
cd crates/volt-ui-kit-security
cargo run --example vault
```

Save a key, quit, and open the demo again: the key is still there. Show asks
for Touch ID or your password before the key appears. Remove deletes it.

## Tests

`cargo test` runs on every platform. The Keychain round trip writes one
item to the login Keychain and removes it, so it runs only on request:

```sh
cargo test secrets -- --ignored
```
