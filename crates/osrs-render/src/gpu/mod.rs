//! Reference-profile wgpu renderer (M11).
//!
//! Reverse-Z `Depth32Float` (clear `0`, strict `Greater`), CCW front faces with back-face
//! culling, authored face bias added to clip-space depth in the vertex shader, and the pinned
//! RuneLite blend state for the alpha pass (`SRC_ALPHA, ONE_MINUS_SRC_ALPHA` color with `ONE, ONE`
//! alpha, depth writes left on). Output is a non-sRGB `Rgba8Unorm` target so values reach the
//! framebuffer unconverted, like the reference's default framebuffer.

use crate::{PackedVertex, SceneGeometry};
use bytemuck::{Pod, Zeroable};
use std::{error::Error, fmt};
use wgpu::util::DeviceExt;

pub const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Edge length of every texture layer (the reference array is 128x128).
pub const TEXTURE_SIZE: u32 = 128;
/// Maximum texture layers addressable by the animation table.
pub const MAX_TEXTURE_LAYERS: usize = 256;
/// Reference projection near term (`Mat4.projection(w, h, 50)`).
pub const REFERENCE_NEAR: f32 = 50.0;

/// GPU setup or readback failure.
#[derive(Debug)]
pub enum GpuError {
    NoAdapter(String),
    Device(String),
    Readback(String),
}

impl fmt::Display for GpuError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAdapter(detail) => write!(formatter, "no suitable GPU adapter: {detail}"),
            Self::Device(detail) => write!(formatter, "GPU device error: {detail}"),
            Self::Readback(detail) => write!(formatter, "GPU readback error: {detail}"),
        }
    }
}

impl Error for GpuError {}

/// Reference camera in scene-local units and radians.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceCamera {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub pitch: f32,
    /// Client zoom scale (pixels per unit at unit distance); `512` is the client default.
    pub scale: f32,
}

type Mat4 = [[f32; 4]; 4]; // column-major: m[column][row]

fn mat4_identity() -> Mat4 {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn mat4_mul(a: Mat4, b: Mat4) -> Mat4 {
    let mut out = [[0.0; 4]; 4];
    for column in 0..4 {
        for row in 0..4 {
            let mut sum = 0.0;
            for k in 0..4 {
                sum += a[k][row] * b[column][k];
            }
            out[column][row] = sum;
        }
    }
    out
}

impl ReferenceCamera {
    /// `scale * projection(w, h, 50) * rotateX(pitch) * rotateY(yaw) * translate(-camera)`, the
    /// exact composition `GpuPlugin` uploads as `worldProj` (column-major).
    pub fn world_projection(&self, width: f32, height: f32) -> [[f32; 4]; 4] {
        let mut scale = mat4_identity();
        scale[0][0] = self.scale;
        scale[1][1] = self.scale;

        let projection: Mat4 = [
            [2.0 / width, 0.0, 0.0, 0.0],
            [0.0, -2.0 / height, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
            [0.0, 0.0, 2.0 * REFERENCE_NEAR, 0.0],
        ];
        let (sp, cp) = self.pitch.sin_cos();
        let rotate_x: Mat4 = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, cp, sp, 0.0],
            [0.0, -sp, cp, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let (sy, cy) = self.yaw.sin_cos();
        let rotate_y: Mat4 = [
            [cy, 0.0, -sy, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [sy, 0.0, cy, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let mut translate = mat4_identity();
        translate[3] = [-self.x, -self.y, -self.z, 1.0];

        mat4_mul(
            mat4_mul(mat4_mul(mat4_mul(scale, projection), rotate_x), rotate_y),
            translate,
        )
    }
}

/// Per-frame inputs.
#[derive(Debug, Clone, Copy)]
pub struct FrameParams {
    pub camera: ReferenceCamera,
    /// Draw levels `<=` this plane whose tiles' minimum plane is `<=` it.
    pub view_plane: u8,
    /// Texture animation clock: the client's `gameCycle & 127`.
    pub tick: u32,
    pub brightness: f32,
    pub clear_color: [f64; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GlobalsUniform {
    world_proj: [[f32; 4]; 4],
    brightness: f32,
    smooth_banding: f32,
    tick: u32,
    pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct AnimationUniform {
    values: [[f32; 4]; MAX_TEXTURE_LAYERS],
}

/// One texture layer in the reference upload format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureLayer {
    /// `TEXTURE_SIZE * TEXTURE_SIZE` row-major `[r, g, b, a]` texels (zero alpha = cutout).
    pub rgba: Vec<[u8; 4]>,
    pub animation_direction: u8,
    pub animation_speed: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ZoneUniform {
    base: [f32; 4],
}

struct DrawRange {
    level: u8,
    min_plane: u8,
    start: u32,
    count: u32,
}

struct GpuZone {
    bind_group: wgpu::BindGroup,
    opaque: Option<wgpu::Buffer>,
    alpha: Option<wgpu::Buffer>,
    opaque_ranges: Vec<DrawRange>,
    alpha_ranges: Vec<DrawRange>,
}

/// Device, queue, and the scene pipelines.
pub struct SceneRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    opaque_pipeline: wgpu::RenderPipeline,
    alpha_pipeline: wgpu::RenderPipeline,
    globals_buffer: wgpu::Buffer,
    globals_layout: wgpu::BindGroupLayout,
    globals_bind_group: wgpu::BindGroup,
    zone_layout: wgpu::BindGroupLayout,
    zones: Vec<GpuZone>,
}

impl SceneRenderer {
    /// Create a headless renderer on the best available adapter.
    pub fn new_headless() -> Result<Self, GpuError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            apply_limit_buckets: false,
        }))
        .map_err(|error| GpuError::NoAdapter(error.to_string()))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .map_err(|error| GpuError::Device(error.to_string()))?;
        Ok(Self::with_device(device, queue))
    }

    /// Build the pipelines on an existing device (for example the eframe-owned one).
    pub fn with_device(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("scene.wgsl").into()),
        });

        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let zone_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("zone"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene"),
            bind_group_layouts: &[Some(&globals_layout), Some(&zone_layout)],
            immediate_size: 0,
        });

        let vertex_attributes = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Sint16x4,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Uint32,
                offset: 8,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Uint16x4,
                offset: 12,
                shader_location: 2,
            },
        ];
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: PackedVertex::SIZE as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &vertex_attributes,
        };

        let make_pipeline = |label: &str, blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Some(vertex_layout.clone())],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR_FORMAT,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Greater),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };

        let opaque_pipeline = make_pipeline("scene-opaque", None);
        let alpha_pipeline = make_pipeline(
            "scene-alpha",
            Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
        );

        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<GlobalsUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_bind_group =
            build_globals_bind_group(&device, &queue, &globals_layout, &globals_buffer, &[]);

        Self {
            device,
            queue,
            opaque_pipeline,
            alpha_pipeline,
            globals_buffer,
            globals_layout,
            globals_bind_group,
            zone_layout,
            zones: Vec::new(),
        }
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Upload the texture array (layer `i` = texture id `i`) and its animation table.
    pub fn set_textures(&mut self, layers: &[Option<TextureLayer>]) {
        self.globals_bind_group = build_globals_bind_group(
            &self.device,
            &self.queue,
            &self.globals_layout,
            &self.globals_buffer,
            layers,
        );
    }

    /// Upload extracted geometry, replacing any previous scene.
    pub fn upload_scene(&mut self, geometry: &SceneGeometry) {
        self.zones.clear();
        for zone in &geometry.zones {
            let origin = zone.origin();
            let zone_uniform = ZoneUniform {
                base: [origin.0 as f32, 0.0, origin.1 as f32, 0.0],
            };
            let zone_buffer = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("zone"),
                    contents: bytemuck::bytes_of(&zone_uniform),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("zone"),
                layout: &self.zone_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: zone_buffer.as_entire_binding(),
                }],
            });

            let mut opaque: Vec<PackedVertex> = Vec::new();
            let mut alpha: Vec<PackedVertex> = Vec::new();
            let mut opaque_ranges = Vec::new();
            let mut alpha_ranges = Vec::new();
            for group in &zone.groups {
                if !group.geometry.opaque.is_empty() {
                    opaque_ranges.push(DrawRange {
                        level: group.level,
                        min_plane: group.min_plane,
                        start: opaque.len() as u32,
                        count: group.geometry.opaque.len() as u32,
                    });
                    opaque.extend_from_slice(&group.geometry.opaque);
                }
                if !group.geometry.alpha.is_empty() {
                    alpha_ranges.push(DrawRange {
                        level: group.level,
                        min_plane: group.min_plane,
                        start: alpha.len() as u32,
                        count: group.geometry.alpha.len() as u32,
                    });
                    alpha.extend_from_slice(&group.geometry.alpha);
                }
            }
            let make = |vertices: &[PackedVertex], label: &str| {
                (!vertices.is_empty()).then(|| {
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some(label),
                            contents: bytemuck::cast_slice(vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        })
                })
            };
            self.zones.push(GpuZone {
                bind_group,
                opaque: make(&opaque, "zone-opaque"),
                alpha: make(&alpha, "zone-alpha"),
                opaque_ranges,
                alpha_ranges,
            });
        }
    }

    /// Bytes of vertex data resident on the GPU.
    pub fn resident_vertex_bytes(&self) -> u64 {
        self.zones
            .iter()
            .flat_map(|zone| [zone.opaque.as_ref(), zone.alpha.as_ref()])
            .flatten()
            .map(wgpu::Buffer::size)
            .sum()
    }

    /// Encode one frame's render pass into `encoder`, targeting caller-owned views.
    ///
    /// The color view must be [`COLOR_FORMAT`] and the depth view [`DEPTH_FORMAT`], both sized
    /// `width x height`. Camera uniforms are written through the queue, so the encoder must be
    /// submitted after this call.
    pub fn encode_frame(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        params: FrameParams,
        width: u32,
        height: u32,
    ) {
        let globals = GlobalsUniform {
            world_proj: params.camera.world_projection(width as f32, height as f32),
            brightness: params.brightness,
            smooth_banding: 1.0,
            tick: params.tick,
            pad: 0.0,
        };
        self.queue
            .write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: params.clear_color[0],
                            g: params.clear_color[1],
                            b: params.clear_color[2],
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.globals_bind_group, &[]);

            pass.set_pipeline(&self.opaque_pipeline);
            for zone in &self.zones {
                let Some(buffer) = &zone.opaque else { continue };
                pass.set_bind_group(1, &zone.bind_group, &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                for range in &zone.opaque_ranges {
                    if range.level <= params.view_plane && range.min_plane <= params.view_plane {
                        pass.draw(range.start..range.start + range.count, 0..1);
                    }
                }
            }

            pass.set_pipeline(&self.alpha_pipeline);
            for zone in &self.zones {
                let Some(buffer) = &zone.alpha else { continue };
                pass.set_bind_group(1, &zone.bind_group, &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                for range in &zone.alpha_ranges {
                    if range.level <= params.view_plane && range.min_plane <= params.view_plane {
                        pass.draw(range.start..range.start + range.count, 0..1);
                    }
                }
            }
        }
    }

    /// Render one frame into a fresh offscreen target and return tightly packed RGBA8 pixels.
    pub fn render_to_rgba(
        &self,
        params: FrameParams,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, GpuError> {
        let color = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("frame-color"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let depth = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("frame-depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        self.encode_frame(
            &mut encoder,
            &color_view,
            &depth_view,
            params,
            width,
            height,
        );

        let unpadded = width * 4;
        let padded = unpadded.div_ceil(256) * 256;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(padded) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &color,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));

        let slice = readback.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| GpuError::Readback(error.to_string()))?;
        receiver
            .recv()
            .map_err(|error| GpuError::Readback(error.to_string()))?
            .map_err(|error| GpuError::Readback(error.to_string()))?;

        let data = slice
            .get_mapped_range()
            .map_err(|error| GpuError::Readback(error.to_string()))?;
        let mut pixels = Vec::with_capacity((unpadded * height) as usize);
        for row in 0..height {
            let start = (row * padded) as usize;
            pixels.extend_from_slice(&data[start..start + unpadded as usize]);
        }
        drop(data);
        readback.unmap();
        Ok(pixels)
    }
}

/// Average a mip level down by one level (2x2 box filter over RGBA).
fn downsample(source: &[[u8; 4]], size: usize) -> Vec<[u8; 4]> {
    let target = (size / 2).max(1);
    let mut out = Vec::with_capacity(target * target);
    for row in 0..target {
        for column in 0..target {
            let mut sum = [0_u32; 4];
            for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                let sy = (row * 2 + dy).min(size - 1);
                let sx = (column * 2 + dx).min(size - 1);
                for (channel, total) in sum.iter_mut().enumerate() {
                    *total += u32::from(source[sy * size + sx][channel]);
                }
            }
            out.push(sum.map(|total| ((total + 2) / 4) as u8));
        }
    }
    out
}

fn build_globals_bind_group(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    globals_buffer: &wgpu::Buffer,
    layers: &[Option<TextureLayer>],
) -> wgpu::BindGroup {
    let layer_count = layers.len().clamp(1, MAX_TEXTURE_LAYERS) as u32;
    let size = TEXTURE_SIZE as usize;
    let mip_levels = TEXTURE_SIZE.ilog2() + 1;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("texture-array"),
        size: wgpu::Extent3d {
            width: TEXTURE_SIZE,
            height: TEXTURE_SIZE,
            depth_or_array_layers: layer_count,
        },
        mip_level_count: mip_levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    let mut animations = AnimationUniform {
        values: [[0.0; 4]; MAX_TEXTURE_LAYERS],
    };
    for (layer_index, layer) in layers.iter().enumerate().take(MAX_TEXTURE_LAYERS) {
        let Some(layer) = layer else { continue };
        let vector = crate::TextureAnimationVector::from_direction_speed(
            layer.animation_direction,
            layer.animation_speed,
        );
        animations.values[layer_index] = [
            f32::from(vector.u_units_per_tick),
            f32::from(vector.v_units_per_tick),
            0.0,
            0.0,
        ];
        let mut level = layer.rgba.clone();
        let mut level_size = size;
        for mip in 0..mip_levels {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: mip,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer_index as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                bytemuck::cast_slice(&level),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(level_size as u32 * 4),
                    rows_per_image: Some(level_size as u32),
                },
                wgpu::Extent3d {
                    width: level_size as u32,
                    height: level_size as u32,
                    depth_or_array_layers: 1,
                },
            );
            if level_size > 1 {
                level = downsample(&level, level_size);
                level_size /= 2;
            }
        }
    }

    // Reference sampler: nearest magnification, nearest-mipmap-linear minification, clamp on S,
    // repeat on T (the audited GL setup only overrides the S wrap).
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("texture-sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let animation_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("texture-animations"),
        contents: bytemuck::bytes_of(&animations),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("globals"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: animation_buffer.as_entire_binding(),
            },
        ],
    })
}
