//! Candidate theme pipelines are validated before replacing the last working HD resources.
use super::*;
use osrs_hd::theme::shader::{ShaderExtension, ShaderStage};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

pub struct ThemeShaderPreparation {
    resources: Option<HdResources>,
    validation: Pin<Box<dyn Future<Output = Option<wgpu::Error>>>>,
    ready: bool,
    vertex_active: bool,
}
impl ThemeShaderPreparation {
    pub fn poll(&mut self) -> Poll<Result<(), String>> {
        match self
            .validation
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(error)) => Poll::Ready(Err(error.to_string())),
            Poll::Ready(None) => {
                self.ready = true;
                Poll::Ready(Ok(()))
            }
        }
    }
}

fn validate(source: &str, label: &str) -> Result<naga::Module, String> {
    let module = naga::front::wgsl::parse_str(source)
        .map_err(|error| format!("{label}: {}", error.emit_to_string(source)))?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .map_err(|error| format!("{label}: {}", error.emit_to_string(source)))?;
    Ok(module)
}

/// Pure validation also runs headlessly: user code cannot add resources or shader entry points.
pub fn shader_sources(extensions: &[ShaderExtension]) -> Result<(String, String), String> {
    composed_sources(extensions).map(|(scene, post, _, _)| (scene, post))
}
fn composed_sources(
    extensions: &[ShaderExtension],
) -> Result<(String, String, String, String), String> {
    let mut bodies = std::collections::BTreeMap::new();
    for extension in extensions {
        extension.validate()?;
        let label = format!("{:?}.wgsl", extension.stage);
        let module = validate(&extension.source, &label)?;
        if !module
            .functions
            .iter()
            .any(|(_, function)| function.name.as_deref() == Some(extension.stage.function()))
        {
            return Err(format!(
                "{label}: missing required function {}",
                extension.stage.function()
            ));
        }
        if !module.entry_points.is_empty() || !module.global_variables.is_empty() {
            return Err(format!(
                "{label}: entry points and resources belong to the renderer"
            ));
        }
        if bodies
            .insert(extension.stage, extension.source.clone())
            .is_some()
        {
            return Err(format!("{label}: duplicate extension stage"));
        }
    }
    let mut scene_extensions = String::new();
    let mut post_extension = String::new();
    for stage in ShaderStage::ALL {
        let body = bodies.remove(&stage).unwrap_or_else(|| stage.template());
        if stage == ShaderStage::Post {
            post_extension.push_str(&body);
        } else {
            scene_extensions.push_str(&body);
        }
    }
    let scene = hd::shadow::shader_source(&format!(
        "{scene_extensions}\n{}\n{}",
        include_str!("hd/shadow_receive.wgsl"),
        include_str!("hd/scene.wgsl")
    ));
    let post = format!("{post_extension}\n{}", include_str!("hd/post.wgsl"));
    validate(&scene, "HD scene")?;
    validate(&post, "HD post")?;
    let sky = hd::shadow::shader_source(&format!(
        "{scene_extensions}\n{}",
        include_str!("hd/sky.wgsl")
    ));
    validate(&sky, "HD sky")?;
    Ok((scene, post, sky, scene_extensions))
}

impl SceneRenderer {
    pub fn prepare_theme_shaders(
        &self,
        extensions: &[ShaderExtension],
    ) -> Result<ThemeShaderPreparation, String> {
        let (scene, post, sky, prelude) = composed_sources(extensions)?;
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let resources = self.build_hd_resources(&scene, &post, &sky, &prelude);
        Ok(ThemeShaderPreparation {
            resources: Some(resources),
            validation: Box::pin(scope.pop()),
            ready: false,
            vertex_active: extensions.iter().any(|s| s.stage == ShaderStage::Vertex),
        })
    }
    pub fn install_theme_shaders(
        &mut self,
        mut preparation: ThemeShaderPreparation,
    ) -> Result<(), String> {
        if !preparation.ready {
            return Err("Shader validation has not completed".into());
        }
        let resources = preparation
            .resources
            .take()
            .ok_or("No prepared pipelines")?;
        self.theme_vertex_active = preparation.vertex_active;
        self.hd.take();
        self.hd
            .set(resources)
            .map_err(|_| "HD pipelines already installed")?;
        self.shadow_geometry_version += 1;
        Ok(())
    }
    pub(super) fn build_hd_resources(
        &self,
        scene: &str,
        post: &str,
        sky: &str,
        prelude: &str,
    ) -> HdResources {
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("theme/scene.wgsl"),
                source: wgpu::ShaderSource::Wgsl(scene.into()),
            });
        let layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("hd-scene"),
                bind_group_layouts: &[
                    Some(&self.globals_layout),
                    Some(&self.zone_layout),
                    Some(&self.hd_layout),
                    Some(&self.hd_shadow_layout),
                ],
                immediate_size: 0,
            });
        HdResources {
            sky: hd::sky::pipeline(&self.device, &self.hd_layout, sky),
            pipelines: build_scene_pipelines(
                &self.device,
                &self.scene_shader,
                &self.scene_layout,
                hd::HDR_FORMAT,
                false,
            ),
            scene: build_scene_pipelines(&self.device, &shader, &layout, hd::HDR_FORMAT, true),
            shadows: hd::shadow::ShadowPipelines::with_extensions(
                &self.device,
                &self.zone_layout,
                &self.hd_layout,
                prelude,
            ),
            post: hd::post::PostPass::with_source(&self.device, post),
        }
    }
}
