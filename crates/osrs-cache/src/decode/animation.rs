use super::{
    ArchiveFileProvenance, BinaryReader, ByteSpan, DecodeResult, DecodeSubject, DecoderContext,
};
use osrs_core::{
    animation_pose::{LegacyAnimationFrame, LegacyFrameTransform, LegacySkeletonTransform},
    ids::FrameId,
};

/// Decode the legacy transform prefix of one skeleton resource.
///
/// The pinned client stores transform types first, then label-list lengths, then
/// the label bytes. Build-241 skeleton resources may append cached-model
/// skeletal metadata. A zero extension count is accepted exactly; nonzero
/// extensions are rejected explicitly until that later skeletal codec is owned.
pub fn decode_legacy_skeleton(
    skeleton_id: u16,
    bytes: &[u8],
    context: &DecoderContext,
    source: &ArchiveFileProvenance,
) -> DecodeResult<Vec<LegacySkeletonTransform>> {
    let mut reader = BinaryReader::new(bytes, context, source)
        .with_subject(DecodeSubject::Skeleton(u32::from(skeleton_id)));
    let count = usize::from(reader.read_u8()?);

    let mut transform_types = Vec::with_capacity(count);
    for _ in 0..count {
        transform_types.push(reader.read_u8()?);
    }

    let mut label_counts = Vec::with_capacity(count);
    for _ in 0..count {
        label_counts.push(usize::from(reader.read_u8()?));
    }

    let mut transforms = Vec::with_capacity(count);
    for (transform_type, label_count) in transform_types.into_iter().zip(label_counts) {
        let mut labels = Vec::with_capacity(label_count);
        for _ in 0..label_count {
            labels.push(u16::from(reader.read_u8()?));
        }
        transforms.push(LegacySkeletonTransform {
            transform_type,
            labels,
        });
    }

    if reader.remaining() > 0 {
        let extension_offset = reader.offset();
        let extension_count = reader.read_u16_be()?;
        if extension_count != 0 {
            return Err(reader.invalid_value(
                "legacy skeleton extension",
                format!(
                    "cached-model skeletal extension count {extension_count} is outside the M8 legacy frame codec"
                ),
                ByteSpan::new(extension_offset, 2),
                None,
            ));
        }
    }

    reader.finish()?;
    Ok(transforms)
}

/// Read the skeleton id embedded in a legacy frame file.
///
/// Callers use this id to load the matching skeleton group before invoking the
/// full frame decoder. The packed sequence `FrameId` still identifies the frame
/// group/file; the embedded skeleton id is a separate cache reference.
pub fn legacy_frame_skeleton_id(
    frame_id: FrameId,
    bytes: &[u8],
    context: &DecoderContext,
    source: &ArchiveFileProvenance,
) -> DecodeResult<u16> {
    let mut reader = BinaryReader::new(bytes, context, source)
        .with_subject(DecodeSubject::Frame(frame_id.get()));
    reader.read_u16_be()
}

/// Decode one legacy animation frame into the canonical pose representation.
///
/// The decoder ports the pinned `Animation` constructor exactly for the owned
/// legacy fields: transform flags are read from the header stream while signed
/// short-smart values come from the payload stream; an omitted scale component
/// defaults to 128; and an unflagged zero-type transform between authored
/// transforms is inserted as the pivot transform expected by `Model.animate`.
pub fn decode_legacy_animation_frame(
    frame_id: FrameId,
    bytes: &[u8],
    expected_skeleton_id: u16,
    skeleton: &[LegacySkeletonTransform],
    context: &DecoderContext,
    source: &ArchiveFileProvenance,
) -> DecodeResult<LegacyAnimationFrame> {
    let mut flags = BinaryReader::new(bytes, context, source)
        .with_subject(DecodeSubject::Frame(frame_id.get()));
    let embedded_skeleton_id = flags.read_u16_be()?;
    if embedded_skeleton_id != expected_skeleton_id {
        return Err(flags.invalid_value(
            "frame skeleton id",
            format!(
                "embedded skeleton {embedded_skeleton_id} does not match loaded skeleton {expected_skeleton_id}"
            ),
            ByteSpan::new(0, 2),
            None,
        ));
    }

    let transform_slots = usize::from(flags.read_u8()?);
    if transform_slots > skeleton.len() {
        return Err(flags.invalid_value(
            "frame transform slot count",
            format!(
                "frame references {transform_slots} skeleton slots but skeleton has {}",
                skeleton.len()
            ),
            ByteSpan::new(2, 1),
            None,
        ));
    }

    let values_offset = 3 + transform_slots;
    let mut values = flags.fork_at(values_offset)?;
    let mut transforms = Vec::with_capacity(transform_slots);
    let mut last_authored_slot = None;

    for slot in 0..transform_slots {
        let mask = flags.read_u8()?;
        if mask == 0 {
            continue;
        }

        if skeleton[slot].transform_type != 0 {
            let search_start = last_authored_slot.map_or(0, |last| last + 1);
            if let Some(pivot_slot) = (search_start..slot)
                .rev()
                .find(|candidate| skeleton[*candidate].transform_type == 0)
            {
                transforms.push(LegacyFrameTransform {
                    skeleton_transform: pivot_slot,
                    x: 0,
                    y: 0,
                    z: 0,
                });
            }
        }

        let default = if skeleton[slot].transform_type == 3 {
            128
        } else {
            0
        };
        let x = if mask & 1 != 0 {
            values.read_short_smart()?
        } else {
            default
        };
        let y = if mask & 2 != 0 {
            values.read_short_smart()?
        } else {
            default
        };
        let z = if mask & 4 != 0 {
            values.read_short_smart()?
        } else {
            default
        };

        transforms.push(LegacyFrameTransform {
            skeleton_transform: slot,
            x,
            y,
            z,
        });
        last_authored_slot = Some(slot);
    }

    values.finish()?;
    Ok(LegacyAnimationFrame {
        skeleton: skeleton.to_vec(),
        transforms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::{DecodeErrorKind, test_support};

    fn skeleton_source(id: u16) -> ArchiveFileProvenance {
        ArchiveFileProvenance::new(1, u32::from(id), Some(0))
    }

    fn frame_source(group: u32, file: u32) -> ArchiveFileProvenance {
        ArchiveFileProvenance::new(0, group, Some(file))
    }

    #[test]
    fn skeleton_decodes_types_and_label_groups_in_reference_order()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = skeleton_source(0x1234);
        let bytes = [2, 0, 5, 1, 2, 7, 8, 9];

        let decoded = decode_legacy_skeleton(0x1234, &bytes, &context, &source)?;

        assert_eq!(
            decoded,
            vec![
                LegacySkeletonTransform {
                    transform_type: 0,
                    labels: vec![7],
                },
                LegacySkeletonTransform {
                    transform_type: 5,
                    labels: vec![8, 9],
                },
            ]
        );
        Ok(())
    }

    #[test]
    fn skeleton_accepts_zero_extension_marker_but_rejects_unowned_modern_extension()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = skeleton_source(77);

        let decoded = decode_legacy_skeleton(77, &[1, 1, 1, 0, 0, 0], &context, &source)?;
        assert_eq!(decoded.len(), 1);

        let error = decode_legacy_skeleton(77, &[1, 1, 1, 0, 0, 1], &context, &source)
            .expect_err("nonzero skeletal extension must remain explicit");
        assert_eq!(error.subject(), Some(&DecodeSubject::Skeleton(77)));
        assert!(matches!(
            error.kind(),
            DecodeErrorKind::InvalidValue {
                field: "legacy skeleton extension",
                ..
            }
        ));
        Ok(())
    }

    #[test]
    fn frame_header_exposes_embedded_skeleton_id() -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = frame_source(0x12, 0x34);
        let frame_id = FrameId::new(0x0012_0034);

        assert_eq!(
            legacy_frame_skeleton_id(frame_id, &[0xab, 0xcd, 0], &context, &source)?,
            0xabcd
        );
        Ok(())
    }

    #[test]
    fn frame_inserts_missing_zero_type_pivot_and_decodes_short_smart()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = frame_source(0x12, 0x34);
        let frame_id = FrameId::new(0x0012_0034);
        let skeleton = vec![
            LegacySkeletonTransform {
                transform_type: 0,
                labels: vec![0],
            },
            LegacySkeletonTransform {
                transform_type: 1,
                labels: vec![1],
            },
        ];
        let bytes = [0x12, 0x34, 2, 0, 1, 69];

        let decoded =
            decode_legacy_animation_frame(frame_id, &bytes, 0x1234, &skeleton, &context, &source)?;

        assert_eq!(
            decoded.transforms,
            vec![
                LegacyFrameTransform {
                    skeleton_transform: 0,
                    x: 0,
                    y: 0,
                    z: 0,
                },
                LegacyFrameTransform {
                    skeleton_transform: 1,
                    x: 5,
                    y: 0,
                    z: 0,
                },
            ]
        );
        Ok(())
    }

    #[test]
    fn omitted_scale_components_default_to_reference_128() -> Result<(), Box<dyn std::error::Error>>
    {
        let context = test_support::target_context()?;
        let source = frame_source(1, 0);
        let frame_id = FrameId::new(0x0001_0000);
        let skeleton = vec![LegacySkeletonTransform {
            transform_type: 3,
            labels: vec![0],
        }];
        let bytes = [0, 9, 1, 1, 65];

        let decoded =
            decode_legacy_animation_frame(frame_id, &bytes, 9, &skeleton, &context, &source)?;

        assert_eq!(
            decoded.transforms,
            vec![LegacyFrameTransform {
                skeleton_transform: 0,
                x: 1,
                y: 128,
                z: 128,
            }]
        );
        Ok(())
    }

    #[test]
    fn frame_rejects_skeleton_mismatch_slot_overrun_and_trailing_payload()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let source = frame_source(1, 0);
        let frame_id = FrameId::new(0x0001_0000);
        let skeleton = vec![LegacySkeletonTransform {
            transform_type: 1,
            labels: vec![0],
        }];

        let mismatch =
            decode_legacy_animation_frame(frame_id, &[0, 8, 1, 0], 9, &skeleton, &context, &source)
                .expect_err("mismatched skeleton id must fail");
        assert!(matches!(
            mismatch.kind(),
            DecodeErrorKind::InvalidValue {
                field: "frame skeleton id",
                ..
            }
        ));

        let overrun = decode_legacy_animation_frame(
            frame_id,
            &[0, 9, 2, 0, 0],
            9,
            &skeleton,
            &context,
            &source,
        )
        .expect_err("slot overrun must fail");
        assert!(matches!(
            overrun.kind(),
            DecodeErrorKind::InvalidValue {
                field: "frame transform slot count",
                ..
            }
        ));

        let trailing = decode_legacy_animation_frame(
            frame_id,
            &[0, 9, 1, 1, 65, 0],
            9,
            &skeleton,
            &context,
            &source,
        )
        .expect_err("trailing frame payload must fail");
        assert_eq!(
            trailing.kind(),
            &DecodeErrorKind::TrailingBytes { remaining: 1 }
        );
        Ok(())
    }
}
