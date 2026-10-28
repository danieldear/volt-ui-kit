//! Drawing adapters without a desktop window, clipboard or event loop.
//! Existing `native` re-exports retain their types and paths.
#[path = "../native/raster.rs"]
pub(crate) mod raster;
pub use raster::{font_families, register_font, Damage, RasterPainter};
#[cfg(feature = "rendering-gpu")]
#[path = "../native/gpu_scene.rs"]
pub(crate) mod gpu_scene;
#[cfg(feature = "rendering-gpu")]
pub use gpu_scene::{estimate_gpu_scene, GpuSceneEstimate};
#[cfg(feature = "rendering-gpu")]
#[path = "../native/gpu.rs"]
pub(crate) mod gpu;
#[cfg(feature = "rendering-gpu")]
pub use gpu::{GpuOffscreenRenderer, GpuPresenter as SurfaceRenderer, PresentStatus};
#[cfg(feature = "rendering-gpu")]
pub use vello::wgpu;
