//! End-to-end GPU gate for the HD scene material binding.
//!
//! This deliberately avoids cache/scene data: a material table entry points at one unmistakable
//! texture-array layer, the same group-2 layout used by the HD scene shader is bound, and a tiny
//! fragment shader samples through that material index. It catches table/layer/bind-group drift
//! that CPU-only material tests cannot see.

use osrs_render::gpu::{
    SceneRenderer,
    hd::{
        layout::{
            GpuMaterial, GpuWaterType, HdEnvironmentUniform, MATERIAL_WGSL, MaterialTable,
            point_lights_buffer, scene_bind_group, scene_bind_group_layout, water_types_buffer,
        },
        materials::{DecodedLayers, MaterialTextureArray, build_mips},
    },
};

fn renderer() -> Option<SceneRenderer> {
    match SceneRenderer::new_headless() {
        Ok(renderer) => Some(renderer),
        Err(error) => {
            eprintln!("skipping: {error}");
            None
        }
    }
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
fn scene_material_binding_samples_the_selected_texture_layer()
-> Result<(), Box<dyn std::error::Error>> {
    let Some(renderer) = renderer() else {
        return Ok(());
    };
    let (device, queue) = (renderer.device(), renderer.queue());

    // Pure magenta survives sRGB decode/encode exactly and is hard to confuse with a fallback.
    let base: Vec<u8> = (0..16).flat_map(|_| [255_u8, 0, 255, 255]).collect();
    let decoded = DecodedLayers {
        size: 4,
        mip_levels: 3,
        layers: vec![Some(build_mips(base, 4))],
        missing: Vec::new(),
    };
    let textures = MaterialTextureArray::upload(device, queue, &decoded)?;

    let mut selected = GpuMaterial::NONE;
    selected.color_map = 0;
    let table = MaterialTable::new(device, &[GpuMaterial::NONE, selected]);

    let layout = scene_bind_group_layout(device);
    let environment = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("hd-material-binding-test-environment"),
        size: std::mem::size_of::<HdEnvironmentUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM,
        mapped_at_creation: false,
    });
    let water = water_types_buffer(device, &[GpuWaterType::none(|_| -1)]);
    let lights = point_lights_buffer(device, &[]);
    let bind_group = scene_bind_group(
        device,
        &layout,
        &table,
        &textures,
        &environment,
        &water,
        &lights,
    );

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("hd-material-binding-test"),
        source: wgpu::ShaderSource::Wgsl(
            format!(
                "{MATERIAL_WGSL}\n\
                 @group(0) @binding(0) var<storage, read> materials: array<Material>;\n\
                 @group(0) @binding(1) var material_textures: texture_2d_array<f32>;\n\
                 @group(0) @binding(2) var material_sampler: sampler;\n\
                 @vertex\n\
                 fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {{\n\
                     let positions = array<vec2<f32>, 3>(\n\
                         vec2<f32>(-1.0, -1.0),\n\
                         vec2<f32>( 3.0, -1.0),\n\
                         vec2<f32>(-1.0,  3.0),\n\
                     );\n\
                     return vec4<f32>(positions[index], 0.0, 1.0);\n\
                 }}\n\
                 @fragment\n\
                 fn fs_main() -> @location(0) vec4<f32> {{\n\
                     let material = materials[1u];\n\
                     return textureSample(\n\
                         material_textures, material_sampler, vec2<f32>(0.5, 0.5), material.color_map\n\
                     );\n\
                 }}"
            )
            .into(),
        ),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("hd-material-binding-test"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("hd-material-binding-test"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("hd-material-binding-test-target"),
        size: wgpu::Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("hd-material-binding-test-readback"),
        size: 256 * 4,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("hd-material-binding-test"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(4),
            },
        },
        wgpu::Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    let pixels = read_buffer(device, &readback)?;
    assert_eq!(&pixels[..4], &[255, 0, 255, 255]);
    Ok(())
}

#[test]
fn hd_scene_shader_keeps_flow_map_stage_before_material_sampling() {
    let shader = include_str!("../src/gpu/hd/scene.wgsl");
    let flow = shader.find("let flow_offset = flow * flow_strength;");
    let material_derivatives = shader.find("let ddx1 = dpdx(uv1);");
    let diffuse = shader.find("let tex1 = sample_color(material1, uv1, ddx1, ddy1);");
    let (Some(flow), Some(material_derivatives), Some(diffuse)) =
        (flow, material_derivatives, diffuse)
    else {
        panic!("HD scene shader must keep flow-map distortion ahead of final material sampling");
    };

    assert!(flow < material_derivatives);
    assert!(material_derivatives < diffuse);
    assert!(shader.contains("material1.flow_map >= 0"));
}

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
