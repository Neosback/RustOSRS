//! Reference-profile wgpu renderer (M11).
//!
//! Reverse-Z `Depth32Float` (clear `0`, strict `Greater`), CCW front faces with back-face
//! culling, authored face bias added to clip-space depth in the vertex shader, and the pinned
//! RuneLite blend state for the alpha pass (`SRC_ALPHA, ONE_MINUS_SRC_ALPHA` color with `ONE, ONE`
//! alpha, depth writes off for the alpha pass, which draws level by level, far zones first, after
//! all opaque geometry). Output is a non-sRGB `Rgba8Unorm` target so values reach the
//! framebuffer unconverted, like the reference's default framebuffer.

use crate::{HdVertex, PackedVertex, SceneGeometry};
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
/// MSAA sample count of the scene pass (RuneLite's GPU plugin defaults to 2x MSAA).
pub const MSAA_SAMPLES: u32 = 4;

pub mod frame_graph;
pub mod hd;
mod overlay;
mod profile;
mod timing;

pub use overlay::LineVertex;
pub use profile::{HdLighting, HdParams, RenderProfile};

use frame_graph::{FrameGraph, TransientDesc, TransientPool};

/// Multisampled attachments of the scene pass, resolved into the caller's color view.
struct MsaaTargets {
    width: u32,
    height: u32,
    color: wgpu::TextureView,
    depth: wgpu::TextureView,
}

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
    /// Current plane: a tile (and everything on it) is drawn when its minimum plane is `<=` this,
    /// exactly like `Scene.draw` (`tile.minPlane <= Scene_plane`) over all planes.
    pub view_plane: u8,
    /// Texture animation clock: the client's `gameCycle & 127`.
    pub tick: u32,
    pub brightness: f32,
    /// RuneLite's "Remove color banding" option (default on): shade with vertex RGB interpolated
    /// across the face. Off converts the interpolated 7-bit HSL per pixel like the CPU renderer.
    pub remove_color_banding: bool,
    pub clear_color: [f64; 3],
    /// RuneLite "Bright textures" (default off).
    pub bright_textures: bool,
    /// RuneLite fog (`Fog depth`, default 0 = off); the fog colour is `clear_color`.
    pub fog: Fog,
    pub colorblind: Colorblind,
    /// Which renderer draws the frame; [`RenderProfile::Reference`] is the pinned default.
    pub profile: RenderProfile,
}

/// RuneLite fog settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fog {
    /// Fog depth in tiles; `0` turns fog off.
    pub depth_tiles: u32,
    /// Draw distance in tiles (RuneLite default 50).
    pub draw_distance_tiles: u32,
}

impl Default for Fog {
    fn default() -> Self {
        Self {
            depth_tiles: 0,
            draw_distance_tiles: 50,
        }
    }
}

/// RuneLite colourblindness correction.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Colorblind {
    pub mode: ColorblindMode,
    /// 0-100.
    pub intensity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorblindMode {
    #[default]
    None,
    Protanope,
    Deuteranope,
    Tritanope,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GlobalsUniform {
    world_proj: [[f32; 4]; 4],
    brightness: f32,
    smooth_banding: f32,
    tick: u32,
    texture_light_mode: f32,
    fog_color: [f32; 4],
    camera_x: f32,
    camera_z: f32,
    draw_distance: f32,
    fog_depth: f32,
    use_fog: u32,
    colorblind_mode: u32,
    colorblind_intensity: f32,
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
    min_plane: u8,
    start: u32,
    count: u32,
}

/// Sorting record of one model's transparent faces inside a zone's alpha buffer.
struct GpuAlphaModel {
    level: u8,
    min_plane: u8,
    first_vertex: u32,
    vertex_count: u32,
    origin: [i32; 3],
    radius: i32,
    packed_faces: Vec<i32>,
}

/// Alpha models farther than this (squared) from a zone's origin corner keep model order only
/// (`GpuPlugin.ALPHA_ZSORT_CLOSE`).
const ALPHA_ZSORT_CLOSE: f32 = 2048.0;
/// `ModelUploader.MAX_DIAMETER`: models with larger sort radius are not drawn sorted.
const ALPHA_MAX_DIAMETER: i32 = 6000;

/// One indexed alpha draw: zone slot in the frame's alpha zone list and its index range.
struct AlphaDraw {
    zone: usize,
    start: u32,
    count: u32,
}

struct GpuZone {
    /// World origin of the zone in local units.
    world_origin: (i32, i32),
    /// Vertical extent (`y`, negative is up) of every vertex in the zone.
    y_range: (f32, f32),
    zone_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    opaque: Option<wgpu::Buffer>,
    alpha: Option<wgpu::Buffer>,
    /// HD vertex streams parallel to `opaque` / `alpha` (`None` for Reference geometry).
    opaque_hd: Option<wgpu::Buffer>,
    alpha_hd: Option<wgpu::Buffer>,
    opaque_ranges: Vec<DrawRange>,
    alpha_models: Vec<GpuAlphaModel>,
}

/// Attachments of the scene pass: multisampled colour and depth, and the single-sample colour the
/// pass resolves into.
struct ScenePassTargets<'a> {
    msaa_color: &'a wgpu::TextureView,
    resolve: &'a wgpu::TextureView,
    msaa_depth: &'a wgpu::TextureView,
}

/// Per-frame draw lists of the scene pass.
#[derive(Clone, Copy)]
struct ScenePassInputs<'a> {
    visible: &'a [&'a GpuZone],
    alpha_zones: &'a [&'a GpuZone],
    alpha_draws: &'a [AlphaDraw],
    alpha_index_buffer: Option<&'a wgpu::Buffer>,
    params: &'a FrameParams,
    /// The HD scene shader and its resources (HD profile with materials loaded).
    hd: Option<HdDraw<'a>>,
}

/// What the scene pass needs to draw zones that carry HD vertex data.
#[derive(Clone, Copy)]
struct HdDraw<'a> {
    pipelines: &'a ScenePipelines,
    bind_group: &'a wgpu::BindGroup,
}

/// Opaque and alpha scene pipelines for one colour format.
struct ScenePipelines {
    opaque: wgpu::RenderPipeline,
    alpha: wgpu::RenderPipeline,
}

/// GPU objects only the HD profile needs.
struct HdResources {
    /// Reference shading into the HDR target, for zones without HD vertex data.
    pipelines: ScenePipelines,
    /// The HD scene shader (two vertex buffers per draw).
    scene: ScenePipelines,
    post: hd::post::PostPass,
}

/// Material table, texture array and lighting bound for the HD scene shader.
struct HdBinding {
    bind_group: wgpu::BindGroup,
    textures: hd::materials::MaterialTextureArray,
}

fn build_scene_pipelines(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    pipeline_layout: &wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
    hd_stream: bool,
) -> ScenePipelines {
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
    // The HD stream: octahedral normal, uv, packed corner colours / materials / water data.
    let hd_attributes = [
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Snorm16x2,
            offset: 0,
            shader_location: 3,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x2,
            offset: 4,
            shader_location: 4,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Uint16x4,
            offset: 12,
            shader_location: 5,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Uint16x4,
            offset: 20,
            shader_location: 6,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Uint16x4,
            offset: 28,
            shader_location: 7,
        },
    ];
    let mut vertex_buffers = vec![Some(vertex_layout.clone())];
    if hd_stream {
        vertex_buffers.push(Some(wgpu::VertexBufferLayout {
            array_stride: HdVertex::SIZE as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &hd_attributes,
        }));
    }

    let make_pipeline = |label: &str, blend: Option<wgpu::BlendState>, depth_write: bool| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &vertex_buffers,
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
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
                depth_write_enabled: Some(depth_write),
                depth_compare: Some(wgpu::CompareFunction::Greater),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: MSAA_SAMPLES,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        })
    };

    ScenePipelines {
        opaque: make_pipeline("scene-opaque", None, true),
        alpha: make_pipeline(
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
            // `Zone.flush` wraps every alpha draw in `glDepthMask(false)`.
            false,
        ),
    }
}

/// Device, queue, and the scene pipelines.
pub struct SceneRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// Reference-profile scene pipelines (`COLOR_FORMAT` target).
    pipelines: ScenePipelines,
    scene_shader: wgpu::ShaderModule,
    scene_layout: wgpu::PipelineLayout,
    /// HD pipelines, built the first time an HD frame is encoded.
    hd: std::cell::OnceCell<HdResources>,
    hd_params: HdParams,
    hd_lighting: HdLighting,
    hd_layout: wgpu::BindGroupLayout,
    hd_env_buffer: wgpu::Buffer,
    hd_binding: Option<HdBinding>,
    /// Transient textures of the HD frame graph (HDR colour, its MSAA and depth targets).
    transients: std::cell::RefCell<TransientPool>,
    globals_buffer: wgpu::Buffer,
    globals_layout: wgpu::BindGroupLayout,
    globals_bind_group: wgpu::BindGroup,
    zone_layout: wgpu::BindGroupLayout,
    /// Streamed regions, keyed by an opaque caller key (for example the map region).
    regions: std::collections::BTreeMap<(i32, i32), Vec<GpuZone>>,
    /// World point that renders at local `(0, 0)`; keeps f32 values small far from the origin.
    render_origin: (i32, i32),
    msaa: std::cell::RefCell<Option<MsaaTargets>>,
    /// Per-frame index buffer for the sorted alpha pass.
    alpha_indices: std::cell::RefCell<Option<(wgpu::Buffer, u64)>>,
    /// Scene-pass GPU timing, when the device supports timestamp queries.
    timer: std::cell::RefCell<Option<timing::GpuTimer>>,
    /// Editor guide lines drawn over the scene.
    lines: overlay::LineOverlay,
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
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits: hd::raise_limits(wgpu::Limits::default(), &adapter.limits()),
            ..Default::default()
        }))
        .map_err(|error| GpuError::Device(error.to_string()))?;
        Ok(Self::with_device(device, queue))
    }

    /// Build the pipelines on an existing device (for example the eframe-owned one).
    pub fn with_device(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let timer = timing::GpuTimer::new(&device, &queue);
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

        let pipelines =
            build_scene_pipelines(&device, &shader, &pipeline_layout, COLOR_FORMAT, false);

        let lines = overlay::LineOverlay::new(&device, &globals_layout);
        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<GlobalsUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_bind_group =
            build_globals_bind_group(&device, &queue, &globals_layout, &globals_buffer, &[]);
        let hd_layout = hd::layout::scene_bind_group_layout(&device);
        let hd_env_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hd-environment"),
            size: std::mem::size_of::<hd::layout::HdEnvironmentUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            device,
            queue,
            pipelines,
            scene_shader: shader,
            scene_layout: pipeline_layout,
            hd: std::cell::OnceCell::new(),
            hd_params: HdParams::default(),
            hd_lighting: HdLighting::default(),
            hd_layout,
            hd_env_buffer,
            hd_binding: None,
            transients: std::cell::RefCell::new(TransientPool::default()),
            globals_buffer,
            globals_layout,
            globals_bind_group,
            zone_layout,
            regions: std::collections::BTreeMap::new(),
            render_origin: (0, 0),
            msaa: std::cell::RefCell::new(None),
            alpha_indices: std::cell::RefCell::new(None),
            timer: std::cell::RefCell::new(timer),
            lines,
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

    /// World point that renders at local `(0, 0)`.
    pub const fn render_origin(&self) -> (i32, i32) {
        self.render_origin
    }

    /// Move the render origin (rewrites zone uniforms). Callers must express camera positions
    /// relative to the new origin afterwards.
    pub fn set_render_origin(&mut self, origin: (i32, i32)) {
        self.render_origin = origin;
        for zones in self.regions.values() {
            for zone in zones {
                let uniform = ZoneUniform {
                    base: [
                        (zone.world_origin.0 - origin.0) as f32,
                        0.0,
                        (zone.world_origin.1 - origin.1) as f32,
                        0.0,
                    ],
                };
                self.queue
                    .write_buffer(&zone.zone_buffer, 0, bytemuck::bytes_of(&uniform));
            }
        }
    }

    /// Upload extracted geometry as the only resident scene.
    pub fn upload_scene(&mut self, geometry: &SceneGeometry) {
        self.regions.clear();
        self.upload_region((0, 0), geometry);
    }

    /// Upload (or replace) one streamed region's zones under `key`.
    pub fn upload_region(&mut self, key: (i32, i32), geometry: &SceneGeometry) {
        let mut gpu_zones = Vec::with_capacity(geometry.zones.len());
        // Re-uploads (animated locs every few cycles) reuse the zone's GPU objects, so a
        // refresh is a buffer write instead of fresh allocations.
        let mut reusable: std::collections::HashMap<(i32, i32), GpuZone> = self
            .regions
            .remove(&key)
            .unwrap_or_default()
            .into_iter()
            .map(|zone| (zone.world_origin, zone))
            .collect();
        for zone in &geometry.zones {
            let world_origin = zone.origin();
            let (old_buffers, old_opaque, old_alpha, old_opaque_hd, old_alpha_hd) =
                match reusable.remove(&world_origin) {
                    Some(old) => (
                        Some((old.zone_buffer, old.bind_group)),
                        old.opaque,
                        old.alpha,
                        old.opaque_hd,
                        old.alpha_hd,
                    ),
                    None => (None, None, None, None, None),
                };
            let zone_uniform = ZoneUniform {
                base: [
                    (world_origin.0 - self.render_origin.0) as f32,
                    0.0,
                    (world_origin.1 - self.render_origin.1) as f32,
                    0.0,
                ],
            };
            let (zone_buffer, bind_group) = match old_buffers {
                Some((buffer, bind_group)) => {
                    self.queue
                        .write_buffer(&buffer, 0, bytemuck::bytes_of(&zone_uniform));
                    (buffer, bind_group)
                }
                None => {
                    let buffer =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("zone"),
                                contents: bytemuck::bytes_of(&zone_uniform),
                                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                            });
                    let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("zone"),
                        layout: &self.zone_layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: buffer.as_entire_binding(),
                        }],
                    });
                    (buffer, bind_group)
                }
            };

            let mut opaque: Vec<PackedVertex> = Vec::new();
            let mut alpha: Vec<PackedVertex> = Vec::new();
            let mut opaque_hd: Vec<HdVertex> = Vec::new();
            let mut alpha_hd: Vec<HdVertex> = Vec::new();
            let mut opaque_ranges = Vec::new();
            let mut alpha_models: Vec<GpuAlphaModel> = Vec::new();
            let mut y_range = (f32::MAX, f32::MIN);
            for group in &zone.groups {
                for vertex in group.geometry.opaque.iter().chain(&group.geometry.alpha) {
                    let y = f32::from(vertex.position[1]);
                    y_range = (y_range.0.min(y), y_range.1.max(y));
                }
                if !group.geometry.opaque.is_empty() {
                    opaque_ranges.push(DrawRange {
                        min_plane: group.min_plane,
                        start: opaque.len() as u32,
                        count: group.geometry.opaque.len() as u32,
                    });
                    opaque.extend_from_slice(&group.geometry.opaque);
                    opaque_hd.extend_from_slice(&group.geometry.hd_opaque);
                }
                if !group.geometry.alpha.is_empty() {
                    let base = alpha.len() as u32;
                    for model in &group.geometry.alpha_models {
                        alpha_models.push(GpuAlphaModel {
                            level: group.level,
                            min_plane: group.min_plane,
                            first_vertex: base + model.first_vertex,
                            vertex_count: model.vertex_count,
                            origin: model.origin,
                            radius: model.radius,
                            packed_faces: model.packed_faces.clone(),
                        });
                    }
                    alpha.extend_from_slice(&group.geometry.alpha);
                    alpha_hd.extend_from_slice(&group.geometry.hd_alpha);
                }
            }
            if y_range.0 > y_range.1 {
                continue;
            }
            // Reuse an existing buffer when the new vertices fit; grow with headroom when a
            // refreshed zone outgrows it (animated zones change size).
            let fit = |old: Option<wgpu::Buffer>, bytes: &[u8], label: &str| {
                if bytes.is_empty() {
                    return None;
                }
                match old {
                    Some(buffer) if buffer.size() >= bytes.len() as u64 => {
                        self.queue.write_buffer(&buffer, 0, bytes);
                        Some(buffer)
                    }
                    Some(_) => {
                        let size = (bytes.len() as u64 * 5 / 4 + 3) & !3;
                        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some(label),
                            size,
                            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                        });
                        self.queue.write_buffer(&buffer, 0, bytes);
                        Some(buffer)
                    }
                    None => Some(self.device.create_buffer_init(
                        &wgpu::util::BufferInitDescriptor {
                            label: Some(label),
                            contents: bytes,
                            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        },
                    )),
                }
            };
            gpu_zones.push(GpuZone {
                world_origin,
                y_range,
                zone_buffer,
                bind_group,
                opaque: fit(old_opaque, bytemuck::cast_slice(&opaque), "zone-opaque"),
                alpha: fit(old_alpha, bytemuck::cast_slice(&alpha), "zone-alpha"),
                opaque_hd: fit(old_opaque_hd, bytemuck::cast_slice(&opaque_hd), "zone-opaque-hd"),
                alpha_hd: fit(old_alpha_hd, bytemuck::cast_slice(&alpha_hd), "zone-alpha-hd"),
                opaque_ranges,
                alpha_models,
            });
        }
        self.regions.insert(key, gpu_zones);
    }

    /// Drop a streamed region's GPU resources.
    pub fn remove_region(&mut self, key: (i32, i32)) {
        self.regions.remove(&key);
    }

    /// Number of resident zones.
    pub fn resident_zone_count(&self) -> usize {
        self.regions.values().map(Vec::len).sum()
    }

    /// Bytes of vertex data resident on the GPU.
    pub fn resident_vertex_bytes(&self) -> u64 {
        self.regions
            .values()
            .flatten()
            .flat_map(|zone| {
                [
                    zone.opaque.as_ref(),
                    zone.alpha.as_ref(),
                    zone.opaque_hd.as_ref(),
                    zone.alpha_hd.as_ref(),
                ]
            })
            .flatten()
            .map(wgpu::Buffer::size)
            .sum()
    }

    /// Encode one frame into `encoder`, targeting caller-owned views.
    ///
    /// The color view must be [`COLOR_FORMAT`] and the depth view [`DEPTH_FORMAT`], both sized
    /// `width x height`. Camera uniforms are written through the queue, so the encoder must be
    /// submitted after this call.
    ///
    /// The frame is a [`FrameGraph`]. [`RenderProfile::Reference`] is a single scene pass that
    /// resolves straight into `color_view`. [`RenderProfile::Hd`] renders the scene into a
    /// multisampled HDR target resolved to a single-sample `Rgba16Float` texture, then runs the
    /// resolve pass into `color_view`.
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
            // The reference uniform is inverted: it is `1` when banding is *not* removed.
            smooth_banding: if params.remove_color_banding {
                0.0
            } else {
                1.0
            },
            tick: params.tick,
            texture_light_mode: if params.bright_textures { 1.0 } else { 0.0 },
            fog_color: [
                params.clear_color[0] as f32,
                params.clear_color[1] as f32,
                params.clear_color[2] as f32,
                1.0,
            ],
            camera_x: params.camera.x,
            camera_z: params.camera.z,
            draw_distance: params.fog.draw_distance_tiles as f32 * 128.0,
            fog_depth: params.fog.depth_tiles as f32 * 128.0,
            use_fog: u32::from(params.fog.depth_tiles > 0),
            colorblind_mode: params.colorblind.mode as u32,
            colorblind_intensity: params.colorblind.intensity,
            pad: 0.0,
        };
        self.queue
            .write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));
        let lighting = &self.hd_lighting;
        let environment = hd::layout::HdEnvironmentUniform {
            camera: [params.camera.x, params.camera.y, params.camera.z, 0.0],
            light_dir: [lighting.light_dir[0], lighting.light_dir[1], lighting.light_dir[2], 0.0],
            light_color: [
                lighting.light_color[0],
                lighting.light_color[1],
                lighting.light_color[2],
                lighting.light_strength,
            ],
            ambient_color: [
                lighting.ambient_color[0],
                lighting.ambient_color[1],
                lighting.ambient_color[2],
                lighting.ambient_strength,
            ],
            water_color: [
                lighting.water_color[0],
                lighting.water_color[1],
                lighting.water_color[2],
                0.0,
            ],
            flags: [u32::from(lighting.normal_mapping), lighting.debug_view, 0, 0],
        };
        self.queue
            .write_buffer(&self.hd_env_buffer, 0, bytemuck::bytes_of(&environment));
        let _ = depth_view;
        let visible: Vec<&GpuZone> = self
            .regions
            .values()
            .flatten()
            .filter(|zone| {
                within_draw_distance(zone, self.render_origin, &params)
                    && zone_visible(zone, self.render_origin, &globals.world_proj)
            })
            .collect();
        let (alpha_zones, alpha_draws, alpha_index_buffer) = self.prepare_alpha(&visible, &params);
        let inputs = ScenePassInputs {
            visible: &visible,
            alpha_zones: &alpha_zones,
            alpha_draws: &alpha_draws,
            alpha_index_buffer: alpha_index_buffer.as_ref(),
            params: &params,
            hd: None,
        };

        let mut graph = FrameGraph::new(width, height);
        let output = graph.import("output");
        match params.profile {
            RenderProfile::Reference => {
                let (msaa_color, msaa_depth) = {
                    let mut slot = self.msaa.borrow_mut();
                    let targets = match slot.take() {
                        Some(targets) if targets.width == width && targets.height == height => {
                            targets
                        }
                        _ => self.create_msaa_targets(width, height),
                    };
                    let views = (targets.color.clone(), targets.depth.clone());
                    *slot = Some(targets);
                    views
                };
                graph.add_pass("scene", &[], &[output], move |encoder, resources| {
                    let Some(resolve) = resources.view(output) else {
                        return;
                    };
                    self.record_scene_pass(
                        encoder,
                        ScenePassTargets {
                            msaa_color: &msaa_color,
                            resolve,
                            msaa_depth: &msaa_depth,
                        },
                        &self.pipelines,
                        false,
                        &inputs,
                    );
                });
            }
            RenderProfile::Hd => {
                let hd = self.hd_resources();
                let msaa_color = graph.transient(
                    "hdr-msaa-color",
                    TransientDesc {
                        format: hd::HDR_FORMAT,
                        samples: MSAA_SAMPLES,
                        sampled: false,
                    },
                );
                let msaa_depth = graph.transient(
                    "hdr-msaa-depth",
                    TransientDesc {
                        format: DEPTH_FORMAT,
                        samples: MSAA_SAMPLES,
                        sampled: false,
                    },
                );
                let hdr = graph.transient(
                    "hdr-color",
                    TransientDesc {
                        format: hd::HDR_FORMAT,
                        samples: 1,
                        sampled: true,
                    },
                );
                graph.add_pass(
                    "scene",
                    &[],
                    &[msaa_color, msaa_depth, hdr],
                    move |encoder, resources| {
                        let (Some(msaa_color), Some(msaa_depth), Some(resolve)) = (
                            resources.view(msaa_color),
                            resources.view(msaa_depth),
                            resources.view(hdr),
                        ) else {
                            return;
                        };
                        let hd_inputs = ScenePassInputs {
                            hd: self.hd_binding.as_ref().map(|binding| HdDraw {
                                pipelines: &hd.scene,
                                bind_group: &binding.bind_group,
                            }),
                            ..inputs
                        };
                        self.record_scene_pass(
                            encoder,
                            ScenePassTargets {
                                msaa_color,
                                resolve,
                                msaa_depth,
                            },
                            &hd.pipelines,
                            true,
                            &hd_inputs,
                        );
                    },
                );
                graph.add_pass("resolve", &[hdr], &[output], move |encoder, resources| {
                    let (Some(hdr), Some(target)) = (resources.view(hdr), resources.view(output))
                    else {
                        return;
                    };
                    hd.post.record(
                        &self.device,
                        &self.queue,
                        encoder,
                        hd::post::PostIo {
                            hdr,
                            target,
                            size: (width, height),
                        },
                        self.hd_params,
                    );
                });
            }
        }
        let mut pool = self.transients.borrow_mut();
        if let Err(error) = graph.execute(&self.device, &mut pool, encoder, &[(output, color_view)])
        {
            eprintln!("frame graph: {error}");
        }
    }

    /// The HD pipelines and resolve pass, built on first use.
    fn hd_resources(&self) -> &HdResources {
        self.hd.get_or_init(|| {
            let shader = self
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("hd/scene.wgsl"),
                    source: wgpu::ShaderSource::Wgsl(
                        format!(
                            "{}\n{}",
                            hd::layout::MATERIAL_WGSL,
                            include_str!("hd/scene.wgsl")
                        )
                        .into(),
                    ),
                });
            let layout = self
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("hd-scene"),
                    bind_group_layouts: &[
                        Some(&self.globals_layout),
                        Some(&self.zone_layout),
                        Some(&self.hd_layout),
                    ],
                    immediate_size: 0,
                });
            HdResources {
                pipelines: build_scene_pipelines(
                    &self.device,
                    &self.scene_shader,
                    &self.scene_layout,
                    hd::HDR_FORMAT,
                    false,
                ),
                scene: build_scene_pipelines(&self.device, &shader, &layout, hd::HDR_FORMAT, true),
                post: hd::post::PostPass::new(&self.device),
            }
        })
    }

    /// Bind the HD material table and texture array. [`RenderProfile::Hd`] frames shade zones that
    /// carry HD vertex data with them; until this is called such zones use Reference shading.
    pub fn set_hd_materials(
        &mut self,
        materials: &[hd::layout::GpuMaterial],
        textures: hd::materials::MaterialTextureArray,
    ) {
        let table = hd::layout::MaterialTable::new(&self.device, materials);
        let bind_group = hd::layout::scene_bind_group(
            &self.device,
            &self.hd_layout,
            &table,
            &textures,
            &self.hd_env_buffer,
        );
        self.hd_binding = Some(HdBinding {
            bind_group,
            textures,
        });
    }

    /// The bound HD material texture array, to fill vanilla texture layers.
    pub fn hd_texture_array(&self) -> Option<&hd::materials::MaterialTextureArray> {
        self.hd_binding.as_ref().map(|binding| &binding.textures)
    }

    /// Lighting of the HD scene shader.
    pub fn set_hd_lighting(&mut self, lighting: HdLighting) {
        self.hd_lighting = lighting;
    }

    /// Settings of the HD resolve pass (used by [`RenderProfile::Hd`] frames).
    pub fn set_hd_params(&mut self, params: HdParams) {
        self.hd_params = params;
    }

    /// The scene pass shared by both profiles: opaque geometry, then the sorted alpha geometry,
    /// then the editor guide lines.
    fn record_scene_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        targets: ScenePassTargets<'_>,
        pipelines: &ScenePipelines,
        hdr: bool,
        inputs: &ScenePassInputs<'_>,
    ) {
        let params = inputs.params;
        let mut timer_guard = self.timer.borrow_mut();
        let timed = timer_guard.as_mut().is_some_and(timing::GpuTimer::begin);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: targets.msaa_color,
                    depth_slice: None,
                    resolve_target: Some(targets.resolve),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: params.clear_color[0],
                            g: params.clear_color[1],
                            b: params.clear_color[2],
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: targets.msaa_depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: timer_guard
                    .as_ref()
                    .filter(|_| timed)
                    .map(timing::GpuTimer::pass_writes),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.globals_bind_group, &[]);

            // The pipeline is switched only when consecutive zones disagree on HD shading.
            let mut hd_active: Option<bool> = None;
            for zone in inputs.visible.iter().copied() {
                let Some(buffer) = &zone.opaque else { continue };
                let hd_buffer = inputs.hd.and(zone.opaque_hd.as_ref());
                if hd_active != Some(hd_buffer.is_some()) {
                    hd_active = Some(hd_buffer.is_some());
                    match inputs.hd.filter(|_| hd_buffer.is_some()) {
                        Some(hd) => {
                            pass.set_pipeline(&hd.pipelines.opaque);
                            pass.set_bind_group(2, hd.bind_group, &[]);
                        }
                        None => pass.set_pipeline(&pipelines.opaque),
                    }
                }
                pass.set_bind_group(1, &zone.bind_group, &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                if let Some(hd_buffer) = hd_buffer {
                    pass.set_vertex_buffer(1, hd_buffer.slice(..));
                }
                for range in &zone.opaque_ranges {
                    if range.min_plane <= params.view_plane {
                        pass.draw(range.start..range.start + range.count, 0..1);
                    }
                }
            }

            // Alpha pass: after all opaque geometry, level by level, far zones first; models inside
            // a zone and faces inside close models are ordered by `prepare_alpha`.
            if let Some(index_buffer) = inputs.alpha_index_buffer {
                let mut hd_active: Option<bool> = None;
                for draw in inputs.alpha_draws {
                    let zone = inputs.alpha_zones[draw.zone];
                    let Some(buffer) = &zone.alpha else { continue };
                    let hd_buffer = inputs.hd.and(zone.alpha_hd.as_ref());
                    if hd_active != Some(hd_buffer.is_some()) {
                        hd_active = Some(hd_buffer.is_some());
                        match inputs.hd.filter(|_| hd_buffer.is_some()) {
                            Some(hd) => {
                                pass.set_pipeline(&hd.pipelines.alpha);
                                pass.set_bind_group(2, hd.bind_group, &[]);
                            }
                            None => pass.set_pipeline(&pipelines.alpha),
                        }
                    }
                    pass.set_bind_group(1, &zone.bind_group, &[]);
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    if let Some(hd_buffer) = hd_buffer {
                        pass.set_vertex_buffer(1, hd_buffer.slice(..));
                    }
                    pass.set_index_buffer(
                        index_buffer.slice(
                            u64::from(draw.start) * 4..u64::from(draw.start + draw.count) * 4,
                        ),
                        wgpu::IndexFormat::Uint32,
                    );
                    pass.draw_indexed(0..draw.count, 0, 0..1);
                }
            }

            self.lines.draw(&mut pass, &self.globals_bind_group, hdr);
        }
        if timed && let Some(timer) = timer_guard.as_ref() {
            timer.resolve(encoder);
        }
    }

    /// Replace the editor guide lines (pairs of vertices, render-local positions).
    pub fn set_overlay_lines(&mut self, lines: &[LineVertex]) {
        self.lines.set(&self.device, &self.queue, lines);
    }

    /// Call after the encoder from [`Self::encode_frame`] was submitted: collects GPU timings.
    pub fn finish_frame(&self) {
        if let Some(timer) = self.timer.borrow_mut().as_mut() {
            timer.finish(&self.device);
        }
    }

    /// Smoothed GPU time of the scene pass in milliseconds (`None` without timestamp queries or
    /// before the first result).
    pub fn gpu_scene_ms(&self) -> Option<f32> {
        self.timer
            .borrow()
            .as_ref()
            .and_then(timing::GpuTimer::scene_ms)
    }

    /// Whether GPU timestamp queries are available on this device.
    pub fn gpu_timing_supported(&self) -> bool {
        self.timer.borrow().is_some()
    }

    /// Order the transparent geometry like RuneLite's `Zone.alphaSort` / `renderAlpha`.
    ///
    /// Zones go far to near; inside a zone and level, models go far to near by origin distance;
    /// inside models near the camera (zone origin within [`ALPHA_ZSORT_CLOSE`]) faces go back to
    /// front in camera-depth bins of the face centroid. Returns the zone list the draws index
    /// into, the draws, and the uploaded index buffer.
    fn prepare_alpha<'a>(
        &self,
        visible: &[&'a GpuZone],
        params: &FrameParams,
    ) -> (Vec<&'a GpuZone>, Vec<AlphaDraw>, Option<wgpu::Buffer>) {
        let camera = params.camera;
        let origin = self.render_origin;
        let zone_base = |zone: &GpuZone| {
            (
                (zone.world_origin.0 - origin.0) as f32,
                (zone.world_origin.1 - origin.1) as f32,
            )
        };
        let distance_squared = |zone: &GpuZone| {
            let (x, z) = zone_base(zone);
            (x + 512.0 - camera.x).powi(2) + (z + 512.0 - camera.z).powi(2)
        };
        let mut alpha_zones: Vec<&GpuZone> = visible
            .iter()
            .copied()
            .filter(|zone| zone.alpha.is_some() && !zone.alpha_models.is_empty())
            .collect();
        alpha_zones.sort_by(|a, b| distance_squared(b).total_cmp(&distance_squared(a)));

        let (sin_yaw, cos_yaw) = camera.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = camera.pitch.sin_cos();
        let mut indices: Vec<u32> = Vec::new();
        let mut draws: Vec<AlphaDraw> = Vec::new();
        for level in 0..=3_u8 {
            for (slot, zone) in alpha_zones.iter().enumerate() {
                let (base_x, base_z) = zone_base(zone);
                let mut models: Vec<(f32, &GpuAlphaModel)> = zone
                    .alpha_models
                    .iter()
                    .filter(|m| m.level == level && m.min_plane <= params.view_plane)
                    .map(|m| {
                        let dx = base_x + m.origin[0] as f32 - camera.x;
                        let dy = m.origin[1] as f32 - camera.y;
                        let dz = base_z + m.origin[2] as f32 - camera.z;
                        (dx * dx + dy * dy + dz * dz, m)
                    })
                    .collect();
                if models.is_empty() {
                    continue;
                }
                // Stable: equal distances keep emission order like `List.sort`.
                models.sort_by(|a, b| b.0.total_cmp(&a.0));
                let close = (camera.x - base_x).powi(2) + (camera.z - base_z).powi(2)
                    < ALPHA_ZSORT_CLOSE * ALPHA_ZSORT_CLOSE;
                let start = indices.len() as u32;
                for (_, model) in models {
                    if !close {
                        indices.extend(model.first_vertex..model.first_vertex + model.vertex_count);
                        continue;
                    }
                    let diameter = 1 + model.radius * 2;
                    if diameter >= ALPHA_MAX_DIAMETER {
                        continue;
                    }
                    let mut faces: Vec<(i32, u32)> = (0..model.packed_faces.len())
                        .map(|face| {
                            let pack = model.packed_faces[face];
                            let x = (pack >> 21) as f32;
                            let y = ((pack << 11) >> 22) as f32;
                            let z = ((pack << 21) >> 21) as f32;
                            let t = (z * cos_yaw - x * sin_yaw).floor();
                            let depth = (y * sin_pitch + t * cos_pitch).floor() as i32;
                            ((depth + model.radius).clamp(0, diameter - 1), face as u32)
                        })
                        .collect();
                    // Highest bin (farthest) first; faces in a bin keep their order.
                    faces.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
                    for (_, face) in faces {
                        let first = model.first_vertex + face * 3;
                        indices.extend(first..first + 3);
                    }
                }
                let count = indices.len() as u32 - start;
                if count > 0 {
                    draws.push(AlphaDraw {
                        zone: slot,
                        start,
                        count,
                    });
                }
            }
        }
        if indices.is_empty() {
            return (alpha_zones, draws, None);
        }
        let needed = indices.len() as u64;
        let mut slot = self.alpha_indices.borrow_mut();
        if slot.as_ref().is_none_or(|(_, capacity)| *capacity < needed) {
            let capacity = needed.next_power_of_two().max(4096);
            *slot = Some((
                self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("alpha-indices"),
                    size: capacity * 4,
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                capacity,
            ));
        }
        let buffer = slot.as_ref().map(|(buffer, _)| buffer.clone());
        if let Some(buffer) = &buffer {
            self.queue
                .write_buffer(buffer, 0, bytemuck::cast_slice(&indices));
        }
        (alpha_zones, draws, buffer)
    }

    fn create_msaa_targets(&self, width: u32, height: u32) -> MsaaTargets {
        let make = |label: &str, format: wgpu::TextureFormat| {
            self.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: MSAA_SAMPLES,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        };
        MsaaTargets {
            width,
            height,
            color: make("scene-msaa-color", COLOR_FORMAT),
            depth: make("scene-msaa-depth", DEPTH_FORMAT),
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

/// Conservative frustum test of one zone's bounding box against the world projection.
///
/// The reference projection has clip `w` equal to camera distance, so a box is outside when all
/// eight corners lie beyond the same clip plane, or all lie closer than the `2 * near` point where
/// depth would exceed `1`.
/// RuneLite's draw distance: zones farther than `draw_distance_tiles` from the camera (square
/// distance to the nearest edge of the zone) are not drawn.
fn within_draw_distance(zone: &GpuZone, origin: (i32, i32), params: &FrameParams) -> bool {
    let limit = params.fog.draw_distance_tiles as f32 * 128.0;
    let base_x = (zone.world_origin.0 - origin.0) as f32;
    let base_z = (zone.world_origin.1 - origin.1) as f32;
    let dx = (base_x - params.camera.x)
        .max(params.camera.x - (base_x + ZONE_LOCAL_EXTENT))
        .max(0.0);
    let dz = (base_z - params.camera.z)
        .max(params.camera.z - (base_z + ZONE_LOCAL_EXTENT))
        .max(0.0);
    dx.max(dz) <= limit
}

fn zone_visible(zone: &GpuZone, origin: (i32, i32), world_proj: &[[f32; 4]; 4]) -> bool {
    let base_x = (zone.world_origin.0 - origin.0) as f32;
    let base_z = (zone.world_origin.1 - origin.1) as f32;
    let extent = ZONE_LOCAL_EXTENT;
    // Zone content may overhang the 1024-unit footprint (large models), so pad generously.
    let pad = 512.0;
    let mut outside = [0_u8; 5]; // counts of corners outside: left, right, bottom, top, near
    for &dx in &[-pad, extent + pad] {
        for &dz in &[-pad, extent + pad] {
            for &y in &[zone.y_range.0 - 256.0, zone.y_range.1 + 256.0] {
                let p = [base_x + dx, y, base_z + dz, 1.0];
                let mut clip = [0.0_f32; 4];
                for (row, value) in clip.iter_mut().enumerate() {
                    *value = (0..4).map(|k| world_proj[k][row] * p[k]).sum();
                }
                let w = clip[3];
                outside[0] += u8::from(clip[0] < -w);
                outside[1] += u8::from(clip[0] > w);
                outside[2] += u8::from(clip[1] < -w);
                outside[3] += u8::from(clip[1] > w);
                outside[4] += u8::from(w < 2.0 * REFERENCE_NEAR);
            }
        }
    }
    outside.iter().all(|&count| count < 8)
}

const ZONE_LOCAL_EXTENT: f32 = 1024.0;
