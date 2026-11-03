# Roadmap

## Now

- **Build a real app with the kit.** Rebuild the mdstar Markdown reader and
  editor on Volt UI Kit, and fix every gap it uncovers in the kit itself.
- **Rich text.** Styled runs within one paragraph (bold, italic, code, links),
  selectable and copyable read-only text, and syntax colouring in the editor.
- **API documentation** for every public item, written once the APIs above
  settle.
- **Publication readiness.** Maintain the public repository, protected CI and
  generated documentation; publish to crates.io only after release approval.

## Next

- Upgrade to current Vello and wgpu.
- Glass over app content: blur and tint the app's own content behind panels on
  the GPU, not just the desktop behind the window.
- Validate the native runtime on Windows and Linux, and screen readers
  (VoiceOver, Narrator, Orca), IME and trackpads on each platform.
- Keyboard navigation through combobox and command palette results;
  independent range-slider thumbs for assistive technology.
- Multiple windows, state restoration and the full OS quit lifecycle.
- File panels and notifications on Windows and Linux.

## Later

- Harden the [experimental web host](docs/web-prototype.md): broaden browser,
  accessibility and input coverage beyond the live Catalog previews.
- Finer-grained updates and partial GPU redraws.
- Layout animation, springs and gestures; variable-height virtual lists.
- [Embedded content and extensible maps](#embedded-content-and-extensible-maps-planned).
- On-device AI integrations on macOS.
- Publish to crates.io once the APIs have held up in real applications.

## Embedded content and extensible maps (planned)

Build reusable embedding infrastructure once, then integrate map providers
through optional adapters. This is planned work, not an existing map component:
the current native capabilities report both `maps` and `embedded_web` as
unavailable. Start with macOS and explicit capability reporting elsewhere;
extend hosting to iOS, other desktop platforms and the web as each path is
implemented and validated.

```text
Rust application declarations and callbacks
    |
    v
Shared Volt Maps API: state, events and capabilities
    |
    +-- Optional Apple Maps adapter
    +-- Optional Google Maps / Mapbox / MapLibre adapters
    +-- Application-owned custom adapter
                |
                v
Embedded-content host: native view / browser DOM / rendering surface
```

### 1. Reusable embedded-content hosting

- Support mounting, updating and unmounting externally rendered content in
  Volt layouts. Provide platform-native view hosting and browser DOM hosting;
  investigate external GPU-surface integration where an engine supports it.
- Coordinate bounds, DPI, scrolling, visibility, clipping, rounded corners,
  overlays and z-order with Volt's layout and renderer. Native views must not
  be assumed to compose like ordinary GPU-painted elements; document limits.
- Integrate pointer/touch gestures, keyboard focus and accessibility. Manage
  UI-thread ownership, suspension/resumption, resource cleanup and GPU
  synchronization without idle redraw loops or leaks.
- Keep this foundation reusable for webviews, video and other native controls,
  rather than building a map-specific windowing exception.

### 2. Shared Rust map contract

- Define application-owned camera state, movement commands, markers, lines,
  polygons, selection/camera callbacks, loading state and typed errors.
- Provide coordinate-to-screen conversion for Volt overlays and explicit
  capability discovery. Unsupported features must not silently do nothing.
- Keep search, geocoding and routing as separate optional services. Allow
  provider-specific extensions rather than limiting every engine to the
  smallest common feature set.

### 3. First complete adapter and validation

- Validate the hosting foundation and map contract together with a native
  Apple MapKit adapter on macOS; extend to iOS with its platform host.
- Add a polished Catalog example with Rust declarations and callbacks, an
  actual interactive map, markers, camera changes and error/unsupported states.
- Test resizing/DPI, scrolling, clipping, Volt overlays, focus/accessibility,
  lifecycle cleanup and resource usage. Preserve existing desktop behavior and
  legacy Volt compatibility; do not claim parity on unvalidated platforms.

### 4. Additional providers and developer extensibility

- Publish a provider-adapter interface so applications or separate crates can
  add engines without modifying Volt's core. Add Google Maps, Mapbox or
  MapLibre integrations incrementally for their supported platform paths.
- Keep SDKs and dependencies opt-in. A Rust-facing adapter may use a native
  SDK or JavaScript bridge; it is not a claim of a Rust-only map renderer.
- Distinguish changing a compatible tile source from changing map engines:
  arbitrary providers are not interchangeable through a URL alone.
- Keep credentials and service configuration developer-owned, retain required
  attribution, and document provider usage requirements. Never embed server
  secrets in browser/WASM assets.
