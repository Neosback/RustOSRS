// HD-profile scene shader: a WGSL port of the 117 HD plugin's `scene_vert.glsl` / `scene_frag.glsl`
// (zone renderer), phase 2 subset: per-corner materials with ground blending, diffuse + normal map +
// specular, vertex-colour and underlay/overlay blending, tangent frame from derivatives, underwater
// tint. Shadows (phase 4), water surfaces (5), point lights (6) and tone mapping (8) are not ported
// yet; lighting comes from a placeholder environment until phase 3.
//
// Ported from the 117 HD plugin (https://github.com/117HD/RLHD). Copyright (c) 2018, Adam
// <Adam@sigterm.info>; (c) 2021, 117 <https://twitter.com/117scape>; (c) 2022, Hooder
// <ahooder@protonmail.com>. All rights reserved. BSD 2-Clause: redistribution and use in source and
// binary forms, with or without modification, are permitted provided that the copyright notice, this
// list of conditions and the following disclaimer are retained (see THIRD_PARTY.md for the full
// text). THIS SOFTWARE IS PROVIDED "AS IS" WITHOUT WARRANTY OF ANY KIND.
//
// Depth is reverse-Z with no far plane, exactly like the Reference shader: clip.z = 2n and
// clip.w = camera distance; the authored face bias is added to clip.z before the divide.

struct Globals {
    world_proj: mat4x4<f32>,
    brightness: f32,
    smooth_banding: f32,
    tick: u32,
    texture_light_mode: f32,
    fog_color: vec4<f32>,
    camera_x: f32,
    camera_z: f32,
    draw_distance: f32,
    fog_depth: f32,
    use_fog: u32,
    colorblind_mode: u32,
    colorblind_intensity: f32,
    _pad: f32,
};

struct Zone {
    base: vec4<f32>,
};

// Lighting environment (`UBOGlobal`): placeholder values until phase 3 ports the environments.
struct Environment {
    // xyz = camera position in render-local units.
    camera: vec4<f32>,
    // xyz = unit vector from the scene towards the light.
    light_dir: vec4<f32>,
    // rgb = linear colour, w = strength.
    light_color: vec4<f32>,
    ambient_color: vec4<f32>,
    // rgb = linear water colour.
    water_color: vec4<f32>,
    // x = normal mapping enabled, y = debug view.
    flags: vec4<u32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var<uniform> zone: Zone;
@group(2) @binding(0) var<storage, read> materials: array<Material>;
@group(2) @binding(1) var material_textures: texture_2d_array<f32>;
@group(2) @binding(2) var material_sampler: sampler;
@group(2) @binding(3) var<uniform> env: Environment;

const FLAG_UPWARDS_NORMALS: u32 = 1u;
const FLAG_FLAT_NORMALS: u32 = 2u;
const FLAG_NO_SHADOW: u32 = 4u;
const FLAG_TERRAIN: u32 = 8u;
const FLAG_VANILLA_UVS: u32 = 16u;

const MATERIAL_INDEX_MASK: u32 = 0x0FFFu;
const MATERIAL_OVERLAY: u32 = 0x1000u;

struct VertexIn {
    @builtin(vertex_index) index: u32,
    @location(0) position: vec4<i32>,
    @location(1) abhsl: u32,
    @location(3) normal: vec2<f32>,
    @location(4) uv: vec2<f32>,
    @location(5) hsl_flags: vec4<u32>,
    @location(6) material_water: vec4<u32>,
    @location(7) depth: vec4<u32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) normal: vec3<f32>,
    // Barycentric weights of the triangle's corners.
    @location(3) blend: vec3<f32>,
    @location(4) @interpolate(flat) hsl: vec3<u32>,
    @location(5) @interpolate(flat) mats: vec3<u32>,
    @location(6) @interpolate(flat) depths: vec3<u32>,
    // x = flags, y = water type, z = plane, w = alpha byte.
    @location(7) @interpolate(flat) info: vec4<u32>,
};

fn oct_decode(e: vec2<f32>) -> vec3<f32> {
    var n = vec3<f32>(e.x, e.y, 1.0 - abs(e.x) - abs(e.y));
    if (n.z < 0.0) {
        let xy = (vec2<f32>(1.0) - abs(n.yx)) * select(vec2<f32>(-1.0), vec2<f32>(1.0), n.xy >= vec2<f32>(0.0));
        n = vec3<f32>(xy, n.z);
    }
    return normalize(n);
}

@vertex
fn vs_main(input: VertexIn) -> VertexOut {
    var out: VertexOut;
    let vert = vec4<f32>(vec3<f32>(input.position.xyz) + zone.base.xyz, 1.0);
    let bias = f32((input.abhsl >> 16u) & 0xffu);

    var screen = globals.world_proj * vert;
    screen.z = screen.z + bias / 128.0;
    out.clip = screen;

    out.position = vert.xyz;
    out.uv = input.uv;
    out.normal = oct_decode(input.normal);
    var blend = vec3<f32>(0.0);
    let corner = input.index % 3u;
    if (corner == 0u) {
        blend.x = 1.0;
    } else if (corner == 1u) {
        blend.y = 1.0;
    } else {
        blend.z = 1.0;
    }
    out.blend = blend;
    out.hsl = input.hsl_flags.xyz;
    out.mats = input.material_water.xyz;
    out.depths = input.depth.xyz;
    out.info = vec4<u32>(
        input.hsl_flags.w,
        input.material_water.w & 0xffu,
        (input.material_water.w >> 8u) & 3u,
        (input.abhsl >> 24u) & 0xffu,
    );
    return out;
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

fn hsl_to_srgb(hsl: vec3<f32>) -> vec3<f32> {
    let c = (1.0 - abs(2.0 * hsl.z - 1.0)) * hsl.y;
    let h = fract(hsl.x) * 6.0;
    let m = hsl.z - c / 2.0;
    let r = clamp(abs(h - 3.0) - 1.0, 0.0, 1.0);
    let g = clamp(2.0 - abs(h - 2.0), 0.0, 1.0);
    let b = clamp(2.0 - abs(h - 4.0), 0.0, 1.0);
    return vec3<f32>(r, g, b) * c + vec3<f32>(m);
}

// Jagex packed HSL (6-bit hue, 3-bit saturation, 7-bit lightness) to linear RGB.
fn packed_hsl_to_linear(packed: u32) -> vec3<f32> {
    let h = f32((packed >> 10u) & 63u) / 64.0 + 0.0078125;
    let s = f32((packed >> 7u) & 7u) / 8.0 + 0.0625;
    let l = f32(packed & 127u) / 128.0;
    return srgb_to_linear(hsl_to_srgb(vec3<f32>(h, s, l)));
}

// ---- fragment ---------------------------------------------------------------------------------

fn has_flag(material: Material, bit: u32) -> bool {
    return (material.flags & bit) != 0u;
}

fn sample_color(material: Material, uv: vec2<f32>, ddx: vec2<f32>, ddy: vec2<f32>) -> vec4<f32> {
    if (material.color_map < 0) {
        return vec4<f32>(1.0);
    }
    var c = textureSampleGrad(material_textures, material_sampler, uv, material.color_map, ddx, ddy);
    c = vec4<f32>(c.rgb * material.brightness, c.a);
    return c;
}

// `sampleNormalMap`: the tangent-space map rotated into world space.
fn sample_normal_map(
    material: Material,
    uv: vec2<f32>,
    ddx: vec2<f32>,
    ddy: vec2<f32>,
    tbn: mat3x3<f32>,
) -> vec3<f32> {
    if (material.normal_map < 0 || env.flags.x == 0u) {
        return tbn[2];
    }
    var n = textureSampleGrad(material_textures, material_sampler, uv, material.normal_map, ddx, ddy).xyz;
    // Undo the automatic sRGB decode: normal maps hold raw values.
    n = linear_to_srgb(n);
    n = vec3<f32>(n.xy * 2.0 - vec2<f32>(1.0), n.z);
    // Flip the normal when the UVs are flipped.
    n = vec3<f32>(n.xy * sign(material.texture_scale.xy), n.z);
    n.z = n.z * material.texture_scale.z;
    return tbn * n;
}

fn cotangent_frame(n: vec3<f32>, p: vec3<f32>, uv: vec2<f32>) -> mat3x3<f32> {
    // OpenGL's window y points up, WebGPU's down: negate the y derivatives to keep the plugin's
    // handedness.
    let dp1 = dpdx(p);
    let dp2 = -dpdy(p);
    let duv1 = dpdx(uv);
    let duv2 = -dpdy(uv);
    let a = cross(dp2, n);
    let b = cross(n, dp1);
    let t = a * duv1.x + b * duv2.x;
    let bt = a * duv1.y + b * duv2.y;
    let invmax = inverseSqrt(max(max(dot(t, t), dot(bt, bt)), 1e-12));
    return mat3x3<f32>(t * invmax, bt * invmax, n);
}

fn corner_uv(material: Material, base: vec2<f32>, time: f32) -> vec2<f32> {
    var uv = base + material.scroll_duration * time;
    uv = (uv - vec2<f32>(0.5)) * material.texture_scale.xy + vec2<f32>(0.5);
    return uv;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    let flags = input.info.x;
    let water_type = input.info.y;
    let is_terrain = (flags & FLAG_TERRAIN) != 0u;
    let blend = input.blend;

    let mat_index1 = input.mats.x & MATERIAL_INDEX_MASK;
    let mat_index2 = input.mats.y & MATERIAL_INDEX_MASK;
    let mat_index3 = input.mats.z & MATERIAL_INDEX_MASK;
    let material1 = materials[mat_index1];
    let material2 = materials[mat_index2];
    let material3 = materials[mat_index3];

    let water_depth = f32(input.depths.x) * blend.x + f32(input.depths.y) * blend.y + f32(input.depths.z) * blend.z;
    let is_underwater = is_terrain && water_depth > 0.0;

    let time = f32(globals.tick) * 0.02;
    var blended_uv = input.uv;
    if ((flags & FLAG_VANILLA_UVS) != 0u && has_flag(material1, 1u)) {
        blended_uv.x = clamp(blended_uv.x, 0.0, 0.984375);
    }
    let uv1 = corner_uv(material1, blended_uv, time);
    let uv2 = corner_uv(material2, blended_uv, time);
    let uv3 = corner_uv(material3, blended_uv, time);
    // Derivatives up front: the texture reads below sit in non-uniform control flow.
    let ddx1 = dpdx(uv1);
    let ddy1 = dpdy(uv1);
    let ddx2 = dpdx(uv2);
    let ddy2 = dpdy(uv2);
    let ddx3 = dpdx(uv3);
    let ddy3 = dpdy(uv3);

    let view_dir = normalize(env.camera.xyz - input.position);
    // WebGPU window y points down (OpenGL's points up), so the cross product is reversed.
    let flat_normal = normalize(cross(dpdx(input.position), -dpdy(input.position)));
    var n = normalize(input.normal);
    if ((flags & FLAG_FLAT_NORMALS) != 0u) {
        n = flat_normal;
    }
    let tbn = cotangent_frame(n, input.position, input.uv * -1.0);

    // Vertex colours and per-corner transparency.
    let alpha = 1.0 - f32(input.info.w) / 255.0;
    let base1 = packed_hsl_to_linear(input.hsl.x);
    let base2 = packed_hsl_to_linear(input.hsl.y);
    let base3 = packed_hsl_to_linear(input.hsl.z);

    let tex1 = sample_color(material1, uv1, ddx1, ddy1);
    let tex2 = sample_color(material2, uv2, ddx2, ddy2);
    let tex3 = sample_color(material3, uv3, ddx3, ddy3);

    let override1 = has_flag(material1, 4u);
    let override2 = has_flag(material2, 4u);
    let override3 = has_flag(material3, 4u);
    let col_a = select(vec4<f32>(tex1.rgb * base1, min(tex1.a, alpha)), tex1, override1);
    let col_b = select(vec4<f32>(tex2.rgb * base2, min(tex2.a, alpha)), tex2, override2);
    let col_c = select(vec4<f32>(tex3.rgb * base3, min(tex3.a, alpha)), tex3, override3);

    // Underlay / overlay blending of terrain corners.
    let overlay = vec3<f32>(
        select(0.0, 1.0, (input.mats.x & MATERIAL_OVERLAY) != 0u),
        select(0.0, 1.0, (input.mats.y & MATERIAL_OVERLAY) != 0u),
        select(0.0, 1.0, (input.mats.z & MATERIAL_OVERLAY) != 0u),
    );
    let underlay = vec3<f32>(1.0) - overlay;
    let overlay_count = overlay.x + overlay.y + overlay.z;
    let underlay_count = underlay.x + underlay.y + underlay.z;
    var underlay_blend = blend * underlay;
    var overlay_blend = blend * overlay;
    if (underlay_count == 0.0 || overlay_count == 0.0) {
        underlay_blend = blend;
        overlay_blend = blend;
    } else {
        underlay_blend = clamp(underlay_blend / max(underlay_blend.x + underlay_blend.y + underlay_blend.z, 1e-6), vec3<f32>(0.0), vec3<f32>(1.0));
        overlay_blend = clamp(overlay_blend / max(overlay_blend.x + overlay_blend.y + overlay_blend.z, 1e-6), vec3<f32>(0.0), vec3<f32>(1.0));
    }
    let underlay_color = col_a * underlay_blend.x + col_b * underlay_blend.y + col_c * underlay_blend.z;
    let overlay_color = col_a * overlay_blend.x + col_b * overlay_blend.y + col_c * overlay_blend.z;
    var overlay_mix = 0.0;
    if (overlay_count > 0.0 && underlay_count > 0.0) {
        var primary = underlay;
        var invert = true;
        if (overlay_count == 1.0) {
            primary = overlay;
            invert = false;
        }
        var result = dot(blend, primary);
        if (invert) {
            result = 1.0 - result;
        }
        overlay_mix = clamp(result * 2.0 - 1.0, 0.0, 1.0);
    }
    var output_color = mix(underlay_color, overlay_color, overlay_mix);
    if (env.flags.y == 1u) {
        return vec4<f32>(linear_to_srgb((base1 * blend.x + base2 * blend.y + base3 * blend.z)), 1.0);
    }
    if (env.flags.y == 2u) {
        return vec4<f32>(fract(blended_uv), 0.0, 1.0);
    }
    if (env.flags.y == 3u) {
        return vec4<f32>(n * 0.5 + vec3<f32>(0.5), 1.0);
    }
    if (env.flags.y == 4u) {
        return vec4<f32>(tbn[0] * 0.5 + vec3<f32>(0.5), 1.0);
    }

    // Normals.
    var normals: vec3<f32>;
    if ((flags & FLAG_UPWARDS_NORMALS) != 0u) {
        normals = vec3<f32>(0.0, -1.0, 0.0);
    } else {
        let n1 = sample_normal_map(material1, uv1, ddx1, ddy1, tbn);
        let n2 = sample_normal_map(material2, uv2, ddx2, ddy2, tbn);
        let n3 = sample_normal_map(material3, uv3, ddx3, ddy3, tbn);
        normals = normalize(n1 * blend.x + n2 * blend.y + n3 * blend.z);
    }

    let light_dir = env.light_dir.xyz;
    let light_dot_normals = dot(normals, light_dir);
    let up_dot_normals = dot(vec3<f32>(0.0, -1.0, 0.0), normals);

    // Specular strength and gloss; semi-transparent faces get a window-like highlight.
    var spec_gloss = vec3<f32>(material1.specular_gloss, material2.specular_gloss, material3.specular_gloss);
    var spec_strength = vec3<f32>(material1.specular_strength, material2.specular_strength, material3.specular_strength);
    spec_strength = spec_strength * vec3<f32>(
        select(1.0, linear_to_srgb(textureSampleGrad(material_textures, material_sampler, uv1, max(material1.roughness_map, 0), ddx1, ddy1).rgb).r, material1.roughness_map >= 0),
        select(1.0, linear_to_srgb(textureSampleGrad(material_textures, material_sampler, uv2, max(material2.roughness_map, 0), ddx2, ddy2).rgb).r, material2.roughness_map >= 0),
        select(1.0, linear_to_srgb(textureSampleGrad(material_textures, material_sampler, uv3, max(material3.roughness_map, 0), ddx3, ddy3).rgb).r, material3.roughness_map >= 0),
    );
    if (alpha * 3.0 < 2.99) {
        spec_gloss = vec3<f32>(30.0);
        spec_strength = vec3<f32>(clamp((1.0 - alpha) * 2.0, 0.0, 1.0));
    }
    let combined_spec_strength = dot(spec_strength, blend);

    // Ambient light with the baked ambient-occlusion maps.
    let ao1 = select(1.0, textureSampleGrad(material_textures, material_sampler, uv1, max(material1.ambient_occlusion_map, 0), ddx1, ddy1).r, material1.ambient_occlusion_map >= 0);
    let ao2 = select(1.0, textureSampleGrad(material_textures, material_sampler, uv2, max(material2.ambient_occlusion_map, 0), ddx2, ddy2).r, material2.ambient_occlusion_map >= 0);
    let ao3 = select(1.0, textureSampleGrad(material_textures, material_sampler, uv3, max(material3.ambient_occlusion_map, 0), ddx3, ddy3).r, material3.ambient_occlusion_map >= 0);
    let ao = ao1 * blend.x + ao2 * blend.y + ao3 * blend.z;
    let ambient_out = env.ambient_color.rgb * env.ambient_color.w * ao;

    // Directional light and its specular highlight.
    let dir_light = env.light_color.rgb * env.light_color.w;
    let light_out = max(light_dot_normals, 0.0) * dir_light;
    let reflect_dir = reflect(-light_dir, normals);
    let v_dot_r = clamp(dot(view_dir, reflect_dir), 1e-10, 1.0);
    let specular = dot(pow(vec3<f32>(v_dot_r), spec_gloss) * spec_strength, blend);
    let light_specular_out = dir_light * specular;

    // Sky light from above.
    let sky_out = max(up_dot_normals, 0.0) * globals.fog_color.rgb * 0.5;

    let composite = ambient_out + light_out + light_specular_out + sky_out;
    if (env.flags.y == 5u) {
        return vec4<f32>(composite, 1.0);
    }
    let unlit = dot(blend, vec3<f32>(
        f32((material1.flags >> 1u) & 1u),
        f32((material2.flags >> 1u) & 1u),
        f32((material3.flags >> 1u) & 1u),
    ));
    var rgb = output_color.rgb * mix(composite, vec3<f32>(1.0), unlit);
    rgb = linear_to_srgb(rgb);

    if (is_underwater) {
        // Tint with the water colour by depth (`sampleUnderwater`, simplified).
        let depth = clamp(water_depth / 1200.0, 0.0, 1.0);
        let tint = linear_to_srgb(env.water_color.rgb);
        rgb = mix(rgb, tint * (0.35 + 0.65 * (1.0 - depth)), 0.35 + 0.55 * depth);
    }
    return vec4<f32>(rgb, output_color.a);
}
