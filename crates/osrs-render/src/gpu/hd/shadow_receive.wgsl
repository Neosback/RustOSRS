// Shadow lookup of the HD scene shader: a WGSL port of `utils/shadows.glsl`,
// `utils/shadow_filtering.glsl` and `utils/shadow_pcss.glsl` (PCF 1x1/2x2/3x3, dithered, averaged
// and PCSS filtering, receiver-plane depth bias, normal bias, packed transparency, terrain map).
// Prepended after `common.wgsl`; `scene.wgsl` calls `sample_shadow_map`.
//
// Ported from the 117 HD plugin. Copyright (c) 2018, Adam <Adam@sigterm.info>; (c) 2021, 117
// <https://twitter.com/117scape>; (c) 2022, Hooder <ahooder@protonmail.com>. BSD 2-Clause (see
// THIRD_PARTY.md). The plugin samples a GL depth texture with `texelFetch`; here the depth
// textures are read with `textureLoad` and compared manually, so every filter works on both maps.

@group(3) @binding(0) var shadow_map: texture_depth_2d;
@group(3) @binding(1) var terrain_shadow_map: texture_depth_2d;

const SHADOW_DEPTH_BITS: u32 = 16u;
const SHADOW_DEPTH_MAX: u32 = 65535u;
const SHADOW_ALPHA_MAX: f32 = 255.0;
const SHADOW_COMBINED_MAX: f32 = 16777215.0;

const SHADOW_FILTER_DITHER: u32 = 1u;
const SHADOW_FILTER_AVERAGE: u32 = 2u;
const SHADOW_FILTER_PCSS: u32 = 3u;

// Angular radius of the sun disk, `radians(0.53 * 0.5)`.
const SUN_SHADOW_RADIUS: f32 = 0.004625123;
const PCSS_BLOCKER_SAMPLES: i32 = 4;
const PCSS_FILTER_SAMPLES: i32 = 8;
const TAU: f32 = 6.283185307;

var<private> shadow_frag: vec2<f32>;

var<private> poisson_disk: array<vec2<f32>, 16> = array<vec2<f32>, 16>(
    vec2<f32>(-0.94201624, -0.39906216),
    vec2<f32>(0.94558609, -0.76890725),
    vec2<f32>(-0.094184101, -0.92938870),
    vec2<f32>(0.34495938, 0.29387760),
    vec2<f32>(-0.91588581, 0.45771432),
    vec2<f32>(-0.81544232, -0.87912464),
    vec2<f32>(-0.38277543, 0.27676845),
    vec2<f32>(0.97484398, 0.75648379),
    vec2<f32>(0.44323325, -0.97511554),
    vec2<f32>(0.53742981, -0.47373420),
    vec2<f32>(-0.26496911, -0.41893023),
    vec2<f32>(0.79197514, 0.19090188),
    vec2<f32>(-0.24188840, 0.99706507),
    vec2<f32>(-0.81409955, 0.91437590),
    vec2<f32>(0.19984126, 0.78641367),
    vec2<f32>(0.14383161, -0.14100790),
);

fn hash12(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash13(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 = p3 + dot(p3, p3.zyx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn shadow_dither_offset(i: i32) -> vec2<f32> {
    if (env.shadow_flags.y == SHADOW_FILTER_DITHER) {
        let first = i32(hash12(shadow_frag) * 16.0);
        return poisson_disk[(first + i) % 16] * 1.25;
    }
    return vec2<f32>(0.0);
}

// One filter tap: 1 when the texel's occluder is in front of the receiver (or its opacity when
// the map packs transparency).
fn fetch_shadow_texel(
    tex: texture_depth_2d,
    has_transparency: bool,
    pixel: vec2<i32>,
    frag_depth_in: f32,
    receiver_plane: vec4<f32>,
    i: i32,
) -> f32 {
    let p = pixel + vec2<i32>(shadow_dither_offset(i));
    let size = vec2<i32>(textureDimensions(tex));
    if (any(p < vec2<i32>(0)) || any(p >= size)) {
        return 0.0;
    }
    let bias = dot(vec2<f32>(p) + vec2<f32>(0.5) - receiver_plane.xy, receiver_plane.zw);
    // The correction is bounded to 64 world units of depth.
    let correction_limit = 64.0 / env.shadow_axis.w;
    let frag_depth = frag_depth_in + clamp(bias, -correction_limit, correction_limit);
    let stored = textureLoad(tex, p, 0);
    if (has_transparency) {
        let packed = u32(stored * SHADOW_COMBINED_MAX + 0.5);
        let depth = f32(packed & SHADOW_DEPTH_MAX) / f32(SHADOW_DEPTH_MAX);
        let alpha = 1.0 - f32(packed >> SHADOW_DEPTH_BITS) / SHADOW_ALPHA_MAX;
        return select(0.0, alpha, depth < frag_depth);
    }
    return select(0.0, 1.0, stored < frag_depth);
}

fn sample_shadow_pcf1x1(
    tex: texture_depth_2d,
    has_transparency: bool,
    frag_depth: f32,
    shadow_pos: vec3<f32>,
    receiver_plane: vec4<f32>,
) -> f32 {
    let pixel = vec2<i32>(shadow_pos.xy * vec2<f32>(textureDimensions(tex)));
    return fetch_shadow_texel(tex, has_transparency, pixel, frag_depth, receiver_plane, 0);
}

fn sample_shadow_pcf2x2(
    tex: texture_depth_2d,
    has_transparency: bool,
    frag_depth: f32,
    shadow_pos: vec3<f32>,
    receiver_plane: vec4<f32>,
) -> f32 {
    // Shift so the 2x2 kernel straddles the sample point.
    let position = shadow_pos.xy * vec2<f32>(textureDimensions(tex)) - vec2<f32>(0.5);
    let offset = vec2<i32>(floor(position));
    let c00 = fetch_shadow_texel(tex, has_transparency, offset + vec2<i32>(0, 0), frag_depth, receiver_plane, 0);
    let c10 = fetch_shadow_texel(tex, has_transparency, offset + vec2<i32>(1, 0), frag_depth, receiver_plane, 1);
    let c01 = fetch_shadow_texel(tex, has_transparency, offset + vec2<i32>(0, 1), frag_depth, receiver_plane, 2);
    let c11 = fetch_shadow_texel(tex, has_transparency, offset + vec2<i32>(1, 1), frag_depth, receiver_plane, 3);
    if (c00 == c10 && c10 == c01 && c01 == c11) {
        return c00;
    }
    if (env.shadow_flags.y == SHADOW_FILTER_AVERAGE) {
        return (c00 + c10 + c01 + c11) * 0.25;
    }
    let lerp = fract(position);
    return mix(mix(c00, c10, lerp.x), mix(c01, c11, lerp.x), lerp.y);
}

fn sample_shadow_pcf3x3(
    tex: texture_depth_2d,
    has_transparency: bool,
    frag_depth: f32,
    shadow_pos: vec3<f32>,
    receiver_plane: vec4<f32>,
) -> f32 {
    let position = shadow_pos.xy * vec2<f32>(textureDimensions(tex));
    let offset = vec2<i32>(position) - vec2<i32>(1);
    let average = env.shadow_flags.y == SHADOW_FILTER_AVERAGE;
    let lerp = fract(position);
    let lerp_x = vec3<f32>(1.0 - lerp.x, 1.0, lerp.x);
    let lerp_y = vec3<f32>(1.0 - lerp.y, 1.0, lerp.y);

    let c00 = fetch_shadow_texel(tex, has_transparency, offset + vec2<i32>(0, 0), frag_depth, receiver_plane, 0);
    let c02 = fetch_shadow_texel(tex, has_transparency, offset + vec2<i32>(0, 2), frag_depth, receiver_plane, 1);
    let c20 = fetch_shadow_texel(tex, has_transparency, offset + vec2<i32>(2, 0), frag_depth, receiver_plane, 2);
    let c22 = fetch_shadow_texel(tex, has_transparency, offset + vec2<i32>(2, 2), frag_depth, receiver_plane, 3);
    if ((c00 == 0.0 || c00 == 1.0) && c00 == c02 && c00 == c20 && c00 == c22) {
        return c00;
    }
    let s01 = fetch_shadow_texel(tex, has_transparency, offset + vec2<i32>(0, 1), frag_depth, receiver_plane, 4);
    let s10 = fetch_shadow_texel(tex, has_transparency, offset + vec2<i32>(1, 0), frag_depth, receiver_plane, 5);
    let s11 = fetch_shadow_texel(tex, has_transparency, offset + vec2<i32>(1, 1), frag_depth, receiver_plane, 6);
    let s12 = fetch_shadow_texel(tex, has_transparency, offset + vec2<i32>(1, 2), frag_depth, receiver_plane, 7);
    let s21 = fetch_shadow_texel(tex, has_transparency, offset + vec2<i32>(2, 1), frag_depth, receiver_plane, 8);
    if (average) {
        return (c00 + s01 + c02 + s10 + s11 + s12 + c20 + s21 + c22) / 9.0;
    }
    let shadow =
        c00 * lerp_x[0] * lerp_y[0] +
        s01 * lerp_x[0] * lerp_y[1] +
        c02 * lerp_x[0] * lerp_y[2] +
        s10 * lerp_x[1] * lerp_y[0] +
        s11 * lerp_x[1] * lerp_y[1] +
        s12 * lerp_x[1] * lerp_y[2] +
        c20 * lerp_x[2] * lerp_y[0] +
        s21 * lerp_x[2] * lerp_y[1] +
        c22 * lerp_x[2] * lerp_y[2];
    return shadow * 0.25;
}

fn pcss_disk_offset(i: i32, count: i32) -> vec2<f32> {
    // Equal-area sunflower pattern; rotated per scene-space cell by the caller.
    let radius = sqrt((f32(i) + 0.5) / f32(count));
    let angle = f32(i) * 2.39996323;
    return radius * vec2<f32>(cos(angle), sin(angle));
}

// Normalised depth and opacity of a blocker texel. Terrain uses ordinary depth; the main map may
// pack transparency into the high bits, which must not enter distances.
fn pcss_blocker(tex: texture_depth_2d, has_transparency: bool, pixel: vec2<i32>) -> vec2<f32> {
    let size = vec2<i32>(textureDimensions(tex));
    if (any(pixel < vec2<i32>(0)) || any(pixel >= size)) {
        return vec2<f32>(1.0, 0.0);
    }
    let value = textureLoad(tex, pixel, 0);
    if (has_transparency) {
        let packed = u32(value * SHADOW_COMBINED_MAX + 0.5);
        return vec2<f32>(
            f32(packed & SHADOW_DEPTH_MAX) / f32(SHADOW_DEPTH_MAX),
            1.0 - f32(packed >> SHADOW_DEPTH_BITS) / SHADOW_ALPHA_MAX,
        );
    }
    return vec2<f32>(value, select(0.0, 1.0, value < 1.0));
}

// Percentage-closer soft shadows (NVIDIA, "Percentage-Closer Soft Shadows"): a blocker search sets
// the penumbra width, which scales the filter radius.
fn sample_shadow_pcss(
    tex: texture_depth_2d,
    has_transparency: bool,
    frag_depth: f32,
    shadow_pos: vec3<f32>,
    receiver_plane: vec4<f32>,
    receiver_position: vec3<f32>,
) -> f32 {
    let map_size = vec2<f32>(textureDimensions(tex));
    let depth_range = env.shadow_axis.w;
    let map_extent = env.shadow_right.w;
    let radius_per_depth = depth_range * tan(SUN_SHADOW_RADIUS) / map_extent;
    // The light's near plane bounds the possible blocker distance: a conservative search cone.
    let search_radius = max(shadow_pos.z, 0.0) * radius_per_depth;
    if (max(search_radius * map_size.x, search_radius * map_size.y) <= 0.5) {
        return sample_shadow_pcf2x2(tex, has_transparency, frag_depth, shadow_pos, receiver_plane);
    }

    let cell = floor(receiver_position * 0.5);
    var angle = TAU * hash13(cell);
    var c = cos(angle);
    var s = sin(angle);
    var rotation = mat2x2<f32>(c, s, -s, c);

    var separation_sum = 0.0;
    var blocker_weight = 0.0;
    let correction_limit = 64.0 / depth_range;
    // Include the centre so a contact shadow cannot disappear between search taps.
    for (var i = -1; i < PCSS_BLOCKER_SAMPLES; i = i + 1) {
        var offset = vec2<f32>(0.0);
        if (i >= 0) {
            offset = (rotation * pcss_disk_offset(i, PCSS_BLOCKER_SAMPLES)) * search_radius;
        }
        let pixel = vec2<i32>(floor((shadow_pos.xy + offset) * map_size));
        let blocker = pcss_blocker(tex, has_transparency, pixel);
        let correction = clamp(
            dot(vec2<f32>(pixel) + vec2<f32>(0.5) - receiver_plane.xy, receiver_plane.zw),
            -correction_limit,
            correction_limit,
        );
        if (blocker.y > 0.0 && blocker.x < frag_depth + correction) {
            // Bias decides visibility, but must not shorten the physical separation.
            separation_sum = separation_sum + max(0.0, shadow_pos.z + correction - blocker.x) * blocker.y;
            blocker_weight = blocker_weight + blocker.y;
        }
    }
    if (blocker_weight == 0.0) {
        return sample_shadow_pcf2x2(tex, has_transparency, frag_depth, shadow_pos, receiver_plane);
    }
    let filter_radius = radius_per_depth * (separation_sum / blocker_weight);
    if (max(filter_radius * map_size.x, filter_radius * map_size.y) <= 0.5) {
        return sample_shadow_pcf2x2(tex, has_transparency, frag_depth, shadow_pos, receiver_plane);
    }

    // Decorrelate the visibility taps from the search that estimated the radius.
    angle = TAU * hash13(cell + vec3<f32>(17.0, 59.0, 113.0));
    c = cos(angle);
    s = sin(angle);
    rotation = mat2x2<f32>(c, s, -s, c);

    var shadow = 0.0;
    for (var i = 0; i < PCSS_FILTER_SAMPLES; i = i + 1) {
        var tap = shadow_pos;
        let tap_offset = (rotation * pcss_disk_offset(i, PCSS_FILTER_SAMPLES)) * filter_radius;
        tap = vec3<f32>(tap.xy + tap_offset, tap.z);
        shadow = shadow + sample_shadow_pcf1x1(tex, has_transparency, frag_depth, tap, receiver_plane);
    }
    return shadow / f32(PCSS_FILTER_SAMPLES);
}

fn sample_shadow_filtered(
    tex: texture_depth_2d,
    has_transparency: bool,
    frag_depth: f32,
    shadow_pos: vec3<f32>,
    receiver_plane: vec4<f32>,
    receiver_position: vec3<f32>,
) -> f32 {
    if (env.shadow_flags.y == SHADOW_FILTER_PCSS) {
        return sample_shadow_pcss(tex, has_transparency, frag_depth, shadow_pos, receiver_plane, receiver_position);
    }
    switch (env.shadow_flags.z) {
        case 3u: {
            return sample_shadow_pcf3x3(tex, has_transparency, frag_depth, shadow_pos, receiver_plane);
        }
        case 2u: {
            return sample_shadow_pcf2x2(tex, has_transparency, frag_depth, shadow_pos, receiver_plane);
        }
        default: {
            return sample_shadow_pcf1x1(tex, has_transparency, frag_depth, shadow_pos, receiver_plane);
        }
    }
}

// `sampleShadowMap`: how much of the directional light is blocked at `frag_pos` (0 lit, 1 in
// shadow). `distortion` offsets the lookup in shadow-map UV space (used for water).
fn sample_shadow_map(
    frag_pos: vec3<f32>,
    distortion: vec2<f32>,
    surface_normal: vec3<f32>,
    apply_bias: bool,
    apply_normal_bias: bool,
) -> f32 {
    if (env.shadow_flags.x == 0u || env.light_dir.w == 0.0) {
        return 0.0;
    }

    // Fade out shadows towards the edge of the shadow draw distance.
    let fade_end = max(env.shadow_params.y, 1.0);
    let fade_start = max(fade_end * 0.5, fade_end - 10.0 * 128.0);
    let fade_out = smoothstep(fade_start, fade_end, length(frag_pos - env.camera.xyz));
    if (fade_out >= 1.0) {
        return 0.0;
    }

    let axis = env.shadow_axis.xyz;
    let map_size = env.shadow_params.z;
    var sample_position = frag_pos;
    var bias = 0.0;
    var depth_precision_bias = 0.0;
    var receiver_depth_per_texel = vec2<f32>(0.0);
    if (apply_bias) {
        // Depth slope of the receiver's plane per shadow texel. A parallel receiver has unbounded
        // slope: keep it finite.
        var n_axis = dot(surface_normal, axis);
        n_axis = select(1.0, -1.0, n_axis < 0.0) * max(abs(n_axis), 1e-7);
        receiver_depth_per_texel = vec2<f32>(
            dot(surface_normal, env.shadow_right.xyz) / n_axis,
            -dot(surface_normal, env.shadow_up.xyz) / n_axis,
        ) * env.shadow_up.w / env.shadow_axis.w;

        // Move the receiver towards the light along its normal.
        if (apply_normal_bias) {
            let side = select(1.0, -1.0, dot(surface_normal, axis) < 0.0);
            sample_position = sample_position + normalize(surface_normal) * side * 3.0;
        }

        // Bound only the extra safety margin to limit detached shadows at grazing angles.
        bias = clamp(
            length(receiver_depth_per_texel),
            env.shadow_params.x,
            env.shadow_params.x * 16.0,
        );
        // The packed depth keeps 16 bits: cover one truncated step plus a step of rounding.
        depth_precision_bias = 2.0 / f32(SHADOW_DEPTH_MAX);
    }

    let clip = env.light_proj * vec4<f32>(sample_position, 1.0);
    var tex = vec2<f32>(clip.x * 0.5 + 0.5, 0.5 - clip.y * 0.5) + distortion;
    tex = clamp(tex, vec2<f32>(0.0), vec2<f32>(1.0));
    let shadow_pos = vec3<f32>(tex, clip.z);
    let receiver_plane = vec4<f32>(tex * map_size, receiver_depth_per_texel);
    let has_transparency = (env.shadow_flags.w & 1u) != 0u;

    var shadow = sample_shadow_filtered(
        shadow_map,
        has_transparency,
        shadow_pos.z - (bias + depth_precision_bias),
        shadow_pos,
        receiver_plane,
        frag_pos,
    );

    if (shadow < 1.0 && (env.shadow_flags.w & 4u) != 0u) {
        let terrain_size = env.shadow_params.w;
        let texel_scale = map_size / terrain_size;
        let terrain_bias = bias * texel_scale + depth_precision_bias;
        let terrain_plane = vec4<f32>(tex * terrain_size, receiver_depth_per_texel * texel_scale);
        // The terrain map holds smooth hill shadows: a 2x2 kernel is enough (and a third cheaper
        // than the 3x3 of the main map).
        var terrain_shadow: f32;
        if (env.shadow_flags.y == SHADOW_FILTER_PCSS) {
            terrain_shadow = sample_shadow_pcss(
                terrain_shadow_map, false, shadow_pos.z - terrain_bias, shadow_pos, terrain_plane, frag_pos);
        } else if (env.shadow_flags.z >= 2u) {
            terrain_shadow = sample_shadow_pcf2x2(
                terrain_shadow_map, false, shadow_pos.z - terrain_bias, shadow_pos, terrain_plane);
        } else {
            terrain_shadow = sample_shadow_pcf1x1(
                terrain_shadow_map, false, shadow_pos.z - terrain_bias, shadow_pos, terrain_plane);
        }
        shadow = max(shadow, terrain_shadow);
    }
    return shadow * (1.0 - fade_out);
}
