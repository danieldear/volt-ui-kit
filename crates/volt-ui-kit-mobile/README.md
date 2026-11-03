# Volt UI Kit mobile host (experimental)

Reusable Rust `Application`, typed platform events and semantic snapshots, with
UIKit/Metal and experimental SurfaceView/Vulkan adapters. Application screens, state and callbacks stay
Rust. The canonical Swift host and build helper live under `apple/`; generated
Xcode projects reference them directly, rather than copying app-specific glue.

See the [iOS guide](../../docs/ios-prototype.md) for creation, testing and limits.
This is an unpublished local/path crate; not a stable mobile SDK or host ABI.
The root toolkit retains its safe-code policy and unchanged default deps.
See the [Android guide](../../docs/android-prototype.md) for its host/template.
Do not infer physical-device GPU or production support from a target check.
