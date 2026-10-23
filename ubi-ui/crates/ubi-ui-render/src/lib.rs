//! Batched GPU primitives and glyph-atlas text, independent of window creation.
mod paint;
mod text;
use bytemuck::{Pod, Zeroable};
pub use text::Text;
use ubi_ui_core::{Error, View};
use ubi_ui_input::Input;
use ubi_ui_layout::{PresentedScene, Scene};
use ubi_ui_theme::Theme;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct Vertex {
    position: [f32; 2],
    world: [f32; 2],
    local: [f32; 2],
    size: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
    clip: [f32; 4],
    params: [f32; 2],
}
/// A renderer-owned CPU command batch; one atlas binding and draw call per frame.
pub struct Frame {
    vertices: Vec<Vertex>,
    pub clear: ubi_ui_theme::Color,
}
impl Frame {
    pub fn quad_count(&self) -> usize {
        self.vertices.len() / 6
    }
}

/// wgpu device/queue and original Ubi primitive pipeline. The platform owns its
/// surface; tests can render the same frames into ordinary textures.
pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_name: String,
    pipeline: wgpu::RenderPipeline,
    atlas: paint::Atlas,
    bind: wgpu::BindGroup,
    buffer: wgpu::Buffer,
    capacity: usize,
    pub text: Text,
}
impl Renderer {
    pub async fn new(
        instance: &wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
        format: wgpu::TextureFormat,
    ) -> Result<Self, Error> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: surface,
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                ..Default::default()
            })
            .await
            .map_err(|e| Error(format!("GPU adapter: {e}")))?;
        Self::with_adapter(&adapter, format).await
    }
    pub async fn with_adapter(
        adapter: &wgpu::Adapter,
        format: wgpu::TextureFormat,
    ) -> Result<Self, Error> {
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Ubi UI"),
                ..Default::default()
            })
            .await
            .map_err(|e| Error(format!("GPU device: {e}")))?;
        let atlas = paint::Atlas::new(&device);
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Ubi glyph atlas"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Ubi glyph atlas"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &atlas.texture.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Ubi 2D"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ui.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Ubi 2D"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        const ATTRS: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Float32x2,3=>Float32x2,4=>Float32x2,5=>Float32x4,6=>Float32x4,7=>Float32x2];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Ubi 2D"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &ATTRS,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Ubi vertices"),
            contents: &[0; 80],
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        Ok(Self {
            device,
            queue,
            adapter_name: adapter.get_info().name,
            pipeline,
            atlas,
            bind,
            buffer,
            capacity: 80,
            text: Text::new()?,
        })
    }
    /// Build a frame in logical pixels. DPI affects only glyph raster resolution.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &mut self,
        view: &View,
        scene: &Scene,
        input: &Input,
        theme: &Theme,
        system_dark: bool,
        dpi: f32,
        viewport: (f32, f32),
    ) -> Result<Frame, Error> {
        self.prepare_core(view, scene, None, input, theme, system_dark, dpi, viewport)
    }
    /// Build a frame from motion-adjusted geometry and append visual exit layers.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_presented(
        &mut self,
        view: &View,
        presented: &PresentedScene,
        input: &Input,
        theme: &Theme,
        system_dark: bool,
        dpi: f32,
        viewport: (f32, f32),
    ) -> Result<Frame, Error> {
        self.prepare_core(
            view,
            &presented.scene,
            Some(presented),
            input,
            theme,
            system_dark,
            dpi,
            viewport,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn prepare_core(
        &mut self,
        view: &View,
        scene: &Scene,
        presented: Option<&PresentedScene>,
        input: &Input,
        theme: &Theme,
        system_dark: bool,
        dpi: f32,
        viewport: (f32, f32),
    ) -> Result<Frame, Error> {
        if !dpi.is_finite()
            || !(0.25..=8.0).contains(&dpi)
            || !viewport.0.is_finite()
            || !viewport.1.is_finite()
            || viewport.0 <= 0.0
            || viewport.1 <= 0.0
        {
            return Err("invalid renderer viewport/DPI".into());
        }
        if self.atlas.nearly_full() {
            self.atlas.reset(&self.queue);
        }
        let result = paint::prepare(
            view,
            scene,
            input,
            theme,
            system_dark,
            dpi,
            viewport,
            presented,
            &mut self.text,
            &mut self.atlas,
            &self.queue,
        );
        if result
            .as_ref()
            .is_err_and(|e| e.0.starts_with("glyph atlas capacity"))
        {
            self.atlas.reset(&self.queue);
            return paint::prepare(
                view,
                scene,
                input,
                theme,
                system_dark,
                dpi,
                viewport,
                presented,
                &mut self.text,
                &mut self.atlas,
                &self.queue,
            );
        }
        result
    }
    /// Submit one batched draw. Idle windows need not call this method.
    pub fn draw(&mut self, target: &wgpu::TextureView, frame: &Frame) {
        let bytes = bytemuck::cast_slice(&frame.vertices);
        if bytes.len() > self.capacity {
            self.capacity = bytes.len().next_power_of_two();
            self.buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Ubi vertices"),
                size: self.capacity as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !bytes.is_empty() {
            self.queue.write_buffer(&self.buffer, 0, bytes);
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Ubi frame"),
            });
        {
            let c = frame.clear;
            let linear = |v: f32| {
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            let attachments = [Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: linear(c.0) as f64,
                        g: linear(c.1) as f64,
                        b: linear(c.2) as f64,
                        a: c.3 as f64,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Ubi frame"),
                color_attachments: &attachments,
                ..Default::default()
            });
            if !bytes.is_empty() {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind, &[]);
                pass.set_vertex_buffer(0, self.buffer.slice(..));
                pass.draw(0..frame.vertices.len() as u32, 0..1);
            }
        }
        self.queue.submit([encoder.finish()]);
    }
    /// Read a rendered RGBA8 texture; used by verification/capture, never frames.
    pub fn read_rgba(
        &self,
        texture: &wgpu::Texture,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, Error> {
        if width == 0 || height == 0 || width > 8192 || height > 8192 {
            return Err("invalid capture size".into());
        }
        let stride = (width * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Ubi capture"),
            size: stride as u64 * height as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(10)),
            })
            .map_err(|e| Error(e.to_string()))?;
        rx.recv_timeout(std::time::Duration::from_secs(10))
            .map_err(|e| Error(e.to_string()))?
            .map_err(|e| Error(e.to_string()))?;
        let mapped = buffer
            .slice(..)
            .get_mapped_range()
            .map_err(|e| Error(e.to_string()))?;
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for row in mapped.chunks(stride as usize).take(height as usize) {
            pixels.extend_from_slice(&row[..(width * 4) as usize]);
        }
        drop(mapped);
        buffer.unmap();
        Ok(pixels)
    }
    pub fn cached_glyphs(&self) -> usize {
        self.atlas.len()
    }
}
