//! Real single-flight worker results with a synthetic triangle; no cache or animation-math oracle.
use super::*;
use crate::app::world_runtime::test_support::*;
use osrs_core::{
    coords::{MapTile, ModelPoint},
    definitions::{DefinitionIdentity, SequenceDefinition},
    ids::{FrameId, ModelId, SequenceId},
    lighting::{LightingParameters, light_model_data},
    model::{
        FacePriority, ModelEncoding, ModelFormatIdentity, SourceModel, SourceModelParts, Triangle,
    },
};
use osrs_world::{AnimatedInstance, AnimatedModel};
use std::time::Instant;

type TestResult = Result<(), Box<dyn std::error::Error>>;

pub(in crate::app) fn animated_candidate() -> Result<RegionGeometry, Box<dyn std::error::Error>> {
    let mut candidate = candidate();
    let target = (*candidate.target).clone();
    let source = SourceModel::from_parts(SourceModelParts {
        identity: DefinitionIdentity::new(ModelId::new(77), target.clone()),
        format: ModelFormatIdentity {
            encoding: ModelEncoding::TrailerFfFd,
            version: Some(15),
        },
        vertices: vec![
            ModelPoint::new(0, 0, 0),
            ModelPoint::new(128, 0, 0),
            ModelPoint::new(0, 64, 128),
        ],
        faces: vec![Triangle::new(0, 1, 2)],
        face_colors: vec![500],
        default_priority: FacePriority::new(0).ok_or("priority")?,
        face_render_types: None,
        face_priorities: None,
        face_alphas: None,
        face_textures: None,
        texture_face_selectors: None,
        face_biases: None,
        texture_triangles: vec![],
        vertex_skins: None,
        face_skins: None,
        skeletal_vertices: None,
    })?;
    let base = light_model_data(&source.to_working_copy(), LightingParameters::for_loc(0, 0))?;
    let sequence = SequenceDefinition {
        identity: DefinitionIdentity::new(SequenceId::new(1), target),
        frame_ids: vec![FrameId::new(1)],
        frame_delays: vec![1000],
        frame_step: Some(1),
        interleave: None,
        max_loops: 99,
        precedence_animating: 0,
        priority: 0,
        restart_mode: 0,
        left_hand_item: None,
        right_hand_item: None,
        skeletal_animation: None,
        skeletal_range: None,
        skeletal_mask: None,
    };
    candidate.animations.push(AnimatedInstance {
        model: Arc::new(AnimatedModel {
            base: Arc::new(base),
            sequence: Arc::new(sequence),
            frames: vec![],
            orientation: 0,
            contour: None,
            random_start: false,
        }),
        hd: None,
        zone: (400, 400),
        level: 0,
        min_plane: 0,
        roof: 0,
        local: (0, 0, 0),
        rotation: 0,
    });
    Ok(candidate)
}

pub(in crate::app) fn finish(world: &mut WorldRuntime) -> Option<AnimationArtifact> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let artifact = world.poll_animation_geometry();
        if world.animations.is_idle() {
            return artifact;
        }
        assert!(Instant::now() < deadline, "animation worker stalled");
        std::thread::yield_now();
    }
}

#[test]
fn aggregate_identity_captures_exact_members_and_dispatch_inputs_without_rejecting_clock_lag()
-> TestResult {
    let mut world = world();
    let candidate = animated_candidate()?;
    let member = identity(&world, &candidate);
    world.region_installed(Arc::clone(&member), &RegionBuildScope::Full, candidate);
    world.set_animation_smoothing(true);
    world.schedule_animation(true, 0.1, (3200, 3200));
    let issued = Arc::clone(world.animation.in_flight.as_ref().ok_or("issued")?);
    // A later camera/clock sample does not issue a second job or relabel the completed one.
    world.schedule_animation(true, 0.3, (3220, 3200));
    assert!(Arc::ptr_eq(
        world.animation.in_flight.as_ref().ok_or("slot")?,
        &issued
    ));
    let artifact = finish(&mut world).ok_or("artifact")?;
    assert!(Arc::ptr_eq(&artifact.identity, &issued));
    assert!(Arc::ptr_eq(
        artifact.identity.members.get(&KEY).ok_or("member")?,
        &member
    ));
    assert_eq!(artifact.geometry.vertex_count(), 3);
    assert_eq!((issued.cycle, issued.advance_cycles), (5, 5));
    assert_eq!(
        (
            issued.center,
            issued.radius,
            issued.smoothing,
            issued.frozen
        ),
        ((3200, 3200), 80, true, false)
    );
    let description = issued.describe(MapTile::new(3200, 3200)).join("\n");
    assert!(description.contains("cycle 5; advance 5"));
    assert!(description.contains(&member.describe()[0]));
    assert!(world.animation.in_flight.is_none());
    world.schedule_animation(true, 0.3, (3220, 3200));
    let next = finish(&mut world).ok_or("recentered artifact")?;
    assert_eq!(
        (
            next.identity.cycle,
            next.identity.advance_cycles,
            next.identity.center
        ),
        (15, 10, (3220, 3200))
    );
    assert!(Arc::ptr_eq(&issued.members, &next.identity.members));
    Ok(())
}

#[test]
fn noop_completion_retires_only_the_pending_context_and_next_force_has_its_own_identity()
-> TestResult {
    let mut world = world();
    seed_installed(&mut world, &animated_candidate()?);
    world.schedule_animation(true, 0.1, (3200, 3200));
    let first = finish(&mut world).ok_or("first")?;
    world.schedule_animation(true, 0.2, (3200, 3200));
    assert!(world.animation.in_flight.is_some());
    assert!(
        finish(&mut world).is_none(),
        "unchanged frame returns no geometry"
    );
    assert!(world.animation.in_flight.is_none());
    world.mark_animation_dirty();
    world.schedule_animation(true, 0.3, (3200, 3200));
    let forced = finish(&mut world).ok_or("forced")?;
    assert_eq!(
        (
            first.identity.cycle,
            forced.identity.cycle,
            forced.identity.advance_cycles
        ),
        (5, 15, 5)
    );
    assert!(Arc::ptr_eq(
        &first.identity.members,
        &forced.identity.members
    ));
    Ok(())
}

#[test]
fn membership_snapshot_survives_replacement_then_retires_with_the_last_artifact() -> TestResult {
    let mut world = world();
    let candidate = animated_candidate()?;
    let old = identity(&world, &candidate);
    world.region_installed(Arc::clone(&old), &RegionBuildScope::Full, candidate.clone());
    world.schedule_animation(true, 0.1, (3200, 3200));
    let first = finish(&mut world).ok_or("first")?;
    let old_members = Arc::downgrade(&first.identity.members);
    let mut changed = candidate;
    changed.presentation.smooth_terrain = true;
    let new = identity(&world, &changed);
    world.region_installed(Arc::clone(&new), &RegionBuildScope::Full, changed);
    assert!(Arc::ptr_eq(
        first.identity.members.get(&KEY).ok_or("old")?,
        &old
    ));
    world.schedule_animation(true, 0.2, (3200, 3200));
    let second = finish(&mut world).ok_or("second")?;
    assert!(Arc::ptr_eq(
        second.identity.members.get(&KEY).ok_or("new")?,
        &new
    ));
    drop(first);
    assert!(
        old_members.upgrade().is_none(),
        "no retained scene generation"
    );
    world.region_unavailable(KEY);
    world.schedule_animation(true, 0.3, (3200, 3200));
    let empty = finish(&mut world).ok_or("empty replacement")?;
    assert_eq!(empty.geometry.vertex_count(), 0);
    assert!(empty.identity.members.is_empty());
    assert_eq!(second.geometry.vertex_count(), 3);
    Ok(())
}

#[test]
fn aggregate_keeps_each_nonempty_producer_and_empty_replacement_removes_only_its_member()
-> TestResult {
    let mut world = world();
    seed_installed(&mut world, &animated_candidate()?);
    let mut second = animated_candidate()?;
    second.region = RegionCoord::new(51, 50);
    second.animations[0].zone = (408, 400);
    let member = identity(&world, &second);
    world.install_animation_region(Arc::clone(&member), second.animations);
    world.schedule_animation(true, 0.1, (3200, 3200));
    let both = finish(&mut world).ok_or("aggregate")?;
    assert_eq!(both.geometry.vertex_count(), 6);
    assert_eq!(
        both.identity.members.keys().copied().collect::<Vec<_>>(),
        vec![KEY, (51, 50)]
    );
    assert!(Arc::ptr_eq(
        both.identity.members.get(&(51, 50)).ok_or("second")?,
        &member
    ));
    let empty = candidate();
    world.region_installed(identity(&world, &empty), &RegionBuildScope::Full, empty);
    world.schedule_animation(true, 0.2, (3200, 3200));
    let survivor = finish(&mut world).ok_or("survivor")?;
    assert_eq!(survivor.geometry.vertex_count(), 3);
    assert!(!survivor.identity.members.contains_key(&KEY));
    assert!(Arc::ptr_eq(
        survivor
            .identity
            .members
            .get(&(51, 50))
            .ok_or("survivor input")?,
        &member
    ));
    assert_eq!(both.identity.members.len(), 2);
    Ok(())
}

#[test]
fn source_replacement_clears_old_members_before_any_new_source_request() -> TestResult {
    let mut world = world();
    seed_installed(&mut world, &animated_candidate()?);
    world.schedule_animation(true, 0.1, (3200, 3200));
    drop(world.replace_streamer(WorldRuntime::spawn_streamer(source())));
    assert!(finish(&mut world).is_none());
    world.schedule_animation(true, 0.2, (3200, 3200));
    let empty = finish(&mut world).ok_or("replacement")?;
    assert_eq!(empty.geometry.vertex_count(), 0);
    assert!(empty.identity.members.is_empty());
    assert_eq!(empty.identity.source, 1);
    Ok(())
}
