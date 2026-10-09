use osrs_cache::decode::{ArchiveFileProvenance, DecoderContext, decode_model_data};
use osrs_cache::profile::TargetProfile;
use osrs_core::ids::ModelId;
use std::panic::{AssertUnwindSafe, catch_unwind};

const TARGET_PROFILE_YAML: &str =
    include_str!("../../../profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml");
const GENERATED_CASES: usize = 512;
const MAX_CASE_BYTES: usize = 192;

fn context() -> Result<DecoderContext, Box<dyn std::error::Error>> {
    let profile = TargetProfile::from_yaml_str(TARGET_PROFILE_YAML)?;
    Ok(DecoderContext::from_profile(&profile)?)
}

fn next_u64(state: &mut u64) -> u64 {
    let mut value = *state;
    value ^= value << 13;
    value ^= value >> 7;
    value ^= value << 17;
    *state = value;
    value
}

fn exercise(bytes: &[u8], context: &DecoderContext, model_id: u32) {
    let source = ArchiveFileProvenance::new(7, model_id, Some(0));
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = decode_model_data(bytes, context, &source, ModelId::new(model_id));
    }));
    assert!(
        result.is_ok(),
        "M4 model decoder panicked for model {model_id} input {bytes:02x?}"
    );
}

fn with_trailer(mut bytes: Vec<u8>, trailer: [u8; 2]) -> Vec<u8> {
    if bytes.len() < 2 {
        bytes.resize(2, 0);
    }
    let len = bytes.len();
    bytes[len - 2] = trailer[0];
    bytes[len - 1] = trailer[1];
    bytes
}

#[test]
fn arbitrary_bounded_bytes_never_panic_m4_model_decoder()
-> Result<(), Box<dyn std::error::Error>> {
    let context = context()?;

    let corpus = [
        Vec::new(),
        vec![0],
        vec![0xff],
        vec![0; 26],
        vec![0xff; 26],
        (0..=191).collect::<Vec<u8>>(),
    ];
    let mut model_id = 1u32;
    for bytes in corpus {
        exercise(&bytes, &context, model_id);
        model_id += 1;
        exercise(&with_trailer(bytes.clone(), [0xff, 0xfd]), &context, model_id);
        model_id += 1;
        exercise(&with_trailer(bytes, [0xff, 0xfe]), &context, model_id);
        model_id += 1;
    }

    let mut state = 0x4d34_4d4f_4445_4c53u64;
    for _ in 0..GENERATED_CASES {
        let length = (next_u64(&mut state) as usize) % (MAX_CASE_BYTES + 1);
        let mut bytes = Vec::with_capacity(length);
        for _ in 0..length {
            bytes.push(next_u64(&mut state) as u8);
        }

        exercise(&bytes, &context, model_id);
        model_id += 1;
        exercise(
            &with_trailer(bytes.clone(), [0xff, 0xfd]),
            &context,
            model_id,
        );
        model_id += 1;
        exercise(&with_trailer(bytes, [0xff, 0xfe]), &context, model_id);
        model_id += 1;
    }

    Ok(())
}
