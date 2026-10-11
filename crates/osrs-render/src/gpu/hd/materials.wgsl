// Material table entry: the WGSL view of `layout::GpuMaterial` (one `MaterialStruct` of the 117 HD
// `UBOMaterials`). Layer fields index the material texture array, `-1` meaning "no texture".
// Ported from the 117 HD plugin. Copyright (c) 2018, Adam <Adam@sigterm.info>; (c) 2021, 117 and
// Hooder (BSD 2-Clause, see THIRD_PARTY.md).

struct Material {
    color_map: i32,
    normal_map: i32,
    displacement_map: i32,
    roughness_map: i32,
    ambient_occlusion_map: i32,
    flow_map: i32,
    shadow_alpha_map: i32,
    // bit 0 has transparency, bit 1 unlit, bit 2 override base colour
    flags: u32,
    subsurface: f32,
    brightness: f32,
    displacement_scale: f32,
    specular_strength: f32,
    specular_gloss: f32,
    flow_map_strength: f32,
    flow_map_duration: vec2<f32>,
    scroll_duration: vec2<f32>,
    texture_scale: vec3<f32>,
};
