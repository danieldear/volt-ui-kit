//! GPU drawing and presentation. Vello encodes/rasterizes the native scene on
//! compute-capable GPUs; only layout, shaping and cached glyph resources use CPU.
use super::{
    gpu_scene::{color, GpuSceneEncoder},
    raster::{Glyph, Shape},
    RasterPainter,
};
use crate::{declarative::PaintBatch, graphics::Image};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use vello::{peniko::ImageData, wgpu};
use winit::window::Window;

pub(super) fn alpha_mode(modes: &[wgpu::CompositeAlphaMode]) -> Option<wgpu::CompositeAlphaMode> {
    [
        wgpu::CompositeAlphaMode::PreMultiplied,
        wgpu::CompositeAlphaMode::PostMultiplied,
    ]
    .into_iter()
    .find(|m| modes.contains(m))
}
fn shader_source(post: bool) -> String {
    format!(
        r#"
@group(0) @binding(0) var pixels:texture_2d<f32>;
@vertex fn vs(@builtin(vertex_index) i:u32)->@builtin(position) vec4<f32> {{
    var p=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));return vec4(p[i],0.,1.);
}}
@fragment fn fs(@builtin(position) pos:vec4<f32>)->@location(0) vec4<f32> {{
    let c=textureLoad(pixels,vec2<i32>(pos.xy),0);
    // Vello's output texture is straight alpha. Adapt on the GPU for the
    // compositor's declared mode; opaque output naturally has alpha=1.
    if ({post}) {{ return c; }}
    return vec4(c.rgb*c.a,c.a);
}}
"#
    )
}
/// Vello 0.6 drops a frame that overflows any of its fixed GPU buffers, leaving
/// the previous image in place (a window that looks frozen). Such frames are
/// drawn in software instead; this is reported once per process.
fn report_fallback_once(reason: &str) {
    static REPORTED: AtomicBool = AtomicBool::new(false);
    if !REPORTED.swap(true, Ordering::Relaxed) {
        eprintln!(
            "Volt UI: frame would overflow Vello's {reason} buffer; drawing such frames in software. Deeply nested clips/opacity over large areas, very dense vector paths/text or many large images cause this."
        );
    }
}
/// Premultiplied RGBA (our `Image` and tiny-skia storage) to straight alpha.
fn unpremultiply(data: &[u8]) -> Vec<u8> {
    let mut out = data.to_vec();
    for p in out.chunks_exact_mut(4) {
        let a = u16::from(p[3]);
        if a != 0 && a != 255 {
            for c in &mut p[..3] {
                *c = ((u16::from(*c) * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    out
}
/// Mark the device unusable on loss or an uncaptured validation error (wgpu's
/// default handler panics), so the host can recreate it or fall back.
fn watch_device(device: &wgpu::Device) -> Arc<AtomicBool> {
    let lost = Arc::new(AtomicBool::new(false));
    let flag = lost.clone();
    device.set_device_lost_callback(move |reason, message| {
        eprintln!("Volt UI: GPU device lost ({reason:?}): {message}");
        flag.store(true, Ordering::Relaxed);
    });
    let flag = lost.clone();
    device.on_uncaptured_error(Box::new(move |error| {
        eprintln!("Volt UI: GPU error: {error}");
        flag.store(true, Ordering::Relaxed);
    }));
    lost
}
/// Images uploaded once as GPU textures and registered with Vello. Vello still
/// packs them into its per-frame atlas, but by GPU copy instead of re-uploading
/// CPU pixels every frame. LRU-bounded; images used this frame stay resident.
#[derive(Default)]
struct TextureImages {
    map: HashMap<usize, Registered>,
    bytes: usize,
    clock: u64,
}
struct Registered {
    /// Keeps the pixel buffer (the key's address) alive while registered.
    _source: Image,
    data: ImageData,
    bytes: usize,
    used: u64,
}
const TEXTURE_BUDGET: usize = 96 * 1024 * 1024;
impl TextureImages {
    fn resolve(
        &mut self,
        image: &Image,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut vello::Renderer,
    ) -> Result<ImageData, String> {
        let id = image.pixels().as_ptr() as usize;
        if let Some(r) = self.map.get_mut(&id) {
            r.used = self.clock;
            return Ok(r.data.clone());
        }
        let (w, h) = (image.width(), image.height());
        if w.max(h) > device.limits().max_texture_dimension_2d {
            return Err("image exceeds GPU texture limits".into());
        }
        let bytes = w as usize * h as usize * 4;
        while self.bytes + bytes > TEXTURE_BUDGET {
            let Some(old) = self
                .map
                .iter()
                .filter(|(_, r)| r.used < self.clock)
                .min_by_key(|(_, r)| r.used)
                .map(|(k, _)| *k)
            else {
                break; // Everything resident is in this frame; exceed briefly.
            };
            let old = self.map.remove(&old).unwrap();
            self.bytes -= old.bytes;
            renderer.unregister_texture(old.data);
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Volt UI image"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        // Vello treats registered textures as straight (unpremultiplied) alpha.
        queue.write_texture(
            texture.as_image_copy(),
            &unpremultiply(image.pixels()),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let data = renderer.register_texture(texture);
        self.bytes += bytes;
        self.map.insert(
            id,
            Registered {
                _source: image.clone(),
                data: data.clone(),
                bytes,
                used: self.clock,
            },
        );
        Ok(data)
    }
}
/// Premultiplied tiny-skia pixels to the straight alpha the present shader expects.
fn straight_rgba(pixmap: &tiny_skia::Pixmap) -> Vec<u8> {
    unpremultiply(pixmap.data())
}
pub(super) enum PresentStatus {
    Presented,
    Retry,
    /// The device was lost or hit an uncaptured error; recreate the presenter.
    Lost,
}
pub(super) struct GpuPresenter {
    instance: wgpu::Instance,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    texture: Option<wgpu::Texture>,
    bind_group: Option<wgpu::BindGroup>,
    renderer: vello::Renderer,
    encoder: GpuSceneEncoder,
    textures: TextureImages,
    lost: Arc<AtomicBool>,
    pub adapter_name: String,
}
impl GpuPresenter {
    pub fn new(window: Arc<Window>, transparent: bool) -> Result<Self, String> {
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| e.to_string())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        let caps = surface.get_capabilities(&adapter);
        let alpha = if transparent {
            alpha_mode(&caps.alpha_modes)
                .ok_or("compositor does not advertise transparent alpha support")?
        } else {
            wgpu::CompositeAlphaMode::Opaque
        };
        let info = adapter.get_info();
        let adapter_name = format!("{} / {:?}", info.name, info.backend);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| {
                matches!(
                    f,
                    wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
                )
            })
            .ok_or("compositor does not advertise an 8-bit non-sRGB surface")?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Volt UI Vello GPU renderer"),
            required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        let lost = watch_device(&device);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: 1,
            height: 1,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: alpha,
            view_formats: vec![],
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let post = alpha == wgpu::CompositeAlphaMode::PostMultiplied;
        let shader = shader_source(post);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(shader.into()),
        });
        let pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipe_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let renderer = vello::Renderer::new(
            &device,
            vello::RendererOptions {
                use_cpu: false,
                antialiasing_support: vello::AaSupport::area_only(),
                ..Default::default()
            },
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            instance,
            window,
            surface,
            device,
            queue,
            config,
            pipeline,
            layout,
            texture: None,
            bind_group: None,
            renderer,
            encoder: GpuSceneEncoder::default(),
            textures: TextureImages::default(),
            lost,
            adapter_name,
        })
    }
    /// Encode with GPU-resident images from this presenter's registry.
    fn encode(
        &mut self,
        batches: &[PaintBatch<Shape, Glyph>],
        painter: &mut RasterPainter,
    ) -> Result<(), String> {
        self.textures.clock += 1;
        let (encoder, textures, device, queue, renderer) = (
            &mut self.encoder,
            &mut self.textures,
            &self.device,
            &self.queue,
            &mut self.renderer,
        );
        let mut resolve = |image: &Image| textures.resolve(image, device, queue, renderer);
        encoder.encode_with(batches, painter, Some(&mut resolve))
    }
    pub fn present(
        &mut self,
        batches: &[PaintBatch<Shape, Glyph>],
        painter: &mut RasterPainter,
        background: [f32; 4],
    ) -> Result<PresentStatus, String> {
        if self.lost.load(Ordering::Relaxed) {
            return Ok(PresentStatus::Lost);
        }
        self.encode(batches, painter)?;
        let viewport = painter.gpu_viewport();
        let w = viewport.width as u32;
        let h = viewport.height as u32;
        if w > self.device.limits().max_texture_dimension_2d
            || h > self.device.limits().max_texture_dimension_2d
        {
            return Err("window dimensions exceed GPU texture limits".into());
        }
        if self.texture.is_none() || self.config.width != w || self.config.height != h {
            self.config.width = w;
            self.config.height = h;
            self.surface.configure(&self.device, &self.config);
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            self.bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                }],
            }));
            self.texture = Some(texture);
        }
        let texture = self.texture.as_ref().unwrap();
        if let Some(reason) = self.encoder.stats().over_budget {
            report_fallback_once(reason);
            let pixmap = painter.rasterize(batches, background)?;
            self.queue.write_texture(
                texture.as_image_copy(),
                &straight_rgba(&pixmap),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w * 4),
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
        } else {
            self.renderer
                .render_to_texture(
                    &self.device,
                    &self.queue,
                    &self.encoder.scene,
                    &texture.create_view(&Default::default()),
                    &vello::RenderParams {
                        base_color: color(background),
                        width: w,
                        height: h,
                        antialiasing_method: vello::AaConfig::Area,
                    },
                )
                .map_err(|e| e.to_string())?;
        }
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                self.surface = self
                    .instance
                    .create_surface(self.window.clone())
                    .map_err(|e| e.to_string())?;
                self.surface.configure(&self.device, &self.config);
                return Ok(PresentStatus::Retry);
            }
            Err(wgpu::SurfaceError::Timeout) => return Ok(PresentStatus::Retry),
            Err(e) => return Err(e.to_string()),
        };
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, self.bind_group.as_ref().unwrap(), &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        frame.present();
        if self.lost.load(Ordering::Relaxed) {
            return Ok(PresentStatus::Lost);
        }
        if std::env::var_os("VOLT_UI_TRACE").is_some() {
            let stats = self.encoder.stats();
            eprintln!("VOLT_UI_GPU backend=vello adapter={} shapes={} glyphs={} layers={} spill_slots={} software_frame={:?} resource_bytes={} framebuffer_upload_bytes={}",self.adapter_name,stats.shapes,stats.glyphs,stats.layers,stats.spill_slots,stats.over_budget,stats.resource_bytes,stats.framebuffer_upload_bytes);
        }
        Ok(PresentStatus::Presented)
    }
}

/// Persistent real GPU renderer for explicit headless snapshots/verification.
/// Readback waits for GPU completion and copies pixels to CPU; native windows
/// do not use this path. Its timing is not native presentation or photon latency.
pub struct GpuOffscreenRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: vello::Renderer,
    encoder: GpuSceneEncoder,
    textures: TextureImages,
    lost: Arc<AtomicBool>,
    target: Option<(wgpu::Texture, wgpu::Buffer, [u32; 2])>,
    pub adapter_name: String,
}
impl GpuOffscreenRenderer {
    pub fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::default();
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .map_err(|e| e.to_string())?;
        let info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Volt UI offscreen GPU"),
            required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        let lost = watch_device(&device);
        let renderer = vello::Renderer::new(
            &device,
            vello::RendererOptions {
                use_cpu: false,
                antialiasing_support: vello::AaSupport::area_only(),
                ..Default::default()
            },
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            device,
            queue,
            renderer,
            encoder: GpuSceneEncoder::default(),
            textures: TextureImages::default(),
            lost,
            target: None,
            adapter_name: format!("{} / {:?}", info.name, info.backend),
        })
    }
    /// CPU-rendered full-window pixels uploaded by this adapter (always zero).
    /// This does not count Vello's scene/asset uploads or GPU memory traffic.
    pub fn framebuffer_upload_bytes(&self) -> usize {
        self.encoder.stats().framebuffer_upload_bytes
    }
    /// Estimated Vello blend-spill slots used by the latest frame. Frames above
    /// the renderer's fixed budget are drawn in software instead of dropped.
    pub fn last_spill_slots(&self) -> u32 {
        self.encoder.stats().spill_slots
    }
    /// The Vello buffer the latest frame would have overflowed (that frame was
    /// then drawn in software), or `None` when it rendered on the GPU.
    pub fn last_over_budget(&self) -> Option<&'static str> {
        self.encoder.stats().over_budget
    }
    /// New CPU-side cached glyph/image bytes constructed in the latest render.
    /// Not a measurement of actual GPU transfers or total GPU memory.
    pub fn resource_bytes(&self) -> usize {
        self.encoder.stats().resource_bytes
    }
    pub fn render(
        &mut self,
        painter: &mut RasterPainter,
        batches: &[PaintBatch<Shape, Glyph>],
        background: [f32; 4],
    ) -> Result<tiny_skia::Pixmap, String> {
        if background
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
        {
            return Err("invalid GPU background".into());
        }
        if self.lost.load(Ordering::Relaxed) {
            return Err("GPU device lost".into());
        }
        self.textures.clock += 1;
        let (encoder, textures, device, queue, renderer) = (
            &mut self.encoder,
            &mut self.textures,
            &self.device,
            &self.queue,
            &mut self.renderer,
        );
        let mut resolve = |image: &Image| textures.resolve(image, device, queue, renderer);
        encoder.encode_with(batches, painter, Some(&mut resolve))?;
        if let Some(reason) = self.encoder.stats().over_budget {
            report_fallback_once(reason);
            return painter.rasterize(batches, background);
        }
        let vp = painter.gpu_viewport();
        let size = [vp.width as u32, vp.height as u32];
        if size.contains(&0)
            || size
                .iter()
                .any(|v| *v > self.device.limits().max_texture_dimension_2d)
        {
            return Err("invalid GPU dimensions".into());
        }
        let stride = (size[0] * 4).div_ceil(256) * 256;
        if self.target.as_ref().is_none_or(|t| t.2 != size) {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Volt UI GPU snapshot"),
                size: wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Volt UI explicit readback"),
                size: u64::from(stride) * u64::from(size[1]),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            self.target = Some((texture, buffer, size));
        }
        let (texture, buffer, _) = self.target.as_ref().unwrap();
        self.renderer
            .render_to_texture(
                &self.device,
                &self.queue,
                &self.encoder.scene,
                &texture.create_view(&Default::default()),
                &vello::RenderParams {
                    base_color: color(background),
                    width: size[0],
                    height: size[1],
                    antialiasing_method: vello::AaConfig::Area,
                },
            )
            .map_err(|e| e.to_string())?;
        let mut commands = self.device.create_command_encoder(&Default::default());
        commands.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(size[1]),
                },
            },
            wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([commands.finish()]);
        let (send, recv) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = send.send(result);
            });
        self.device
            .poll(wgpu::PollType::Wait)
            .map_err(|e| e.to_string())?;
        recv.recv_timeout(std::time::Duration::from_secs(30))
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        let mut result =
            tiny_skia::Pixmap::new(size[0], size[1]).ok_or("invalid GPU snapshot size")?;
        {
            let data = buffer.slice(..).get_mapped_range();
            for (dst, src) in result
                .data_mut()
                .chunks_exact_mut(size[0] as usize * 4)
                .zip(data.chunks_exact(stride as usize))
            {
                dst.copy_from_slice(&src[..dst.len()]);
                // tiny-skia Pixmap (and PNG saving) require premultiplied RGBA;
                // Vello's render target is straight RGBA. Only explicit
                // verification readback does this CPU conversion.
                for p in dst.chunks_exact_mut(4) {
                    for i in 0..3 {
                        p[i] = ((u16::from(p[i]) * u16::from(p[3]) + 127) / 255) as u8;
                    }
                }
            }
        }
        buffer.unmap();
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn premultiplied_and_straight_alpha_shaders_parse_and_validate() {
        for post in [false, true] {
            let module = wgpu::naga::front::wgsl::parse_str(&shader_source(post)).unwrap();
            wgpu::naga::valid::Validator::new(
                wgpu::naga::valid::ValidationFlags::all(),
                wgpu::naga::valid::Capabilities::empty(),
            )
            .validate(&module)
            .unwrap();
        }
    }
    #[test]
    fn transparent_modes_are_explicit_and_prefer_premultiplied() {
        use wgpu::CompositeAlphaMode::*;
        assert_eq!(
            alpha_mode(&[Opaque, PostMultiplied, PreMultiplied]),
            Some(PreMultiplied)
        );
        assert_eq!(alpha_mode(&[Opaque, PostMultiplied]), Some(PostMultiplied));
        assert_eq!(alpha_mode(&[Opaque, Auto, Inherit]), None);
    }
}
