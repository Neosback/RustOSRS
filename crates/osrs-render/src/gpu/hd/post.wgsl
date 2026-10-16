// HD resolve pass: reads the HDR scene colour and writes the displayable image.
//
// Phase 0 placeholder: exposure plus a vignette that proves the pass runs. The plugin's tone
// mapping, colour filters and colour blindness (`utils/tone_mapping.glsl`, `color_filters.glsl`)
// replace the body in phase 8. The scene shader still outputs display-referred colour, so values
// are written as they are, exactly like the Reference profile's non-sRGB target.

struct Frame {
    exposure: f32,
    vignette: f32,
    width: f32,
    height: f32,
    time: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var hdr: texture_2d<f32>;

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
};

// Fullscreen triangle.
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOut {
    var out: VertexOut;
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    out.clip = vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    let texel = textureLoad(hdr, vec2<i32>(input.clip.xy), 0);
    var color = texel.rgb * frame.exposure;
    let uv = input.clip.xy / vec2<f32>(frame.width, frame.height);
    let edge = length(uv - vec2<f32>(0.5, 0.5)) * 1.41421356;
    color = theme_post(color, vec3<f32>(uv, 0.0), frame.time);
    color = color * (1.0 - frame.vignette * smoothstep(0.4, 1.0, edge));
    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), clamp(texel.a, 0.0, 1.0));
}
