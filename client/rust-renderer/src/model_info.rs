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
    fn negative_half_height_uses_javascript_rounding_rule() {
        assert_eq!(js_round_div_8(-4), 0);
        assert_eq!(js_round_div_8(-12), -1);
    }
}
