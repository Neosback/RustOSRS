//! GPU-side data of the HD profile: the material table, the layer plan and the pixels of the
//! material texture array, prepared on a worker thread and installed on the render thread.

use super::HdContext;
use crate::{TEXTURE_SIZE, TextureTable};
use osrs_hd::environment::EnvironmentSet;
use osrs_render::gpu::{
    SceneRenderer,
    hd::{
        layout::GpuMaterial,
        materials::{
            DEFAULT_RESOLUTION, DecodedLayers, LayerPlan, MaterialArrayError, MaterialTextureArray,
            MipChain, vanilla_mip_chain,
        },
    },
};
use std::{path::Path, sync::Arc};

/// Everything [`SceneRenderer::set_hd_materials`] needs, decoded on the CPU.
pub struct HdGpuData {
    /// The material table: index `0` is `NONE`, then [`super::HdContext::materials`] in order.
    pub materials: Vec<GpuMaterial>,
    pub plan: LayerPlan,
    pub decoded: DecodedLayers,
    /// Vanilla texture layers: `(layer, pixels)` for materials without an image file.
    pub vanilla: Vec<(u32, MipChain)>,
    /// The per-area lighting environments, for the render thread's environment state.
    pub environments: Arc<EnvironmentSet>,
}

impl HdContext {
    /// Decode the textures and pack the material table. `assets` is the `assets/117hd` directory
    /// (textures live in `scene/textures`), `textures` the cache's vanilla textures.
    pub fn prepare_gpu(
        &self,
        assets: &Path,
        textures: &TextureTable,
        resolution: u32,
    ) -> Result<HdGpuData, MaterialArrayError> {
        let plan = LayerPlan::from_table(&self.materials.materials, &self.materials.definitions);
        let decoded = DecodedLayers::load(&assets.join("scene/textures"), &plan, resolution)?;

        let mut table = vec![GpuMaterial::NONE];
        for material in &self.materials.materials {
            let mut packed = GpuMaterial::from_hd(material, |name| plan.layer_of(name));
            // Animated vanilla textures scroll their replacement material too.
            let image = material
                .vanilla_texture_index
                .filter(|index| *index >= 0.0)
                .and_then(|index| textures.images.get(index as usize))
                .and_then(Option::as_ref);
            if let Some(image) = image
                && image.animation_direction != 0
            {
                let speed = f32::from(image.animation_speed) * 50.0 / 128.0;
                let radians = f32::from(image.animation_direction) * -std::f32::consts::FRAC_PI_2;
                packed.scroll_duration[0] += radians.cos() * speed;
                packed.scroll_duration[1] += radians.sin() * speed;
            }
            table.push(packed);
        }

        let mut vanilla = Vec::new();
        for (layer, source) in plan.layers.iter().enumerate() {
            if decoded.layers.get(layer).is_some_and(Option::is_some) {
                continue;
            }
            let Some(image) = source
                .vanilla_index
                .and_then(|index| usize::try_from(index).ok())
                .and_then(|index| textures.images.get(index))
                .and_then(Option::as_ref)
            else {
                continue;
            };
            vanilla.push((
                layer as u32,
                vanilla_mip_chain(&image.rgba, TEXTURE_SIZE as u32, decoded.size),
            ));
        }
        Ok(HdGpuData {
            materials: table,
            plan,
            decoded,
            vanilla,
            environments: Arc::clone(&self.environments),
        })
    }
}

impl HdGpuData {
    /// Default texture resolution of the plugin (256 px).
    pub const RESOLUTION: u32 = DEFAULT_RESOLUTION;

    /// Upload the texture array and bind it with the material table.
    pub fn install(self, renderer: &mut SceneRenderer) -> Result<(), MaterialArrayError> {
        let array =
            MaterialTextureArray::upload(renderer.device(), renderer.queue(), &self.decoded)?;
        for (layer, chain) in &self.vanilla {
            array.write_layer(renderer.queue(), *layer, chain);
        }
        renderer.set_hd_materials(&self.materials, array);
        Ok(())
    }
}
