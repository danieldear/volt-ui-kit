# Volt UI Kit mobile host (experimental)

Reusable Rust `Application`, typed platform events and semantic snapshots, with
an iOS-only UIKit/Metal adapter. Application screens, state and callbacks stay
Rust. The canonical Swift host and build helper live under `apple/`; generated
Xcode projects reference them directly, rather than copying app-specific glue.

See the [iOS guide](../../docs/ios-prototype.md) for creation, testing and limits.
This is an unpublished local/path crate; not a stable mobile SDK or host ABI.
The root toolkit retains its safe-code policy and unchanged default deps.
Android has no native host/template yet. Do not infer Android runtime support
from the common Rust API or a successful target check.
