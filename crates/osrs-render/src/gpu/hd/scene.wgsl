// HD-profile scene shader: a WGSL port of the 117 HD plugin's `scene_vert.glsl` / `scene_frag.glsl`
// (zone renderer), phase 2 subset: per-corner materials with ground blending, diffuse + normal map +
// parallax displacement + specular, vertex-colour and underlay/overlay blending, tangent frame from
// derivatives, underwater
// tint, area environments (ambient/directional light, fog, ground fog, sky light, underglow,
// lightning; phase 3). Shadows (phase 4), water surfaces (5), point lights (6) and tone mapping (8)
// are not ported yet. Water (phase 5): water surfaces (`sampleWater`), the underwater tint and
// caustics (`sampleUnderwater`), underwater flow distortion and the draw-distance depth clamp.
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

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var<uniform> zone: Zone;
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
    var vert = vec4<f32>(vec3<f32>(input.position.xyz) + zone.base.xyz, 1.0);
    let bias = f32((input.abhsl >> 16u) & 0xffu);

    // Clamp underwater vertices to the water surface along the draw distance border, excluding a
    // water depth of 1, which marks geometry already flush with the surface.
    let water_depth_here = input.depth[input.index % 3u];
    if (water_depth_here > 1u) {
        let chunk = TILE_SIZE * 8.0;
        let cam = floor(env.camera.xz / chunk) * chunk + chunk / 2.0;
        let d = floor(abs(vert.xz - cam) / TILE_SIZE);
        if (max(d.x, d.y) > floor(env.fog_params.x / 8.0) * 8.0 + 3.0) {
            vert.y = vert.y - f32(water_depth_here);
        }
    }

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

// ---- fog (`utils/fog.glsl`, sky-less path) --------------------------------------------------

const TILE_SIZE: f32 = 128.0;

fn fog_factor_linear(distance: f32, start: f32, end: f32) -> f32 {
    return 1.0 - clamp((distance - start) / (end - start), 0.0, 1.0);
}

// `calculateFogAmount`. The streamed world has no hard scene edge, so the plugin's scene-edge
// clamps are dropped: the fog edge is the draw-distance box around the camera.
fn calculate_fog_amount(position: vec3<f32>) -> f32 {
    let fog_depth = env.fog_color.w;
    if (fog_depth == 0.0) {
        return 0.0;
    }
    let draw_distance = env.fog_params.x;
    var draw_distance2 = draw_distance * TILE_SIZE;

    // The client draws one less tile to the north and east than to the south and west.
    let fog_west = env.camera.x - draw_distance2;
    let fog_east = env.camera.x + draw_distance2 - TILE_SIZE;
    let fog_south = env.camera.z - draw_distance2;
    let fog_north = env.camera.z + draw_distance2 - TILE_SIZE;

    let x_dist = min(position.x - fog_west, fog_east - position.x);
    let z_dist = min(position.z - fog_south, fog_north - position.z);
    let nearest = min(x_dist, z_dist);
    let second_nearest = max(x_dist, z_dist);
    let rounding = 1.5;
    let rounding_squared = rounding * rounding;
    let fog_distance = nearest - rounding * TILE_SIZE * max(0.0,
        (nearest + rounding_squared) / (second_nearest + rounding_squared));
    let edge_fog = fog_factor_linear(fog_distance, 0.0, 5.0 * TILE_SIZE);

    // Two arbitrary distance fogs, eyeballed for a similar look at different draw distances.
    let fog_start1 = draw_distance2 * 0.85;
    let distance1 = length(env.camera.xz - position.xz);
    let distance_fog1 = clamp((distance1 - fog_start1) / (draw_distance2 * 0.15), 0.0, 1.0);

    let min_fog_start = 0.0;
    let max_fog_start = 0.3;
    draw_distance2 = min(draw_distance, 90.0) * TILE_SIZE;
    let fog_depth_multiplier = clamp(fog_depth, 0.0, 1000.0) / 1000.0;
    let fog_start2 = (max_fog_start - fog_depth_multiplier * (max_fog_start - min_fog_start)) * draw_distance2;
    let cam_to_vertex = length(env.camera.xyz - vec3<f32>(position.x, (position.y + env.camera.y) / 2.0, position.z));
    let distance2 = max(cam_to_vertex - fog_start2, 0.0) / max(draw_distance2 - fog_start2, 1.0);
    let density = fog_depth / 100.0;
    let distance_fog2 = 1.0 - clamp(exp(-distance2 * density), 0.0, 1.0);

    return max(max(distance_fog1, distance_fog2), edge_fog);
}

// Distance fog and ground fog combined: the share of the sky colour at `position`.
fn combined_fog_amount(position: vec3<f32>) -> f32 {
    let eye_distance = length(position - env.camera.xyz);
    let close_fade_distance = 1500.0;
    var ground_fog = 1.0 - clamp(
        (position.y - env.fog_params.y) / (env.fog_params.z - env.fog_params.y),
        0.0,
        1.0,
    );
    ground_fog = mix(0.0, env.fog_params.w, ground_fog);
    ground_fog = ground_fog * clamp(eye_distance / close_fade_distance, 0.0, 1.0);
    let fog_amount = calculate_fog_amount(position);
    return 1.0 - (1.0 - fog_amount) * (1.0 - ground_fog);
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

// `animationFrame(vec2)` from RLHD's misc.glsl. Durations are independent per axis; zero keeps
// that axis stationary instead of dividing by zero.
fn animation_frame(duration: vec2<f32>, time: f32) -> vec2<f32> {
    var frame = vec2<f32>(0.0);
    if (duration.x != 0.0) {
        frame.x = fract(time / duration.x);
    }
    if (duration.y != 0.0) {
        frame.y = fract(time / duration.y);
    }
    return frame;
}

struct DisplacementSample {
    uv: vec2<f32>,
    delta: vec3<f32>,
};

// `sampleDisplacementMap` from RLHD's displacement.glsl, without its optional self-shadowing
// compile mode (the normal plugin toggle enables POM at level 1). Height maps live in the sRGB
// texture array, so convert the sampled linear value back to the authored code value just like
// upstream's `linearToSrgb` call.
fn sample_displacement_map(
    material: Material,
    ts_view_dir: vec3<f32>,
    uv: vec2<f32>,
    uv_ddx: vec2<f32>,
    uv_ddy: vec2<f32>,
) -> DisplacementSample {
    var result = DisplacementSample(uv, vec3<f32>(0.0));
    if (material.displacement_map < 0 || material.displacement_scale == 0.0) {
        return result;
    }

    let scale = material.displacement_scale;
    let cos_view = normalize(ts_view_dir).z;
    let num_layers = mix(1.0, 16.0, 1.0 - clamp(cos_view * cos_view, 0.0, 1.0));
    let height_per_layer = 1.0 / num_layers;

    var view_z = ts_view_dir.z;
    if (abs(view_z) < 0.0001) {
        view_z = select(-0.0001, 0.0001, view_z >= 0.0);
    }
    let delta_xy_per_z = ts_view_dir.xy / view_z * scale;

    // Keep half displacement centered on the original surface, matching RLHD.
    result.uv = result.uv + delta_xy_per_z * 0.5;
    result.delta = result.delta - vec3<f32>(delta_xy_per_z, scale) * 0.5;

    var height = 0.0;
    var previous_height = 0.0;
    var layer = 1.0;
    for (var step = 0u; step < 16u; step = step + 1u) {
        if (layer < 0.0 || height > layer) {
            break;
        }
        previous_height = height;
        let sampled = textureSampleGrad(
            material_textures,
            material_sampler,
            result.uv - delta_xy_per_z * layer,
            material.displacement_map,
            uv_ddx,
            uv_ddy,
        ).r;
        height = linear_to_srgb(vec3<f32>(sampled)).r;
        layer = layer - height_per_layer;
    }

    let overshoot = height - layer;
    let undershoot = (layer + height_per_layer) - previous_height;
    let denominator = overshoot + undershoot;
    let correction = select(0.0, overshoot / denominator, abs(denominator) > 0.000001);
    height = mix(height, previous_height, correction);

    let ts_delta = vec3<f32>(delta_xy_per_z, scale) * height;
    result.uv = result.uv - ts_delta.xy;
    result.delta = result.delta + ts_delta;
    return result;
}

// ---- water (`utils/water.glsl`, `utils/caustics.glsl`) --------------------------------------

fn sample_caustics_channel(flow1: vec2<f32>, flow2: vec2<f32>, ddx: vec2<f32>, ddy: vec2<f32>) -> f32 {
    let layer = max(env.water_layers.z, 0);
    return min(
        textureSampleGrad(material_textures, material_sampler, flow1, layer, ddx, ddy).r,
        textureSampleGrad(material_textures, material_sampler, flow2, layer, ddx, ddy).r,
    );
}

// Caustics with chromatic aberration.
fn sample_caustics_aberrated(
    flow1: vec2<f32>,
    flow2: vec2<f32>,
    aberration: f32,
    ddx: vec2<f32>,
    ddy: vec2<f32>,
) -> vec3<f32> {
    let r = sample_caustics_channel(flow1 + aberration * vec2<f32>(1.0, 1.0), flow2 + aberration * vec2<f32>(1.0, 1.0), ddx, ddy);
    let g = sample_caustics_channel(flow1 + aberration * vec2<f32>(1.0, -1.0), flow2 + aberration * vec2<f32>(1.0, -1.0), ddx, ddy);
    let b = sample_caustics_channel(flow1 + aberration * vec2<f32>(-1.0, -1.0), flow2 + aberration * vec2<f32>(-1.0, -1.0), ddx, ddy);
    return vec3<f32>(r, g, b);
}

fn caustics_enabled() -> bool {
    return env.misc.y != 0.0 && env.water_layers.z >= 0;
}

// `sampleUnderwater`: depth colour fade and caustics on submerged terrain. `rgb` is sRGB.
fn sample_underwater(
    rgb_in: vec3<f32>,
    water_type: WaterType,
    position: vec3<f32>,
    depth: f32,
    light_dot_normals: f32,
) -> vec3<f32> {
    var rgb = rgb_in;
    let lowest_color_level = 500.0;
    let mid_color_level = 150.0;
    let surface_level = position.y - depth;
    let depth_color = water_type.depth_color.rgb;
    if (depth < mid_color_level) {
        rgb = rgb * mix(vec3<f32>(1.0), depth_color, depth / mid_color_level);
    } else if (depth < lowest_color_level) {
        rgb = rgb * mix(depth_color, vec3<f32>(0.0), (depth - mid_color_level) / (lowest_color_level - mid_color_level));
    } else {
        rgb = vec3<f32>(0.0);
    }

    if (caustics_enabled()) {
        let max_caustics_depth = 128.0 * 4.0;
        var caustics_uv = world_uvs(position, 1.75);
        var depth_multiplier = (position.y - surface_level - max_caustics_depth) / -max_caustics_depth;
        depth_multiplier = depth_multiplier * depth_multiplier;
        caustics_uv = caustics_uv * 0.75;
        let direction = vec2<f32>(1.0, -2.0);
        let flow1 = caustics_uv + water_animation(17.0) * direction;
        let flow2 = caustics_uv * 1.5 + water_animation(23.0) * -direction;
        let ddx = dpdx(caustics_uv);
        let ddy = dpdy(caustics_uv);
        let caustics = sample_caustics_aberrated(flow1, flow2, 0.005, ddx, ddy);
        let caustics_color = env.caustics.rgb * env.caustics.w;
        rgb = rgb * (1.0 + caustics * caustics_color * depth_multiplier * light_dot_normals * env.light_color.w);
    }
    return rgb;
}

fn specular_term(view_dir: vec3<f32>, reflect_dir: vec3<f32>, gloss: f32, strength: f32) -> f32 {
    let v_dot_r = clamp(dot(view_dir, reflect_dir), 1e-10, 1.0);
    return pow(v_dot_r, gloss) * strength;
}

// `sampleWater` (sky-less path): the surface colour in sRGB and its opacity. `shore` is the
// per-corner lightness of the surface faces (`127` open water, `0` next to land), `blend` the
// barycentric weights.
fn sample_water(
    water_type_index: u32,
    view_dir: vec3<f32>,
    position: vec3<f32>,
    face_uv: vec2<f32>,
    blend: vec3<f32>,
    shore_corners: vec3<f32>,
) -> vec4<f32> {
    let water_type = water_types[water_type_index];
    let duration = water_type.duration;

    // Texture coordinates and their screen-space derivatives (the animation offsets are constant
    // across a pixel quad).
    let anchor1 = world_uvs(position, 3.0).yx;
    let anchor2 = world_uvs(position, 5.0);
    let anchor3 = world_uvs(position, 7.0);
    let anchor_flow = world_uvs(position, 15.0);
    let ddx1 = dpdx(anchor1);
    let ddy1 = dpdy(anchor1);
    let ddx2 = dpdx(anchor2);
    let ddy2 = dpdy(anchor2);
    let ddx3 = dpdx(anchor3);
    let ddy3 = dpdy(anchor3);
    let ddx_flow = dpdx(anchor_flow);
    let ddy_flow = dpdy(anchor_flow);
    let ddx_foam = dpdx(face_uv);
    let ddy_foam = dpdy(face_uv);

    var uv1 = anchor1 - water_animation(28.0 * duration);
    var uv2 = anchor2 + water_animation(24.0 * duration);
    var uv3 = anchor3 + water_animation(21.0 * duration);
    var uv_foam = face_uv;

    let flow_uv = anchor_flow + water_animation(50.0 * duration);
    let flow_strength = 0.025;
    var uv_flow = vec2<f32>(0.0);
    if (env.water_layers.x >= 0) {
        uv_flow = textureSampleGrad(material_textures, material_sampler, flow_uv, env.water_layers.x, ddx_flow, ddy_flow).xy;
    }
    uv1 = uv1 + uv_flow * flow_strength;
    uv2 = uv2 + uv_flow * flow_strength;
    uv3 = uv3 + uv_flow * flow_strength;
    uv_foam = uv_foam + uv_flow * flow_strength;

    // Normal maps (`vec3(x * s, z, y * s)` negated), summed from three scales.
    let normal_layer = max(water_type.normal_map, 0);
    var n1 = linear_to_srgb(textureSampleGrad(material_textures, material_sampler, uv1, normal_layer, ddx1, ddy1).xyz);
    var n2 = linear_to_srgb(textureSampleGrad(material_textures, material_sampler, uv2, normal_layer, ddx2, ddy2).xyz);
    var n3 = linear_to_srgb(textureSampleGrad(material_textures, material_sampler, uv3, normal_layer, ddx3, ddy3).xyz);
    let strength = water_type.normal_strength;
    n1 = -vec3<f32>((n1.x * 2.0 - 1.0) * strength, n1.z, (n1.y * 2.0 - 1.0) * strength);
    n2 = -vec3<f32>((n2.x * 2.0 - 1.0) * strength, n2.z, (n2.y * 2.0 - 1.0) * strength);
    n3 = -vec3<f32>((n3.x * 2.0 - 1.0) * strength, n3.z, (n3.y * 2.0 - 1.0) * strength);
    let normals = normalize(n1 + n2 + n3);
    var foam_mask = 0.0;
    if (env.water_layers.y >= 0) {
        foam_mask = textureSampleGrad(material_textures, material_sampler, uv_foam, env.water_layers.y, ddx_foam, ddy_foam).r;
    }

    let light_dir = env.light_dir.xyz;
    let light_dot_normals = dot(normals, light_dir);
    let down_dot_normals = -normals.y;
    let view_dot_normals = dot(view_dir, normals);

    let distortion = uv_flow * 0.00075;
    let shadow = sample_shadow_map(position, distortion, vec3<f32>(0.0), false, false);
    let inverse_shadow = 1.0 - shadow;

    // Lighting.
    let ambient_out = env.ambient_color.rgb * env.ambient_color.w;
    let dir_light = env.light_color.rgb * env.light_color.w * inverse_shadow;
    let light_out = max(light_dot_normals, 0.0) * dir_light;
    let reflect_dir = reflect(-light_dir, normals);
    let light_specular_out = dir_light * specular_term(view_dir, reflect_dir, water_type.specular_gloss, water_type.specular_strength);
    let sky_out = max(down_dot_normals, 0.0) * env.fog_color.rgb * 0.5;
    let lightning_out = max(down_dot_normals, 0.0) * vec3<f32>(1.0) * env.misc.x;
    let underglow_out = env.underglow.rgb * max(normals.y, 0.0) * env.underglow.w;

    // Fresnel reflection of the sky, approximated by three water colours.
    let base_opacity = 0.4;
    let fresnel = 1.0 - clamp(view_dot_normals, 0.0, 1.0);
    var final_fresnel = clamp(mix(base_opacity, 1.0, fresnel * 1.2), 0.0, 1.0);
    var surface_color: vec3<f32>;
    if (final_fresnel < 0.5) {
        surface_color = mix(env.water_dark.rgb, env.water_mid.rgb, final_fresnel * 2.0);
    } else {
        surface_color = mix(env.water_mid.rgb, env.water_light.rgb, (final_fresnel - 0.5) * 2.0);
    }
    let surface_color_out = surface_color * max(water_type.specular_strength, 0.2);

    let composite_light = ambient_out + light_out + light_specular_out + sky_out + lightning_out
        + underglow_out + surface_color_out;

    var base_color = water_type.surface_color.rgb * composite_light;
    base_color = mix(base_color, surface_color, water_type.fresnel_amount);
    if (water_type.fresnel_amount == 0.85) {
        // Sailing hack.
        base_color = base_color * 0.75;
    }

    // Foam along the shoreline.
    let shore_line_mask = 1.0 - dot(blend, shore_corners / 127.0);
    let max_foam_amount = 0.8;
    var foam_amount = min(shore_line_mask, max_foam_amount);
    let foam_distance = 0.7;
    var foam_color = water_type.foam_color.rgb * foam_mask * composite_light;
    let foam_edge = 1.0 - (1.0 - foam_amount) / foam_distance;
    foam_amount = clamp(foam_edge * foam_edge * foam_edge, 0.0, 1.0) * f32(water_type.has_foam);
    foam_amount = foam_amount * foam_color.r;
    base_color = mix(base_color, foam_color, foam_amount);
    let specular_composite = mix(light_specular_out, vec3<f32>(0.0), foam_amount);
    let flat_fresnel = 1.0 - dot(view_dir, vec3<f32>(0.0, -1.0, 0.0));
    final_fresnel = max(final_fresnel, flat_fresnel);
    base_color = base_color + light_specular_out / 3.0;

    var alpha = max(water_type.base_opacity, max(foam_amount, max(final_fresnel, length(specular_composite / 3.0))));
    if (water_type.is_flat != 0u) {
        base_color = mix(water_type.depth_color.rgb, base_color, alpha);
        alpha = 1.0;
    }
    return vec4<f32>(base_color, alpha);
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
    let is_water = is_terrain && water_type > 0u && !is_underwater;
    shadow_frag = input.clip.xy;

    if (is_water) {
        // The water surface: shaded by `sample_water`, fogged and blended over what is below.
        let water_view = normalize(env.camera.xyz - input.position);
        let shore = vec3<f32>(
            f32(input.hsl.x & 127u),
            f32(input.hsl.y & 127u),
            f32(input.hsl.z & 127u),
        );
        let surface = sample_water(water_type, water_view, input.position, input.uv, blend, shore);
        let fog = combined_fog_amount(input.position);
        var water_rgb = mix(surface.rgb, linear_to_srgb(env.fog_color.rgb), fog);
        let water_alpha = fog + surface.a * (1.0 - fog);
        water_rgb = clamp(water_rgb, vec3<f32>(0.0), vec3<f32>(1.0));
        return vec4<f32>(water_rgb, water_alpha);
    }

    let time = f32(globals.tick) * 0.02;
    var blended_uv = input.uv;
    if ((flags & FLAG_VANILLA_UVS) != 0u && has_flag(material1, 1u)) {
        blended_uv.x = clamp(blended_uv.x, 0.0, 0.984375);
    }
    var uv1 = corner_uv(material1, blended_uv, time);
    var uv2 = corner_uv(material2, blended_uv, time);
    var uv3 = corner_uv(material3, blended_uv, time);

    // RLHD uses the first corner material as the single flow map for the whole fragment, then
    // applies the same distortion to all three material UVs before any diffuse/normal/PBR sample.
    // Keep derivatives outside the conditional so WebGPU's uniformity rules are satisfied even
    // when the selected material index varies between primitives.
    var flow_uv = uv1 - animation_frame(material1.flow_map_duration, time);
    var flow_strength = material1.flow_map_strength;
    if (is_underwater) {
        // Distort underwater textures with a world-anchored flow.
        flow_uv = world_uvs(input.position, 1.5)
            + water_animation(10.0 * water_types[water_type].duration) * vec2<f32>(1.0, -1.0);
        flow_strength = 0.075;
    }
    let flow_ddx = dpdx(flow_uv);
    let flow_ddy = dpdy(flow_uv);
    if (material1.flow_map >= 0 && flow_strength != 0.0) {
        let flow = textureSampleGrad(
            material_textures,
            material_sampler,
            flow_uv,
            material1.flow_map,
            flow_ddx,
            flow_ddy,
        ).xy;
        let flow_offset = flow * flow_strength;
        uv1 = uv1 + flow_offset;
        uv2 = uv2 + flow_offset;
        uv3 = uv3 + flow_offset;
    }

    let view_dir = normalize(env.camera.xyz - input.position);
    // WebGPU window y points down (OpenGL's points up), so the cross product is reversed.
    let flat_normal = normalize(cross(dpdx(input.position), -dpdy(input.position)));
    var n = normalize(input.normal);
    if ((flags & FLAG_FLAT_NORMALS) != 0u) {
        n = flat_normal;
    }
    let tbn = cotangent_frame(n, input.position, input.uv * -1.0);

    // RLHD parallax occlusion mapping: walk each corner material's height map in tangent space,
    // then average the three tangent-space fragment offsets. The unmodified derivatives are passed
    // explicitly to the height samples; final derivatives are taken after POM for all later maps.
    let pom_ddx1 = dpdx(uv1);
    let pom_ddy1 = dpdy(uv1);
    let pom_ddx2 = dpdx(uv2);
    let pom_ddy2 = dpdy(uv2);
    let pom_ddx3 = dpdx(uv3);
    let pom_ddy3 = dpdy(uv3);
    var frag_position = input.position;
    if (env.flags.w != 0u) {
        let ts_view_dir = transpose(tbn) * view_dir;
        let pom1 = sample_displacement_map(material1, ts_view_dir, uv1, pom_ddx1, pom_ddy1);
        let pom2 = sample_displacement_map(material2, ts_view_dir, uv2, pom_ddx2, pom_ddy2);
        let pom3 = sample_displacement_map(material3, ts_view_dir, uv3, pom_ddx3, pom_ddy3);
        uv1 = pom1.uv;
        uv2 = pom2.uv;
        uv3 = pom3.uv;
        var frag_delta = (pom1.delta + pom2.delta + pom3.delta) / 3.0;
        // Prevent the displaced surface from casting the original flat surface onto itself.
        frag_delta.z = max(0.0, frag_delta.z);
        frag_position = frag_position + tbn * frag_delta;
    }

    // Derivatives up front: the texture reads below sit in non-uniform control flow.
    let ddx1 = dpdx(uv1);
    let ddy1 = dpdy(uv1);
    let ddx2 = dpdx(uv2);
    let ddy2 = dpdy(uv2);
    let ddx3 = dpdx(uv3);
    let ddy3 = dpdy(uv3);

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

    // Directional shadow of the shadow map.
    var shadow = 0.0;
    if ((flags & FLAG_NO_SHADOW) == 0u) {
        shadow = sample_shadow_map(
            frag_position,
            vec2<f32>(0.0),
            flat_normal,
            true,
            (flags & FLAG_SHADOW_NORMAL_BIAS) != 0u,
        );
    }
    let inverse_shadow = 1.0 - shadow;
    if (env.flags.y == 6u) {
        return vec4<f32>(vec3<f32>(inverse_shadow), 1.0);
    }
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
    var dir_light = env.light_color.rgb * env.light_color.w;
    // Caustics in an underwater environment light everything with the directional light.
    if (env.flags.z != 0u && caustics_enabled()) {
        let caustics_uv = world_uvs(input.position, 12.8);
        let direction = vec2<f32>(1.0, -1.0);
        let drift = water_animation(231.0) * vec2<f32>(1.0, -2.0);
        let flow1 = caustics_uv + water_animation(19.0) * direction + drift;
        let flow2 = caustics_uv * 1.25 + water_animation(37.0) * -direction + drift;
        let caustics = vec3<f32>(sample_caustics_channel(flow1, flow2, dpdx(caustics_uv), dpdy(caustics_uv))) * 2.0;
        let caustics_color = env.caustics.rgb * env.caustics.w;
        dir_light = dir_light + caustics * caustics_color * light_dot_normals * pow(env.light_color.w, 1.5);
    }
    dir_light = dir_light * inverse_shadow;
    let light_out = max(light_dot_normals, 0.0) * dir_light;
    let reflect_dir = reflect(-light_dir, normals);
    let v_dot_r = clamp(dot(view_dir, reflect_dir), 1e-10, 1.0);
    let specular = dot(pow(vec3<f32>(v_dot_r), spec_gloss) * spec_strength, blend);
    let light_specular_out = dir_light * specular;

    // Sky light from above, tinted by the fog colour.
    let sky_out = max(up_dot_normals, 0.0) * env.fog_color.rgb * 0.5;

    // Lightning flashes light surfaces facing up.
    let lightning_out = max(up_dot_normals, 0.0) * vec3<f32>(0.25) * env.misc.x;

    // Underglow lights surfaces facing down (`normals.y` is positive downwards).
    let underglow_out = env.underglow.rgb * max(normals.y, 0.0) * env.underglow.w;

    let composite = ambient_out + light_out + light_specular_out + sky_out + lightning_out
        + underglow_out;
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
        rgb = sample_underwater(rgb, water_types[water_type], input.position, water_depth, light_dot_normals);
    }

    // Fog: distance fog and the ground fog layer, fading into the sky colour.
    if (!is_underwater) {
        rgb = mix(rgb, linear_to_srgb(env.fog_color.rgb), combined_fog_amount(input.position));
    }
    // `applyColorAdjustments` clamps to the displayable range.
    rgb = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(rgb, output_color.a);
}
