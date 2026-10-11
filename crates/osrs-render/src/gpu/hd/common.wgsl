// Shared declarations of the HD shaders (scene, shadow): the lighting environment, the material
// bindings of group 2 and the colour helpers. Prepended after `materials.wgsl`.
//
// Ported from the 117 HD plugin (`uniforms/global.glsl`, `utils/color_utils.glsl`). Copyright (c)
// 2018, Adam <Adam@sigterm.info>; (c) 2021, 117 <https://twitter.com/117scape>; (c) 2022, Hooder
// <ahooder@protonmail.com>. BSD 2-Clause (see THIRD_PARTY.md).

// Lighting environment (`UBOGlobal`): the area environment's light, fog and water fields, and the
// shadow camera.
struct Environment {
    // xyz = camera position in render-local units, w = animation clock in seconds.
    camera: vec4<f32>,
    // xyz = unit vector from the scene towards the light, w = the light casts shadows.
    light_dir: vec4<f32>,
    // rgb = linear colour, w = strength.
    light_color: vec4<f32>,
    ambient_color: vec4<f32>,
    // rgb = underglow colour, w = strength.
    underglow: vec4<f32>,
    // rgb = linear fog (sky) colour, w = fog depth (0 = no fog).
    fog_color: vec4<f32>,
    // x = draw distance in tiles, y/z = ground fog start/end, w = ground fog opacity.
    fog_params: vec4<f32>,
    // rgb = linear water colour.
    water_color: vec4<f32>,
    // x = lightning brightness, y = underwater caustics on, z = output gamma.
    misc: vec4<f32>,
    // x = normal mapping enabled, y = debug view, z = underwater environment.
    flags: vec4<u32>,
    // Render-local position to the light's clip space: x, y in -1..1 (y up), z in 0..1 (0 near the
    // light).
    light_proj: mat4x4<f32>,
    // xyz = shadow map right axis, w = map edge in world units.
    shadow_right: vec4<f32>,
    // xyz = shadow map up axis, w = world size of a texel.
    shadow_up: vec4<f32>,
    // xyz = axis towards the light, w = depth range in world units.
    shadow_axis: vec4<f32>,
    // x = bias of one texel in normalised depth, y = shadow draw distance (world units), z = map
    // size in texels, w = terrain map size in texels.
    shadow_params: vec4<f32>,
    // x = shadows on, y = filter kind (0 PCF, 1 dither, 2 average, 3 PCSS), z = kernel size,
    // w = bit 0 transparency, bit 1 detailed (texture alpha), bit 2 terrain shadows.
    shadow_flags: vec4<u32>,
    // Water surface colours by brightness (sRGB).
    water_light: vec4<f32>,
    water_mid: vec4<f32>,
    water_dark: vec4<f32>,
    // rgb = underwater caustics colour, w = strength.
    caustics: vec4<f32>,
    // xy = world position of render-local (0, 0).
    scene_base: vec4<f32>,
    // Texture layers of the water flow map, foam, caustics map and underwater flow map.
    water_layers: vec4<i32>,
};

// One water type (`UBOWaterTypes`). Colours are sRGB, like the plugin's struct.
struct WaterType {
    is_flat: u32,
    specular_strength: f32,
    specular_gloss: f32,
    normal_strength: f32,
    base_opacity: f32,
    has_foam: u32,
    duration: f32,
    fresnel_amount: f32,
    surface_color: vec4<f32>,
    foam_color: vec4<f32>,
    depth_color: vec4<f32>,
    normal_map: i32,
};

@group(2) @binding(0) var<storage, read> materials: array<Material>;
@group(2) @binding(1) var material_textures: texture_2d_array<f32>;
@group(2) @binding(2) var material_sampler: sampler;
@group(2) @binding(3) var<uniform> env: Environment;
@group(2) @binding(4) var<storage, read> water_types: array<WaterType>;

// Per-face flags of the HD vertex stream (`HdVertex::hsl_flags.w`).
const FLAG_UPWARDS_NORMALS: u32 = 1u;
const FLAG_FLAT_NORMALS: u32 = 2u;
const FLAG_NO_SHADOW: u32 = 4u;
const FLAG_TERRAIN: u32 = 8u;
const FLAG_VANILLA_UVS: u32 = 16u;
const FLAG_NO_SHADOW_CAST: u32 = 32u;
const FLAG_SHADOW_NORMAL_BIAS: u32 = 64u;

const MATERIAL_INDEX_MASK: u32 = 0x0FFFu;
const MATERIAL_OVERLAY: u32 = 0x1000u;

// World-anchored texture coordinates (`worldUvs`): one repeat every `128 * scale` units.
fn world_uvs(position: vec3<f32>, scale: f32) -> vec2<f32> {
    return -(env.scene_base.xy + position.xz) / (128.0 * scale);
}

// A value in `0..1` cycling every `duration` seconds (`animationFrame`).
fn water_animation(duration: f32) -> f32 {
    if (duration == 0.0) {
        return 0.0;
    }
    return (env.camera.w % duration) / duration;
}

fn water_animation2(duration: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(water_animation(duration.x), water_animation(duration.y));
}

// ---- colour helpers (`utils/color_utils.glsl`) ----------------------------------------------

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(
        pow((c + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4)),
        c / 12.92,
        c < vec3<f32>(0.04045),
    );
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    return select(
        1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055),
        c * 12.92,
        c < vec3<f32>(0.0031308),
    );
}
