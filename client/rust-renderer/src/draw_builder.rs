#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedDrawList {
    pub flat_ranges: Vec<u32>,
    pub planes: Vec<u8>,
}

pub fn prepare_draw_list(command_fields: &[u32]) -> Result<PreparedDrawList, &'static str> {
    if !command_fields.len().is_multiple_of(4) {
        return Err("draw command packet must contain quadruples");
    }

    let command_count = command_fields.len() / 4;
    let mut flat_ranges = Vec::with_capacity(command_count * 3);
    let mut planes = Vec::with_capacity(command_count);

    for command in command_fields.chunks_exact(4) {
        flat_ranges.extend_from_slice(&command[..3]);
        planes.push(u8::try_from(command[3]).unwrap_or(u8::MAX));
    }

    Ok(PreparedDrawList {
        flat_ranges,
        planes,
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub struct RustPreparedDrawList {
    flat_ranges: Vec<u32>,
    planes: Vec<u8>,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
impl RustPreparedDrawList {
    pub fn flat_ranges(&self) -> Vec<u32> {
        self.flat_ranges.clone()
    }

    pub fn planes(&self) -> Vec<u8> {
        self.planes.clone()
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn build_draw_list(
    command_fields: &[u32],
) -> Result<RustPreparedDrawList, wasm_bindgen::JsValue> {
    let prepared = prepare_draw_list(command_fields).map_err(wasm_bindgen::JsValue::from_str)?;
    Ok(RustPreparedDrawList {
        flat_ranges: prepared.flat_ranges,
        planes: prepared.planes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_ranges_and_plane_metadata_from_commands() {
        let prepared = prepare_draw_list(&[0, 6, 1, 0, 24, 12, 3, 2]).unwrap();

        assert_eq!(prepared.flat_ranges, vec![0, 6, 1, 24, 12, 3]);
        assert_eq!(prepared.planes, vec![0, 2]);
    }

    #[test]
    fn rejects_malformed_command_packets() {
        assert_eq!(
            prepare_draw_list(&[0, 3, 1]).unwrap_err(),
            "draw command packet must contain quadruples"
        );
    }
}
