// Editor line overlay (grid lines, bounds): depth-tested against the scene, no depth writes,
// alpha blended. Positions are render-local world units. A small clip-space bias pulls the lines
// toward the camera so they win over the terrain they are draped on.

struct Globals {
    world_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexIn) -> VertexOut {
    var out: VertexOut;
    var clip = globals.world_proj * vec4<f32>(input.position, 1.0);
    clip.z = clip.z + 8.0 / 128.0;
    out.clip = clip;
    out.color = input.color;
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    return input.color;
}
