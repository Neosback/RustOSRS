//! Phase 0 of the 117 HD port on a real (headless) GPU: the profile switch, the HDR frame graph,
//! the material buffer layout and the material texture array.
//!
//! Tests skip themselves when no adapter is available.

use osrs_hd::HdData;
use osrs_render::gpu::{
    Colorblind, Fog, FrameParams, HdParams, ReferenceCamera, RenderProfile, SceneRenderer,
    hd::{
        layout::{
            GpuMaterial, MATERIAL_WGSL, MaterialTable, material_bind_group,
            material_bind_group_layout,
        },
        materials::{DEFAULT_RESOLUTION, DecodedLayers, LayerPlan, MaterialTextureArray},
    },
};
use std::path::PathBuf;

const CLEAR: [f64; 3] = [0.55, 0.7, 0.9];

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/117hd")
}

fn renderer() -> Option<SceneRenderer> {
    match SceneRenderer::new_headless() {
        Ok(renderer) => Some(renderer),
        Err(error) => {
            eprintln!("skipping: {error}");
            None
        }
    }
}

fn params(profile: RenderProfile) -> FrameParams {
    FrameParams {
        camera: ReferenceCamera {
            x: 0.0,
            y: -2000.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.75,
            scale: 512.0,
        },
        view_plane: 0,
        tick: 0,
        brightness: 0.8,
        remove_color_banding: true,
        clear_color: CLEAR,
        bright_textures: false,
        fog: Fog::default(),
        colorblind: Colorblind::default(),
        profile,
    }
}

fn pixel(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let index = ((y * width + x) * 4) as usize;
    [
        pixels[index],
        pixels[index + 1],
        pixels[index + 2],
        pixels[index + 3],
    ]
}

fn read_buffer(
    device: &wgpu::Device,
    buffer: &wgpu::Buffer,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let slice = buffer.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device.poll(wgpu::PollType::wait_indefinitely())?;
    receiver.recv()??;
    let data = slice.get_mapped_range()?.to_vec();
    buffer.unmap();
    Ok(data)
}

#[test]
fn reference_profile_clears_to_the_sky_colour() -> Result<(), Box<dyn std::error::Error>> {
    let Some(renderer) = renderer() else {
        return Ok(());
    };
    let pixels = renderer.render_to_rgba(params(RenderProfile::Reference), 64, 48)?;
    let expected = CLEAR.map(|value| (value * 255.0).round() as u8);
    for (x, y) in [(0, 0), (32, 24), (63, 47)] {
        // Float to unorm conversion rounds differently between GPUs: allow one code.
        for (got, want) in pixel(&pixels, 64, x, y).iter().zip(expected) {
            assert!(got.abs_diff(want) <= 1, "{got} vs {want}");
        }
    }
    Ok(())
}

#[test]
fn hd_profile_runs_the_resolve_pass() -> Result<(), Box<dyn std::error::Error>> {
    let Some(mut renderer) = renderer() else {
        return Ok(());
    };
    let reference = renderer.render_to_rgba(params(RenderProfile::Reference), 64, 48)?;

    // Neutral resolve settings reproduce the Reference image through the HDR target.
    renderer.set_hd_params(HdParams {
        exposure: 1.0,
        placeholder_vignette: 0.0,
    });
    let neutral = renderer.render_to_rgba(params(RenderProfile::Hd), 64, 48)?;
    let max_delta = reference
        .iter()
        .zip(&neutral)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or(0);
    assert!(
        max_delta <= 1,
        "HDR round trip changed pixels by {max_delta}"
    );

    // The default placeholder vignette darkens the corners and leaves the centre alone.
    renderer.set_hd_params(HdParams::default());
    let vignette = renderer.render_to_rgba(params(RenderProfile::Hd), 64, 48)?;
    let centre = pixel(&vignette, 64, 32, 24);
    let corner = pixel(&vignette, 64, 0, 0);
    for (got, want) in centre.iter().zip(pixel(&reference, 64, 32, 24)) {
        assert!(got.abs_diff(want) <= 1, "centre {centre:?} vs reference");
    }
    assert!(
        corner[2] < centre[2] / 2 + centre[2] / 4,
        "corner {corner:?} not darker than centre {centre:?}"
    );

    // Resizing reallocates the transient targets.
    let resized = renderer.render_to_rgba(params(RenderProfile::Hd), 80, 40)?;
    assert_eq!(resized.len(), 80 * 40 * 4);
    Ok(())
}

#[test]
fn reference_frames_do_not_depend_on_hd_state() -> Result<(), Box<dyn std::error::Error>> {
    let Some(mut renderer) = renderer() else {
        return Ok(());
    };
    let before = renderer.render_to_rgba(params(RenderProfile::Reference), 64, 48)?;
    renderer.set_hd_params(HdParams {
        exposure: 3.0,
        placeholder_vignette: 1.0,
    });
    renderer.render_to_rgba(params(RenderProfile::Hd), 64, 48)?;
    let after = renderer.render_to_rgba(params(RenderProfile::Reference), 64, 48)?;
    assert_eq!(before, after);
    Ok(())
}

/// Uploads two materials, reads every field back through a compute shader that indexes them as the
/// WGSL `Material` struct, and compares with the Rust values.
#[test]
fn material_table_layout_matches_wgsl() -> Result<(), Box<dyn std::error::Error>> {
    let Some(renderer) = renderer() else {
        return Ok(());
    };
    let (device, queue) = (renderer.device(), renderer.queue());
    let mut first = GpuMaterial::NONE;
    first.color_map = 7;
    first.normal_map = 8;
    first.displacement_map = 9;
    first.roughness_map = 10;
    first.ambient_occlusion_map = 11;
    first.flow_map = 12;
    first.shadow_alpha_map = 13;
    first.flags = 0b101;
    first.subsurface = 0.25;
    first.brightness = 1.5;
    first.displacement_scale = 0.375;
    first.specular_strength = 0.5;
    first.specular_gloss = 30.0;
    first.flow_map_strength = 2.0;
    first.flow_map_duration = [3.0, 4.0];
    first.scroll_duration = [5.0, 6.0];
    first.texture_scale = [0.5, 0.25, 0.125];
    let mut second = first;
    second.color_map = -1;
    second.texture_scale = [8.0, 16.0, 32.0];
    second.scroll_duration = [-1.0, -2.0];
    let table = MaterialTable::new(device, &[first, second]);

    const FIELDS: usize = 24;
    let shader = format!(
        "{MATERIAL_WGSL}
@group(0) @binding(0) var<storage, read> materials: array<Material>;
@group(0) @binding(1) var<storage, read_write> out: array<f32>;
@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let m = materials[id.x];
    let o = id.x * {FIELDS}u;
    out[o + 0u] = f32(m.color_map);
    out[o + 1u] = f32(m.normal_map);
    out[o + 2u] = f32(m.displacement_map);
    out[o + 3u] = f32(m.roughness_map);
    out[o + 4u] = f32(m.ambient_occlusion_map);
    out[o + 5u] = f32(m.flow_map);
    out[o + 6u] = f32(m.shadow_alpha_map);
    out[o + 7u] = f32(m.flags);
    out[o + 8u] = m.subsurface;
    out[o + 9u] = m.brightness;
    out[o + 10u] = m.displacement_scale;
    out[o + 11u] = m.specular_strength;
    out[o + 12u] = m.specular_gloss;
    out[o + 13u] = m.flow_map_strength;
    out[o + 14u] = m.flow_map_duration.x;
    out[o + 15u] = m.flow_map_duration.y;
    out[o + 16u] = m.scroll_duration.x;
    out[o + 17u] = m.scroll_duration.y;
    out[o + 18u] = m.texture_scale.x;
    out[o + 19u] = m.texture_scale.y;
    out[o + 20u] = m.texture_scale.z;
}}"
    );
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("material-layout-test"),
        source: wgpu::ShaderSource::Wgsl(shader.into()),
    });
    let storage = |binding, read_only| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    };
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[storage(0, true), storage(1, false)],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        module: &module,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });
    let out_size = (2 * FIELDS * 4) as u64;
    let out = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: out_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: out_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: table.buffer().as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: out.as_entire_binding(),
            },
        ],
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(2, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&out, 0, &readback, 0, out_size);
    queue.submit(Some(encoder.finish()));
    let values: Vec<f32> = bytemuck_cast(&read_buffer(device, &readback)?);

    for (index, material) in [first, second].iter().enumerate() {
        let expected = [
            material.color_map as f32,
            material.normal_map as f32,
            material.displacement_map as f32,
            material.roughness_map as f32,
            material.ambient_occlusion_map as f32,
            material.flow_map as f32,
            material.shadow_alpha_map as f32,
            material.flags as f32,
            material.subsurface,
            material.brightness,
            material.displacement_scale,
            material.specular_strength,
            material.specular_gloss,
            material.flow_map_strength,
            material.flow_map_duration[0],
            material.flow_map_duration[1],
            material.scroll_duration[0],
            material.scroll_duration[1],
            material.texture_scale[0],
            material.texture_scale[1],
            material.texture_scale[2],
        ];
        assert_eq!(
            &values[index * FIELDS..index * FIELDS + expected.len()],
            expected
        );
    }

    // The bind group layout accepts a real table and texture array together.
    let decoded = DecodedLayers {
        size: 4,
        mip_levels: 3,
        layers: vec![None],
        missing: Vec::new(),
    };
    let array = MaterialTextureArray::upload(device, queue, &decoded)?;
    let _ = material_bind_group(device, &material_bind_group_layout(device), &table, &array);
    Ok(())
}

fn bytemuck_cast(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| f32::from_le_bytes(*chunk))
        .collect()
}

#[test]
fn material_texture_array_loads_the_vendored_textures() -> Result<(), Box<dyn std::error::Error>> {
    let Some(renderer) = renderer() else {
        return Ok(());
    };
    let (device, queue) = (renderer.device(), renderer.queue());
    let data = HdData::load(assets())?;
    let plan = LayerPlan::from_materials(&data.materials);
    assert!(plan.layers.len() > 100, "{} layers", plan.layers.len());
    assert!(plan.layers.len() <= device.limits().max_texture_array_layers as usize);

    let textures = assets().join("scene/textures");
    let decoded = DecodedLayers::load(&textures, &plan, DEFAULT_RESOLUTION)?;
    assert_eq!(decoded.layers.len(), plan.layers.len());
    assert_eq!(decoded.mip_levels, 9);
    // Every layer without an image file must be one the vanilla texture fills in later.
    let not_vanilla: Vec<_> = plan
        .layers
        .iter()
        .filter(|layer| layer.vanilla_index.is_none() && decoded.missing.contains(&layer.material))
        .map(|layer| layer.material.as_str())
        .collect();
    assert!(
        not_vanilla.len() <= 3,
        "textureless, non-vanilla layers: {not_vanilla:?}"
    );
    let loaded = decoded.layers.iter().flatten().count();
    assert!(loaded > 100, "{loaded} images decoded");

    let array = MaterialTextureArray::upload(device, queue, &decoded)?;
    assert_eq!(array.layer_count() as usize, plan.layers.len());
    assert_eq!(array.mip_levels(), 9);
    assert_eq!(array.size(), DEFAULT_RESOLUTION);

    // Read the first decoded layer back: base level and the 1x1 top of its mip chain.
    let (layer, chain) = decoded
        .layers
        .iter()
        .enumerate()
        .find_map(|(index, chain)| chain.as_ref().map(|chain| (index as u32, chain)))
        .ok_or("no decoded layer")?;
    for (mip, expected) in [(0_u32, &chain[0]), (8, &chain[8])] {
        let level = (DEFAULT_RESOLUTION >> mip).max(1);
        let row = (level * 4).div_ceil(256) * 256;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(row * level),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: array.texture(),
                mip_level: mip,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(level),
                },
            },
            wgpu::Extent3d {
                width: level,
                height: level,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(Some(encoder.finish()));
        let data = read_buffer(device, &buffer)?;
        let mut packed = Vec::new();
        for y in 0..level {
            let start = (y * row) as usize;
            packed.extend_from_slice(&data[start..start + (level * 4) as usize]);
        }
        assert_eq!(&packed, expected, "layer {layer} mip {mip}");
    }
    Ok(())
}
