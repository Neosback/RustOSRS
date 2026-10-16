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

mod counters;
pub mod frame_graph;
pub mod hd;
mod overlay;
mod profile;
mod resources;
pub mod theme;
mod timing;
pub use counters::RenderCounters;
pub use resources::texture_storage_bytes;

pub use overlay::LineVertex;
pub use profile::{HdLighting, HdParams, RenderProfile};

use osrs_hd::shadows::{ShadowCamera, ShadowFit, ShadowSettings};

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
#[derive(Debug, Clone, Copy, PartialEq)]
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
    level: u8,
    min_plane: u8,
    start: u32,
    count: u32,
    /// Vertices at the start of the range that are terrain (HD geometry only).
    terrain_count: u32,
    /// Vertices at the end of the range that use cutout materials (HD geometry only).
    cutout_count: u32,
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

/// The light's camera for this frame and the zones drawn into the shadow map.
/// What a drawn shadow map depends on: while it is unchanged the map is reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShadowSignature {
    view_proj: [u32; 16],
    size: u32,
    geometry_version: u64,
    mode: osrs_hd::shadows::ShadowMode,
    transparency: bool,
    terrain: bool,
    roof_shadows: bool,
    view_plane: u8,
}

/// The shadow map moves in steps of this many world units (two tiles), so it is redrawn only
/// after the camera moved that far, not every frame.
const SHADOW_SNAP_UNITS: f32 = 256.0;

struct ShadowPlan<'a> {
    /// The map has to be drawn again (its signature changed).
    redraw: bool,
    camera: ShadowCamera,
    zones: Vec<&'a GpuZone>,
    /// Shadow draw distance in render units.
    radius: f32,
    /// Shadow map edge in texels.
    size: u32,
}

/// The shadow fields of [`hd::layout::HdEnvironmentUniform`].
#[derive(Default)]
struct ShadowUniform {
    light_proj: [[f32; 4]; 4],
    right: [f32; 4],
    up: [f32; 4],
    axis: [f32; 4],
    params: [f32; 4],
    flags: [u32; 4],
}

/// What the scene pass needs to draw zones that carry HD vertex data.
#[derive(Clone, Copy)]
struct HdDraw<'a> {
    pipelines: &'a ScenePipelines,
    bind_group: &'a wgpu::BindGroup,
    /// The shadow maps (group 3).
    shadow_group: &'a wgpu::BindGroup,
}

/// Opaque and alpha scene pipelines for one colour format.
struct ScenePipelines {
    opaque: wgpu::RenderPipeline,
    /// Opaque with alpha-to-coverage, for cutout materials (HD stream only).
    cutout: Option<wgpu::RenderPipeline>,
    alpha: wgpu::RenderPipeline,
}

/// GPU objects only the HD profile needs.
struct HdResources {
    sky: wgpu::RenderPipeline,
    /// Reference shading into the HDR target, for zones without HD vertex data.
    pipelines: ScenePipelines,
    /// The HD scene shader (two vertex buffers per draw).
    scene: ScenePipelines,
    shadows: hd::shadow::ShadowPipelines,
    post: hd::post::PostPass,
}

/// Material table, texture array and lighting bound for the HD scene shader.
struct HdBinding {
    bind_group: wgpu::BindGroup,
    table: hd::layout::MaterialTable,
    textures: hd::materials::MaterialTextureArray,
}

fn build_scene_pipelines(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    pipeline_layout: &wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
    hd_stream: bool,
) -> ScenePipelines {
    let [vertex_layout, hd_layout] = hd::shadow::hd_vertex_buffers();
    let mut vertex_buffers = vec![Some(vertex_layout)];
    if hd_stream {
        vertex_buffers.push(Some(hd_layout));
    }

    // The cutout pipeline turns the fragment alpha into MSAA coverage, which cuts out the texels
    // of transparent materials (leaves, fences) with depth writes and no sorting. Solid geometry
    // keeps a plain opaque pipeline, which tile-based GPUs can resolve without shading hidden
    // fragments.
    let make_pipeline =
        |label: &str, blend: Option<wgpu::BlendState>, depth_write: bool, coverage: bool| {
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
                    alpha_to_coverage_enabled: coverage,
                },
                multiview_mask: None,
                cache: None,
            })
        };

    ScenePipelines {
        opaque: make_pipeline("scene-opaque", None, true, false),
        cutout: hd_stream.then(|| make_pipeline("scene-cutout", None, true, true)),
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
    theme_vertex_active: bool,
    hd_lighting: HdLighting,
    hd_layout: wgpu::BindGroupLayout,
    hd_env_buffer: wgpu::Buffer,
    hd_binding: Option<HdBinding>,
    /// The water type table (storage buffer, `WaterType.NONE` first) and the special water layers.
    hd_water_types: wgpu::Buffer,
    hd_water_layers: hd::layout::WaterLayers,
    /// Shadow options and the shadow maps they ask for (rebuilt when the size changes).
    hd_shadows: ShadowSettings,
    hd_shadow_layout: wgpu::BindGroupLayout,
    hd_shadow_maps: std::cell::RefCell<Option<hd::shadow::ShadowMaps>>,
    /// One-texel maps bound while shadows are off.
    hd_shadow_dummy: hd::shadow::ShadowMaps,
    /// Edge of the previous shadow fit (shimmer hysteresis).
    hd_shadow_size: std::cell::Cell<Option<f32>>,
    /// Signature of the shadow map currently held by the textures.
    hd_shadow_drawn: std::cell::Cell<Option<ShadowSignature>>,
    /// Bumped whenever geometry that can cast shadows changes.
    shadow_geometry_version: u64,
    counters: counters::FrameCounters,
    /// Transient textures of the HD frame graph (HDR colour, its MSAA and depth targets).
    transients: std::cell::RefCell<TransientPool>,
    globals_buffer: wgpu::Buffer,
    globals_layout: wgpu::BindGroupLayout,
    globals_bind_group: wgpu::BindGroup,
    reference_texture_layers: u32,
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
    auxiliary_timers: std::cell::RefCell<[Option<timing::GpuTimer>; 3]>,
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
        let auxiliary_timers = std::array::from_fn(|_| {
            std::env::var_os("RUSTOSRS_PERF_REPORT")
                .and_then(|_| timing::GpuTimer::new(&device, &queue))
        });
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
        let hd_shadow_layout = hd::shadow::maps_bind_group_layout(&device);
        let hd_water_types =
            hd::layout::water_types_buffer(&device, &[hd::layout::GpuWaterType::none(|_| -1)]);
        let hd_shadow_dummy = hd::shadow::ShadowMaps::new(&device, &hd_shadow_layout, 1);
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
            theme_vertex_active: false,
            hd_lighting: HdLighting::default(),
            hd_layout,
            hd_env_buffer,
            hd_binding: None,
            hd_water_types,
            hd_water_layers: hd::layout::WaterLayers::default(),
            hd_shadows: ShadowSettings::default(),
            hd_shadow_dummy,
            hd_shadow_layout,
            hd_shadow_maps: std::cell::RefCell::new(None),
            hd_shadow_size: std::cell::Cell::new(None),
            hd_shadow_drawn: std::cell::Cell::new(None),
            shadow_geometry_version: 0,
            counters: counters::FrameCounters::default(),
            transients: std::cell::RefCell::new(TransientPool::default()),
            globals_buffer,
            globals_layout,
            globals_bind_group,
            reference_texture_layers: 1,
            zone_layout,
            regions: std::collections::BTreeMap::new(),
            render_origin: (0, 0),
            msaa: std::cell::RefCell::new(None),
            alpha_indices: std::cell::RefCell::new(None),
            timer: std::cell::RefCell::new(timer),
            auxiliary_timers: std::cell::RefCell::new(auxiliary_timers),
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
        self.reference_texture_layers = layers.len().clamp(1, MAX_TEXTURE_LAYERS) as u32;
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

    /// Work counted since the previous call: draws, vertices, uploaded bytes and shadow redraws.
    pub fn take_counters(&self) -> RenderCounters {
        self.counters.take()
    }

    /// Upload extracted geometry as the only resident scene.
    pub fn upload_scene(&mut self, geometry: &SceneGeometry) {
        self.regions.clear();
        self.shadow_geometry_version += 1;
        self.upload_region((0, 0), geometry);
    }

    /// Upload (or replace) one streamed region's zones under `key`.
    pub fn upload_region(&mut self, key: (i32, i32), geometry: &SceneGeometry) {
        self.upload_zones(key, geometry.zones.iter());
    }

    /// Upload borrowed zones without copying their CPU vertex streams into a temporary scene.
    pub fn upload_zones<'a>(
        &mut self,
        key: (i32, i32),
        zones: impl ExactSizeIterator<Item = &'a crate::ZoneGeometry>,
    ) {
        let mut gpu_zones = Vec::with_capacity(zones.len());
        // Re-uploads (animated locs every few cycles) reuse the zone's GPU objects, so a
        // refresh is a buffer write instead of fresh allocations.
        let previous = self.regions.remove(&key).unwrap_or_default();
        let had_hd = previous
            .iter()
            .any(|zone| zone.opaque_hd.is_some() || zone.alpha_hd.is_some());
        let mut reusable: std::collections::HashMap<(i32, i32), GpuZone> = previous
            .into_iter()
            .map(|zone| (zone.world_origin, zone))
            .collect();
        for zone in zones {
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
                        level: group.level,
                        min_plane: group.min_plane,
                        start: opaque.len() as u32,
                        count: group.geometry.opaque.len() as u32,
                        terrain_count: group.geometry.hd_terrain_opaque,
                        cutout_count: group.geometry.hd_cutout_opaque,
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
                self.counters.upload(bytes.len());
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
                opaque_hd: fit(
                    old_opaque_hd,
                    bytemuck::cast_slice(&opaque_hd),
                    "zone-opaque-hd",
                ),
                alpha_hd: fit(
                    old_alpha_hd,
                    bytemuck::cast_slice(&alpha_hd),
                    "zone-alpha-hd",
                ),
                opaque_ranges,
                alpha_models,
            });
        }
        // Only geometry with HD streams casts shadows (the per-cycle animated zone does not).
        if gpu_zones
            .iter()
            .any(|zone| zone.opaque_hd.is_some() || zone.alpha_hd.is_some())
            || had_hd
        {
            self.shadow_geometry_version += 1;
        }
        self.regions.insert(key, gpu_zones);
    }

    /// Drop a streamed region's GPU resources.
    pub fn remove_region(&mut self, key: (i32, i32)) {
        if self.regions.remove(&key).is_some_and(|zones| {
            zones
                .iter()
                .any(|zone| zone.opaque_hd.is_some() || zone.alpha_hd.is_some())
        }) {
            self.shadow_geometry_version += 1;
        }
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
        let _ = depth_view;
        self.encode_color_frame(encoder, color_view, params, width, height);
    }

    /// Encode into the caller's color target. Depth is owned by the renderer's MSAA/frame graph;
    /// no unused caller-side depth allocation is required.
    pub fn encode_color_frame(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
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
        let shadow_plan = if params.profile == RenderProfile::Hd {
            self.plan_shadows(&params)
        } else {
            None
        };
        if let Some(plan) = &shadow_plan {
            self.ensure_shadow_maps(plan.size);
        }
        let maps_guard = self.hd_shadow_maps.borrow();
        let shadow_maps = match (&shadow_plan, maps_guard.as_ref()) {
            (Some(_), Some(maps)) => maps,
            _ => &self.hd_shadow_dummy,
        };
        let hd = &self.hd_lighting;
        let lighting = &hd.lighting;
        let shadow_uniform = self.shadow_uniform(shadow_plan.as_ref(), shadow_maps);
        let with4 = |v: [f32; 3]| [v[0], v[1], v[2], 0.0];
        let (sy, cy) = params.camera.yaw.sin_cos();
        let (sp, cp) = params.camera.pitch.sin_cos();
        let screen_x = width as f32 / (2.0 * params.camera.scale);
        let screen_y = height as f32 / (2.0 * params.camera.scale);
        let environment = hd::layout::HdEnvironmentUniform {
            sky_right: [cy * screen_x, 0.0, sy * screen_x, 0.0],
            sky_down: [sy * sp * screen_y, cp * screen_y, -cy * sp * screen_y, 0.0],
            sky_forward: [-sy * cp, sp, cy * cp, 0.0],
            light_proj: shadow_uniform.light_proj,
            shadow_right: shadow_uniform.right,
            shadow_up: shadow_uniform.up,
            shadow_axis: shadow_uniform.axis,
            shadow_params: shadow_uniform.params,
            shadow_flags: shadow_uniform.flags,
            water_light: with4(lighting.water_color_light),
            water_mid: with4(lighting.water_color_mid),
            water_dark: with4(lighting.water_color_dark),
            caustics: [
                lighting.caustics_color[0],
                lighting.caustics_color[1],
                lighting.caustics_color[2],
                lighting.caustics_strength,
            ],
            scene_base: [
                self.render_origin.0 as f32,
                self.render_origin.1 as f32,
                0.0,
                0.0,
            ],
            water_layers: [
                self.hd_water_layers.flow_map,
                self.hd_water_layers.foam,
                self.hd_water_layers.caustics,
                self.hd_water_layers.underwater_flow_map,
            ],
            camera: [
                params.camera.x,
                params.camera.y,
                params.camera.z,
                hd.elapsed_seconds,
            ],
            light_dir: [
                lighting.light_dir[0],
                lighting.light_dir[1],
                lighting.light_dir[2],
                f32::from(u8::from(lighting.casts_shadows)),
            ],
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
            underglow: [
                lighting.underglow_color[0],
                lighting.underglow_color[1],
                lighting.underglow_color[2],
                lighting.underglow_strength,
            ],
            fog_color: [
                lighting.fog_color[0],
                lighting.fog_color[1],
                lighting.fog_color[2],
                lighting.fog_depth,
            ],
            fog_params: [
                params.fog.draw_distance_tiles as f32,
                lighting.ground_fog_start,
                lighting.ground_fog_end,
                lighting.ground_fog_opacity,
            ],
            water_color: [
                lighting.water_color[0],
                lighting.water_color[1],
                lighting.water_color[2],
                0.0,
            ],
            misc: [
                lighting.lightning_brightness,
                f32::from(u8::from(hd.underwater_caustics)),
                hd.gamma,
                0.0,
            ],
            flags: [
                u32::from(hd.normal_mapping),
                hd.debug_view,
                u32::from(lighting.underwater_environment),
                u32::from(hd.parallax_mapping),
            ],
        };
        self.queue
            .write_buffer(&self.hd_env_buffer, 0, bytemuck::bytes_of(&environment));
        let mut visible: Vec<&GpuZone> = self
            .regions
            .values()
            .flatten()
            .filter(|zone| {
                within_draw_distance(zone, self.render_origin, &params)
                    && zone_visible(zone, self.render_origin, &globals.world_proj)
            })
            .collect();
        self.counters.set_visible_zones(visible.len());
        if params.profile == RenderProfile::Hd {
            // The HD fragment shader is expensive: draw the opaque zones front to back so the
            // depth test rejects hidden fragments early. (The Reference profile keeps its order:
            // coplanar faces resolve by draw order.)
            let origin = self.render_origin;
            let camera = params.camera;
            let distance = |zone: &GpuZone| {
                let x =
                    (zone.world_origin.0 - origin.0) as f32 + ZONE_LOCAL_EXTENT / 2.0 - camera.x;
                let z =
                    (zone.world_origin.1 - origin.1) as f32 + ZONE_LOCAL_EXTENT / 2.0 - camera.z;
                x * x + z * z
            };
            visible.sort_by(|a, b| distance(a).total_cmp(&distance(b)));
        }
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
                // An unchanged shadow map is not drawn again: only the passes that run produce
                // the token the scene pass waits for.
                let shadow_token = shadow_plan
                    .as_ref()
                    .filter(|plan| plan.redraw)
                    .map(|_| graph.token("shadow-maps"));
                if let (Some(plan), Some(token)) = (&shadow_plan, shadow_token) {
                    let binding = self.hd_binding.as_ref();
                    graph.add_pass("shadow", &[], &[token], move |encoder, _| {
                        if let Some(binding) = binding {
                            self.record_shadow_pass(
                                encoder,
                                plan,
                                shadow_maps,
                                hd,
                                binding,
                                params.view_plane,
                            );
                        }
                    });
                }
                let scene_reads: Vec<_> = shadow_token.into_iter().collect();
                graph.add_pass(
                    "scene",
                    &scene_reads,
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
                                shadow_group: shadow_maps.bind_group(),
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
                    let mut timers = self.auxiliary_timers.borrow_mut();
                    let timer = &mut timers[2];
                    let timed = timer.as_mut().is_some_and(timing::GpuTimer::begin);
                    hd.post.record(
                        &self.device,
                        &self.queue,
                        encoder,
                        hd::post::PostIo {
                            hdr,
                            target,
                            size: (width, height),
                            elapsed_seconds: self.hd_lighting.elapsed_seconds,
                            timestamps: timer
                                .as_ref()
                                .filter(|_| timed)
                                .map(timing::GpuTimer::pass_writes),
                        },
                        self.hd_params,
                    );
                    if timed && let Some(timer) = timer.as_ref() {
                        timer.resolve(encoder);
                    }
                });
            }
        }
        let mut pool = self.transients.borrow_mut();
        if let Err(error) = graph.execute(&self.device, &mut pool, encoder, &[(output, color_view)])
        {
            eprintln!("frame graph: {error}");
        }
    }

    /// Shadow options of the HD profile (Properties > Shading > Shadows).
    pub fn set_hd_shadows(&mut self, settings: ShadowSettings) {
        self.hd_shadows = settings;
    }

    /// GPU memory the shadow maps currently take, in bytes.
    pub fn hd_shadow_map_bytes(&self) -> u64 {
        self.hd_shadow_maps
            .borrow()
            .as_ref()
            .map_or(0, hd::shadow::ShadowMaps::bytes)
    }

    /// Frame the shadow map around the camera and pick the zones that can cast into it. `None`
    /// when shadows are off, the light casts none, or there is nothing to shadow.
    fn plan_shadows(&self, params: &FrameParams) -> Option<ShadowPlan<'_>> {
        let settings = self.hd_shadows;
        let lighting = &self.hd_lighting.lighting;
        if !settings.enabled() || !lighting.casts_shadows || self.hd_binding.is_none() {
            return None;
        }
        let radius = settings
            .distance_tiles
            .min(params.fog.draw_distance_tiles)
            .max(1) as f32
            * 128.0;
        let origin = self.render_origin;
        let camera = params.camera;
        let has_hd = |zone: &&GpuZone| zone.opaque_hd.is_some() || zone.alpha_hd.is_some();
        let near = |zone: &&GpuZone| {
            let centre_x = (zone.world_origin.0 - origin.0) as f32 + ZONE_LOCAL_EXTENT / 2.0;
            let centre_z = (zone.world_origin.1 - origin.1) as f32 + ZONE_LOCAL_EXTENT / 2.0;
            (centre_x - camera.x).abs().max((centre_z - camera.z).abs())
                <= radius + ZONE_LOCAL_EXTENT
        };
        let (mut y_min, mut y_max) = (f32::MAX, f32::MIN);
        for zone in self.regions.values().flatten().filter(has_hd).filter(near) {
            y_min = y_min.min(zone.y_range.0 - 256.0);
            y_max = y_max.max(zone.y_range.1 + 256.0);
        }
        if y_min > y_max {
            return None;
        }
        let size = hd::shadow::map_size(&settings, self.device.limits().max_texture_dimension_2d);
        let fitted = ShadowCamera::fit(&ShadowFit {
            light_dir: lighting.light_dir,
            focus: [camera.x, 0.0, camera.z],
            radius,
            y_range: (y_min, y_max),
            resolution: size,
            previous_size: self.hd_shadow_size.get(),
            snap_world: SHADOW_SNAP_UNITS,
        });
        self.hd_shadow_size.set(Some(fitted.size));
        let zones = self
            .regions
            .values()
            .flatten()
            .filter(has_hd)
            .filter(|zone| zone_overlaps_shadow(zone, origin, &fitted))
            .collect();
        let signature = ShadowSignature {
            view_proj: std::array::from_fn(|i| fitted.view_proj[i / 4][i % 4].to_bits()),
            size,
            geometry_version: self.shadow_geometry_version,
            mode: settings.mode,
            transparency: settings.transparency,
            terrain: settings.terrain_shadows,
            roof_shadows: settings.roof_shadows,
            view_plane: params.view_plane,
        };
        let redraw = self.hd_shadow_drawn.get() != Some(signature);
        if redraw && std::env::var_os("RUSTOSRS_SHADOW_DEBUG").is_some() {
            eprintln!("shadow map redrawn: {signature:?}");
        }
        if redraw {
            self.counters.shadow_redraw();
        }
        self.hd_shadow_drawn.set(Some(signature));
        Some(ShadowPlan {
            redraw,
            camera: fitted,
            zones,
            radius,
            size,
        })
    }

    /// (Re)create the shadow maps when their size changed.
    fn ensure_shadow_maps(&self, size: u32) {
        let mut slot = self.hd_shadow_maps.borrow_mut();
        if slot.as_ref().is_none_or(|maps| maps.size != size) {
            *slot = Some(hd::shadow::ShadowMaps::new(
                &self.device,
                &self.hd_shadow_layout,
                size,
            ));
        }
    }

    /// The shadow fields of the environment uniform.
    fn shadow_uniform(
        &self,
        plan: Option<&ShadowPlan<'_>>,
        maps: &hd::shadow::ShadowMaps,
    ) -> ShadowUniform {
        use osrs_hd::shadows::FilterKind;
        let Some(plan) = plan else {
            return ShadowUniform::default();
        };
        let settings = &self.hd_shadows;
        let camera = &plan.camera;
        let kind = match settings.filtering.kind() {
            FilterKind::Pcf => 0,
            FilterKind::Dither => 1,
            FilterKind::Average => 2,
            FilterKind::Pcss => 3,
        };
        let bits = u32::from(settings.transparency)
            | u32::from(settings.mode == osrs_hd::shadows::ShadowMode::Detailed) << 1
            | u32::from(settings.terrain_shadows) << 2;
        let with = |v: [f32; 3], w: f32| [v[0], v[1], v[2], w];
        ShadowUniform {
            light_proj: camera.view_proj,
            right: with(camera.right, camera.size),
            up: with(camera.up, camera.texel_world),
            axis: with(camera.axis, camera.depth_range),
            params: [
                camera.bias_scale,
                plan.radius,
                maps.size as f32,
                maps.terrain_size as f32,
            ],
            flags: [1, kind, settings.filtering.kernel(), bits],
        }
    }

    /// Fill the shadow maps: the main map with everything that casts, the terrain map with the
    /// ground plane's terrain.
    fn record_shadow_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        plan: &ShadowPlan<'_>,
        maps: &hd::shadow::ShadowMaps,
        hd: &HdResources,
        binding: &HdBinding,
        view_plane: u8,
    ) {
        let settings = &self.hd_shadows;
        for terrain_only in [false, true] {
            if terrain_only && !settings.terrain_shadows {
                continue;
            }
            let (view, pipeline) = if terrain_only {
                (maps.terrain_view(), &hd.shadows.terrain)
            } else if settings.transparency {
                (maps.view(), &hd.shadows.packed)
            } else {
                (maps.view(), &hd.shadows.plain)
            };
            let mut timers = self.auxiliary_timers.borrow_mut();
            let timer = &mut timers[usize::from(terrain_only)];
            let timed = timer.as_mut().is_some_and(timing::GpuTimer::begin);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(if terrain_only {
                    "terrain-shadow"
                } else {
                    "shadow"
                }),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: timer
                    .as_ref()
                    .filter(|_| timed)
                    .map(timing::GpuTimer::pass_writes),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(2, &binding.bind_group, &[]);
            for zone in &plan.zones {
                pass.set_bind_group(1, &zone.bind_group, &[]);
                if let (Some(buffer), Some(hd_buffer)) = (&zone.opaque, &zone.opaque_hd) {
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.set_vertex_buffer(1, hd_buffer.slice(..));
                    for range in &zone.opaque_ranges {
                        // Hidden roofs only cast when roof shadows are on.
                        if !(settings.roof_shadows || range.min_plane <= view_plane) {
                            continue;
                        }
                        // The ground plane's terrain only exists in the terrain map; the main
                        // map takes the models (and upper floors) of the group.
                        let ground = range.level == 0;
                        let terrain_end = range.start + range.terrain_count.min(range.count);
                        let end = range.start + range.count;
                        let drawn = match (terrain_only, ground) {
                            (true, true) => range.start..terrain_end,
                            (true, false) => continue,
                            (false, true) => terrain_end..end,
                            (false, false) => range.start..end,
                        };
                        if !drawn.is_empty() {
                            self.counters.draw(drawn.end - drawn.start);
                            pass.draw(drawn, 0..1);
                        }
                    }
                }
                if !terrain_only
                    && let (Some(buffer), Some(hd_buffer)) = (&zone.alpha, &zone.alpha_hd)
                {
                    let count = zone
                        .alpha_models
                        .iter()
                        .map(|model| model.first_vertex + model.vertex_count)
                        .max()
                        .unwrap_or(0);
                    if count > 0 {
                        pass.set_vertex_buffer(0, buffer.slice(..));
                        pass.set_vertex_buffer(1, hd_buffer.slice(..));
                        self.counters.draw(count);
                        pass.draw(0..count, 0..1);
                    }
                }
            }
            drop(pass);
            if timed && let Some(timer) = timer.as_ref() {
                timer.resolve(encoder);
            }
        }
    }

    /// The HD pipelines and resolve pass, built on first use.
    fn hd_resources(&self) -> &HdResources {
        self.hd.get_or_init(|| {
            let scene = hd::shadow::shader_source(&format!(
                "{}\n{}\n{}",
                osrs_hd::theme::shader::ShaderStage::ALL
                    .into_iter()
                    .filter(|stage| *stage != osrs_hd::theme::shader::ShaderStage::Post)
                    .map(|stage| stage.template())
                    .collect::<Vec<_>>()
                    .join("\n"),
                include_str!("hd/shadow_receive.wgsl"),
                include_str!("hd/scene.wgsl")
            ));
            let post = format!(
                "{}\n{}",
                osrs_hd::theme::shader::ShaderStage::Post.template(),
                include_str!("hd/post.wgsl")
            );
            self.build_hd_resources(
                &scene,
                &post,
                &hd::shadow::shader_source(include_str!("hd/sky.wgsl")),
                "",
            )
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
        self.shadow_geometry_version += 1;
        let bind_group = self.hd_scene_bind_group(&table, &textures);
        self.hd_binding = Some(HdBinding {
            bind_group,
            table,
            textures,
        });
    }

    /// Bind the water type table (index `0` is `WaterType.NONE`) and the layers of the special
    /// water materials. Rebinds the scene resources when materials are already set.
    pub fn set_hd_water(
        &mut self,
        types: &[hd::layout::GpuWaterType],
        layers: hd::layout::WaterLayers,
    ) {
        self.hd_water_types = hd::layout::water_types_buffer(&self.device, types);
        self.hd_water_layers = layers;
        if let Some(binding) = &self.hd_binding {
            let bind_group = self.hd_scene_bind_group(&binding.table, &binding.textures);
            if let Some(binding) = self.hd_binding.as_mut() {
                binding.bind_group = bind_group;
            }
        }
    }

    fn hd_scene_bind_group(
        &self,
        table: &hd::layout::MaterialTable,
        textures: &hd::materials::MaterialTextureArray,
    ) -> wgpu::BindGroup {
        hd::layout::scene_bind_group(
            &self.device,
            &self.hd_layout,
            table,
            textures,
            &self.hd_env_buffer,
            &self.hd_water_types,
        )
    }

    /// The bound HD material texture array, to fill vanilla texture layers.
    pub fn hd_texture_array(&self) -> Option<&hd::materials::MaterialTextureArray> {
        self.hd_binding.as_ref().map(|binding| &binding.textures)
    }

    /// Lighting of the HD scene shader.
    pub fn set_hd_lighting(&mut self, lighting: HdLighting) {
        if self.theme_vertex_active && self.hd_lighting.elapsed_seconds != lighting.elapsed_seconds
        {
            self.hd_shadow_drawn.set(None);
        }
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
        // The HD profile clears to the area's fog (sky) colour.
        let clear = if hdr {
            self.hd_lighting
                .lighting
                .fog_color
                .map(|c| f64::from(osrs_hd::environment::linear_to_srgb(c)))
        } else {
            params.clear_color
        };
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
                            r: clear[0],
                            g: clear[1],
                            b: clear[2],
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
            if hdr && let Some(binding) = &self.hd_binding {
                pass.set_pipeline(&self.hd_resources().sky);
                pass.set_bind_group(2, &binding.bind_group, &[]);
                pass.draw(0..3, 0..1);
            }
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
                            pass.set_bind_group(3, hd.shadow_group, &[]);
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
                        // Cutout vertices sit at the end of an HD range: drawn below.
                        let solid = if hd_buffer.is_some() {
                            range.count - range.cutout_count.min(range.count)
                        } else {
                            range.count
                        };
                        if solid > 0 {
                            self.counters.draw(solid);
                            pass.draw(range.start..range.start + solid, 0..1);
                        }
                    }
                }
            }

            // Cutout materials: opaque with alpha-to-coverage.
            if let Some(hd) = inputs.hd
                && let Some(cutout) = &hd.pipelines.cutout
            {
                let mut bound = false;
                for zone in inputs.visible.iter().copied() {
                    let (Some(buffer), Some(hd_buffer)) = (&zone.opaque, &zone.opaque_hd) else {
                        continue;
                    };
                    if zone
                        .opaque_ranges
                        .iter()
                        .all(|range| range.cutout_count == 0 || range.min_plane > params.view_plane)
                    {
                        continue;
                    }
                    if !bound {
                        bound = true;
                        pass.set_pipeline(cutout);
                        pass.set_bind_group(2, hd.bind_group, &[]);
                        pass.set_bind_group(3, hd.shadow_group, &[]);
                    }
                    pass.set_bind_group(1, &zone.bind_group, &[]);
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.set_vertex_buffer(1, hd_buffer.slice(..));
                    for range in &zone.opaque_ranges {
                        if range.min_plane <= params.view_plane && range.cutout_count > 0 {
                            let end = range.start + range.count;
                            self.counters.draw(range.cutout_count.min(range.count));
                            pass.draw(end - range.cutout_count.min(range.count)..end, 0..1);
                        }
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
                                pass.set_bind_group(3, hd.shadow_group, &[]);
                                pass.set_bind_group(3, hd.shadow_group, &[]);
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
                    self.counters.draw(draw.count);
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
        for timer in self.auxiliary_timers.borrow_mut().iter_mut().flatten() {
            timer.finish(&self.device);
        }
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
            self.counters.upload(indices.len() * 4);
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
        let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        self.encode_color_frame(&mut encoder, &color_view, params, width, height);

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

/// Whether a zone's bounds (padded for overhanging models) project onto the shadow map.
fn zone_overlaps_shadow(zone: &GpuZone, origin: (i32, i32), camera: &ShadowCamera) -> bool {
    let base_x = (zone.world_origin.0 - origin.0) as f32;
    let base_z = (zone.world_origin.1 - origin.1) as f32;
    let pad = 512.0;
    let (mut min, mut max) = ([f32::MAX; 2], [f32::MIN; 2]);
    for dx in [-pad, ZONE_LOCAL_EXTENT + pad] {
        for dz in [-pad, ZONE_LOCAL_EXTENT + pad] {
            for y in [zone.y_range.0 - 256.0, zone.y_range.1 + 256.0] {
                let [x, projected_y, _] = camera.project([base_x + dx, y, base_z + dz]);
                min = [min[0].min(x), min[1].min(projected_y)];
                max = [max[0].max(x), max[1].max(projected_y)];
            }
        }
    }
    max[0] >= -1.0 && min[0] <= 1.0 && max[1] >= -1.0 && min[1] <= 1.0
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
