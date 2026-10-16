use super::*;
use crate::{CollisionSideEffect, Footprint};
use osrs_core::{
    coords::{MapTile, PlaneIndex, RegionCoord, SceneTile, SourcePlane},
    definitions::LocType,
    ids::ObjectId,
};
use sha2::{Digest, Sha256};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn loaded_builder() -> CollisionSnapshotBuilder {
    let mut builder = CollisionSnapshotBuilder::reference(MapTile::new(0, 0));
    for x in 0..2 {
        for y in 0..2 {
            builder.load_region(RegionCoord::new(x, y));
        }
    }
    builder
}

fn object(
    source: u8,
    tile: MapTile,
    effect: CollisionSideEffect,
) -> Result<CollisionObject, &'static str> {
    Ok(CollisionObject {
        object_id: ObjectId::new(42),
        tile,
        source_plane: SourcePlane::new(source).ok_or("source plane")?,
        loc_type: LocType::new(10),
        orientation: 0,
        effect,
    })
}

#[test]
fn initial_writes_match_all_pinned_java_cases() -> TestResult {
    let source = SourcePlane::new(0).ok_or("source plane")?;
    let plane = source.index();
    let fixture = include_str!("../../tests/fixtures/collision-deob.txt");
    let mut cases = 0;
    for row in fixture
        .lines()
        .filter(|row| !row.starts_with('#') && !row.is_empty())
    {
        let columns: Vec<_> = row.split_whitespace().collect();
        assert_eq!(columns.len(), 10, "{row}");
        let values = columns[1..9]
            .iter()
            .map(|value| value.parse::<u32>())
            .collect::<Result<Vec<_>, _>>()?;
        let [loaded, loc_type, angle, projectile, x, y, width, height] = values[..] else {
            return Err("case layout".into());
        };
        let tile = MapTile::new(x as i32, y as i32);
        let mut builder = if loaded != 0 {
            loaded_builder()
        } else {
            CollisionSnapshotBuilder::reference(MapTile::new(0, 0))
        };
        let effect = match columns[0] {
            "empty" => CollisionSideEffect::None,
            "floor" => {
                builder.terrain(source, tile, 1, false);
                CollisionSideEffect::None
            }
            "decor" => CollisionSideEffect::FloorDecorationBlock,
            "wall" => CollisionSideEffect::Boundary {
                loc_type: LocType::new(loc_type as u8),
                orientation: angle as u8,
                blocks_projectiles: projectile != 0,
            },
            "object" => CollisionSideEffect::GameObject {
                footprint: Footprint::new(width as u16, height as u16),
                blocks_projectiles: projectile != 0,
            },
            other => return Err(format!("unknown oracle kind {other}").into()),
        };
        builder.object(object(0, tile, effect)?, false)?;
        let snapshot = builder.build();
        let mut digest = Sha256::new();
        for x in 0..104 {
            for y in 0..104 {
                digest.update(
                    snapshot
                        .cell(plane, MapTile::new(x, y))
                        .ok_or("oracle cell")?
                        .flags
                        .to_be_bytes(),
                );
            }
        }
        assert_eq!(format!("{:x}", digest.finalize()), columns[9], "{row}");
        cases += 1;
    }
    assert_eq!(cases, 94, "oracle inventory cannot silently shrink");
    Ok(())
}

#[test]
fn bridge_projects_clipping_but_keeps_original_provenance() -> TestResult {
    let mut builder = loaded_builder();
    let tile = MapTile::new(32, 32);
    let zero = SourcePlane::new(0).ok_or("source plane")?;
    let one = SourcePlane::new(1).ok_or("source plane")?;
    let two = SourcePlane::new(2).ok_or("source plane")?;
    builder.terrain(zero, tile, 1, true);
    builder.terrain(one, tile, 1, true);
    builder.object(
        object(2, tile, CollisionSideEffect::FloorDecorationBlock)?,
        true,
    )?;
    builder.object(
        object(0, tile, CollisionSideEffect::FloorDecorationBlock)?,
        true,
    )?;
    let snapshot = builder.build();
    let ground = snapshot.cell(zero.index(), tile).ok_or("cell")?;
    assert_eq!(ground.flags, FLOOR);
    assert_eq!(ground.terrain_source_planes, 1 << 1);
    assert_eq!(
        snapshot.cell(one.index(), tile).ok_or("cell")?.flags,
        FLOOR_DECORATION
    );
    assert_eq!(snapshot.cell(two.index(), tile).ok_or("cell")?.flags, 0);
    let causes: Vec<_> = snapshot.objects_at(one.index(), tile).collect();
    assert_eq!(causes.len(), 1);
    assert_eq!(causes[0].0.source_plane, two);
    Ok(())
}

#[test]
fn neighbouring_wall_and_overlapping_objects_keep_all_causes() -> TestResult {
    let tile = MapTile::new(32, 32);
    let plane = PlaneIndex::new(0).ok_or("plane")?;
    let mut builder = loaded_builder();
    builder.object(
        object(
            0,
            tile,
            CollisionSideEffect::Boundary {
                loc_type: LocType::new(0),
                orientation: 0,
                blocks_projectiles: true,
            },
        )?,
        false,
    )?;
    for id in [1, 2] {
        let mut item = object(
            0,
            MapTile::new(31, 32),
            CollisionSideEffect::GameObject {
                footprint: Footprint::new(2, 3),
                blocks_projectiles: false,
            },
        )?;
        item.object_id = ObjectId::new(id);
        builder.object(item, false)?;
    }
    let snapshot = builder.build();
    let causes: Vec<_> = snapshot.objects_at(plane, MapTile::new(31, 32)).collect();
    assert_eq!(causes.len(), 3);
    assert_eq!(causes[0].1, 8 | 4096);
    assert_eq!(causes[0].0.tile, tile);
    assert_eq!(
        snapshot
            .cell(plane, MapTile::new(31, 32))
            .ok_or("cell")?
            .flags,
        OBJECT | 8 | 4096
    );
    Ok(())
}

#[test]
fn unloaded_land_padding_and_outside_are_never_reported_as_loaded() -> TestResult {
    let mut builder = CollisionSnapshotBuilder::reference(MapTile::new(-64, -64));
    builder.load_region(RegionCoord::new(-1, -1));
    let snapshot = builder.build();
    let plane = PlaneIndex::new(0).ok_or("plane")?;
    assert!(
        snapshot
            .cell(plane, MapTile::new(-32, -32))
            .ok_or("loaded")?
            .loaded
    );
    assert!(
        !snapshot
            .cell(plane, MapTile::new(20, 20))
            .ok_or("missing")?
            .loaded
    );
    assert!(
        !snapshot
            .cell(plane, MapTile::new(-64, -64))
            .ok_or("padding")?
            .loaded
    );
    assert!(
        snapshot
            .cell(plane, MapTile::new(i32::MAX, i32::MAX))
            .is_none()
    );
    assert_eq!(
        snapshot
            .objects_at(plane, MapTile::new(i32::MIN, i32::MIN))
            .count(),
        0
    );
    Ok(())
}

#[test]
fn invalid_boundaries_fail_and_oversized_rectangles_clip_without_large_allocation() -> TestResult {
    for (kind, orientation) in [(4, 0), (0, 4), (255, 255)] {
        assert!(
            CollisionStencil::from_effect(CollisionSideEffect::Boundary {
                loc_type: LocType::new(kind),
                orientation,
                blocks_projectiles: true,
            })
            .is_err()
        );
    }
    let stencil = CollisionStencil::from_effect(CollisionSideEffect::GameObject {
        footprint: Footprint::new(u16::MAX, u16::MAX),
        blocks_projectiles: false,
    })?;
    let mut count = 0;
    stencil.visit(SceneTile::new(102, 102), 104, 104, |_, _| count += 1);
    assert_eq!(count, 4);
    Ok(())
}

#[test]
fn rebuild_removes_only_the_deleted_contribution_and_missing_defs_are_explicit() -> TestResult {
    let tile = MapTile::new(32, 32);
    let plane = PlaneIndex::new(0).ok_or("plane")?;
    let build = |count| -> Result<_, Box<dyn std::error::Error>> {
        let mut builder = loaded_builder();
        for _ in 0..count {
            builder.object(
                object(0, tile, CollisionSideEffect::FloorDecorationBlock)?,
                false,
            )?;
        }
        if count == 0 {
            builder.missing_definition();
        }
        Ok(builder.build())
    };
    for count in [2, 1] {
        let snapshot = build(count)?;
        assert_eq!(
            snapshot.cell(plane, tile).ok_or("cell")?.flags,
            FLOOR_DECORATION
        );
        assert_eq!(snapshot.objects_at(plane, tile).count(), count);
    }
    let removed = build(0)?;
    assert_eq!(removed.cell(plane, tile).ok_or("cell")?.flags, 0);
    assert_eq!(removed.unresolved_definitions(), 1);
    Ok(())
}
