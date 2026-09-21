const PRIME1: u32 = 0x9e37_79b1;
const PRIME2: u32 = 0x85eb_ca77;
const PRIME3: u32 = 0xc2b2_ae3d;
const PRIME4: u32 = 0x27d4_eb2f;
const PRIME5: u32 = 0x1656_67b1;

fn round(acc: u32, input: u32) -> u32 {
    acc.wrapping_add(input.wrapping_mul(PRIME2))
        .rotate_left(13)
        .wrapping_mul(PRIME1)
}

/// Standard xxHash32 with the same seed-0 contract used by the TypeScript
/// `Hasher.hash32` path.
pub fn xxhash32(bytes: &[u8], seed: u32) -> u32 {
    let mut offset = 0usize;
    let mut hash = if bytes.len() >= 16 {
        let mut v1 = seed.wrapping_add(PRIME1).wrapping_add(PRIME2);
        let mut v2 = seed.wrapping_add(PRIME2);
        let mut v3 = seed;
        let mut v4 = seed.wrapping_sub(PRIME1);

        while offset + 16 <= bytes.len() {
            v1 = round(
                v1,
                u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()),
            );
            v2 = round(
                v2,
                u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()),
            );
            v3 = round(
                v3,
                u32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap()),
            );
            v4 = round(
                v4,
                u32::from_le_bytes(bytes[offset + 12..offset + 16].try_into().unwrap()),
            );
            offset += 16;
        }

        v1.rotate_left(1)
            .wrapping_add(v2.rotate_left(7))
            .wrapping_add(v3.rotate_left(12))
            .wrapping_add(v4.rotate_left(18))
    } else {
        seed.wrapping_add(PRIME5)
    };

    hash = hash.wrapping_add(bytes.len() as u32);

    while offset + 4 <= bytes.len() {
        let lane = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        hash = hash.wrapping_add(lane.wrapping_mul(PRIME3));
        hash = hash.rotate_left(17).wrapping_mul(PRIME4);
        offset += 4;
    }

    while offset < bytes.len() {
        hash = hash.wrapping_add((bytes[offset] as u32).wrapping_mul(PRIME5));
        hash = hash.rotate_left(11).wrapping_mul(PRIME1);
        offset += 1;
    }

    hash ^= hash >> 15;
    hash = hash.wrapping_mul(PRIME2);
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(PRIME3);
    hash ^ (hash >> 16)
}

pub fn hash_model_parts(parts: &[&[i32]]) -> u32 {
    let value_count = parts.iter().map(|part| part.len()).sum::<usize>();
    let mut bytes = Vec::with_capacity(value_count.saturating_mul(4));
    for part in parts {
        for value in *part {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    xxhash32(&bytes, 0)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hash_model_geometry(
    face_colors1: &[i32],
    face_colors2: &[i32],
    face_colors3: &[i32],
    vertices_x: &[i32],
    vertices_y: &[i32],
    vertices_z: &[i32],
    texture_ids: &[i32],
) -> u32 {
    hash_model_parts(&[
        face_colors1,
        face_colors2,
        face_colors3,
        vertices_x,
        vertices_y,
        vertices_z,
        texture_ids,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xxhash32_matches_standard_seed_zero_vectors() {
        assert_eq!(xxhash32(b"", 0), 0x02cc_5d05);
        assert_eq!(xxhash32(b"a", 0), 0x550d_7456);
        assert_eq!(xxhash32(b"abc", 0), 0x32d1_53ff);
    }

    #[test]
    fn model_hash_uses_typescript_part_order_and_little_endian_words() {
        let parts: [&[i32]; 7] = [&[1, -2], &[3], &[4], &[5, 6], &[7], &[8], &[9, -1]];
        let mut expected = Vec::new();
        for part in parts {
            for value in part {
                expected.extend_from_slice(&value.to_le_bytes());
            }
        }

        assert_eq!(hash_model_parts(&parts), xxhash32(&expected, 0));
    }
}
