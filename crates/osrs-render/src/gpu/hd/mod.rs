//! 117 HD render profile: engine foundations (phase 0 of `docs/plans/117HD-PORT.md`).
//!
//! * [`layout`]: GPU buffer layouts shared with the HD shaders (material table, frame uniform).
//! * [`materials`]: the material texture-array loader and the layer plan derived from `osrs-hd`.
//! * [`shadow`]: the shadow maps, their bind group and the depth-only shadow pipelines.
//! * [`post`]: the resolve pass that turns the HDR scene target into the displayable image.

pub mod layout;
pub mod materials;
pub(super) mod post;
pub mod shadow;
pub(super) mod sky;

/// Colour format of the HD offscreen scene target.
pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Texture-array layers the HD material array may use (Metal and most desktop GPUs allow 2048; the
/// WebGPU default is 256, below the ~385 layers of the plugin's material set).
pub const HD_TEXTURE_LAYERS: u32 = 1024;

/// `base` with the limits the HD profile needs raised to what `adapter` supports. Raising a limit
/// never changes what the Reference profile draws.
pub fn raise_limits(mut base: wgpu::Limits, adapter: &wgpu::Limits) -> wgpu::Limits {
    base.max_texture_array_layers = base
        .max_texture_array_layers
        .max(adapter.max_texture_array_layers.min(HD_TEXTURE_LAYERS));
    base
}
