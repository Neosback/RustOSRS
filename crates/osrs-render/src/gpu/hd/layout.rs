//! GPU buffer layouts for the HD profile.
//!
//! Adapted from the 117 HD plugin. Copyright (c) 2018, Adam <Adam@sigterm.info>; (c) 2021, 117 and Hooder (BSD 2-Clause, see THIRD_PARTY.md).
//!
//! Every struct is `repr(C)` with explicit padding so its size and field offsets equal the WGSL
//! struct it mirrors (`materials.wgsl`); `tests/hd_phase0.rs` checks that on a real device.

use bytemuck::{Pod, Zeroable};
use osrs_hd::Material;
use wgpu::util::DeviceExt;

/// WGSL declaration of [`GpuMaterial`], to be prepended to HD shaders that index the table.
pub const MATERIAL_WGSL: &str = include_str!("materials.wgsl");

/// `Material.MAX_MATERIAL_INDEX`: the packed per-vertex material index is 12 bits wide.
pub const MAX_MATERIAL_INDEX: usize = (1 << 12) - 1;

pub const MATERIAL_FLAG_HAS_TRANSPARENCY: u32 = 1;
pub const MATERIAL_FLAG_UNLIT: u32 = 1 << 1;
pub const MATERIAL_FLAG_OVERRIDE_BASE_COLOR: u32 = 1 << 2;

/// One entry of the material table (`UBOMaterials.MaterialStruct`), uploaded as a storage buffer
/// array. 96 bytes: the WGSL `vec3` at offset 80 forces 16-byte struct alignment.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct GpuMaterial {
    pub color_map: i32,
    pub normal_map: i32,
    pub displacement_map: i32,
    pub roughness_map: i32,
    pub ambient_occlusion_map: i32,
    pub flow_map: i32,
    pub shadow_alpha_map: i32,
    pub flags: u32,
    pub subsurface: f32,
    pub brightness: f32,
    pub displacement_scale: f32,
    pub specular_strength: f32,
    pub specular_gloss: f32,
    pub flow_map_strength: f32,
    pub flow_map_duration: [f32; 2],
    pub scroll_duration: [f32; 2],
    pub pad0: [f32; 2],
    pub texture_scale: [f32; 3],
    pub pad1: f32,
}

const _: () = assert!(std::mem::size_of::<GpuMaterial>() == 96);

impl GpuMaterial {
    /// `Material.NONE`: no textures, neutral shading.
    pub const NONE: Self = Self {
        color_map: -1,
        normal_map: -1,
        displacement_map: -1,
        roughness_map: -1,
        ambient_occlusion_map: -1,
        flow_map: -1,
        shadow_alpha_map: -1,
        flags: 0,
        subsurface: 0.0,
        brightness: 1.0,
        displacement_scale: 0.1,
        specular_strength: 0.0,
        specular_gloss: 0.0,
        flow_map_strength: 0.0,
        flow_map_duration: [0.0; 2],
        scroll_duration: [0.0; 2],
        pad0: [0.0; 2],
        texture_scale: [1.0; 3],
        pad1: 0.0,
    };

    /// Pack a `materials.json` entry (`Material.fillMaterialStruct`).
    ///
    /// Phase 0 reads only the entry's own fields: parent default inheritance, replacements and the
    /// vanilla scroll animation belong to the phase-2 material resolution. `layer_of` maps a
    /// material name to its texture-array layer (`-1` when it has none).
    pub fn from_hd(material: &Material, layer_of: impl Fn(&str) -> i32) -> Self {
        let named = |name: &Option<String>| name.as_deref().map_or(-1, &layer_of);
        let mut flags = 0;
        if material.has_transparency == Some(true) {
            flags |= MATERIAL_FLAG_HAS_TRANSPARENCY;
        }
        if material.unlit == Some(true) {
            flags |= MATERIAL_FLAG_UNLIT;
        }
        if material.override_base_color == Some(true) {
            flags |= MATERIAL_FLAG_OVERRIDE_BASE_COLOR;
        }
        let pair = |values: &Option<Vec<f32>>| match values.as_deref() {
            Some(&[x, y, ..]) => [x, y],
            _ => [0.0; 2],
        };
        let scale = match material.texture_scale.as_deref() {
            Some(&[x, y, z, ..]) => [x, y, z],
            Some(&[x, y]) => [x, y, 1.0],
            _ => [1.0; 3],
        };
        Self {
            color_map: layer_of(&material.name),
            normal_map: named(&material.normal_map),
            displacement_map: named(&material.displacement_map),
            roughness_map: named(&material.roughness_map),
            ambient_occlusion_map: named(&material.ambient_occlusion_map),
            flow_map: named(&material.flow_map),
            shadow_alpha_map: named(&material.shadow_alpha_map),
            flags,
            subsurface: material.subsurface.unwrap_or(0.0),
            brightness: material.brightness.map_or(1.0, srgb_to_linear),
            displacement_scale: material.displacement_scale.unwrap_or(0.1),
            specular_strength: material.specular_strength.unwrap_or(0.0),
            specular_gloss: material.specular_gloss.unwrap_or(0.0),
            flow_map_strength: material.flow_map_strength.unwrap_or(0.0),
            flow_map_duration: pair(&material.flow_map_duration),
            scroll_duration: pair(&material.scroll_speed),
            pad0: [0.0; 2],
            // The shader multiplies UVs by the reciprocal (`divide(vec(1), textureScale)`).
            texture_scale: scale.map(|value| if value == 0.0 { 1.0 } else { 1.0 / value }),
            pad1: 0.0,
        }
    }
}

/// `ColorUtils.srgbToLinear` (the `LinearAdapter` applied to material brightness).
pub fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// Uniform of the HD resolve pass (the start of the plugin's `UBOGlobal`): 16 bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct HdFrameUniform {
    pub exposure: f32,
    pub vignette: f32,
    pub width: f32,
    pub height: f32,
}

const _: () = assert!(std::mem::size_of::<HdFrameUniform>() == 16);

/// Storage buffer holding the material table.
pub struct MaterialTable {
    buffer: wgpu::Buffer,
    len: usize,
}

impl MaterialTable {
    /// Upload `materials` (at least one entry: index 0 is `NONE`).
    pub fn new(device: &wgpu::Device, materials: &[GpuMaterial]) -> Self {
        let none = [GpuMaterial::NONE];
        let materials = if materials.is_empty() {
            &none
        } else {
            materials
        };
        Self {
            buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("hd-materials"),
                contents: bytemuck::cast_slice(materials),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            }),
            len: materials.len(),
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }
}

/// Bind group layout of the HD material resources (group 2 of the HD scene shader): the material
/// table as a read-only storage buffer, the texture array and its sampler. Two sampled-texture
/// slots in all, far below Metal's 16 per stage.
pub fn material_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("hd-materials"),
        entries: &material_layout_entries(),
    })
}

fn material_layout_entries() -> Vec<wgpu::BindGroupLayoutEntry> {
    vec![
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
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
    ]
}

/// Material table, texture array and sampler bound together.
pub fn material_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    table: &MaterialTable,
    textures: &super::materials::MaterialTextureArray,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("hd-materials"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: table.buffer().as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(textures.view()),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(textures.sampler()),
            },
        ],
    })
}

/// Lighting environment of the HD scene shader (`Environment` in `scene.wgsl`), 96 bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct HdEnvironmentUniform {
    /// Camera position (render-local), `w` unused.
    pub camera: [f32; 4],
    /// Unit vector towards the light.
    pub light_dir: [f32; 4],
    /// Linear colour, strength in `w`.
    pub light_color: [f32; 4],
    pub ambient_color: [f32; 4],
    pub water_color: [f32; 4],
    /// `x`: normal mapping enabled.
    pub flags: [u32; 4],
}

const _: () = assert!(std::mem::size_of::<HdEnvironmentUniform>() == 96);

/// Bind group layout of the HD scene shader's group 2: the material table, texture array,
/// sampler and the lighting environment.
pub fn scene_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let mut entries = material_layout_entries();
    entries.push(wgpu::BindGroupLayoutEntry {
        binding: 3,
        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    });
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("hd-scene"),
        entries: &entries,
    })
}

/// Material table, texture array, sampler and environment bound together.
pub fn scene_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    table: &MaterialTable,
    textures: &super::materials::MaterialTextureArray,
    environment: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("hd-scene"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: table.buffer().as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(textures.view()),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(textures.sampler()),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: environment.as_entire_binding(),
            },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::offset_of;

    #[test]
    fn material_offsets_match_the_wgsl_struct() {
        assert_eq!(offset_of!(GpuMaterial, flags), 28);
        assert_eq!(offset_of!(GpuMaterial, subsurface), 32);
        assert_eq!(offset_of!(GpuMaterial, flow_map_strength), 52);
        assert_eq!(offset_of!(GpuMaterial, flow_map_duration), 56);
        assert_eq!(offset_of!(GpuMaterial, scroll_duration), 64);
        assert_eq!(offset_of!(GpuMaterial, texture_scale), 80);
    }

    #[test]
    fn texture_scale_is_stored_as_a_reciprocal() {
        let material = Material {
            name: "X".into(),
            texture_scale: Some(vec![2.0, 4.0, 1.0]),
            ..Material::default()
        };
        let packed = GpuMaterial::from_hd(&material, |_| 7);
        assert_eq!(packed.texture_scale, [0.5, 0.25, 1.0]);
        assert_eq!(packed.color_map, 7);
        assert_eq!(packed.normal_map, -1);
    }

    #[test]
    fn flags_pack_like_the_plugin() {
        let material = Material {
            has_transparency: Some(true),
            unlit: Some(true),
            ..Material::default()
        };
        assert_eq!(
            GpuMaterial::from_hd(&material, |_| -1).flags,
            MATERIAL_FLAG_HAS_TRANSPARENCY | MATERIAL_FLAG_UNLIT
        );
    }
}
