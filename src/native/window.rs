use super::RasterPainter;
use crate::{
    declarative::{Bindings, Document, DocumentError, Handlers, InputOutcome, Runtime},
    ui::Element,
    Theme, Viewport,
};
use softbuffer::{Context, Surface};
use std::{
    io::Read,
    num::NonZeroU32,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, ModifiersState, NamedKey},
    window::{Window, WindowAttributes, WindowId},
};

pub struct NativeOptions {
    pub title: String,
    pub size: [u32; 2],
    /// F5 reload source. No file watcher or periodic polling is installed.
    pub document_path: Option<PathBuf>,
}
impl Default for NativeOptions {
    fn default() -> Self {
        Self {
            title: "Volt UI Kit".into(),
            size: [860, 640],
            document_path: None,
        }
    }
}
impl NativeOptions {
    /// Select custom chrome without adding fields to the legacy options struct.
    pub fn with_chrome(self, chrome: WindowChrome) -> WindowOptions {
        WindowOptions {
            native: self,
            chrome,
            reduced_motion: false,
        }
    }
}

/// Window decoration policy. Native remains the backwards-compatible default.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WindowChrome {
    #[default]
    Native,
    /// macOS full-size content with a transparent title bar and hidden title.
    /// Native traffic lights remain available; callers reserve header space.
    /// Other platforms retain their native decorated title bar as a fallback.
    Overlay,
    /// No system decorations. Applications must supply their own window controls.
    Borderless,
}

/// Additive host configuration for effects and custom chrome.
///
/// Explicit declarative drag regions are honored without making controls drag
/// the window. `VOLT_UI_REDUCED_MOTION=1` also disables animation at startup.
pub struct WindowOptions {
    pub native: NativeOptions,
    pub chrome: WindowChrome,
    pub reduced_motion: bool,
}
impl WindowOptions {
    pub fn with_chrome(mut self, chrome: WindowChrome) -> Self {
        self.chrome = chrome;
        self
    }
    /// Opt out of animations while retaining the final appearance.
    pub fn with_reduced_motion(mut self, reduced_motion: bool) -> Self {
        self.reduced_motion = reduced_motion;
        self
    }
}
impl From<NativeOptions> for WindowOptions {
    fn from(native: NativeOptions) -> Self {
        native.with_chrome(WindowChrome::Native)
    }
}
impl Default for WindowOptions {
    fn default() -> Self {
        NativeOptions::default().into()
    }
}

/// Application state and callbacks remain owned by the caller; update projects
/// state into named document bindings after each successful activation.
pub fn run<C: 'static>(
    options: NativeOptions,
    source: &str,
    context: C,
    handlers: Handlers<'static, C>,
    update: impl Fn(&C, &mut Bindings) + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    run_with_options(options.into(), source, context, handlers, update)
}
/// Run a JSON declaration with additive chrome and animation configuration.
pub fn run_with_options<C: 'static>(
    options: WindowOptions,
    source: &str,
    context: C,
    handlers: Handlers<'static, C>,
    update: impl Fn(&C, &mut Bindings) + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut bindings = Bindings::default();
    update(&context, &mut bindings);
    let runtime = Runtime::new(Document::parse(source)?, &handlers, &bindings)?;
    run_host(
        options,
        runtime,
        context,
        handlers,
        bindings,
        Content::Document(Box::new(update)),
    )
}
/// Run a Rust view with direct callbacks and application-owned state. The body is
/// composed initially and after actual actions, never for hover/resize/redraw.
/// Invalid subsequent views retain the previous tree/handlers, not previous state.
pub fn run_ui<C: 'static>(
    options: NativeOptions,
    context: C,
    view: impl Fn(&C) -> Element<C> + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    run_ui_with_options(options.into(), context, view)
}
/// Run a Rust view with custom chrome or reduced-motion configuration.
///
/// Animation frames update presentation only and do not recompose the view or
/// invoke application callbacks. See [`WindowChrome::Overlay`] for platform
/// behavior and native traffic-light clearance requirements.
pub fn run_ui_with_options<C: 'static>(
    options: WindowOptions,
    context: C,
    view: impl Fn(&C) -> Element<C> + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    if options.native.document_path.is_some() {
        return Err(
            "run_ui does not accept a JSON reload path; rebuild after Rust source changes".into(),
        );
    }
    let (document, handlers) = view(&context).build()?.into_parts();
    let bindings = Bindings::default();
    let runtime = Runtime::new(document, &handlers, &bindings)?;
    run_host(
        options,
        runtime,
        context,
        handlers,
        bindings,
        Content::Rust(Box::new(view)),
    )
}
fn run_host<C: 'static>(
    options: WindowOptions,
    mut runtime: Runtime,
    context: C,
    handlers: Handlers<'static, C>,
    bindings: Bindings,
    content: Content<C>,
) -> Result<(), Box<dyn std::error::Error>> {
    let reduced_motion = reduced_motion_enabled(
        options.reduced_motion,
        std::env::var("VOLT_UI_REDUCED_MOTION").ok().as_deref(),
    );
    runtime.set_reduced_motion(reduced_motion);
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App {
        options,
        runtime,
        context,
        handlers,
        bindings,
        content,
        window: None,
        surface: None,
        surface_size: None,
        trace: std::env::var("VOLT_UI_TRACE").is_ok_and(|value| value == "1"),
        pending_release: None,
        painter: RasterPainter::new(Viewport::new(860., 640., 1.).unwrap()),
        pointer: None,
        modifiers: ModifiersState::empty(),
        activation_key: None,
        error: None,
        frames: 0,
        animation_clock: AnimationClock::new(Instant::now()),
        focused: true,
        occluded: false,
        reduced_motion,
    };
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.error {
        return Err(error.into());
    }
    Ok(())
}
type Update<C> = Box<dyn Fn(&C, &mut Bindings)>;
type View<C> = Box<dyn Fn(&C) -> Element<C>>;
enum Content<C> {
    Document(Update<C>),
    Rust(View<C>),
}
impl<C: 'static> Content<C> {
    fn after_input(
        &self,
        outcome: InputOutcome,
        context: &C,
        runtime: &mut Runtime,
        handlers: &mut Handlers<'static, C>,
        bindings: &mut Bindings,
    ) -> Result<(), DocumentError> {
        if !outcome.activated {
            return Ok(());
        }
        let mut next_bindings = Bindings::default();
        match self {
            Self::Document(update) => {
                update(context, &mut next_bindings);
                runtime
                    .document()
                    .validate_connections(handlers, &next_bindings)?;
            }
            Self::Rust(view) => {
                let (document, next_handlers) = view(context).build()?.into_parts();
                runtime.replace_document(document, &next_handlers, &next_bindings)?;
                // No fallible operation between tree install and handler install.
                *handlers = next_handlers;
            }
        }
        *bindings = next_bindings;
        Ok(())
    }
}
struct App<C> {
    options: WindowOptions,
    runtime: Runtime,
    context: C,
    handlers: Handlers<'static, C>,
    bindings: Bindings,
    content: Content<C>,
    window: Option<Arc<Window>>,
    surface: Option<Surface<Arc<Window>, Arc<Window>>>,
    surface_size: Option<[u32; 2]>,
    trace: bool,
    pending_release: Option<Instant>,
    painter: RasterPainter,
    pointer: Option<[f32; 2]>,
    modifiers: ModifiersState,
    activation_key: Option<NamedKey>,
    error: Option<String>,
    frames: u64,
    animation_clock: AnimationClock,
    focused: bool,
    occluded: bool,
    reduced_motion: bool,
}
impl<C: 'static> App<C> {
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl ToString) {
        self.error = Some(error.to_string());
        event_loop.exit();
    }
    fn redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
    fn animations_can_run(&self) -> bool {
        self.focused
            && !self.occluded
            && self.window.as_ref().is_some_and(|window| {
                let size = window.inner_size();
                size.width != 0 && size.height != 0
            })
    }
    fn advance_animations(&mut self, now: Instant) -> bool {
        let can_run = self.animations_can_run();
        // Targets are resolved during paint. While paused, even newly resolved
        // targets must snap instead of creating another invisible animation.
        self.runtime
            .set_reduced_motion(self.reduced_motion || !can_run);
        if !can_run {
            self.runtime.settle_animations();
            self.animation_clock.reset();
            return false;
        }
        self.runtime
            .advance_animations(self.animation_clock.elapsed(now))
    }
    fn settle_animations(&mut self) {
        self.runtime
            .set_reduced_motion(self.reduced_motion || !self.animations_can_run());
        self.runtime.settle_animations();
        self.animation_clock.reset();
    }
    fn layout(&mut self) -> Result<(), String> {
        let Some(w) = &self.window else { return Ok(()) };
        let size = w.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        };
        let v = Viewport::new(
            size.width as f32,
            size.height as f32,
            w.scale_factor() as f32,
        )
        .ok_or("invalid viewport")?;
        self.painter.set_viewport(v);
        self.runtime
            .layout(v, &self.bindings, &mut self.painter)
            .map_err(|e| e.to_string())?;
        if let Some([x, y]) = self.pointer {
            self.runtime.pointer_move(x, y);
        }
        Ok(())
    }
    fn after_input(&mut self, outcome: InputOutcome) -> Result<(), String> {
        let result = self.content.after_input(
            outcome,
            &self.context,
            &mut self.runtime,
            &mut self.handlers,
            &mut self.bindings,
        );
        match result {
            Err(error) if matches!(self.content, Content::Rust(_)) => {
                eprintln!("Rust UI rejected: {error}; retaining previous tree and callbacks (application state is not rolled back)");
                self.runtime.cancel_input();
                self.activation_key = None;
                if let Some(w) = &self.window {
                    w.set_title(&format!(
                        "{} — UI rejected (see stderr)",
                        self.options.native.title
                    ));
                }
                self.redraw();
                return Ok(());
            }
            Err(error) => return Err(error.to_string()),
            Ok(()) if outcome.activated && matches!(self.content, Content::Rust(_)) => {
                self.activation_key = None;
                if let Some(w) = &self.window {
                    w.set_title(&self.options.native.title);
                }
            }
            Ok(()) => {}
        }
        if outcome.activated {
            self.layout()?;
            self.redraw();
        } else if outcome.changed {
            self.redraw();
        } else {
            self.pending_release = None;
        }
        Ok(())
    }
    fn render(&mut self) -> Result<(), String> {
        // The host clock also advances here so a delayed OS redraw never paints
        // an obsolete animation frame. This never rebuilds Rust application UI.
        self.advance_animations(Instant::now());
        let started = self.trace.then(Instant::now);
        self.layout()?;
        let layout_done = self.trace.then(Instant::now);
        let Some(w) = &self.window else { return Ok(()) };
        let size = w.inner_size();
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return Ok(());
        };
        let theme = Theme::dark();
        let shape_calls_before = self.painter.shape_draw_calls;
        let text_misses_before = self.painter.text_cache_misses;
        let batches = self.runtime.paint(&mut self.painter, &theme);
        let paint_done = self.trace.then(Instant::now);
        let pixmap = self.painter.rasterize(&batches, theme.background)?;
        let raster_done = self.trace.then(Instant::now);
        let surface = self.surface.as_mut().ok_or("missing surface")?;
        let new_size = [size.width, size.height];
        if needs_surface_resize(self.surface_size, new_size) {
            surface.resize(width, height).map_err(|e| e.to_string())?;
            self.surface_size = Some(new_size);
        }
        let mut buffer = surface.buffer_mut().map_err(|e| e.to_string())?;
        for (dst, pixel) in buffer.iter_mut().zip(pixmap.pixels()) {
            *dst = (u32::from(pixel.red()) << 16)
                | (u32::from(pixel.green()) << 8)
                | u32::from(pixel.blue());
        }
        let pack_done = self.trace.then(Instant::now);
        buffer.present().map_err(|e| e.to_string())?;
        self.frames += 1;
        self.animation_clock.presented(Instant::now());
        if let Some(start) = started {
            let presented = Instant::now();
            let layout = layout_done.unwrap();
            let paint = paint_done.unwrap();
            let raster = raster_done.unwrap();
            let pack = pack_done.unwrap();
            let ms = |a: Instant, b: Instant| b.duration_since(a).as_secs_f64() * 1000.;
            let release = self.pending_release.take().map_or_else(
                || "none".to_owned(),
                |released| format!("{:.3}", ms(released, presented)),
            );
            // Present return is measured, not the display's scanout/photon time.
            eprintln!("VOLT_UI_TRACE frame={} pixels={}x{} layout_ms={:.3} paint_ms={:.3} raster_ms={:.3} pack_ms={:.3} present_ms={:.3} total_ms={:.3} mouse_release_to_present_ms={} shape_calls={} text_misses={}",
                self.frames, size.width, size.height, ms(start, layout), ms(layout, paint),
                ms(paint, raster), ms(raster, pack), ms(pack, presented), ms(start, presented),
                release, self.painter.shape_draw_calls - shape_calls_before,
                self.painter.text_cache_misses - text_misses_before);
        }
        Ok(())
    }
    fn reload(&mut self) {
        let Some(path) = &self.options.native.document_path else {
            return;
        };
        let result = load_document(path)
            .map_err(|e| e.to_string())
            .and_then(|source| {
                self.runtime
                    .reload(&source, &self.handlers, &self.bindings)
                    .map_err(|e| e.to_string())
            });
        if let Some(w) = &self.window {
            match result {
                Ok(()) => {
                    w.set_title(&self.options.native.title);
                    self.activation_key = None;
                    self.redraw();
                }
                Err(error) => {
                    eprintln!("{}: {error}", path.display());
                    w.set_title(&format!(
                        "{} — Reload rejected (see stderr)",
                        self.options.native.title
                    ));
                }
            }
        }
    }
}
impl<C: 'static> ApplicationHandler for App<C> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| -> Result<_, String> {
            let w = Arc::new(
                event_loop
                    .create_window(window_attributes(&self.options))
                    .map_err(|e| e.to_string())?,
            );
            let context = Context::new(w.clone()).map_err(|e| e.to_string())?;
            let surface = Surface::new(&context, w.clone()).map_err(|e| e.to_string())?;
            Ok((w, surface))
        })();
        match result {
            Ok((w, surface)) => {
                self.window = Some(w);
                self.surface = Some(surface);
                self.surface_size = None;
                self.runtime.invalidate_layout();
                self.focused = true;
                self.occluded = false;
                self.animation_clock.reset();
                self.redraw();
            }
            Err(e) => self.fail(event_loop, e),
        }
    }
    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.surface = None;
        self.surface_size = None;
        self.pending_release = None;
        self.window = None;
        self.runtime.cancel_input();
        self.settle_animations();
        self.pointer = None;
        self.activation_key = None;
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self.window.as_ref().is_none_or(|w| w.id() != id) {
            return;
        }
        // Advance visuals before hit testing and dispatch, but never delay input
        // until an animation completes. Idle hosts have no periodic timer.
        let visual_changed = self.advance_animations(Instant::now());
        if visual_changed && !self.runtime.is_animating() {
            self.redraw();
        }
        let result = (|| -> Result<(), String> {
            match event {
                WindowEvent::CloseRequested => {
                    eprintln!(
                        "Closed after {} frames; {} shaped-text cache misses",
                        self.frames, self.painter.text_cache_misses
                    );
                    event_loop.exit();
                }
                WindowEvent::RedrawRequested => self.render()?,
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    self.runtime.cancel_input();
                    self.activation_key = None;
                    self.runtime.invalidate_layout();
                    self.layout()?;
                    self.redraw();
                }
                WindowEvent::CursorMoved { position, .. } => {
                    self.layout()?;
                    self.pointer = Some([position.x as f32, position.y as f32]);
                    if self
                        .runtime
                        .pointer_move(position.x as f32, position.y as f32)
                    {
                        self.redraw();
                    }
                }
                WindowEvent::CursorLeft { .. } => {
                    self.pointer = None;
                    if self.runtime.pointer_leave() {
                        self.redraw();
                    }
                }
                WindowEvent::MouseInput {
                    state,
                    button: MouseButton::Left,
                    ..
                } => {
                    if self.trace && state == ElementState::Released {
                        self.pending_release = Some(Instant::now());
                    }
                    self.layout()?;
                    if let Some([x, y]) = self.pointer {
                        match state {
                            ElementState::Pressed => {
                                self.runtime.pointer_move(x, y);
                                self.activation_key = None;
                                if self.runtime.drag_region_hit(x, y) {
                                    // A drag must not leave a click or keyboard
                                    // activation captured underneath the header.
                                    self.runtime.cancel_input();
                                    if let Some(window) = &self.window {
                                        if let Err(error) = window.drag_window() {
                                            eprintln!("Window drag unavailable: {error}");
                                        }
                                    }
                                } else {
                                    self.runtime.pointer_down(x, y);
                                }
                                self.redraw();
                            }
                            ElementState::Released => {
                                let outcome = self
                                    .runtime
                                    .pointer_up_outcome(x, y, &mut self.handlers, &mut self.context)
                                    .map_err(|e| e.to_string())?;
                                self.after_input(outcome)?;
                            }
                        }
                    } else {
                        self.runtime.cancel_input();
                        self.redraw();
                    }
                }
                WindowEvent::Focused(false) => {
                    self.focused = false;
                    self.pointer = None;
                    self.activation_key = None;
                    self.runtime.cancel_input();
                    self.settle_animations();
                    self.redraw();
                }
                WindowEvent::Focused(true) => {
                    self.focused = true;
                    self.animation_clock.reset();
                }
                WindowEvent::Occluded(occluded) => {
                    self.occluded = occluded;
                    if occluded {
                        self.runtime.cancel_input();
                        self.activation_key = None;
                        self.settle_animations();
                    } else {
                        self.animation_clock.reset();
                        self.redraw();
                    }
                }
                WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
                WindowEvent::KeyboardInput { event, .. } => {
                    if let Key::Named(key) = event.logical_key {
                        match key {
                            NamedKey::F5
                                if event.state == ElementState::Pressed && !event.repeat =>
                            {
                                self.reload()
                            }
                            NamedKey::Tab if event.state == ElementState::Pressed => {
                                self.activation_key = None;
                                self.runtime.focus_next(self.modifiers.shift_key());
                                self.redraw();
                            }
                            NamedKey::Enter | NamedKey::Space => match event.state {
                                ElementState::Pressed
                                    if !event.repeat && self.activation_key.is_none() =>
                                {
                                    self.activation_key = Some(key);
                                    if self.runtime.key_down() {
                                        self.redraw();
                                    }
                                }
                                ElementState::Released if self.activation_key == Some(key) => {
                                    self.activation_key = None;
                                    let outcome = self
                                        .runtime
                                        .key_up_outcome(&mut self.handlers, &mut self.context)
                                        .map_err(|e| e.to_string())?;
                                    self.after_input(outcome)?;
                                }
                                _ => {}
                            },
                            NamedKey::Escape => {
                                self.activation_key = None;
                                if self.runtime.cancel_input() {
                                    self.redraw();
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.fail(event_loop, e);
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let changed = self.advance_animations(now);
        let active = self.animations_can_run() && self.runtime.is_animating();
        if active && self.animation_clock.frame_due(now) {
            self.redraw();
            // Consume this deadline even if the platform coalesces the redraw;
            // never repeatedly wake at a deadline that is already in the past.
            self.animation_clock.presented(now);
        } else if changed && !active {
            self.redraw();
        }
        event_loop.set_control_flow(match self.animation_clock.deadline(now, active) {
            Some(deadline) => ControlFlow::WaitUntil(deadline),
            None => ControlFlow::Wait,
        });
    }
}

// Return (native decorations, integrated macOS title bar). Keeping this decision
// separate permits platform-independent fallback tests without an OS window.
fn chrome_policy(chrome: WindowChrome, macos: bool) -> (bool, bool) {
    match chrome {
        WindowChrome::Native => (true, false),
        WindowChrome::Overlay => (true, macos),
        WindowChrome::Borderless => (false, false),
    }
}
fn window_attributes(options: &WindowOptions) -> WindowAttributes {
    let (decorations, integrated_titlebar) =
        chrome_policy(options.chrome, cfg!(target_os = "macos"));
    let attributes = Window::default_attributes()
        .with_title(&options.native.title)
        .with_inner_size(LogicalSize::new(
            options.native.size[0],
            options.native.size[1],
        ))
        .with_min_inner_size(LogicalSize::new(320, 240))
        .with_decorations(decorations);
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::WindowAttributesExtMacOS;
        if integrated_titlebar {
            attributes
                .with_titlebar_transparent(true)
                .with_fullsize_content_view(true)
                .with_title_hidden(true)
        } else {
            attributes
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        debug_assert!(!integrated_titlebar);
        attributes
    }
}

fn reduced_motion_enabled(option: bool, environment: Option<&str>) -> bool {
    option || environment == Some("1")
}

const ANIMATION_FRAME: Duration = Duration::from_nanos(16_666_667);
struct AnimationClock {
    epoch: Instant,
    next_frame: Option<Instant>,
}
impl AnimationClock {
    fn new(now: Instant) -> Self {
        Self {
            epoch: now,
            next_frame: None,
        }
    }
    // Runtime takes an absolute monotonic timestamp, not a per-frame delta.
    fn elapsed(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.epoch)
    }
    fn deadline(&mut self, now: Instant, active: bool) -> Option<Instant> {
        if !active {
            self.next_frame = None;
        } else if self.next_frame.is_none() {
            self.next_frame = Some(now + ANIMATION_FRAME);
        }
        self.next_frame
    }
    fn frame_due(&self, now: Instant) -> bool {
        self.next_frame.is_some_and(|deadline| now >= deadline)
    }
    fn presented(&mut self, now: Instant) {
        self.next_frame = Some(now + ANIMATION_FRAME);
    }
    fn reset(&mut self) {
        // Do not rewind Runtime's absolute clock across focus/suspend cycles.
        self.next_frame = None;
    }
}

fn needs_surface_resize(previous: Option<[u32; 2]>, next: [u32; 2]) -> bool {
    previous != Some(next)
}

pub fn load_document(path: &std::path::Path) -> Result<String, std::io::Error> {
    if !path.metadata()?.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "document must be a regular file",
        ));
    }
    let file = std::fs::File::open(path)?;
    let mut source = String::new();
    file.take(262_145).read_to_string(&mut source)?;
    if source.len() > 262_144 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "document exceeds 256 KiB limit",
        ));
    }
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn window_options_are_additive_and_native_stays_default() {
        // This exhaustive literal protects the original public options contract.
        let native = NativeOptions {
            title: "Original client".into(),
            size: [640, 480],
            document_path: None,
        };
        let default = WindowOptions::from(native);
        assert_eq!(default.chrome, WindowChrome::Native);
        assert!(!default.reduced_motion);
        assert_eq!(default.native.title, "Original client");
        let overlay = NativeOptions::default()
            .with_chrome(WindowChrome::Overlay)
            .with_reduced_motion(true);
        assert_eq!(overlay.chrome, WindowChrome::Overlay);
        assert!(overlay.reduced_motion);
        assert_eq!(overlay.native.size, [860, 640]);
    }
    #[test]
    fn chrome_decisions_preserve_native_controls_and_nonmac_fallback() {
        assert_eq!(chrome_policy(WindowChrome::Native, true), (true, false));
        assert_eq!(chrome_policy(WindowChrome::Overlay, true), (true, true));
        assert_eq!(chrome_policy(WindowChrome::Overlay, false), (true, false));
        assert_eq!(
            chrome_policy(WindowChrome::Borderless, true),
            (false, false)
        );
        assert_eq!(
            chrome_policy(WindowChrome::Borderless, false),
            (false, false)
        );
        let borderless =
            window_attributes(&NativeOptions::default().with_chrome(WindowChrome::Borderless));
        assert!(!borderless.decorations);
        let overlay =
            window_attributes(&NativeOptions::default().with_chrome(WindowChrome::Overlay));
        assert!(overlay.decorations);
    }
    #[test]
    fn reduced_motion_environment_only_opts_in() {
        assert!(reduced_motion_enabled(false, Some("1")));
        assert!(reduced_motion_enabled(true, Some("0")));
        assert!(!reduced_motion_enabled(false, Some("0")));
        assert!(!reduced_motion_enabled(false, Some("true")));
        assert!(!reduced_motion_enabled(false, None));
    }
    #[test]
    fn animation_clock_waits_when_idle_and_bounds_active_frame_pacing() {
        let start = Instant::now();
        let mut clock = AnimationClock::new(start);
        assert_eq!(clock.elapsed(start), Duration::ZERO);
        assert_eq!(clock.deadline(start, false), None);
        assert_eq!(clock.deadline(start, true), Some(start + ANIMATION_FRAME));
        // Repeated input must not move the outstanding frame deadline farther out.
        assert_eq!(
            clock.deadline(start + Duration::from_millis(3), true),
            Some(start + ANIMATION_FRAME)
        );
        assert!(!clock.frame_due(start + Duration::from_millis(15)));
        assert!(clock.frame_due(start + ANIMATION_FRAME));
        clock.presented(start + ANIMATION_FRAME);
        assert_eq!(
            clock.deadline(start + ANIMATION_FRAME, true),
            Some(start + ANIMATION_FRAME * 2)
        );
        assert_eq!(
            clock.elapsed(start + Duration::from_millis(25)),
            Duration::from_millis(25)
        );
        assert_eq!(
            clock.deadline(start + Duration::from_millis(25), false),
            None
        );
        clock.reset();
        assert_eq!(
            clock.elapsed(start + Duration::from_secs(60)),
            Duration::from_secs(60)
        );
    }
    #[test]
    fn late_frame_deadlines_rebase_instead_of_catching_up_in_a_busy_loop() {
        let start = Instant::now();
        let mut clock = AnimationClock::new(start);
        clock.deadline(start, true);
        let delayed = start + Duration::from_secs(3);
        assert!(clock.frame_due(delayed));
        assert_eq!(clock.elapsed(delayed), Duration::from_secs(3));
        clock.presented(delayed);
        assert_eq!(
            clock.deadline(delayed, true),
            Some(delayed + ANIMATION_FRAME)
        );
        assert!(!clock.frame_due(delayed));
        // Idle clears the outstanding deadline rather than continuously waking.
        assert_eq!(clock.deadline(delayed, false), None);
        assert!(!clock.frame_due(delayed + Duration::from_secs(5)));
    }
    #[test]
    fn surface_resize_is_only_needed_for_new_dimensions() {
        assert!(needs_surface_resize(None, [800, 600]));
        assert!(!needs_surface_resize(Some([800, 600]), [800, 600]));
        assert!(needs_surface_resize(Some([800, 600]), [1200, 900]));
    }
    #[test]
    fn bounded_file_read() {
        let path =
            std::env::temp_dir().join(format!("volt-ui-kit-document-{}.json", std::process::id()));
        std::fs::write(&path, " ".repeat(262_145)).unwrap();
        assert!(load_document(&path).is_err());
        std::fs::write(&path, "{}").unwrap();
        assert_eq!(load_document(&path).unwrap(), "{}");
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod rust_view_tests {
    use super::*;
    use crate::ui::{button, column, text};
    use std::{cell::Cell, rc::Rc};
    #[derive(Default)]
    struct State {
        count: usize,
        reject: bool,
        captures: Vec<usize>,
    }
    fn view(state: &State) -> Element<State> {
        let epoch = state.count;
        column![
            text(format!("Count: {}", state.count)).font_size(if state.reject { 1. } else { 20. }),
            button("Add").key("add").on_click(move |state: &mut State| {
                state.count += 1;
                state.captures.push(epoch);
            }),
        ]
        .into()
    }
    fn fixture() -> (
        State,
        Runtime,
        Handlers<'static, State>,
        Content<State>,
        Rc<Cell<usize>>,
    ) {
        let state = State::default();
        let calls = Rc::new(Cell::new(0));
        let body_calls = calls.clone();
        let content = Content::Rust(Box::new(move |state| {
            body_calls.set(body_calls.get() + 1);
            view(state)
        }));
        let Content::Rust(body) = &content else {
            unreachable!()
        };
        let (doc, handlers) = body(&state).build().unwrap().into_parts();
        let runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
        (state, runtime, handlers, content, calls)
    }
    fn layout(runtime: &mut Runtime, painter: &mut RasterPainter) {
        runtime
            .layout(
                Viewport::new(600., 400., 1.).unwrap(),
                &Bindings::default(),
                painter,
            )
            .unwrap();
    }
    #[test]
    fn body_runs_after_dispatch_not_hover_render_resize_or_cancelled_releases() {
        let (mut state, mut runtime, mut handlers, content, calls) = fixture();
        let mut values = Bindings::default();
        let mut painter = RasterPainter::new(Viewport::new(600., 400., 1.).unwrap());
        layout(&mut runtime, &mut painter);
        for _ in 0..3 {
            runtime.pointer_move(10., 10.);
            runtime.paint(&mut painter, &Theme::dark());
            runtime.invalidate_layout();
            layout(&mut runtime, &mut painter);
        }
        runtime.focus_next(false);
        runtime.key_down();
        runtime.cancel_input();
        let cancelled = runtime.key_up_outcome(&mut handlers, &mut state).unwrap();
        content
            .after_input(cancelled, &state, &mut runtime, &mut handlers, &mut values)
            .unwrap();
        let [x, y, w, h] = runtime
            .boxes()
            .iter()
            .find(|b| b.id == "add")
            .unwrap()
            .bounds
            .bounds();
        runtime.pointer_down(x + w * 0.5, y + h * 0.5);
        let cancelled = runtime
            .pointer_up_outcome(999., 999., &mut handlers, &mut state)
            .unwrap();
        assert!(cancelled.changed);
        assert!(!cancelled.activated);
        content
            .after_input(cancelled, &state, &mut runtime, &mut handlers, &mut values)
            .unwrap();
        assert_eq!(calls.get(), 1);
        assert_eq!(state.count, 0);
        for _ in 0..2 {
            runtime.key_down();
            let activated = runtime.key_up_outcome(&mut handlers, &mut state).unwrap();
            content
                .after_input(activated, &state, &mut runtime, &mut handlers, &mut values)
                .unwrap();
            layout(&mut runtime, &mut painter);
        }
        assert_eq!(calls.get(), 3);
        assert_eq!(state.count, 2);
        assert_eq!(state.captures, [0, 1]); // fresh closures, not stale first body
        assert_eq!(runtime.focused(), Some("add"));
    }
    #[test]
    fn responsive_resize_changes_layout_without_calling_view_or_replacing_callbacks() {
        use crate::ui::prelude::*;
        let calls = Rc::new(Cell::new(0));
        let body_calls = calls.clone();
        let content = Content::Rust(Box::new(move |_: &usize| {
            body_calls.set(body_calls.get() + 1);
            let mut captured_calls = 0;
            stack![
                text("First").key("first").width(40.).height(20.),
                button("Add").key("add").width(40.).height(20.).on_click(
                    move |state: &mut usize| {
                        captured_calls += 1;
                        *state = captured_calls;
                    }
                ),
            ]
            .direction(responsive(Direction::Vertical).at(Breakpoint::Md, Direction::Horizontal))
            .gap(8., Axis::Both)
            .into()
        }));
        let Content::Rust(body) = &content else {
            unreachable!()
        };
        let mut state = 0;
        let (doc, mut handlers) = body(&state).build().unwrap().into_parts();
        let mut values = Bindings::default();
        let mut runtime = Runtime::new(doc, &handlers, &values).unwrap();
        let mut painter = RasterPainter::new(Viewport::new(600., 400., 1.).unwrap());
        for (width, scale) in [(767., 1.), (768., 1.5), (769., 2.), (767., 2.)] {
            let viewport = Viewport::new(width * scale, 400. * scale, scale).unwrap();
            painter.set_viewport(viewport);
            runtime.cancel_input();
            runtime.invalidate_layout();
            runtime.layout(viewport, &values, &mut painter).unwrap();
            let [x, y, w, h] = runtime
                .boxes()
                .iter()
                .find(|b| b.id == "add")
                .unwrap()
                .bounds
                .bounds();
            assert_eq!(
                [x / scale, y / scale],
                if width >= 768. { [48., 0.] } else { [0., 28.] }
            );
            if runtime.focused().is_none() {
                runtime.focus_next(false);
            }
            runtime.pointer_move(x + w / 2., y + h / 2.);
            // Resize/hover/render dispatch no application action.
            content
                .after_input(
                    InputOutcome::default(),
                    &state,
                    &mut runtime,
                    &mut handlers,
                    &mut values,
                )
                .unwrap();
            runtime.paint(&mut painter, &Theme::dark());
            assert_eq!(calls.get(), 1);
            assert_eq!(runtime.focused(), Some("add"));
        }
        assert_eq!(state, 0);
        runtime.key_down();
        let outcome = runtime.key_up_outcome(&mut handlers, &mut state).unwrap();
        content
            .after_input(outcome, &state, &mut runtime, &mut handlers, &mut values)
            .unwrap();
        assert_eq!(state, 1);
        assert_eq!(calls.get(), 2); // Recompose only after the dispatched action.
    }
    #[test]
    fn animation_ticks_never_recompose_and_clicks_dispatch_before_motion_finishes() {
        use crate::ui::{Animation, Interaction};
        let calls = Rc::new(Cell::new(0));
        let body_calls = calls.clone();
        let content = Content::Rust(Box::new(move |_: &usize| {
            body_calls.set(body_calls.get() + 1);
            button("Save")
                .key("save")
                .hover_style(|style| style.opacity(0.4))
                .animate_on(Interaction::Hover, Animation::linear().duration_ms(200))
                .on_click(|state: &mut usize| *state += 1)
                .into()
        }));
        let Content::Rust(body) = &content else {
            unreachable!()
        };
        let mut state = 0;
        let (document, mut handlers) = body(&state).build().unwrap().into_parts();
        let mut bindings = Bindings::default();
        let mut runtime = Runtime::new(document, &handlers, &bindings).unwrap();
        let mut painter = RasterPainter::new(Viewport::new(600., 400., 1.).unwrap());
        layout(&mut runtime, &mut painter);
        runtime.paint(&mut painter, &Theme::dark());
        let [x, y, width, height] = runtime.boxes()[0].bounds.bounds();
        let x = x + width / 2.;
        let y = y + height / 2.;
        runtime.pointer_move(x, y);
        runtime.paint(&mut painter, &Theme::dark());
        assert!(runtime.is_animating());
        runtime.pointer_down(x, y);
        let outcome = runtime
            .pointer_up_outcome(x, y, &mut handlers, &mut state)
            .unwrap();
        assert!(outcome.activated);
        assert_eq!(state, 1); // No waiting for the 200 ms animation.
        content
            .after_input(outcome, &state, &mut runtime, &mut handlers, &mut bindings)
            .unwrap();
        layout(&mut runtime, &mut painter);
        runtime.paint(&mut painter, &Theme::dark());
        for timestamp in [16, 32, 64, 128, 256, 512] {
            runtime.advance_animations(Duration::from_millis(timestamp));
            runtime.paint(&mut painter, &Theme::dark());
        }
        assert!(!runtime.is_animating());
        assert_eq!(calls.get(), 2); // Initial body and the one actual action only.
        assert_eq!(state, 1);
    }
    #[test]
    fn drag_headers_include_text_but_never_enabled_or_disabled_controls() {
        use crate::ui::prelude::*;
        let element: Element<()> = panel![
            text("Header title").key("title").width(100.).height(30.),
            button("Save")
                .key("save")
                .width(100.)
                .height(30.)
                .on_click(|_| {}),
            button("Disabled")
                .key("disabled")
                .width(100.)
                .height(30.)
                .enabled(false)
                .on_click(|_| {}),
        ]
        .key("header")
        .drag_region(true)
        .into();
        let (document, handlers) = element.build().unwrap().into_parts();
        let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
        let mut painter = RasterPainter::new(Viewport::new(600., 400., 1.).unwrap());
        layout(&mut runtime, &mut painter);
        for (id, draggable) in [("title", true), ("save", false), ("disabled", false)] {
            let [x, y, width, height] = runtime
                .boxes()
                .iter()
                .find(|item| item.id == id)
                .unwrap()
                .bounds
                .bounds();
            assert_eq!(
                runtime.drag_region_hit(x + width / 2., y + height / 2.),
                draggable,
                "{id}"
            );
        }
        assert!(runtime.drag_region_hit(590., 10.)); // Blank header space.
        assert!(!runtime.drag_region_hit(-1., 10.));
    }
    #[test]
    fn unrelated_topmost_layer_blocks_window_drag_from_header_behind_it() {
        use crate::ui::prelude::*;
        let element: Element<()> = zstack![
            panel![]
                .key("header")
                .width(300.)
                .height(120.)
                .drag_region(true),
            panel![text("Overlay")].key("cover").width(80.).height(40.),
        ]
        .into();
        let (document, handlers) = element.build().unwrap().into_parts();
        let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
        let mut painter = RasterPainter::new(Viewport::new(600., 400., 1.).unwrap());
        layout(&mut runtime, &mut painter);
        assert!(!runtime.drag_region_hit(300., 200.)); // Visible overlay, not header.
        assert!(runtime.drag_region_hit(160., 150.)); // Exposed header beside overlay.
    }
    #[test]
    fn rejected_body_preserves_tree_handlers_focus_and_mutated_state_then_recovers() {
        let (mut state, mut runtime, mut handlers, content, calls) = fixture();
        let mut values = Bindings::default();
        let mut painter = RasterPainter::new(Viewport::new(600., 400., 1.).unwrap());
        layout(&mut runtime, &mut painter);
        runtime.focus_next(false);
        state.reject = true;
        runtime.key_down();
        let action = runtime.key_up_outcome(&mut handlers, &mut state).unwrap();
        assert!(content
            .after_input(action, &state, &mut runtime, &mut handlers, &mut values)
            .is_err());
        assert_eq!(state.count, 1);
        assert_eq!(calls.get(), 2);
        assert_eq!(runtime.focused(), Some("add"));
        assert!(runtime.boxes().iter().any(|b| b.text == "Count: 0"));
        state.reject = false;
        runtime.key_down();
        let action = runtime.key_up_outcome(&mut handlers, &mut state).unwrap();
        content
            .after_input(action, &state, &mut runtime, &mut handlers, &mut values)
            .unwrap();
        layout(&mut runtime, &mut painter);
        assert_eq!(state.count, 2);
        assert_eq!(state.captures, [0, 0]); // retained old closure
        assert!(runtime.boxes().iter().any(|b| b.text == "Count: 2"));
        assert_eq!(calls.get(), 3);
    }
    #[test]
    fn json_path_is_rejected_before_rust_body_or_event_loop() {
        let calls = Rc::new(Cell::new(0));
        let body_calls = calls.clone();
        let result = run_ui(
            NativeOptions {
                document_path: Some("ignored.json".into()),
                ..Default::default()
            },
            State::default(),
            move |state| {
                body_calls.set(body_calls.get() + 1);
                view(state)
            },
        );
        assert!(result.is_err());
        assert_eq!(calls.get(), 0);
    }
    #[test]
    fn additive_options_preserve_json_path_rejection_before_rust_body() {
        let calls = Rc::new(Cell::new(0));
        let body_calls = calls.clone();
        let result = run_ui_with_options(
            NativeOptions {
                document_path: Some("ignored.json".into()),
                ..Default::default()
            }
            .with_chrome(WindowChrome::Overlay)
            .with_reduced_motion(true),
            State::default(),
            move |state| {
                body_calls.set(body_calls.get() + 1);
                view(state)
            },
        );
        assert!(result.is_err());
        assert_eq!(calls.get(), 0);
    }
}
