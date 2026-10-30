# Volt Web Host — experimental

Reusable, opt-in WASM hosting for **actual Volt components**, not HTML/CSS
reimplementations. The default crate uses `volt-ui-kit/rendering`; its optional
`gpu` feature uses the same Vello renderer with asynchronous initialization.
Desktop/default consumers do not depend on this crate or browser bindings.

```text
Rust state + ui builders + callbacks
                |
            WebView<C>           shared layout/input/paint runtime
                |
          BrowserHost<C>         canvas surface / optional WebGPU
                |
          web/volt-host.js       DOM events, text/IME proxy, semantic tree,
                                bounded RAF and observer teardown
```

The application exports a small wasm-bindgen wrapper around its generic Rust
host. See the runnable [web example](../../examples/web/src/lib.rs) and
[setup/limitations guide](../../docs/web-prototype.md). No React, DOM-based
replacement controls, desktop window loop, Swift or Java is involved.

`WebEvent::decode` validates browser messages. Hosts accept logical coordinates
and bounded viewports (8MP, 8192px maximum, scale 0.25–4). Password values never
appear in the semantic snapshot, including its selection metadata. Passwords
use a write-only `type=password` keyboard/IME sink, cleared after committed edits
and on blur/disposal. Existing Rust secrets are never loaded into that sink;
copy/cut never export password text. Code views likewise use a small caret-anchored
sink rather than copying their virtualized document to the DOM. The application
receives the existing `CodeInput` events and owns document edits and undo.

Public text fields synchronize DOM UTF-16 selection with valid Rust UTF-8 byte
boundaries. Native composition is painted as a draft; commit is separate and
blur, hiding/removal and read-only gates cancel stale input. Clipboard events
for code are synchronous browser copy/cut/paste events, not background clipboard
polls or permission requests. A basic ARIA tree supports keyboard button activation,
selection tabs and accessible names; it is **not full screen-reader certification**.

`BrowserHost::new(..., true)` asynchronously requests WebGPU and returns errors.
For software fallback, mount `false` on a **new canvas**: browsers do not permit
switching an existing canvas between WebGPU and 2D contexts. `attachVolt` reports
the actual renderer and releases handlers/observers/state through `dispose()`.
Software drawing uses tiny-skia in Rust and ImageData only for presentation.

No background timer: redraw is requested after changes or while Volt reports
active animation, and stops offscreen/in a hidden page. No network, storage,
permissions or OS integrations are requested by this host.
