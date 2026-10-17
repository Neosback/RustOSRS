// Engine-owned sky entry points. Directions remain stable under camera translation and rebasing.
struct SkyVertex { @builtin(position) clip: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex
fn vs_sky(@builtin(vertex_index) index: u32) -> SkyVertex {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    var out: SkyVertex;
    out.clip = vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(out.clip.x, -out.clip.y);
    return out;
}

// Fade across the perceived horizon, matching 117HD celestialHorizonFade
fn celestial_horizon_fade(view_up: f32, body_up: f32) -> f32 {
    let horizon_offset = 0.05;
    return smoothstep(-0.09, 0.04, view_up + horizon_offset) *
        smoothstep(-0.0697, -0.0087, body_up);
}

@fragment
fn fs_sky(input: SkyVertex) -> @location(0) vec4<f32> {
    let direction = normalize(env.sky_forward.xyz + input.uv.x * env.sky_right.xyz + input.uv.y * env.sky_down.xyz);
    let fog_srgb = linear_to_srgb(env.fog_color.rgb);
    var base_color = theme_sky(fog_srgb, direction, env.camera.w);

    // If sky gradient is enabled, blend from horizon fog color to zenith color
    let up_amount = -direction.y;
    let zenith_srgb = linear_to_srgb(env.sky_zenith.rgb);
    let horizon_width = max(env.sky_zenith.w, 0.1);
    let grad_factor = clamp(up_amount / horizon_width, 0.0, 1.0);
    base_color = mix(base_color, zenith_srgb, grad_factor * 0.7);

    // Sun disk calculation (matching 117HD sun sampling)
    var sun_out = vec3<f32>(0.0);
    if (env.sun_dir.w > 0.0) {
        let sun_dot = dot(direction, env.sun_dir.xyz);
        let sun_edge = 0.99945; // acos(0.99945) ~ 1.9 degrees
        let sun_aa = max(fwidth(sun_dot), 1e-6);
        let sun_disk = smoothstep(sun_edge - sun_aa, sun_edge + sun_aa, sun_dot);
        let horizon_fade = celestial_horizon_fade(up_amount, env.sun_dir.y);
        let sun_intensity = env.sun_color.w * 15.0;
        sun_out = env.sun_color.rgb * sun_intensity * sun_disk * horizon_fade;
    }

    // Moon disk calculation (matching 117HD moon sampling)
    var moon_out = vec3<f32>(0.0);
    if (env.moon_color.w > 0.0) {
        let moon_dot = dot(direction, env.moon_dir.xyz);
        let moon_edge = 0.9996; // ~1.6 degrees
        let moon_aa = max(fwidth(moon_dot), 1e-6);
        let moon_disk = smoothstep(moon_edge - moon_aa, moon_edge + moon_aa, moon_dot);
        let horizon_fade = celestial_horizon_fade(up_amount, env.moon_dir.y);
        let moon_intensity = env.moon_color.w * 3.0;
        moon_out = env.moon_color.rgb * moon_intensity * moon_disk * horizon_fade;
    }

    let color = base_color + sun_out + moon_out;
    return vec4<f32>(max(color, vec3<f32>(0.0)), 1.0);
}
