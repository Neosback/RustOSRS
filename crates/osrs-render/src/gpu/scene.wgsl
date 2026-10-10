// Reference-profile scene shader: a WGSL port of RuneLite's `vert.glsl` / `frag.glsl` /
// `hsl_to_rgb.glsl` without fog, textures, tint, or colorblind modes (those arrive in M12).
//
// Depth is reverse-Z with no far plane: the projection matrix produces clip.z = 2n and clip.w =
// camera distance, so depth = 2n / distance. Authored face bias is added to clip.z before the
// perspective divide, exactly like `screenPos.z += bias / 128.0`.

struct Globals {
    world_proj: mat4x4<f32>,
    brightness: f32,
    smooth_banding: f32,
    // Texture animation clock: the client's `gameCycle & 127`.
    tick: u32,
    _pad: f32,
};

struct TextureAnimations {
    // `.xy` = per-tick UV scroll (units of 1/128 texture) for texture `index`.
    values: array<vec4<f32>, 256>,
};

struct Zone {
    base: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var textures: texture_2d_array<f32>;
@group(0) @binding(2) var tex_sampler: sampler;
@group(0) @binding(3) var<uniform> animations: TextureAnimations;
@group(1) @binding(0) var<uniform> zone: Zone;

struct VertexIn {
    @location(0) position: vec4<i32>,
    @location(1) abhsl: u32,
    @location(2) tex_uv: vec4<u32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) alpha: f32,
    @location(1) @interpolate(linear, centroid) hsl: f32,
    @location(2) @interpolate(flat) tex: u32,
    @location(3) uv: vec2<f32>,
    // Vertex RGB (`fColor.rgb`), interpolated perspective-correct.
    @location(4) color: vec3<f32>,
};

fn hsl_to_rgb(hsl: vec3<f32>) -> vec3<f32> {
    let hue = hsl.x / 64.0 + 0.0078125;
    let sat = hsl.y / 8.0 + 0.0625;
    let lum = hsl.z;

    let var11 = lum / 128.0;
    var var19: f32;
    if (var11 < 0.5) {
        var19 = var11 * (1.0 + sat);
    } else {
        var19 = var11 + sat - var11 * sat;
    }
    let var21 = 2.0 * var11 - var19;
    var var23 = hue + 0.3333333333333333;
    if (var23 > 1.0) {
        var23 = var23 - 1.0;
    }
    var var27 = hue - 0.3333333333333333;
    if (var27 < 0.0) {
        var27 = var27 + 1.0;
    }

    var r: f32;
    if (6.0 * var23 < 1.0) {
        r = var21 + (var19 - var21) * 6.0 * var23;
    } else if (2.0 * var23 < 1.0) {
        r = var19;
    } else if (3.0 * var23 < 2.0) {
        r = var21 + (var19 - var21) * (0.6666666666666666 - var23) * 6.0;
    } else {
        r = var21;
    }

    var g: f32;
    if (6.0 * hue < 1.0) {
        g = var21 + (var19 - var21) * 6.0 * hue;
    } else if (2.0 * hue < 1.0) {
        g = var19;
    } else if (3.0 * hue < 2.0) {
        g = var21 + (var19 - var21) * (0.6666666666666666 - hue) * 6.0;
    } else {
        g = var21;
    }

    var b: f32;
    if (6.0 * var27 < 1.0) {
        b = var21 + (var19 - var21) * 6.0 * var27;
    } else if (2.0 * var27 < 1.0) {
        b = var19;
    } else if (3.0 * var27 < 2.0) {
        b = var21 + (var19 - var21) * (0.6666666666666666 - var27) * 6.0;
    } else {
        b = var21;
    }

    let e = globals.brightness;
    return vec3<f32>(pow(r, e), pow(g, e), pow(b, e));
}

@vertex
fn vs_main(input: VertexIn) -> VertexOut {
    var out: VertexOut;
    let vert = vec4<f32>(vec3<f32>(input.position.xyz) + zone.base.xyz, 1.0);
    let a = f32((input.abhsl >> 24u) & 0xffu) / 255.0;
    let bias = f32((input.abhsl >> 16u) & 0xffu);

    var screen = globals.world_proj * vert;
    screen.z = screen.z + bias / 128.0;
    out.clip = screen;
    out.alpha = 1.0 - a;
    out.hsl = f32(input.abhsl & 0xffffu);
    out.color = hsl_to_rgb(vec3<f32>(
        f32((input.abhsl >> 10u) & 63u),
        f32((input.abhsl >> 7u) & 7u),
        f32(input.abhsl & 127u),
    ));

    // Texture id + 1 (0 = untextured) and signed 8.8 UVs packed as u16 pairs.
    let tex = input.tex_uv.x;
    let u = f32(i32(input.tex_uv.y) - select(0, 65536, input.tex_uv.y >= 32768u));
    let v = f32(i32(input.tex_uv.z) - select(0, 65536, input.tex_uv.z >= 32768u));
    var uv = vec2<f32>(u, v) / 256.0;
    if (tex > 0u) {
        let anim = animations.values[min(tex - 1u, 255u)].xy;
        uv = uv + f32(globals.tick) * anim * (1.0 / 128.0);
    }
    out.tex = tex;
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    // Derivatives must be computed in uniform control flow, before the texture branch.
    let dx = dpdx(input.uv);
    let dy = dpdy(input.uv);

    if (input.tex > 0u) {
        let layer = i32(input.tex) - 1;
        let level0 = textureSampleLevel(textures, tex_sampler, input.uv, layer, 0.0);
        if (level0.a < 1.0) {
            discard;
        }
        let sampled = textureSampleGrad(textures, tex_sampler, input.uv, layer, dx, dy);
        let rgb = pow(sampled.rgb, vec3<f32>(globals.brightness));
        // Textured faces carry a 7-bit baked lightness (`fHsl / 127`).
        let light = input.hsl / 127.0;
        return vec4<f32>(rgb * light, input.alpha);
    }

    let packed = i32(input.hsl);
    let hsl = vec3<f32>(f32((packed >> 10) & 63), f32((packed >> 7) & 7), f32(packed & 127));
    // `smoothBanding` is 0 when banding is removed (vertex RGB) and 1 for per-pixel HSL.
    let rgb = mix(input.color, hsl_to_rgb(hsl), globals.smooth_banding);
    return vec4<f32>(rgb, input.alpha);
}
