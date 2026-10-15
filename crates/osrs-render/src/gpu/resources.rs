//! Owned allocation sizes, excluding driver overhead, alignment and shared UI allocations.
//! These counts may overlap process RSS on unified-memory systems.
use super::*;
use std::collections::BTreeMap;

/// Uncompressed texture storage including every mip and sample. Current render targets and
/// material arrays all use uncompressed formats; driver tiling/alignment is deliberately excluded.
pub fn texture_storage_bytes(
    width: u32,
    height: u32,
    layers: u32,
    mips: u32,
    samples: u32,
    bytes_per_texel: u32,
) -> u64 {
    (0..mips.min(32))
        .map(|mip| {
            u64::from((width >> mip).max(1))
                * u64::from((height >> mip).max(1))
                * u64::from(layers)
                * u64::from(samples)
                * u64::from(bytes_per_texel)
        })
        .sum()
}

impl SceneRenderer {
    /// Latest timestamp result for each measured pass: `(sequence, milliseconds)`.
    pub fn gpu_pass_samples(&self) -> BTreeMap<&'static str, (u32, f32)> {
        let mut result = BTreeMap::new();
        if let Some(sample) = self
            .timer
            .borrow()
            .as_ref()
            .and_then(timing::GpuTimer::latest_sample)
        {
            result.insert("scene", sample);
        }
        for (name, timer) in ["shadow", "terrain_shadow", "post"]
            .into_iter()
            .zip(self.auxiliary_timers.borrow().iter())
        {
            if let Some(sample) = timer.as_ref().and_then(timing::GpuTimer::latest_sample) {
                result.insert(name, sample);
            }
        }
        result
    }

    /// Byte sizes of owned resources, grouped by purpose. Not an estimate of total driver VRAM.
    pub fn resource_bytes(&self) -> BTreeMap<&'static str, u64> {
        let mut result = BTreeMap::from([
            ("geometry", self.resident_vertex_bytes()),
            (
                "reference_textures",
                texture_storage_bytes(
                    TEXTURE_SIZE,
                    TEXTURE_SIZE,
                    self.reference_texture_layers,
                    TEXTURE_SIZE.ilog2() + 1,
                    1,
                    4,
                ),
            ),
            (
                "uniforms",
                self.globals_buffer.size()
                    + self.hd_env_buffer.size()
                    + self.hd_water_types.size()
                    + (MAX_TEXTURE_LAYERS * 16) as u64
                    + self
                        .regions
                        .values()
                        .flatten()
                        .map(|zone| zone.zone_buffer.size())
                        .sum::<u64>(),
            ),
            (
                "alpha_indices",
                self.alpha_indices
                    .borrow()
                    .as_ref()
                    .map_or(0, |(buffer, _)| buffer.size()),
            ),
            ("guide_lines", self.lines.buffer_bytes()),
            (
                "frame_graph_targets",
                self.transients.borrow().allocated_bytes(),
            ),
            (
                "shadow_maps",
                self.hd_shadow_map_bytes() + self.hd_shadow_dummy.bytes(),
            ),
            (
                "timing_readback",
                self.timer
                    .borrow()
                    .as_ref()
                    .map_or(0, timing::GpuTimer::buffer_bytes),
            ),
        ]);
        if let Some(msaa) = self.msaa.borrow().as_ref() {
            result.insert(
                "reference_msaa",
                texture_storage_bytes(msaa.width, msaa.height, 1, 1, MSAA_SAMPLES, 8),
            );
        }
        if let Some(binding) = &self.hd_binding {
            let texture = binding.textures.texture();
            result.insert(
                "hd_material_textures",
                texture_storage_bytes(
                    texture.width(),
                    texture.height(),
                    texture.depth_or_array_layers(),
                    texture.mip_level_count(),
                    texture.sample_count(),
                    4,
                ),
            );
            result.insert("hd_material_table", binding.table.buffer().size());
        }
        if self.hd.get().is_some() {
            result.insert(
                "hd_post_uniform",
                std::mem::size_of::<hd::layout::HdFrameUniform>() as u64,
            );
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn texture_counts_include_layers_mips_and_samples() {
        assert_eq!(texture_storage_bytes(4, 2, 3, 3, 1, 4), (8 + 2 + 1) * 3 * 4);
        assert_eq!(texture_storage_bytes(4, 2, 1, 1, 4, 8), 4 * 2 * 4 * 8);
    }
}
