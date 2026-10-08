//! Opt-in standalone Solid host: winit + software rasterization, no GPU blur.
mod raster;
mod window;
pub use raster::RasterPainter;
pub use window::{load_document, run, run_ui, NativeOptions};
