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
@fragment
fn fs_sky(input: SkyVertex) -> @location(0) vec4<f32> {
    let direction = normalize(env.sky_forward.xyz + input.uv.x * env.sky_right.xyz + input.uv.y * env.sky_down.xyz);
    let color = theme_sky(linear_to_srgb(env.fog_color.rgb), direction, env.camera.w);
    return vec4<f32>(max(color, vec3<f32>(0.0)), 1.0);
}
