use std::collections::HashSet;

pub const FACE_RECORD_STRIDE: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceFilter {
    All,
    Opaque,
    Transparent,
}

impl FaceFilter {
    fn from_i32(value: i32) -> Result<Self, &'static str> {
        match value {
            -1 => Ok(Self::All),
            0 => Ok(Self::Opaque),
            1 => Ok(Self::Transparent),
            _ => Err("face filter must be -1 (all), 0 (opaque), or 1 (transparent)"),
        }
    }
}

fn transparency_to_alpha(transparency: i8) -> i32 {
    let value = if transparency == -1 {
        253
    } else {
        i32::from(transparency as u8)
    };
    (256 - value).clamp(0, 0xff)
}

pub fn prepare_model_faces(
    face_colors3: &[i32],
    face_alphas: &[i8],
    priorities: &[i8],
    render_layers: &[u8],
    texture_ids: &[i16],
    transparent_texture_ids: &[i32],
    filter: FaceFilter,
) -> Vec<i32> {
    let transparent_textures: HashSet<i32> = transparent_texture_ids.iter().copied().collect();
    let mut out = Vec::with_capacity(face_colors3.len().saturating_mul(FACE_RECORD_STRIDE));

    for (index, hsl_c) in face_colors3.iter().copied().enumerate() {
        if hsl_c == -2 {
            continue;
        }

        let texture_id = texture_ids.get(index).copied().map(i32::from).unwrap_or(-1);
        let alpha = face_alphas
            .get(index)
            .copied()
            .map(transparency_to_alpha)
            .unwrap_or(0xff);
        if alpha == 0 {
            continue;
        }

        let is_transparent =
            alpha < 0xff || (texture_id != -1 && transparent_textures.contains(&texture_id));
        match filter {
            FaceFilter::Opaque if is_transparent => continue,
            FaceFilter::Transparent if !is_transparent => continue,
            _ => {}
        }

        let priority = priorities.get(index).copied().map(i32::from).unwrap_or(0);
        let render_layer = render_layers
            .get(index)
            .copied()
            .map(i32::from)
            .unwrap_or(-1);

        out.extend_from_slice(&[index as i32, alpha, priority, render_layer, texture_id]);
    }

    out
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn build_model_faces(
    face_colors3: &[i32],
    face_alphas: &[i8],
    priorities: &[i8],
    render_layers: &[u8],
    texture_ids: &[i16],
    transparent_texture_ids: &[i32],
    filter: i32,
) -> Result<Vec<i32>, wasm_bindgen::JsValue> {
    let filter = FaceFilter::from_i32(filter).map_err(wasm_bindgen::JsValue::from_str)?;
    Ok(prepare_model_faces(
        face_colors3,
        face_alphas,
        priorities,
        render_layers,
        texture_ids,
        transparent_texture_ids,
        filter,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirrors_typescript_face_alpha_and_filtering_rules() {
        let colors = [10, -2, 20, 30, 40];
        let alphas = [0, 0, 1, -1, 0];
        let priorities = [1, 2, 3, 4, 5];
        let render_layers = [0, 1, 2, 3, 4];
        let textures = [-1, -1, -1, -1, 7];

        let all = prepare_model_faces(
            &colors,
            &alphas,
            &priorities,
            &render_layers,
            &textures,
            &[7],
            FaceFilter::All,
        );
        assert_eq!(
            all,
            vec![
                0, 255, 1, 0, -1, 2, 255, 3, 2, -1, 3, 3, 4, 3, -1, 4, 255, 5, 4, 7,
            ]
        );

        let opaque = prepare_model_faces(
            &colors,
            &alphas,
            &priorities,
            &render_layers,
            &textures,
            &[7],
            FaceFilter::Opaque,
        );
        assert_eq!(opaque, vec![0, 255, 1, 0, -1, 2, 255, 3, 2, -1]);

        let transparent = prepare_model_faces(
            &colors,
            &alphas,
            &priorities,
            &render_layers,
            &textures,
            &[7],
            FaceFilter::Transparent,
        );
        assert_eq!(transparent, vec![3, 3, 4, 3, -1, 4, 255, 5, 4, 7]);
    }

    #[test]
    fn fully_transparent_faces_are_removed() {
        assert!(
            prepare_model_faces(&[10], &[-2], &[0], &[], &[], &[], FaceFilter::All,).is_empty()
        );
    }
}
