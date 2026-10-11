# Local notifications and shared tray commands (Unreleased)

Updated: 2026-10-11. This is a bounded macOS-first slice, **not the complete
notification/action lifecycle**. Catalog is the acceptance app; older showcases
and Volt's separately vendored toolkit are unchanged.

## Independent opt-in feature

Enable `native-notifications` for macOS UserNotifications; it includes `native`,
but does not require `native-desktop` or GPU. Conversely, neither desktop menus nor
GPU implicitly enable notifications. Other platforms/builds return `Unsupported`.
One new pinned framework crate, `objc2-user-notifications 0.3.2`, uses narrowly
selected features and existing `block2 0.6.2`. Rust 1.87 and toolkit
`forbid(unsafe_code)` remain; dependencies still contain native FFI internally.
Default core dependencies remain unchanged.

```text
Deliberate user action
        |
main-thread + packaged .app preflight
        |
request ALERT permission ------> granted / denied / native error
        |                           (decision then, not live settings)
developer explicitly sends
        |
validated stable ID + silent local content + optional one-shot delay
        |
OS completion -----------------> accepted / native error
        |
OS policy: authorization, Focus, foreground, preferences...
        |
visible notification?          NOT guaranteed by accepted
```

## API

```rust,no_run
use std::time::Duration;
use volt_ui_kit::native::notifications::{self, LocalNotification, NotificationError};

fn explicit_user_action() -> Result<(), NotificationError> {
    notifications::request_authorization(|decision| {
        // Marshal the decision into your UI's bounded mailbox + UiSender.
        // Do not mutate UI state from this unspecified OS callback thread.
        let _ = decision;
    })?;
    Ok(())
}

fn send_after_authorization() -> Result<(), NotificationError> {
    let note = LocalNotification::new(
        "build.ready", "Build ready", "All checks passed.",
    ).after(Duration::from_secs(8));
    notifications::send(&note, |accepted| {
        // Accepted is NOT proof of a banner or visible delivery.
        let _ = accepted;
    })
}

fn remove_after_submission_completed() -> Result<(), NotificationError> {
    notifications::remove("build.ready")
}
```

The direct API intentionally does **not** own application permission policy.
Construction, validation, sending and removal never call authorization for you.
Keep the request in a deliberate developer/user action. Catalog requires its last
authorization result to be granted before enabling send; applications can supply
their own policy.

| API / capability | Contract |
|---|---|
| `capabilities()` | Compiled adapter, not permission, bundle readiness or runtime availability |
| `authorization`, `local_requests`, `removal` | Available on macOS with this feature |
| `settings_query`, `actions`, `foreground_presentation` | Explicitly false in this slice |
| `request_authorization(callback)` | Alert-only, explicit decision; granted/denied/native error |
| `send(&note, callback)` | Silent local request; callback success means OS accepted it |
| `remove(id)` | Requests removal of matching pending AND delivered entries; no completion receipt |

IDs are nonempty ASCII letters/digits/dot/hyphen/underscore, at most 64 bytes.
Titles are nonblank, at most 128 UTF-8 bytes, without control characters.
Bodies are nonblank, at most 4096 bytes; newline is the only allowed control.
No truncation, attachments, sounds, badges, remote push or arbitrary categories.
No delay means immediate submission; an explicit delay must be 1 second–24 hours,
one-shot only. Matching IDs use native replacement semantics; callers own the
number of distinct IDs they schedule.

Start operations on the native main thread, from a running `.app` with a bundle
identifier. The preflight runs **before** initializing the notification center,
so unbundled Cargo executables return `MissingAppBundle`, not a notification
center assertion. Invalid configuration, unsupported builds and wrong-thread
calls return synchronous errors **without invoking the callback**.

Otherwise a completion runs at most once on an unspecified OS thread. Callbacks
must not panic, block or mutate UI directly; there is no promised completion
deadline. Non-null native error pointers become generic `NativeFailure`; this
safe adapter does not dereference them to extract native details. Serialized
submission/removal is application-owned: removal does not cancel an in-flight add.
Closing the app does not automatically cancel submitted OS requests.

No custom UserNotifications delegate is installed. Therefore no forced foreground
banner or notification action-click callback is promised. A fresh permission
decision is not a settings subscription; later OS changes can invalidate it.

## Packaged Catalog test

Use a fresh output path and **your own identity** for your application:

```sh
cargo build --release --locked \
  --features native-gpu,native-desktop,native-notifications --example catalog
cargo run --locked --features dev-tools --bin cargo-volt -- bundle \
  --binary target/release/examples/catalog \
  --output VoltDesktopQA.app --name 'Volt Desktop QA' \
  --identifier dev.example.voltdesktopqa --version 0.3.0 \
  --minimum-system-version 11.0
open VoltDesktopQA.app
```

Choose **Desktop services** in Catalog. No permission or test notification is
requested at launch. The page requests alert permission explicitly, submits a
generic local message eight seconds later, and removes its fixed
`volt.catalog.test` ID. Hide/switch away from Catalog before the delay when testing
background presentation. Focus/preferences can still suppress it. Packaging does
not grant permission or provide Developer ID/notarization; see
[packaging](MACOS_PACKAGING.md).

The page uses a one-slot completion mailbox plus event-driven `UiSender`.
If the wake queue is full, another queued Catalog message drains the slot.
No notification worker thread, unbounded result queue or idle polling is added.
Only one OS operation is started at a time. Reset is disabled while busy and
resets the UI witness only, not OS authorization or requests. Stale generations
cannot replace a newer witness; errors remain visible.

With `native-desktop`, the V-shaped template status item shares the application
registry's navigation, count, send and removal IDs. Enablement is rechecked and
modal guards still apply. Its Count item updates the same visible counter as the
button, menu and shortcut. This is a tray **menu**, not icon-click callbacks,
hide-to-tray/window restoration or multi-window behavior.

## Verification and remaining acceptance

Automated checks exercise content bounds, capability precision, synchronous
errors without callback, wrong-thread/unsupported safety, one-shot completion,
stale results, busy/permission registry guards and explicit accepted-not-displayed
status. Catalog's software and hardware-offscreen GPU page tests cover the new
page at both scales. The all-feature/all-target suite passed 499 tests (13
ignored); 29 doctests passed (one ignored), strict Clippy passed, and Rust 1.87
plus Linux cross-compilation passed. Default-only tests passed 40 (one ignored);
native without notifications passed its four notification-contract and 19 Catalog
tests. These are not native notification delivery tests.

An actual main-thread **unbundled** executable exercised authorization, valid
send and removal: all returned `MissingAppBundle` before center initialization,
with no permission callback, prompt or notification. Receipt:
`work/desktop-services-2026-10-11/preflight.log` (local, ignored).

The disposable QA bundle was built with a locally verified ad-hoc signature.
At this writing, native permission/banner/tray-click acceptance is **pending**:
the Mac was locked when launch was attempted. The user authorized the disposable
app's notification permission; no permission prompt was operated while locked.
No OS settings were changed or native delivery claimed.

Still required: native permission decision, background display observation,
removal and tray count/enablement acceptance; denied/revoked policy on additional
identities/OS versions; broader signing/distribution acceptance. Settings/action
delegates, Windows/Linux adapters and native multi-window lifecycle are separate
work, not completed by this slice.

Primary API references:
[Apple authorization](https://developer.apple.com/documentation/usernotifications/asking-permission-to-use-notifications),
[notification center](https://developer.apple.com/documentation/usernotifications/unusernotificationcenter),
[pinned safe bindings](https://docs.rs/objc2-user-notifications/0.3.2/objc2_user_notifications/struct.UNUserNotificationCenter.html).
