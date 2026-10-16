// Shadow-map pass of the HD profile: a WGSL port of `shadow_vert.glsl` / `shadow_frag.glsl` (zone
// renderer path; no geometry shader). Geometry is drawn from the light's point of view into a depth
// texture. With shadow transparency the fragment shader packs the face's opacity (high bits) and
// depth (low 16 bits) into the output depth, so less transparent fragments win first and equally
// transparent ones by depth; `shadow_receive.wgsl` unpacks it. The terrain map (`TERRAIN_ONLY`)
// holds only the ground plane's terrain tiles, with plain depth.
//
// Ported from the 117 HD plugin. Copyright (c) 2018, Adam <Adam@sigterm.info>; (c) 2021, 117
// <https://twitter.com/117scape>; (c) 2022, Hooder <ahooder@protonmail.com>. BSD 2-Clause (see
// THIRD_PARTY.md).

struct Zone {
    base: vec4<f32>,
};

@group(1) @binding(0) var<uniform> zone: Zone;

override TERRAIN_ONLY: bool = false;

struct VertexIn {
    @builtin(vertex_index) index: u32,
    @location(0) position: vec4<i32>,
    @location(1) abhsl: u32,
    @location(3) normal: vec2<f32>,
    @location(4) uv: vec2<f32>,
    @location(5) hsl_flags: vec4<u32>,
    @location(6) material_water: vec4<u32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) opacity: f32,
    // x = colour map layer + 1, y = shadow alpha map layer + 1 (0 = none), z = vanilla UVs.
    @location(2) @interpolate(flat) maps: vec3<u32>,
};

// `SHADOW_DEFAULT_OPACITY_THRESHOLD`: faces at most this opaque cast no shadow (clickboxes);
// without shadow transparency the plugin raises it so glass stays transparent.
fn opacity_threshold() -> f32 {
    return select(0.71, 0.01, (env.shadow_flags.w & 1u) != 0u);
}

fn decode_shadow_normal(e: vec2<f32>) -> vec3<f32> {
    var n = vec3<f32>(e, 1.0 - abs(e.x) - abs(e.y));
    if (n.z < 0.0) {
        n = vec3<f32>((vec2<f32>(1.0) - abs(n.yx)) * select(vec2<f32>(-1.0), vec2<f32>(1.0), n.xy >= vec2<f32>(0.0)), n.z);
    }
    return normalize(n);
}
@vertex
fn vs_shadow(input: VertexIn) -> VertexOut {
    var out: VertexOut;
    let position = theme_vertex(vec3<f32>(input.position.xyz) + zone.base.xyz, decode_shadow_normal(input.normal), env.camera.w);
    let corner = input.index % 3u;
    let flags = input.hsl_flags.w;
    let opacity = 1.0 - f32((input.abhsl >> 24u) & 0xffu) / 255.0;
    let water_type = input.material_water.w & 0xffu;
    let plane = (input.material_water.w >> 8u) & 3u;
    let is_terrain = (flags & FLAG_TERRAIN) != 0u;
    let ground_plane = is_terrain && plane == 0u;
    let water_or_underwater = is_terrain && water_type > 0u;

    var disabled: bool;
    if (TERRAIN_ONLY) {
        disabled = !ground_plane || water_or_underwater;
    } else {
        disabled = ground_plane || water_or_underwater || opacity <= opacity_threshold()
            || (flags & FLAG_NO_SHADOW_CAST) != 0u;
    }

    // The three vertices of a disabled triangle collapse onto one point outside the light's view.
    var clip = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    var uv = vec2<f32>(0.0);
    var maps = vec3<u32>(0u);
    if (!disabled) {
        clip = env.light_proj * vec4<f32>(position, 1.0);
        if (!TERRAIN_ONLY) {
            let material = materials[input.material_water[corner] & MATERIAL_INDEX_MASK];
            // Bias faces with transparency to avoid self-shadowing.
            if ((material.flags & 1u) != 0u) {
                clip.z = clip.z + 0.006;
            }
            uv = input.uv + material.scroll_duration * env.camera.w;
            uv = (uv - vec2<f32>(0.5)) * material.texture_scale.xy + vec2<f32>(0.5);
            maps = vec3<u32>(
                u32(material.color_map + 1),
                u32(material.shadow_alpha_map + 1),
                select(0u, 1u, (flags & FLAG_VANILLA_UVS) != 0u),
            );
        }
    }
    out.clip = clip;
    out.uv = uv;
    out.opacity = opacity;
    out.maps = maps;
    return out;
}

// Face opacity times the colour map's alpha and the shadow alpha map.
fn face_opacity(input: VertexOut) -> f32 {
    var opacity = input.opacity;
    if ((env.shadow_flags.w & 2u) != 0u && input.maps.x != 0u) {
        var uv = input.uv;
        // Vanilla tree textures rely on UVs being clamped horizontally, which HD does not do:
        // hide those fragments instead.
        if (input.maps.z != 0u) {
            uv.x = clamp(uv.x, 0.0, 0.984375);
        }
        opacity = opacity * textureSampleLevel(
            material_textures, material_sampler, uv, i32(input.maps.x) - 1, 0.0).a;
        if (input.maps.y != 0u) {
            let alpha = textureSampleLevel(
                material_textures, material_sampler, uv, i32(input.maps.y) - 1, 0.0).r;
            opacity = opacity * linear_to_srgb(vec3<f32>(alpha)).x;
        }
    }
    return opacity;
}

// Transparent shadows: pack opacity and depth into the output depth.
@fragment
fn fs_packed(input: VertexOut) -> @builtin(frag_depth) f32 {
    let opacity = face_opacity(input);
    let alpha_bits = u32(clamp((1.0 - opacity) * 255.0, 0.0, 255.0));
    let depth_bits = u32(clamp(input.clip.z, 0.0, 1.0) * 65535.0);
    return f32((alpha_bits << 16u) | depth_bits) / 16777215.0;
}

// Opaque shadows: texture alpha only cuts holes.
@fragment
fn fs_plain(input: VertexOut) {
    if (face_opacity(input) < 0.01) {
        discard;
    }
}
