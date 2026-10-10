//! Optional native host: software rasterization or opt-in Vello GPU drawing.
//! Native backdrop blur is an OS request, not a widget-renderer blur simulation.
mod assets;
mod raster;
pub use assets::{decode_image, load_image};
#[cfg(feature = "native-gpu")]
mod gpu;
#[cfg(feature = "native-gpu")]
pub use gpu::GpuOffscreenRenderer;
#[cfg(feature = "native-gpu")]
mod gpu_scene;
#[cfg(all(test, feature = "native-gpu"))]
mod gpu_tests;
mod messages;
pub use messages::{UiSendError, UiSender};
mod window;
pub use raster::RasterPainter;
pub use window::{
    load_document, run, run_ui, run_ui_with_messages, run_ui_with_messages_and_theme,
    run_ui_with_options, run_ui_with_theme, run_with_options, NativeOptions, SurfaceOptions,
    WindowBackground, WindowChrome, WindowOptions,
};
