//! Reusable iOS-only C boundary. UIKit retains the layer until host destruction.
//! This is the only pointer-bearing adapter; application and toolkit code stay safe.
use crate::{
    application::HostedApplication, Application, HostContext, PlatformEvent, HOST_ABI_VERSION,
};
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    ffi::{c_char, c_void, CString},
    panic::{catch_unwind, AssertUnwindSafe},
};
use volt_ui_kit::rendering::{register_font, wgpu, PresentStatus, SurfaceRenderer};
struct Host {
    // Drop the surface before UIKit releases its CAMetalLayer.
    renderer: Option<SurfaceRenderer>,
    ui: Box<dyn HostedApplication>,
    snapshot: CString,
    frames: u64,
}
#[derive(Default)]
struct Registry {
    next: u64,
    hosts: BTreeMap<u64, Host>,
}
type Factory = Box<dyn Fn(HostContext) -> Result<Box<dyn HostedApplication>, String>>;
extern "C" {
    fn volt_app_register() -> bool;
    fn pthread_main_np() -> i32;
}
fn main_thread() -> Result<(), String> {
    // SAFETY: pthread_main_np takes no pointers and is supplied by libSystem.
    if unsafe { pthread_main_np() } != 0 {
        Ok(())
    } else {
        Err("iOS host must be used on UIKit's main thread".into())
    }
}
/// Called by export_ios_app!, not manually by application code.
#[doc(hidden)]
pub fn register<C: 'static>(factory: fn(HostContext) -> Result<Application<C>, String>) -> bool {
    FACTORY.with(|slot| match slot.try_borrow_mut() {
        Ok(mut slot) => {
            *slot = Some(Box::new(move |context| {
                factory(context).map(|app| Box::new(app) as Box<dyn HostedApplication>)
            }));
            true
        }
        Err(_) => false,
    })
}
#[no_mangle]
pub extern "C" fn volt_host_abi_version() -> u32 {
    HOST_ABI_VERSION
}
thread_local! {
    static FACTORY: RefCell<Option<Factory>> = RefCell::new(None);
    static HOSTS: RefCell<Registry> = RefCell::new(Registry::default());
    static ERROR: RefCell<CString> = RefCell::new(CString::new("").unwrap());
}
fn error(s: String) {
    ERROR.with(|e| *e.borrow_mut() = CString::new(s.replace('\0', " ")).unwrap());
}
fn guard<T: Default>(f: impl FnOnce() -> Result<T, String>) -> T {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            error(e);
            T::default()
        }
        Err(_) => {
            error("Rust prototype stopped after an internal error; relaunch the app.".into());
            T::default()
        }
    }
}
/// # Safety
/// `layer` is a live CAMetalLayer, retained by UIKit until volt_destroy.
/// font is readable for len bytes for this call only. Calls stay on main thread.
#[no_mangle]
pub unsafe extern "C" fn volt_create(layer: *mut c_void, font: *const u8, len: usize) -> u64 {
    guard(|| {
        main_thread()?;
        // SAFETY: export_ios_app! supplies this pointer-free registration symbol.
        if !unsafe { volt_app_register() } {
            return Err("Rust app registration failed".into());
        }
        if layer.is_null() || font.is_null() || len == 0 || len > 64 * 1024 * 1024 {
            return Err("Missing system font or Metal layer".into());
        }
        // SAFETY: UIKit passes a live Data buffer for the duration of this call.
        let families = register_font(unsafe { std::slice::from_raw_parts(font, len) }.to_vec())?;
        let family = families
            .iter()
            .find(|s| s.contains("SF") && s.contains("Text"))
            .or_else(|| families.first())
            .ok_or("No font family")?
            .clone();
        let ui = FACTORY.with(|slot| {
            slot.borrow()
                .as_ref()
                .ok_or("Rust app factory not registered")?(HostContext {
                font_family: family,
            })
        })?;
        let instance = wgpu::Instance::default();
        // SAFETY: the UIView retains its layer until volt_destroy drops surface.
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(layer))
        }
        .map_err(|e| e.to_string())?;
        let renderer = if ui.allow_software() {
            SurfaceRenderer::from_surface_with_software_fallback(instance, surface, false)?
        } else {
            SurfaceRenderer::from_surface(instance, surface, false)?
        };
        eprintln!(
            "Volt iOS: {} (platform Metal surface, Rust UI)",
            renderer.adapter_name
        );
        HOSTS.with(|r| {
            let mut r = r.borrow_mut();
            r.next = r.next.checked_add(1).ok_or("Host ID exhausted")?;
            if r.hosts.len() >= 16 {
                return Err("at most 16 iOS views can be mounted".into());
            }
            let id = r.next;
            r.hosts.insert(
                id,
                Host {
                    renderer: Some(renderer),
                    ui,
                    snapshot: CString::default(),
                    frames: 0,
                },
            );
            Ok(id)
        })
    })
}
#[no_mangle]
pub extern "C" fn volt_destroy(id: u64) {
    guard(|| {
        main_thread()?;
        HOSTS.with(|r| {
            r.borrow_mut().hosts.remove(&id);
        });
        Ok(())
    })
}
#[no_mangle]
pub extern "C" fn volt_error() -> *const c_char {
    ERROR.with(|e| e.borrow().as_ptr())
}
impl Host {
    fn draw(&mut self) -> Result<i32, String> {
        if self.ui.suspended() {
            return Ok(1);
        }
        let renderer = self
            .renderer
            .as_mut()
            .ok_or("GPU unavailable; relaunch the prototype")?;
        match self.ui.draw(renderer)? {
            PresentStatus::Presented => {
                self.frames += 1;
                Ok(1)
            }
            PresentStatus::Retry => Ok(2),
            PresentStatus::Lost => {
                let old = self.renderer.take().unwrap();
                self.renderer = Some(old.recreate(false)?);
                Ok(2)
            }
        }
    }
    fn event(&mut self, event: PlatformEvent) -> Result<i32, String> {
        self.ui.handle(event)?;
        self.draw()
    }
}
/// # Safety
/// `bytes` is readable for `len` bytes during this call; UIKit uses Data.
#[no_mangle]
pub unsafe extern "C" fn volt_event(id: u64, bytes: *const u8, len: usize) -> i32 {
    guard(|| {
        main_thread()?;
        if bytes.is_null() || len > 65536 {
            return Err("Invalid platform event size".into());
        }
        // SAFETY: foreign caller retains the immutable buffer throughout call.
        let v = PlatformEvent::decode(unsafe { std::slice::from_raw_parts(bytes, len) })?;
        HOSTS.with(|r| {
            r.borrow_mut()
                .hosts
                .get_mut(&id)
                .ok_or("Unknown iOS view")?
                .event(v)
        })
    })
}
/// Returned UTF-8 buffer is borrowed until the next snapshot or destroy call.
#[no_mangle]
pub extern "C" fn volt_snapshot(id: u64) -> *const c_char {
    guard(|| {
        main_thread()?;
        HOSTS.with(|r| {
            let mut r = r.borrow_mut();
            let h = r.hosts.get_mut(&id).ok_or("Unknown iOS view")?;
            let backend = h
                .renderer
                .as_ref()
                .map(|renderer| {
                    let mode = if renderer.uses_gpu_drawing() {
                        "Vello GPU drawing"
                    } else {
                        "software drawing + Metal presentation"
                    };
                    format!("{}; {}", renderer.adapter_name, mode)
                })
                .unwrap_or_else(|| "unavailable".into());
            let mut snapshot = h.ui.snapshot();
            snapshot["frames"] = Value::from(h.frames);
            snapshot["backend"] = Value::from(backend);
            h.snapshot = CString::new(snapshot.to_string()).map_err(|e| e.to_string())?;
            Ok(h.snapshot.as_ptr())
        })
    })
}
