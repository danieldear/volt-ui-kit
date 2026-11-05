//! WASM-only surface adapter. No DOM component design is implemented here.
use super::WebView;
#[cfg(feature = "gpu")]
use volt_ui_kit::rendering::{wgpu, PresentStatus, SurfaceRenderer};
use volt_ui_kit::UiTheme;
use wasm_bindgen::{Clamped, JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData};

enum Renderer {
    Canvas(CanvasRenderingContext2d),
    #[cfg(feature = "gpu")]
    Gpu(Box<SurfaceRenderer>),
}
/// Owns one canvas and Rust view. Dropping it releases the surface/device/state.
/// GPU initialization failure is returned; the JS host can offer software on a
/// fresh canvas (a browser canvas cannot change its context type once bound).
pub struct BrowserHost<C: 'static> {
    pub view: WebView<C>,
    canvas: HtmlCanvasElement,
    renderer: Option<Renderer>,
    frames: u64,
}
impl<C: 'static> BrowserHost<C> {
    pub async fn new(
        canvas: HtmlCanvasElement,
        view: WebView<C>,
        gpu: bool,
    ) -> Result<Self, JsValue> {
        let renderer = Self::create_renderer(&canvas, gpu).await?;
        Ok(Self {
            view,
            canvas,
            renderer: Some(renderer),
            frames: 0,
        })
    }
    async fn create_renderer(canvas: &HtmlCanvasElement, gpu: bool) -> Result<Renderer, JsValue> {
        Ok(if gpu {
            #[cfg(feature = "gpu")]
            {
                let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                    backends: wgpu::Backends::BROWSER_WEBGPU,
                    ..wgpu::InstanceDescriptor::new_without_display_handle()
                });
                let surface = instance
                    .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                Renderer::Gpu(Box::new(
                    SurfaceRenderer::from_surface_async(instance, surface, false)
                        .await
                        .map_err(|e| JsValue::from_str(&e))?,
                ))
            }
            #[cfg(not(feature = "gpu"))]
            {
                return Err(JsValue::from_str("WebGPU is not enabled in this build"));
            }
        } else {
            Renderer::Canvas(
                canvas
                    .get_context("2d")?
                    .ok_or_else(|| JsValue::from_str("Canvas2D is unavailable"))?
                    .dyn_into()?,
            )
        })
    }
    /// Release GPU surfaces and heavy paint caches while retaining Rust demo state.
    pub fn pause(&mut self) {
        self.renderer = None;
        let _ = self.view.dispatch(super::WebEvent::Suspend { value: true });
        self.view.release_draw_cache();
    }
    pub async fn resume(&mut self, canvas: HtmlCanvasElement, gpu: bool) -> Result<(), JsValue> {
        let renderer = Self::create_renderer(&canvas, gpu).await?;
        self.canvas = canvas;
        self.renderer = Some(renderer);
        self.view
            .dispatch(super::WebEvent::Suspend { value: false })
            .map_err(|e| JsValue::from_str(&e))?;
        Ok(())
    }
    pub fn backend(&self) -> &'static str {
        match self.renderer.as_ref() {
            Some(Renderer::Canvas(_)) => "software / Canvas2D",
            #[cfg(feature = "gpu")]
            Some(Renderer::Gpu(_)) => "Vello / WebGPU",
            None => "paused",
        }
    }
    pub fn snapshot(&self) -> String {
        let mut value = self.view.snapshot();
        value["backend"] = self.backend().into();
        value["frames"] = self.frames.into();
        value.to_string()
    }
    /// Returns true only when a transient surface condition needs another frame.
    pub fn render(&mut self, theme: &UiTheme) -> Result<bool, JsValue> {
        let viewport = self.view.viewport;
        if self.canvas.width() != viewport.width as u32 {
            self.canvas.set_width(viewport.width as u32);
        }
        if self.canvas.height() != viewport.height as u32 {
            self.canvas.set_height(viewport.height as u32);
        }
        let Some(renderer) = self.renderer.as_mut() else {
            return Ok(false);
        };
        let retry = match renderer {
            Renderer::Canvas(context) => {
                let (pixels, w, h) = self.view.rgba(theme).map_err(|e| JsValue::from_str(&e))?;
                context.put_image_data(
                    &ImageData::new_with_u8_clamped_array_and_sh(Clamped(&pixels), w, h)?,
                    0.,
                    0.,
                )?;
                false
            }
            #[cfg(feature = "gpu")]
            Renderer::Gpu(renderer) => match self
                .view
                .present(renderer, theme)
                .map_err(|e| JsValue::from_str(&e))?
            {
                PresentStatus::Presented => false,
                PresentStatus::Retry => true,
                PresentStatus::Lost => {
                    return Err(JsValue::from_str(
                        "WebGPU device lost; reload this preview or select software rendering",
                    ))
                }
            },
        };
        self.frames += 1;
        Ok(retry)
    }
}
