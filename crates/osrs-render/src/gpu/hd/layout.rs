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

/// HD resolve inputs, with a deterministic clock for theme effects.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct HdFrameUniform {
    pub exposure: f32,
    pub vignette: f32,
    pub width: f32,
    pub height: f32,
    pub time: f32,
    pub padding: [f32; 3],
}

const _: () = assert!(std::mem::size_of::<HdFrameUniform>() == 32);

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

/// One water type (`UBOWaterTypes.WaterTypeStruct`), uploaded as a storage buffer array; 96 bytes.
/// Index `0` is `WaterType.NONE`, then the entries of `water_types.json` in file order.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct GpuWaterType {
    pub is_flat: u32,
    pub specular_strength: f32,
    pub specular_gloss: f32,
    pub normal_strength: f32,
    pub base_opacity: f32,
    pub has_foam: u32,
    pub duration: f32,
    pub fresnel_amount: f32,
    /// sRGB, like the plugin's struct (it converts with `linearToSrgb`).
    pub surface_color: [f32; 4],
    pub foam_color: [f32; 4],
    pub depth_color: [f32; 4],
    /// Texture layer of the normal map.
    pub normal_map: i32,
    pub pad: [i32; 3],
}

const _: () = assert!(std::mem::size_of::<GpuWaterType>() == 96);

impl GpuWaterType {
    /// `WaterType` with its defaults (the plugin's field initialisers).
    pub fn from_hd(water: &osrs_hd::WaterType, layer_of: impl Fn(&str) -> i32) -> Self {
        use osrs_hd::environment::{color_to_linear, linear_to_srgb3, srgb_to_linear3};
        let colour = |value: &Option<osrs_hd::Color>, default: [f32; 3]| {
            let linear = value.as_ref().and_then(color_to_linear).unwrap_or(default);
            let srgb = linear_to_srgb3(linear);
            [srgb[0], srgb[1], srgb[2], 0.0]
        };
        let fallback_normal = layer_of("WATER_NORMAL_MAP_1");
        Self {
            is_flat: u32::from(water.flat == Some(true)),
            specular_strength: water.specular_strength.unwrap_or(0.5),
            specular_gloss: water.specular_gloss.unwrap_or(500.0),
            normal_strength: water.normal_strength.unwrap_or(0.09),
            base_opacity: water.base_opacity.unwrap_or(0.5),
            has_foam: u32::from(water.has_foam != Some(false)),
            duration: water.duration.unwrap_or(1.0),
            fresnel_amount: water.fresnel_amount.unwrap_or(1.0),
            surface_color: colour(&water.surface_color, [1.0; 3]),
            foam_color: colour(
                &water.foam_color,
                srgb_to_linear3([176.0 / 255.0, 164.0 / 255.0, 146.0 / 255.0]),
            ),
            depth_color: colour(
                &water.depth_color,
                srgb_to_linear3([0.0, 117.0 / 255.0, 142.0 / 255.0]),
            ),
            normal_map: water
                .normal_map
                .as_deref()
                .map_or(fallback_normal, layer_of),
            pad: [0; 3],
        }
    }

    /// `WaterType.NONE`: the defaults.
    pub fn none(layer_of: impl Fn(&str) -> i32) -> Self {
        Self::from_hd(&osrs_hd::WaterType::default(), layer_of)
    }
}

/// Storage buffer holding the water type table (at least one entry).
pub fn water_types_buffer(device: &wgpu::Device, types: &[GpuWaterType]) -> wgpu::Buffer {
    let none = [GpuWaterType::none(|_| -1)];
    let types = if types.is_empty() { &none } else { types };
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("hd-water-types"),
        contents: bytemuck::cast_slice(types),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    })
}

/// Texture layers of the special water materials (`MAT_WATER_FLOW_MAP`, `MAT_WATER_FOAM`,
/// `MAT_CAUSTICS_MAP`, `UNDERWATER_FLOW_MAP`), `-1` when missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaterLayers {
    pub flow_map: i32,
    pub foam: i32,
    pub caustics: i32,
    pub underwater_flow_map: i32,
}

impl Default for WaterLayers {
    fn default() -> Self {
        Self {
            flow_map: -1,
            foam: -1,
            caustics: -1,
            underwater_flow_map: -1,
        }
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

/// Lighting environment of the HD scene shader (`Environment` in `scene.wgsl`): the lighting,
/// fog and water fields of the plugin's `UBOGlobal` the water colours and the shadow camera and sky basis, 448 bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct HdEnvironmentUniform {
    /// Camera position (render-local), animation clock in seconds in `w`.
    pub camera: [f32; 4],
    /// Unit vector towards the light, `w` = the light casts shadows.
    pub light_dir: [f32; 4],
    /// Linear colour, strength in `w`.
    pub light_color: [f32; 4],
    pub ambient_color: [f32; 4],
    /// Underglow colour and strength.
    pub underglow: [f32; 4],
    /// Linear fog (sky) colour, fog depth in `w` (`0` = no fog).
    pub fog_color: [f32; 4],
    /// Draw distance in tiles, ground fog start, end and opacity.
    pub fog_params: [f32; 4],
    pub water_color: [f32; 4],
    /// Lightning brightness; the rest is reserved.
    pub misc: [f32; 4],
    /// `x`: normal mapping enabled, `y`: debug view, `z`: underwater environment,
    /// `w`: parallax occlusion mapping enabled.
    pub flags: [u32; 4],
    /// Render-local position to the light's clip space (column-major).
    pub light_proj: [[f32; 4]; 4],
    /// Shadow map right axis and the map edge in world units.
    pub shadow_right: [f32; 4],
    /// Shadow map up axis and the world size of a texel.
    pub shadow_up: [f32; 4],
    /// Axis towards the light and the depth range in world units.
    pub shadow_axis: [f32; 4],
    /// Texel depth bias, shadow draw distance, map size and terrain map size.
    pub shadow_params: [f32; 4],
    /// `x`: shadows on, `y`: filter kind, `z`: kernel size, `w`: bit 0 transparency, bit 1
    /// detailed, bit 2 terrain shadows.
    pub shadow_flags: [u32; 4],
    /// Water surface colours by brightness (sRGB), `w` unused.
    pub water_light: [f32; 4],
    pub water_mid: [f32; 4],
    pub water_dark: [f32; 4],
    /// Underwater caustics colour and strength.
    pub caustics: [f32; 4],
    /// World position of render-local `(0, 0)` (`x`, `z`) for world-anchored water UVs.
    pub scene_base: [f32; 4],
    /// Texture layers of the water flow map, foam, caustics map and underwater flow map.
    pub water_layers: [i32; 4],
    pub sky_right: [f32; 4],
    pub sky_down: [f32; 4],
    pub sky_forward: [f32; 4],
    /// xyz = sun direction in world/sky coordinates, w = sun enabled / disk scale (> 0 when visible).
    pub sun_dir: [f32; 4],
    /// rgb = sun linear color, w = sun intensity scale.
    pub sun_color: [f32; 4],
    /// xyz = moon direction in world/sky coordinates, w = moon phase in `0.0..=1.0`.
    pub moon_dir: [f32; 4],
    /// rgb = moon linear disk color, w = moon visibility / intensity scale.
    pub moon_color: [f32; 4],
    /// rgb = sky zenith color, w = horizon gradient width.
    pub sky_zenith: [f32; 4],
}

const _: () = assert!(std::mem::size_of::<HdEnvironmentUniform>() == 160 + 64 + 80 + 96 + 48 + 80);

/// Point light uploaded to GPU storage buffer (`PointLight` in `scene.wgsl`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable, PartialEq)]
pub struct GpuPointLight {
    /// xyz = render-local position, w = radiusSquared
    pub position: [f32; 4],
    /// rgb = linear color * strength, w = unused
    pub color: [f32; 4],
}

/// Create a GPU storage buffer holding point lights.
pub fn point_lights_buffer(device: &wgpu::Device, lights: &[GpuPointLight]) -> wgpu::Buffer {
    let slice = if lights.is_empty() {
        &[GpuPointLight::default()]
    } else {
        lights
    };
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("hd-point-lights"),
        contents: bytemuck::cast_slice(slice),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    })
}

/// Bind group layout of the HD scene shader's group 2: the material table, texture array,
/// sampler, the lighting environment, the water types and point lights.
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
    entries.push(wgpu::BindGroupLayoutEntry {
        binding: 4,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    });
    entries.push(wgpu::BindGroupLayoutEntry {
        binding: 5,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
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

/// Material table, texture array, sampler, environment, water types and point lights bound together.
pub fn scene_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    table: &MaterialTable,
    textures: &super::materials::MaterialTextureArray,
    environment: &wgpu::Buffer,
    water_types: &wgpu::Buffer,
    point_lights: &wgpu::Buffer,
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
            wgpu::BindGroupEntry {
                binding: 4,
                resource: water_types.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: point_lights.as_entire_binding(),
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
    fn water_types_use_the_plugins_defaults_and_srgb_colours() {
        let layer = |name: &str| match name {
            "WATER_NORMAL_MAP_1" => 7,
            "WATER_NORMAL_MAP_2" => 8,
            _ => -1,
        };
        let plain = GpuWaterType::from_hd(&osrs_hd::WaterType::default(), layer);
        assert_eq!(plain.specular_strength, 0.5);
        assert_eq!(plain.specular_gloss, 500.0);
        assert_eq!(plain.normal_strength, 0.09);
        assert_eq!(plain.base_opacity, 0.5);
        assert_eq!(plain.fresnel_amount, 1.0);
        assert_eq!(plain.has_foam, 1);
        assert_eq!(plain.is_flat, 0);
        // Without its own map a water type uses the first generic normal map.
        assert_eq!(plain.normal_map, 7);
        // The colours are stored in sRGB: the default depth colour is rgb(0, 117, 142).
        assert!(
            (plain.depth_color[1] - 117.0 / 255.0).abs() < 1e-3,
            "{plain:?}"
        );
        assert!((plain.foam_color[0] - 176.0 / 255.0).abs() < 1e-3);
        assert!(
            plain.surface_color[..3]
                .iter()
                .all(|c| (c - 1.0).abs() < 1e-5)
        );

        let water = osrs_hd::WaterType {
            name: "X".into(),
            flat: Some(true),
            has_foam: Some(false),
            normal_map: Some("WATER_NORMAL_MAP_2".into()),
            surface_color: Some(osrs_hd::Color::Hex("#69809C".into())),
            ..osrs_hd::WaterType::default()
        };
        let packed = GpuWaterType::from_hd(&water, layer);
        assert_eq!(
            (packed.is_flat, packed.has_foam, packed.normal_map),
            (1, 0, 8)
        );
        assert!((packed.surface_color[0] - 0x69 as f32 / 255.0).abs() < 1e-3);
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
