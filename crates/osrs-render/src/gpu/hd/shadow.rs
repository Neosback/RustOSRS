//! Shadow-map resources of the HD profile (phase 4): the depth textures, the bind group the scene
//! shader samples them through (group 3) and the depth-only pipelines that fill them.
//!
//! The pass draws the zones' HD vertex streams from the light's point of view
//! (`shadow_cast.wgsl`); the scene shader looks them up in `shadow_receive.wgsl`. The light's
//! camera comes from [`osrs_hd::shadows::ShadowCamera`].

use super::layout::MATERIAL_WGSL;
use crate::{HdVertex, PackedVertex};
use osrs_hd::shadows::ShadowSettings;

/// Depth format of the shadow maps: 32-bit float, so the packed opacity + depth value of the
/// transparent shadows (24 bits) is stored exactly.
pub const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Vertex attributes of the Reference stream (slot 0) the HD shaders read.
pub(in crate::gpu) const SCENE_ATTRIBUTES: [wgpu::VertexAttribute; 3] = [
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

/// Vertex attributes of the HD stream (slot 1): octahedral normal, uv, packed corner colours,
/// materials and water data.
pub(in crate::gpu) const HD_ATTRIBUTES: [wgpu::VertexAttribute; 5] = [
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

/// The vertex buffer layouts of a draw with both streams.
pub(in crate::gpu) fn hd_vertex_buffers() -> [wgpu::VertexBufferLayout<'static>; 2] {
    [
        wgpu::VertexBufferLayout {
            array_stride: PackedVertex::SIZE as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &SCENE_ATTRIBUTES,
        },
        wgpu::VertexBufferLayout {
            array_stride: HdVertex::SIZE as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &HD_ATTRIBUTES,
        },
    ]
}

/// WGSL of the shared declarations plus a stage file.
pub(in crate::gpu) fn shader_source(stage: &str) -> String {
    format!(
        // The branches that skip derivative uses depend on per-face (flat) data: every pixel of a
        // quad takes the same one.
        "diagnostic(off, derivative_uniformity);\n{}\n{}\n{}",
        MATERIAL_WGSL,
        include_str!("common.wgsl"),
        stage
    )
}

/// The shadow-map textures the scene shader samples (group 3).
pub struct ShadowMaps {
    pub size: u32,
    pub terrain_size: u32,
    texture: wgpu::Texture,
    terrain_texture: wgpu::Texture,
    view: wgpu::TextureView,
    terrain_view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

/// Layout of group 3: the main and the terrain shadow map as depth textures.
pub fn maps_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let entry = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Depth,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("hd-shadow-maps"),
        entries: &[entry(0), entry(1)],
    })
}

impl ShadowMaps {
    /// Maps of `size` texels (the terrain map is half that). `size == 1` makes the placeholder
    /// bound while shadows are off.
    pub fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, size: u32) -> Self {
        let make = |label: &str, size: u32| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: SHADOW_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        };
        let terrain_size = (size / 2).max(1);
        let texture = make("hd-shadow-map", size);
        let terrain_texture = make("hd-terrain-shadow-map", terrain_size);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let terrain_view = terrain_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hd-shadow-maps"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&terrain_view),
                },
            ],
        });
        Self {
            size,
            terrain_size,
            texture,
            terrain_texture,
            view,
            terrain_view,
            bind_group,
        }
    }

    pub const fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub const fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub const fn terrain_view(&self) -> &wgpu::TextureView {
        &self.terrain_view
    }

    /// Bytes of GPU memory both maps take.
    pub fn bytes(&self) -> u64 {
        let texels = u64::from(self.texture.width()) * u64::from(self.texture.height())
            + u64::from(self.terrain_texture.width()) * u64::from(self.terrain_texture.height());
        texels * 4
    }
}

/// Edge of the shadow maps for `settings`, capped to what the device allows.
pub fn map_size(settings: &ShadowSettings, max_texture_dimension: u32) -> u32 {
    settings
        .resolution
        .texels()
        .min(max_texture_dimension)
        .max(1)
}

/// The three depth-only pipelines: the main map with packed transparency, the main map with plain
/// depth, and the terrain map.
pub struct ShadowPipelines {
    pub packed: wgpu::RenderPipeline,
    pub plain: wgpu::RenderPipeline,
    pub terrain: wgpu::RenderPipeline,
}

impl ShadowPipelines {
    /// `scene_layout` is the HD scene bind group layout (materials, textures, environment).
    pub fn new(
        device: &wgpu::Device,
        zone_layout: &wgpu::BindGroupLayout,
        scene_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hd/shadow_cast.wgsl"),
            source: wgpu::ShaderSource::Wgsl(
                shader_source(include_str!("shadow_cast.wgsl")).into(),
            ),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hd-shadow"),
            bind_group_layouts: &[None, Some(zone_layout), Some(scene_layout)],
            immediate_size: 0,
        });
        let buffers = hd_vertex_buffers().map(Some);
        let make = |label: &str, terrain: bool, fragment: Option<&str>| {
            let constants = [("TERRAIN_ONLY", f64::from(u8::from(terrain)))];
            let compilation_options = wgpu::PipelineCompilationOptions {
                constants: &constants,
                ..Default::default()
            };
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_shadow"),
                    compilation_options: compilation_options.clone(),
                    buffers: &buffers,
                },
                fragment: fragment.map(|entry_point| wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry_point),
                    compilation_options: compilation_options.clone(),
                    targets: &[],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: SHADOW_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        Self {
            packed: make("hd-shadow-packed", false, Some("fs_packed")),
            plain: make("hd-shadow-plain", false, Some("fs_plain")),
            terrain: make("hd-shadow-terrain", true, None),
        }
    }
}
