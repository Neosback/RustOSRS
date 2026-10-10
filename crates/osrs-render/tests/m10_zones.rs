use osrs_core::coords::StoragePlane;
use osrs_render::{
    RenderTileBounds, RenderZoneError, RenderZoneKey, SemanticGeneration, ZoneInvalidationTracker,
};

fn plane(value: u8) -> Result<StoragePlane, Box<dyn std::error::Error>> {
    StoragePlane::new(value).ok_or_else(|| format!("invalid storage plane {value}").into())
}

fn generation(serial: u64, fill: u8) -> SemanticGeneration {
    SemanticGeneration::new(serial, [fill; 32])
}

#[test]
fn zone_key_uses_storage_plane_and_euclidean_eight_tile_partitioning()
-> Result<(), Box<dyn std::error::Error>> {
    let p2 = plane(2)?;

    assert_eq!(
        RenderZoneKey::from_tile(p2, 0, 0),
        RenderZoneKey::new(p2, 0, 0)
    );
    assert_eq!(
        RenderZoneKey::from_tile(p2, 7, 7),
        RenderZoneKey::new(p2, 0, 0)
    );
    assert_eq!(
        RenderZoneKey::from_tile(p2, 8, 15),
        RenderZoneKey::new(p2, 1, 1)
    );
    assert_eq!(
        RenderZoneKey::from_tile(p2, -1, -1),
        RenderZoneKey::new(p2, -1, -1)
    );
    assert_eq!(
        RenderZoneKey::from_tile(p2, -8, -8),
        RenderZoneKey::new(p2, -1, -1)
    );
    assert_eq!(
        RenderZoneKey::from_tile(p2, -9, -9),
        RenderZoneKey::new(p2, -2, -2)
    );
    assert_ne!(
        RenderZoneKey::from_tile(plane(1)?, 0, 0),
        RenderZoneKey::from_tile(p2, 0, 0)
    );
    Ok(())
}

#[test]
fn footprint_enumerates_every_touched_zone_in_stable_z_major_order()
-> Result<(), Box<dyn std::error::Error>> {
    let p1 = plane(1)?;
    let bounds = RenderTileBounds::from_min_size(7, 7, 3, 10)?;

    assert_eq!((bounds.min_x(), bounds.min_z()), (7, 7));
    assert_eq!((bounds.max_x(), bounds.max_z()), (9, 16));
    assert_eq!(
        bounds.zones(p1).collect::<Vec<_>>(),
        vec![
            RenderZoneKey::new(p1, 0, 0),
            RenderZoneKey::new(p1, 1, 0),
            RenderZoneKey::new(p1, 0, 1),
            RenderZoneKey::new(p1, 1, 1),
            RenderZoneKey::new(p1, 0, 2),
            RenderZoneKey::new(p1, 1, 2),
        ]
    );
    Ok(())
}

#[test]
fn bounds_reject_zero_extent_and_signed_range_overflow() {
    assert_eq!(
        RenderTileBounds::from_min_size(0, 0, 0, 1),
        Err(RenderZoneError::ZeroWidth)
    );
    assert_eq!(
        RenderTileBounds::from_min_size(0, 0, 1, 0),
        Err(RenderZoneError::ZeroDepth)
    );
    assert_eq!(
        RenderTileBounds::from_min_size(0, 0, u32::MAX, 1),
        Err(RenderZoneError::WidthTooLarge {
            width_tiles: u32::MAX,
        })
    );
    assert_eq!(
        RenderTileBounds::from_min_size(i32::MAX, 0, 2, 1),
        Err(RenderZoneError::XBoundsOverflow)
    );
    assert_eq!(
        RenderTileBounds::from_min_size(0, i32::MAX, 1, 2),
        Err(RenderZoneError::ZBoundsOverflow)
    );
}

#[test]
fn changed_zone_rejects_old_generation_build_ticket() -> Result<(), Box<dyn std::error::Error>> {
    let key = RenderZoneKey::new(plane(0)?, 4, 5);
    let first = generation(10, 0x10);
    let second = generation(11, 0x11);
    let mut tracker = ZoneInvalidationTracker::new(first);

    tracker.invalidate_zone(key);
    let old_ticket = tracker
        .begin_build(key)
        .ok_or("missing first build ticket")?;
    assert_eq!(old_ticket.key(), key);
    assert_eq!(old_ticket.required_generation(), first);

    tracker.advance_generation(second, [key])?;
    assert_eq!(tracker.current_generation(), second);
    assert_eq!(tracker.required_generation(key), Some(second));
    assert_eq!(
        tracker.complete_build(old_ticket),
        Err(RenderZoneError::StaleBuildTicket {
            key,
            expected_serial: 11,
            ticket_serial: 10,
        })
    );

    let current_ticket = tracker
        .begin_build(key)
        .ok_or("missing current build ticket")?;
    tracker.complete_build(current_ticket)?;
    assert!(!tracker.is_dirty(key));
    assert_eq!(
        tracker.complete_build(current_ticket),
        Err(RenderZoneError::ZoneAlreadyClean { key })
    );
    Ok(())
}

#[test]
fn unchanged_zone_can_finish_older_ticket_after_global_generation_advance()
-> Result<(), Box<dyn std::error::Error>> {
    let p0 = plane(0)?;
    let unchanged = RenderZoneKey::new(p0, 1, 1);
    let changed = RenderZoneKey::new(p0, 2, 1);
    let first = generation(20, 0x20);
    let second = generation(21, 0x21);
    let mut tracker = ZoneInvalidationTracker::new(first);

    tracker.invalidate_zone(unchanged);
    tracker.invalidate_zone(changed);
    let unchanged_ticket = tracker
        .begin_build(unchanged)
        .ok_or("missing unchanged-zone ticket")?;
    let changed_ticket = tracker
        .begin_build(changed)
        .ok_or("missing changed-zone ticket")?;

    tracker.advance_generation(second, [changed])?;

    tracker.complete_build(unchanged_ticket)?;
    assert!(!tracker.is_dirty(unchanged));
    assert_eq!(tracker.required_generation(unchanged), Some(first));

    assert_eq!(
        tracker.complete_build(changed_ticket),
        Err(RenderZoneError::StaleBuildTicket {
            key: changed,
            expected_serial: 21,
            ticket_serial: 20,
        })
    );
    assert!(tracker.is_dirty(changed));
    Ok(())
}

#[test]
fn invalidating_bounds_tracks_dirty_zones_in_stable_key_order()
-> Result<(), Box<dyn std::error::Error>> {
    let p3 = plane(3)?;
    let mut tracker = ZoneInvalidationTracker::new(generation(30, 0x30));
    let bounds = RenderTileBounds::from_min_size(-1, 7, 10, 2)?;

    tracker.invalidate_bounds(p3, bounds);

    assert_eq!(
        tracker.dirty_zones().collect::<Vec<_>>(),
        vec![
            RenderZoneKey::new(p3, -1, 0),
            RenderZoneKey::new(p3, -1, 1),
            RenderZoneKey::new(p3, 0, 0),
            RenderZoneKey::new(p3, 0, 1),
            RenderZoneKey::new(p3, 1, 0),
            RenderZoneKey::new(p3, 1, 1),
        ]
    );
    Ok(())
}

#[test]
fn generation_advance_must_be_strictly_monotonic() -> Result<(), Box<dyn std::error::Error>> {
    let mut tracker = ZoneInvalidationTracker::new(generation(40, 0x40));

    assert_eq!(
        tracker.advance_generation(generation(40, 0x41), []),
        Err(RenderZoneError::GenerationNotNewer {
            current_serial: 40,
            next_serial: 40,
        })
    );
    assert_eq!(
        tracker.advance_generation(generation(39, 0x39), []),
        Err(RenderZoneError::GenerationNotNewer {
            current_serial: 40,
            next_serial: 39,
        })
    );
    Ok(())
}

#[test]
fn repeated_same_generation_invalidation_is_idempotent_for_build_ownership()
-> Result<(), Box<dyn std::error::Error>> {
    let key = RenderZoneKey::new(plane(2)?, -3, 8);
    let mut tracker = ZoneInvalidationTracker::new(generation(50, 0x50));

    tracker.invalidate_zone(key);
    let ticket = tracker.begin_build(key).ok_or("missing build ticket")?;
    tracker.invalidate_zone(key);
    tracker.complete_build(ticket)?;

    assert!(!tracker.is_dirty(key));
    Ok(())
}
