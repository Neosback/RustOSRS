//! User shader validation runs without a GPU; candidate activation is tested on a real adapter.
#![allow(clippy::unwrap_used)] // Test fixtures intentionally fail at the offending value.
use osrs_hd::theme::shader::{ShaderExtension, ShaderStage};
use osrs_render::gpu::{SceneRenderer, theme::shader_sources};

#[test]
fn templates_validate_and_bad_shader_diagnostics_keep_source_location() {
    let templates = ShaderStage::ALL.map(ShaderExtension::new);
    shader_sources(&templates).unwrap();
    let mut invalid = ShaderExtension::new(ShaderStage::Surface);
    invalid.source = "fn theme_surface(color: vec3<f32>, position: vec3<f32>, time: f32) -> vec3<f32> {\n return unknown;\n}".into();
    let error = shader_sources(&[invalid]).unwrap_err();
    assert!(
        error.contains("unknown") && error.contains("Surface.wgsl"),
        "{error}"
    );
}
#[test]
fn resources_entrypoints_and_wrong_signatures_are_rejected() {
    for source in [
        "@group(0) @binding(8) var<uniform> hacked: vec4<f32>;",
        "@compute @workgroup_size(1) fn custom_entry() {}",
        "fn theme_surface() -> f32 { return 1.0; }",
    ] {
        let mut extension = ShaderExtension::new(ShaderStage::Surface);
        extension.source = source.into();
        assert!(shader_sources(&[extension]).is_err());
    }
}
#[test]
fn candidate_pipelines_validate_before_activation() -> Result<(), Box<dyn std::error::Error>> {
    let mut renderer = match SceneRenderer::new_headless() {
        Ok(renderer) => renderer,
        Err(error) => {
            eprintln!("GPU unavailable: {error}");
            return Ok(());
        }
    };
    let mut candidate =
        renderer.prepare_theme_shaders(&ShaderStage::ALL.map(ShaderExtension::new))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        match candidate.poll() {
            std::task::Poll::Ready(result) => {
                result?;
                break;
            }
            std::task::Poll::Pending if std::time::Instant::now() < deadline => {
                std::thread::yield_now()
            }
            _ => return Err("GPU validation timed out".into()),
        }
    }
    renderer.install_theme_shaders(candidate)?;
    Ok(())
}

#[test]
fn sky_and_post_hooks_change_hd_pixels_and_failed_edits_keep_the_working_image()
-> Result<(), Box<dyn std::error::Error>> {
    use osrs_render::gpu::{
        Colorblind, Fog, FrameParams, HdParams, ReferenceCamera, RenderProfile,
        hd::{
            layout::GpuMaterial,
            materials::{DecodedLayers, MaterialTextureArray},
        },
    };
    let mut renderer = match SceneRenderer::new_headless() {
        Ok(renderer) => renderer,
        Err(error) => {
            eprintln!("GPU unavailable: {error}");
            return Ok(());
        }
    };
    let decoded = DecodedLayers {
        size: 1,
        mip_levels: 1,
        layers: vec![None],
        missing: Vec::new(),
    };
    let textures = MaterialTextureArray::upload(renderer.device(), renderer.queue(), &decoded)?;
    renderer.set_hd_materials(&[GpuMaterial::NONE], textures);
    renderer.set_hd_params(HdParams {
        exposure: 1.0,
        placeholder_vignette: 0.0,
    });
    let params = FrameParams {
        camera: ReferenceCamera {
            x: 0.0,
            y: -100.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.5,
            scale: 64.0,
        },
        view_plane: 0,
        tick: 0,
        brightness: 0.8,
        remove_color_banding: true,
        clear_color: [0.1, 0.2, 0.3],
        bright_textures: false,
        fog: Fog::default(),
        colorblind: Colorblind::default(),
        profile: RenderProfile::Hd,
    };
    let reference = FrameParams {
        profile: RenderProfile::Reference,
        ..params
    };
    let before_reference = renderer.render_to_rgba(reference, 32, 24)?;
    let mut sky = ShaderExtension::new(ShaderStage::Sky);
    sky.source = "fn theme_sky (color: vec3<f32>, position: vec3<f32>, time: f32) -> vec3<f32> { return vec3<f32>(1.0, 0.0, 0.0); }".into();
    let mut post = ShaderExtension::new(ShaderStage::Post);
    post.source = "fn theme_post(color: vec3<f32>, position: vec3<f32>, time: f32) -> vec3<f32> { return color.grb; }".into();
    let mut candidate = renderer.prepare_theme_shaders(&[sky, post])?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        match candidate.poll() {
            std::task::Poll::Ready(result) => {
                result?;
                break;
            }
            std::task::Poll::Pending if std::time::Instant::now() < deadline => {
                std::thread::yield_now()
            }
            _ => return Err("GPU validation timed out".into()),
        }
    }
    renderer.install_theme_shaders(candidate)?;
    let image = renderer.render_to_rgba(params, 32, 24)?;
    assert_eq!(
        &image[(12 * 32 + 16) * 4..(12 * 32 + 16) * 4 + 4],
        &[0, 255, 0, 255]
    );
    let mut broken = ShaderExtension::new(ShaderStage::Sky);
    broken.source = "invalid WGSL".into();
    assert!(renderer.prepare_theme_shaders(&[broken]).is_err());
    assert_eq!(renderer.render_to_rgba(params, 32, 24)?, image);
    assert_eq!(
        renderer.render_to_rgba(reference, 32, 24)?,
        before_reference
    );
    Ok(())
}
