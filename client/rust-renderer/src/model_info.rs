/// Model-instance payload matching SceneBuffer.ts::createModelInfoTextureData.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelInfo {
    pub scene_x: i32,
    pub scene_z: i32,
    pub height_offset: i32,
    pub level: i32,
    pub plane_cull_level: i32,
    pub contour_ground: i32,
    pub priority: i32,
    pub interact_type: i32,
    pub interact_id: i32,
}

impl ModelInfo {
    /// Returns one RGBA16UI texel with the exact current TypeScript bit layout.
    pub fn encode_texel(self) -> [u16; 4] {
        let word0 = self.scene_x | (self.level << 14);
        let word1 = self.scene_z | (self.contour_ground << 14);
        let rounded_height = js_round_div_8(self.height_offset);
        let word2 = (self.priority & 0x7)
            | ((self.interact_id >> 16) << 3)
            | (self.interact_type << 4)
            | (self.plane_cull_level << 6)
            | (rounded_height << 8);

        [
            word0 as u16,
            word1 as u16,
            word2 as u16,
            self.interact_id as u16,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelInfoDrawCommand {
    pub instances: Vec<ModelInfo>,
}

/// Builds the RGBA16UI texture packet currently produced by
/// createModelInfoTextureData in SceneBuffer.ts.
pub fn create_model_info_texture_data(commands: &[ModelInfoDrawCommand]) -> Vec<u16> {
    let instance_count: usize = commands.iter().map(|command| command.instances.len()).sum();

    // Match SceneBuffer.ts exactly. The TypeScript allocation formula counts
    // command header words before rounding to a 16-texel row, then multiplies
    // by four channels. It over-allocates relative to the logical texel count
    // for some command/instance mixes, but packet parity is more important
    // than "fixing" that behavior during the renderer migration.
    let data_length = (commands.len() * 4 + instance_count).div_ceil(16) * 16;
    let texel_count = data_length.max(16);
    let mut texture = vec![0u16; texel_count * 4];

    let mut instance_offset = 0usize;
    for (draw_index, command) in commands.iter().enumerate() {
        texture[draw_index * 4] = (commands.len() + instance_offset) as u16;
        instance_offset += command.instances.len();
    }

    let mut instance_index = 0usize;
    for command in commands {
        for instance in &command.instances {
            let texel = instance.encode_texel();
            let base = commands.len() * 4 + instance_index * 4;
            texture[base..base + 4].copy_from_slice(&texel);
            instance_index += 1;
        }
    }

    texture
}

const MODEL_INFO_FIELD_STRIDE: usize = 9;

/// Builds the same RGBA16UI packet as `create_model_info_texture_data` from
/// flat typed-array friendly inputs.
///
/// `command_instance_counts` contains one instance count per draw command.
/// `instance_fields` contains stride-9 records in command/instance order:
/// [scene_x, scene_z, height_offset, level, plane_cull_level,
///  contour_ground, priority, interact_type, interact_id].
pub fn create_model_info_texture_data_flat(
    command_instance_counts: &[u32],
    instance_fields: &[i32],
) -> Result<Vec<u16>, String> {
    let instance_count = command_instance_counts
        .iter()
        .try_fold(0usize, |total, count| {
            total
                .checked_add(*count as usize)
                .ok_or_else(|| "model-info instance count overflow".to_string())
        })?;

    let expected_fields = instance_count
        .checked_mul(MODEL_INFO_FIELD_STRIDE)
        .ok_or_else(|| "model-info field count overflow".to_string())?;

    if instance_fields.len() != expected_fields {
        return Err(format!(
            "model-info field packet has {} values, expected {expected_fields}",
            instance_fields.len()
        ));
    }

    let command_count = command_instance_counts.len();
    let data_length = (command_count * 4 + instance_count).div_ceil(16) * 16;
    let texel_count = data_length.max(16);
    let mut texture = vec![0u16; texel_count * 4];

    let mut instance_offset = 0usize;
    for (draw_index, count) in command_instance_counts.iter().enumerate() {
        texture[draw_index * 4] = (command_count + instance_offset) as u16;
        instance_offset += *count as usize;
    }

    for instance_index in 0..instance_count {
        let field_offset = instance_index * MODEL_INFO_FIELD_STRIDE;
        let info = ModelInfo {
            scene_x: instance_fields[field_offset],
            scene_z: instance_fields[field_offset + 1],
            height_offset: instance_fields[field_offset + 2],
            level: instance_fields[field_offset + 3],
            plane_cull_level: instance_fields[field_offset + 4],
            contour_ground: instance_fields[field_offset + 5],
            priority: instance_fields[field_offset + 6],
            interact_type: instance_fields[field_offset + 7],
            interact_id: instance_fields[field_offset + 8],
        };
        let texel = info.encode_texel();
        let base = command_count * 4 + instance_index * 4;
        texture[base..base + 4].copy_from_slice(&texel);
    }

    Ok(texture)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn build_model_info_texture_data(
    command_instance_counts: &[u32],
    instance_fields: &[i32],
) -> Result<Vec<u16>, wasm_bindgen::JsValue> {
    create_model_info_texture_data_flat(command_instance_counts, instance_fields)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error))
}

fn js_round_div_8(value: i32) -> i32 {
    ((value as f64 / 8.0) + 0.5).floor() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_info_texel_matches_typescript_bit_layout() {
        let info = ModelInfo {
            scene_x: 320,
            scene_z: 448,
            height_offset: 64,
            level: 2,
            plane_cull_level: 3,
            contour_ground: 1,
            priority: 6,
            interact_type: 2,
            interact_id: 0x12345,
        };

        let texel = info.encode_texel();

        assert_eq!(texel[0], (320 | (2 << 14)) as u16);
        assert_eq!(texel[1], (448 | (1 << 14)) as u16);
        assert_eq!(
            texel[2],
            ((6 & 0x7) | ((0x12345 >> 16) << 3) | (2 << 4) | (3 << 6) | (8 << 8)) as u16
        );
        assert_eq!(texel[3], 0x2345);
    }

    #[test]
    fn texture_packet_places_draw_offsets_before_instances() {
        let first = ModelInfo {
            scene_x: 1,
            scene_z: 2,
            height_offset: 0,
            level: 0,
            plane_cull_level: 0,
            contour_ground: 3,
            priority: 0,
            interact_type: 0,
            interact_id: 0xffff,
        };
        let second = ModelInfo {
            scene_x: 3,
            scene_z: 4,
            ..first
        };

        let packet = create_model_info_texture_data(&[
            ModelInfoDrawCommand {
                instances: vec![first, second],
            },
            ModelInfoDrawCommand {
                instances: vec![second],
            },
        ]);

        assert_eq!(packet.len(), 64);
        assert_eq!(packet[0], 2);
        assert_eq!(packet[4], 4);
        assert_eq!(&packet[8..12], &first.encode_texel());
        assert_eq!(&packet[12..16], &second.encode_texel());
        assert_eq!(&packet[16..20], &second.encode_texel());
    }

    #[test]
    fn texture_packet_matches_typescript_padding_rule() {
        let commands = (0..5)
            .map(|_| ModelInfoDrawCommand { instances: vec![] })
            .collect::<Vec<_>>();

        let packet = create_model_info_texture_data(&commands);

        // SceneBuffer.ts:
        // ceil((5 * 4 + 0) / 16) * 16 texels * 4 channels = 128 u16.
        assert_eq!(packet.len(), 128);
    }

    #[test]
    fn flat_packet_builder_matches_structured_packet() {
        let first = ModelInfo {
            scene_x: 320,
            scene_z: 448,
            height_offset: 64,
            level: 2,
            plane_cull_level: 3,
            contour_ground: 1,
            priority: 6,
            interact_type: 2,
            interact_id: 0x12345,
        };
        let second = ModelInfo {
            scene_x: -128,
            scene_z: 64,
            height_offset: -12,
            level: 1,
            plane_cull_level: 1,
            contour_ground: 0,
            priority: 3,
            interact_type: 1,
            interact_id: 0x23456,
        };

        let structured = create_model_info_texture_data(&[
            ModelInfoDrawCommand {
                instances: vec![first, second],
            },
            ModelInfoDrawCommand {
                instances: vec![second],
            },
        ]);

        let fields = [
            first.scene_x,
            first.scene_z,
            first.height_offset,
            first.level,
            first.plane_cull_level,
            first.contour_ground,
            first.priority,
            first.interact_type,
            first.interact_id,
            second.scene_x,
            second.scene_z,
            second.height_offset,
            second.level,
            second.plane_cull_level,
            second.contour_ground,
            second.priority,
            second.interact_type,
            second.interact_id,
            second.scene_x,
            second.scene_z,
            second.height_offset,
            second.level,
            second.plane_cull_level,
            second.contour_ground,
            second.priority,
            second.interact_type,
            second.interact_id,
        ];

        let flat = create_model_info_texture_data_flat(&[2, 1], &fields).unwrap();
        assert_eq!(flat, structured);
    }

    #[test]
    fn flat_packet_builder_rejects_malformed_input() {
        assert!(create_model_info_texture_data_flat(&[1], &[0; 8]).is_err());
        assert!(create_model_info_texture_data_flat(&[0], &[0; 9]).is_err());
    }

    #[test]
    fn negative_half_height_uses_javascript_rounding_rule() {
        assert_eq!(js_round_div_8(-4), 0);
        assert_eq!(js_round_div_8(-12), -1);
    }
}
