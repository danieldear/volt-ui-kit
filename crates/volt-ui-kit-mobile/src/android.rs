//! Experimental Android JNI adapter. All calls stay on the Android main Looper.
//! A retained ANativeWindow outlives its Vulkan surface, including on teardown.
#![allow(clippy::missing_const_for_thread_local)] // False positive in Android emulated TLS macro; initializers below are const.
use crate::{application::HostedApplication, Application, HostContext, PlatformEvent};
pub use jni;
use jni::{
    objects::{JByteArray, JObject, JString},
    sys::{jlong, jstring},
    JNIEnv,
};
use serde_json::json;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    panic::{catch_unwind, AssertUnwindSafe},
    ptr::NonNull,
};
use volt_ui_kit::rendering::{register_font, wgpu, PresentStatus, SurfaceRenderer};

struct Window(NonNull<ndk_sys::ANativeWindow>);
impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: fromSurface acquired this reference; surface is already dropped.
        unsafe {
            ndk_sys::ANativeWindow_release(self.0.as_ptr());
        }
    }
}
struct SurfaceSession {
    renderer: Option<SurfaceRenderer>,
    _window: Window,
}
struct Host {
    surface: Option<SurfaceSession>,
    app: Box<dyn HostedApplication>,
    frames: u64,
}
#[derive(Default)]
struct Registry {
    next: i64,
    hosts: BTreeMap<i64, Host>,
}
// Android emulated TLS triggers this lint inside std's macro expansion even
// with both user initializers explicitly const. Keep the exception local.
thread_local! {
    static HOSTS:RefCell<Registry>=const {RefCell::new(Registry{next:0,hosts:BTreeMap::new()})};
    static ERROR:RefCell<String>=const {RefCell::new(String::new())};
}
fn main_thread(env: &mut JNIEnv<'_>) -> Result<(), String> {
    let current = env
        .call_static_method(
            "android/os/Looper",
            "myLooper",
            "()Landroid/os/Looper;",
            &[],
        )
        .map_err(|e| e.to_string())?
        .l()
        .map_err(|e| e.to_string())?;
    let main = env
        .call_static_method(
            "android/os/Looper",
            "getMainLooper",
            "()Landroid/os/Looper;",
            &[],
        )
        .map_err(|e| e.to_string())?
        .l()
        .map_err(|e| e.to_string())?;
    if env
        .is_same_object(&current, &main)
        .map_err(|e| e.to_string())?
        && !current.is_null()
    {
        Ok(())
    } else {
        Err("Volt Android host must run on the main Looper".into())
    }
}
fn guard<T>(f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|payload| {
        let detail = payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "unknown panic".into());
        Err(format!(
            "Rust mobile host error: {detail}; relaunch the app"
        ))
    })
}
fn output(env: &mut JNIEnv<'_>, result: Result<serde_json::Value, String>) -> jstring {
    let value = match result {
        Ok(v) => v,
        Err(error) => {
            ERROR.with(|s| *s.borrow_mut() = error.clone());
            json!({"error":error})
        }
    };
    env.new_string(value.to_string())
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
#[doc(hidden)]
pub fn create<C: 'static>(
    env: &mut JNIEnv<'_>,
    font: JByteArray<'_>,
    factory: fn(HostContext) -> Result<Application<C>, String>,
) -> jlong {
    match guard(|| {
        main_thread(env)?;
        let len = env.get_array_length(&font).map_err(|e| e.to_string())?;
        if len <= 0 || len > 64 * 1024 * 1024 {
            return Err("missing or oversized Android system font".into());
        }
        let bytes = env.convert_byte_array(&font).map_err(|e| e.to_string())?;
        let families = register_font(bytes)?;
        let family = families
            .first()
            .ok_or("no font family in Android system font")?
            .clone();
        let app = factory(HostContext {
            font_family: family,
        })?;
        HOSTS.with(|r| {
            let mut r = r.borrow_mut();
            if r.hosts.len() >= 16 {
                return Err("at most 16 mobile hosts may be mounted".into());
            }
            r.next = r.next.checked_add(1).ok_or("host IDs exhausted")?;
            let id = r.next;
            r.hosts.insert(
                id,
                Host {
                    surface: None,
                    app: Box::new(app),
                    frames: 0,
                },
            );
            Ok(id)
        })
    }) {
        Ok(id) => id,
        Err(error) => {
            ERROR.with(|s| *s.borrow_mut() = error);
            0
        }
    }
}
impl Host {
    fn draw(&mut self) -> Result<i32, String> {
        if self.app.suspended() {
            return Ok(1);
        }
        let Some(surface) = &mut self.surface else {
            return Ok(1);
        };
        let renderer = surface
            .renderer
            .as_mut()
            .ok_or("Android renderer unavailable; recreate surface")?;
        match self.app.draw(renderer)? {
            PresentStatus::Presented => {
                self.frames += 1;
                Ok(1)
            }
            PresentStatus::Retry => Ok(2),
            PresentStatus::Lost => {
                let old = surface.renderer.take().ok_or("missing renderer")?;
                surface.renderer = Some(old.recreate(false)?);
                Ok(2)
            }
        }
    }
    fn snapshot(&self, status: i32) -> serde_json::Value {
        let mut s = self.app.snapshot();
        let backend = self
            .surface
            .as_ref()
            .and_then(|s| s.renderer.as_ref())
            .map(|r| {
                format!(
                    "{}; {}",
                    r.adapter_name,
                    if r.uses_gpu_drawing() {
                        "Vello GPU drawing"
                    } else {
                        "software drawing + Vulkan presentation"
                    }
                )
            })
            .unwrap_or_else(|| "surface detached".into());
        s["backend"] = json!(backend);
        s["frames"] = json!(self.frames);
        s["status"] = json!(status);
        s
    }
}
#[doc(hidden)]
pub fn attach(env: &mut JNIEnv<'_>, id: jlong, surface: JObject<'_>) -> jstring {
    let result = guard(|| {
        main_thread(env)?;
        if surface.is_null() {
            return Err("missing Android Surface".into());
        }
        // SAFETY: JNI provides a live Surface/local reference for this call.
        // fromSurface acquires a native reference retained in SurfaceSession.
        let window = unsafe {
            ndk_sys::ANativeWindow_fromSurface(
                env.get_native_interface().cast(),
                surface.as_raw().cast(),
            )
        };
        let window = Window(NonNull::new(window).ok_or("Surface has no native window")?);
        let handle = raw_window_handle::AndroidNdkWindowHandle::new(window.0.cast());
        let instance = wgpu::Instance::default();
        // SAFETY: Window owns its native reference until after renderer drops.
        let gpu_surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(raw_window_handle::RawDisplayHandle::Android(
                    raw_window_handle::AndroidDisplayHandle::new(),
                )),
                raw_window_handle: raw_window_handle::RawWindowHandle::AndroidNdk(handle),
            })
        }
        .map_err(|e| e.to_string())?;
        HOSTS.with(|r| {
            let mut r = r.borrow_mut();
            let host = r.hosts.get_mut(&id).ok_or("unknown mobile host")?;
            // Drop the old surface and reference before mounting its replacement.
            host.surface = None;
            let renderer = if host.app.allow_software() {
                SurfaceRenderer::from_surface_with_software_fallback(instance, gpu_surface, false)?
            } else {
                SurfaceRenderer::from_surface(instance, gpu_surface, false)?
            };
            host.surface = Some(SurfaceSession {
                renderer: Some(renderer),
                _window: window,
            });
            Ok(host.snapshot(1))
        })
    });
    output(env, result)
}
#[doc(hidden)]
pub fn event(env: &mut JNIEnv<'_>, id: jlong, event: JString<'_>) -> jstring {
    let result = guard(|| {
        main_thread(env)?;
        // JNI length is UTF-16 code units. Bound before allocating UTF-8.
        let len = env
            .call_method(&event, "length", "()I", &[])
            .map_err(|e| e.to_string())?
            .i()
            .map_err(|e| e.to_string())?;
        if !(1..=65_536).contains(&len) {
            return Err("oversized platform event".into());
        }
        let text: String = env.get_string(&event).map_err(|e| e.to_string())?.into();
        let event = PlatformEvent::decode(text.as_bytes())?;
        HOSTS.with(|r| {
            let mut r = r.borrow_mut();
            let host = r.hosts.get_mut(&id).ok_or("unknown mobile host")?;
            host.app.handle(event)?;
            let status = host.draw()?;
            Ok(host.snapshot(status))
        })
    });
    output(env, result)
}
#[doc(hidden)]
pub fn detach(env: &mut JNIEnv<'_>, id: jlong) {
    if let Err(error) = guard(|| {
        main_thread(env)?;
        HOSTS.with(|r| {
            if let Some(h) = r.borrow_mut().hosts.get_mut(&id) {
                h.surface = None;
                h.app.handle(PlatformEvent::Cancel {})?;
            }
            Ok(())
        })
    }) {
        ERROR.with(|s| *s.borrow_mut() = error);
    }
}
#[doc(hidden)]
pub fn destroy(env: &mut JNIEnv<'_>, id: jlong) {
    if let Err(error) = guard(|| {
        main_thread(env)?;
        HOSTS.with(|r| r.borrow_mut().hosts.remove(&id));
        Ok(())
    }) {
        ERROR.with(|s| *s.borrow_mut() = error);
    }
}
#[doc(hidden)]
pub fn last_error(env: JNIEnv<'_>) -> jstring {
    ERROR.with(|s| {
        env.new_string(&*s.borrow())
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut())
    })
}
