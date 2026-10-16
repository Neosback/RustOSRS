//! Versioned, engine-owned extension signatures. Extensions cannot declare resources or entry points.
use serde::{Deserialize, Serialize};
pub const SHADER_INTERFACE_VERSION: u32 = 1;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ShaderStage {
    Vertex,
    Surface,
    Water,
    Sky,
    Post,
}
impl ShaderStage {
    pub const ALL: [Self; 5] = [
        Self::Vertex,
        Self::Surface,
        Self::Water,
        Self::Sky,
        Self::Post,
    ];
    pub const fn function(self) -> &'static str {
        match self {
            Self::Vertex => "theme_vertex",
            Self::Surface => "theme_surface",
            Self::Water => "theme_water",
            Self::Sky => "theme_sky",
            Self::Post => "theme_post",
        }
    }
    pub fn template(self) -> String {
        let (input, output) = match self {
            Self::Vertex => (
                "position: vec3<f32>, normal: vec3<f32>, time: f32",
                "position",
            ),
            Self::Surface => (
                "color: vec3<f32>, base_color: vec3<f32>, position: vec3<f32>, normal: vec3<f32>, view_dir: vec3<f32>, time: f32",
                "color",
            ),
            _ => ("color: vec3<f32>, position: vec3<f32>, time: f32", "color"),
        };
        format!(
            "// Theme shader interface v1\nfn {}({input}) -> vec3<f32> {{\n    return {output};\n}}\n",
            self.function()
        )
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShaderExtension {
    pub version: u32,
    pub stage: ShaderStage,
    pub source: String,
}
impl ShaderExtension {
    pub fn new(stage: ShaderStage) -> Self {
        Self {
            version: SHADER_INTERFACE_VERSION,
            stage,
            source: stage.template(),
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != SHADER_INTERFACE_VERSION {
            return Err("Unsupported shader interface version".into());
        }
        if self.source.len() > 64 * 1024 {
            return Err("Shader exceeds 64 KiB".into());
        }
        // Syntax and interface checks are performed with Naga in osrs-render before activation.
        Ok(())
    }
}
