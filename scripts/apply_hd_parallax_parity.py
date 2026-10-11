from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one match, found {count}\n--- pattern ---\n{old}")
    file.write_text(text.replace(old, new, 1))


# Renderer-facing setting. RLHD exposes this separately from normal mapping and defaults it on.
replace_once(
    "crates/osrs-render/src/gpu/profile.rs",
    """    pub normal_mapping: bool,
    /// Animation clock of the water, caustics and scrolling textures, in seconds.
""",
    """    pub normal_mapping: bool,
    /// Parallax occlusion mapping for materials with displacement maps (RLHD default: on).
    pub parallax_mapping: bool,
    /// Animation clock of the water, caustics and scrolling textures, in seconds.
""",
)
replace_once(
    "crates/osrs-render/src/gpu/profile.rs",
    """            normal_mapping: true,
            elapsed_seconds: 0.0,
""",
    """            normal_mapping: true,
            parallax_mapping: true,
            elapsed_seconds: 0.0,
""",
)

# Carry the setting through the existing environment uniform without growing the GPU layout.
replace_once(
    "crates/osrs-render/src/gpu/hd/layout.rs",
    """    /// `x`: normal mapping enabled, `y`: debug view, `z`: underwater environment.
    pub flags: [u32; 4],
""",
    """    /// `x`: normal mapping enabled, `y`: debug view, `z`: underwater environment,
    /// `w`: parallax occlusion mapping enabled.
    pub flags: [u32; 4],
""",
)
replace_once(
    "crates/osrs-render/src/gpu/mod.rs",
    """            flags: [
                u32::from(hd.normal_mapping),
                hd.debug_view,
                u32::from(lighting.underwater_environment),
                0,
            ],
""",
    """            flags: [
                u32::from(hd.normal_mapping),
                hd.debug_view,
                u32::from(lighting.underwater_environment),
                u32::from(hd.parallax_mapping),
            ],
""",
)

# Persist and expose the plugin-equivalent toggle in the editor.
replace_once(
    "crates/osrs-editor/src/settings.rs",
    """    /// HD: normal mapping on materials that have a normal map.
    pub hd_normal_mapping: bool,
    /// HD: per-area environments (lighting, fog, sky and water colour; Properties > Shading).
""",
    """    /// HD: normal mapping on materials that have a normal map.
    pub hd_normal_mapping: bool,
    /// HD: parallax occlusion mapping on materials that have displacement maps.
    pub hd_parallax_mapping: bool,
    /// HD: per-area environments (lighting, fog, sky and water colour; Properties > Shading).
""",
)
replace_once(
    "crates/osrs-editor/src/settings.rs",
    """            hd_normal_mapping: true,
            hd_environment: EnvironmentSettings::default(),
""",
    """            hd_normal_mapping: true,
            hd_parallax_mapping: true,
            hd_environment: EnvironmentSettings::default(),
""",
)
replace_once(
    "crates/osrs-editor/src/settings.rs",
    """        assert!(settings.properties_panel);
        Ok(())
""",
    """        assert!(settings.properties_panel);
        assert!(settings.hd_parallax_mapping);
        Ok(())
""",
)
replace_once(
    "crates/osrs-editor/src/app/hd.rs",
    """            normal_mapping: self.prefs.hd_normal_mapping,
            elapsed_seconds: (elapsed % 4096.0) as f32,
""",
    """            normal_mapping: self.prefs.hd_normal_mapping,
            parallax_mapping: self.prefs.hd_parallax_mapping,
            elapsed_seconds: (elapsed % 4096.0) as f32,
""",
)
replace_once(
    "crates/osrs-editor/src/app/properties.rs",
    """            ui.checkbox(&mut self.prefs.hd_normal_mapping, "Normal mapping")
                .on_hover_text("Affects how light interacts with certain materials.");
            ui.checkbox(
""",
    """            ui.checkbox(&mut self.prefs.hd_normal_mapping, "Normal mapping")
                .on_hover_text("Affects how light interacts with certain materials.");
            ui.checkbox(
                &mut self.prefs.hd_parallax_mapping,
                "Parallax occlusion mapping",
            )
            .on_hover_text("Adds depth to materials with displacement maps at higher GPU cost.");
            ui.checkbox(
""",
)
replace_once(
    "crates/osrs-editor/src/app/properties.rs",
    """            "HD is a work in progress: materials, normal maps, specular, ground blending and \\
             area environments and shadows are in. Water, lights and the sky are still to come. \\
             Animated objects still use Reference shading.",
""",
    """            "HD is a work in progress: materials, normal maps, parallax displacement, specular, \\
             ground blending, animated-object HD materials, area environments and shadows are in. \\
             Water, dynamic lights and the full sky are still to come.",
""",
)

shader = "crates/osrs-render/src/gpu/hd/scene.wgsl"
replace_once(
    shader,
    """// (zone renderer), phase 2 subset: per-corner materials with ground blending, diffuse + normal map +
// specular, vertex-colour and underlay/overlay blending, tangent frame from derivatives, underwater
""",
    """// (zone renderer), phase 2 subset: per-corner materials with ground blending, diffuse + normal map +
// parallax displacement + specular, vertex-colour and underlay/overlay blending, tangent frame from
// derivatives, underwater
""",
)

# Port the normal (non-self-shadowing) RLHD displacement.glsl path. The explicit gradients keep the
# texture reads legal in WebGPU's control-flow model while preserving stable mip selection.
replace_once(
    shader,
    """fn animation_frame(duration: vec2<f32>, time: f32) -> vec2<f32> {
    var frame = vec2<f32>(0.0);
    if (duration.x != 0.0) {
        frame.x = fract(time / duration.x);
    }
    if (duration.y != 0.0) {
        frame.y = fract(time / duration.y);
    }
    return frame;
}

@fragment
""",
    """fn animation_frame(duration: vec2<f32>, time: f32) -> vec2<f32> {
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

@fragment
""",
)

old_stage = """    // Derivatives up front: the texture reads below sit in non-uniform control flow.
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

"""
new_stage = """    let view_dir = normalize(env.camera.xyz - input.position);
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

"""
replace_once(shader, old_stage, new_stage)
replace_once(
    shader,
    """        shadow = sample_shadow_map(
            input.position,
""",
    """        shadow = sample_shadow_map(
            frag_position,
""",
)

# Keep the source-order gate lint-clean and add a POM gate. SceneRenderer construction in the GPU
# test still compiles the complete WGSL pipeline on an available headless adapter.
test = "crates/osrs-render/tests/hd_material_binding.rs"
text = Path(test).read_text()
text = text.replace(
    '''    let flow = shader
        .find("let flow_offset = flow * material1.flow_map_strength;")
        .expect("HD scene shader must apply the RLHD material flow map");
    let material_derivatives = shader
        .find("let ddx1 = dpdx(uv1);")
        .expect("HD scene shader must derive the final material UVs");
    let diffuse = shader
        .find("let tex1 = sample_color(material1, uv1, ddx1, ddy1);")
        .expect("HD scene shader must sample material 1 with the final UV");
''',
    '''    let flow = shader.find("let flow_offset = flow * material1.flow_map_strength;");
    let material_derivatives = shader.find("let ddx1 = dpdx(uv1);");
    let diffuse = shader.find("let tex1 = sample_color(material1, uv1, ddx1, ddy1);");
    let (Some(flow), Some(material_derivatives), Some(diffuse)) =
        (flow, material_derivatives, diffuse)
    else {
        panic!("HD scene shader must keep flow-map distortion ahead of final material sampling");
    };
''',
    1,
)
if "hd_scene_shader_applies_parallax_before_sampling_and_uses_displaced_shadows" in text:
    raise SystemExit("parallax regression test already exists")
text += r'''

#[test]
fn hd_scene_shader_applies_parallax_before_sampling_and_uses_displaced_shadows() {
    let shader = include_str!("../src/gpu/hd/scene.wgsl");
    let pom = shader.find("let pom1 = sample_displacement_map(");
    let material_derivatives = shader.find("let ddx1 = dpdx(uv1);");
    let diffuse = shader.find("let tex1 = sample_color(material1, uv1, ddx1, ddy1);");
    let displaced_shadow = shader.find("sample_shadow_map(\n            frag_position,");
    let (Some(pom), Some(material_derivatives), Some(diffuse), Some(displaced_shadow)) =
        (pom, material_derivatives, diffuse, displaced_shadow)
    else {
        panic!("HD scene shader must apply POM before sampling and shadow the displaced fragment");
    };

    assert!(shader.contains("fn sample_displacement_map("));
    assert!(shader.contains("material.displacement_map < 0"));
    assert!(shader.contains("env.flags.w != 0u"));
    assert!(pom < material_derivatives);
    assert!(material_derivatives < diffuse);
    assert!(pom < displaced_shadow);
}
'''
Path(test).write_text(text)
