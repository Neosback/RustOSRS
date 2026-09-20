use crate::draw::DrawRange;
use crate::packet::{RendererPacketError, validate_draw_ranges, validate_geometry};

/// Renderer-owned metadata for one static map-square upload.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StaticMapState {
    pub map_x: f32,
    pub map_y: f32,
    pub border_size: i32,
    pub height_map_size: u32,
    pub height_map_planes: u32,
    pub time_loaded: f32,
}

impl StaticMapState {
    pub fn validate(self) -> Result<(), &'static str> {
        if !self.map_x.is_finite() || !self.map_y.is_finite() || !self.time_loaded.is_finite() {
            return Err("static map state contains a non-finite value");
        }
        if self.border_size < 0 {
            return Err("scene border size cannot be negative");
        }
        if self.height_map_size == 0 || self.height_map_planes == 0 {
            return Err("height-map dimensions must be positive");
        }
        Ok(())
    }

    pub fn expected_height_samples(self) -> Result<usize, &'static str> {
        self.validate()?;
        (self.height_map_size as usize)
            .checked_mul(self.height_map_size as usize)
            .and_then(|value| value.checked_mul(self.height_map_planes as usize))
            .ok_or("height-map dimensions overflow")
    }
}

/// Validates the numeric boundary before anything is sent to WebGL.
pub fn validate_static_scene_packet(
    packed_vertices: &[u32],
    indices: &[u32],
    model_info_rgba16ui: &[u16],
    height_map_r16i: &[i16],
    draw_ranges: &[DrawRange],
    state: StaticMapState,
) -> Result<(), String> {
    validate_geometry(packed_vertices, indices).map_err(packet_error)?;
    validate_draw_ranges(draw_ranges, indices.len()).map_err(packet_error)?;
    state.validate().map_err(str::to_owned)?;

    if model_info_rgba16ui.is_empty() || model_info_rgba16ui.len() % (16 * 4) != 0 {
        return Err(
            "model-info texture data must contain complete 16-wide RGBA16UI rows".to_owned(),
        );
    }

    let expected = state.expected_height_samples().map_err(str::to_owned)?;
    if height_map_r16i.len() != expected {
        return Err(format!(
            "height-map packet has {} samples, expected {expected}",
            height_map_r16i.len()
        ));
    }

    Ok(())
}

fn packet_error(error: RendererPacketError) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> StaticMapState {
        StaticMapState {
            map_x: 50.0,
            map_y: 50.0,
            border_size: 1,
            height_map_size: 2,
            height_map_planes: 1,
            time_loaded: 5.0,
        }
    }

    #[test]
    fn validates_complete_static_packet() {
        assert!(validate_static_scene_packet(
            &[1, 2, 3],
            &[0],
            &[0; 64],
            &[0; 4],
            &[DrawRange::new(0, 1, 1)],
            state(),
        )
        .is_ok());
    }

    #[test]
    fn rejects_malformed_model_info_texture() {
        let error = validate_static_scene_packet(
            &[1, 2, 3],
            &[0],
            &[0; 4],
            &[0; 4],
            &[DrawRange::new(0, 1, 1)],
            state(),
        )
        .unwrap_err();
        assert!(error.contains("model-info texture"));
    }
}
