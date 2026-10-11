from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one match, found {count}\n--- pattern ---\n{old}")
    file.write_text(text.replace(old, new, 1))


shader = "crates/osrs-render/src/gpu/hd/scene.wgsl"

replace_once(
    shader,
    """fn corner_uv(material: Material, base: vec2<f32>, time: f32) -> vec2<f32> {
    var uv = base + material.scroll_duration * time;
    uv = (uv - vec2<f32>(0.5)) * material.texture_scale.xy + vec2<f32>(0.5);
    return uv;
}
""",
    """fn corner_uv(material: Material, base: vec2<f32>, time: f32) -> vec2<f32> {
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
""",
)

replace_once(
    shader,
    """    let uv1 = corner_uv(material1, blended_uv, time);
    let uv2 = corner_uv(material2, blended_uv, time);
    let uv3 = corner_uv(material3, blended_uv, time);
    // Derivatives up front: the texture reads below sit in non-uniform control flow.
    let ddx1 = dpdx(uv1);
    let ddy1 = dpdy(uv1);
    let ddx2 = dpdx(uv2);
    let ddy2 = dpdy(uv2);
    let ddx3 = dpdx(uv3);
    let ddy3 = dpdy(uv3);
""",
    """    var uv1 = corner_uv(material1, blended_uv, time);
    var uv2 = corner_uv(material2, blended_uv, time);
    var uv3 = corner_uv(material3, blended_uv, time);

    // RLHD uses the first corner material as the single flow map for the whole fragment, then
    // applies the same distortion to all three material UVs before any diffuse/normal/PBR sample.
    // Keep derivatives outside the conditional so WebGPU's uniformity rules are satisfied even
    // when the selected material index varies between primitives.
    let flow_uv = uv1 - animation_frame(material1.flow_map_duration, time);
    let flow_ddx = dpdx(flow_uv);
    let flow_ddy = dpdy(flow_uv);
    if (material1.flow_map >= 0 && material1.flow_map_strength != 0.0) {
        let flow = textureSampleGrad(
            material_textures,
            material_sampler,
            flow_uv,
            material1.flow_map,
            flow_ddx,
            flow_ddy,
        ).xy;
        let flow_offset = flow * material1.flow_map_strength;
        uv1 = uv1 + flow_offset;
        uv2 = uv2 + flow_offset;
        uv3 = uv3 + flow_offset;
    }

    // Derivatives up front: the texture reads below sit in non-uniform control flow.
    let ddx1 = dpdx(uv1);
    let ddy1 = dpdy(uv1);
    let ddx2 = dpdx(uv2);
    let ddy2 = dpdy(uv2);
    let ddx3 = dpdx(uv3);
    let ddy3 = dpdy(uv3);
""",
)

# Keep a narrow regression gate that checks the full scene shader still has the RLHD flow stage in
# front of the material sampling derivatives. SceneRenderer construction in the other test in this
# file also validates the resulting WGSL/pipeline on a real headless adapter when one is available.
test = "crates/osrs-render/tests/hd_material_binding.rs"
text = Path(test).read_text()
if "hd_scene_shader_keeps_flow_map_stage_before_material_sampling" in text:
    raise SystemExit("flow-map regression test already exists")
text += r'''

#[test]
fn hd_scene_shader_keeps_flow_map_stage_before_material_sampling() {
    let shader = include_str!("../src/gpu/hd/scene.wgsl");
    let flow = shader
        .find("let flow_offset = flow * material1.flow_map_strength;")
        .expect("HD scene shader must apply the RLHD material flow map");
    let material_derivatives = shader
        .find("let ddx1 = dpdx(uv1);")
        .expect("HD scene shader must derive the final material UVs");
    let diffuse = shader
        .find("let tex1 = sample_color(material1, uv1, ddx1, ddy1);")
        .expect("HD scene shader must sample material 1 with the final UV");

    assert!(flow < material_derivatives);
    assert!(material_derivatives < diffuse);
    assert!(shader.contains("material1.flow_map >= 0"));
}
'''
Path(test).write_text(text)
