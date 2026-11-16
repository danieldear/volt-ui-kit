# Volt UI Kit mobile host (experimental)

Reusable Rust `Application`, typed platform events and page snapshots, with
UIKit/Metal and experimental SurfaceView/Vulkan adapters. Application screens, state and callbacks stay
Rust. On iOS the canonical Swift shell under `apple/` supplies UIKit's
navigation bar and back swipe, scroll views, switches, sliders and text
editors over Volt's drawing; generated Xcode projects reference it directly,
rather than copying app-specific glue.

See the [iOS guide](../../docs/ios-prototype.md) for creation, testing and limits.
This is an unpublished local/path crate; not a stable mobile SDK or host ABI.
The root toolkit retains its safe-code policy and unchanged default deps.
See the [Android guide](../../docs/android-prototype.md) for its host/template.
The iOS shell is tested on a physical iPhone; it is still not production support.
