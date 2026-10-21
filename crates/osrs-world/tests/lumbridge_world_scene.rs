//! Full cache -> world scene assembly on the real Lumbridge window.
//!
//! `RUSTOSRS_TARGET_CACHE_DIR=... cargo test --release -p osrs-world -- --ignored --nocapture`

use osrs_world::{SceneWindow, TerrainPresentation, WorldDefinitions, build_world_scene};

#[test]
#[ignore = "requires the pinned build-241 cache (RUSTOSRS_TARGET_CACHE_DIR)"]
fn lumbridge_window_assembles_end_to_end() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::var("RUSTOSRS_TARGET_CACHE_DIR")?;
    let mut definitions = WorldDefinitions::open(dir)?;
    let window = SceneWindow::new(3176, 3176).ok_or("window alignment")?;
    let scene = build_world_scene(&mut definitions, window, TerrainPresentation::default())?;

    println!("loc stats: {:?}", scene.loc_stats);
    println!("merge report: {:?}", scene.merge_report);
    println!("lit flat models: {}", scene.lit.len());
    println!("locs recorded: {}", scene.locs.len());

    assert!(scene.loc_stats.decoded > 1_000);
    assert!(scene.loc_stats.placed > 500);
    assert!(!scene.lit.is_empty());
    Ok(())
}

#[test]
#[ignore = "requires the pinned build-241 cache (RUSTOSRS_TARGET_CACHE_DIR)"]
fn region_artifact_retains_exact_source_and_extraction_inputs()
-> Result<(), Box<dyn std::error::Error>> {
    use osrs_core::coords::RegionCoord;
    use osrs_world::{RegionRenderPath, build_region_geometry};
    let mut definitions = WorldDefinitions::open(std::env::var("RUSTOSRS_TARGET_CACHE_DIR")?)?;
    let target = definitions.context().target_provenance().clone();
    let presentation = TerrainPresentation {
        smooth_terrain: true,
        hd_geometry: true,
        ..Default::default()
    };
    // Requested HD without an HD context is the explicitly distinguished Reference fallback.
    let artifact = build_region_geometry(&mut definitions, RegionCoord::new(50, 50), presentation)?
        .ok_or("missing Lumbridge source")?;
    assert_eq!(artifact.target.as_ref(), &target);
    assert_eq!(artifact.presentation, presentation);
    assert_eq!(artifact.render_path, RegionRenderPath::ReferenceFallback);
    assert!(!artifact.geometry.zones.is_empty());
    assert_eq!(artifact.region, RegionCoord::new(50, 50));
    Ok(())
}

#[test]
#[ignore = "requires the pinned build-241 cache (RUSTOSRS_TARGET_CACHE_DIR)"]
fn actor_results_capture_real_decoder_target_generation_and_reference_fallback()
-> Result<(), Box<dyn std::error::Error>> {
    use osrs_core::{
        actor_snapshot::{PlaytestIdentity, PlaytestSnapshot, PlaytestStamp},
        coords::{MapTile, PlaneIndex},
        player::PlayerLook,
        scenario::ActorLocation,
    };
    use osrs_world::{
        RegionRenderPath,
        npc::{NpcFrameInput, PlayerFrame, extract_npcs, extract_player_frame},
    };
    let mut definitions = WorldDefinitions::open(std::env::var("RUSTOSRS_TARGET_CACHE_DIR")?)?;
    let target = definitions.context().target_provenance().clone();
    let mut reference_player = None;
    for (requested, path) in [
        (false, RegionRenderPath::Reference),
        (true, RegionRenderPath::ReferenceFallback),
    ] {
        let stamp = PlaytestStamp {
            identity: PlaytestIdentity::default(),
            server_tick: 3,
            client_cycle: 90,
        };
        let npc = extract_npcs(
            &mut definitions,
            NpcFrameInput {
                snapshot: PlaytestSnapshot {
                    stamp,
                    actors: vec![],
                },
                ground_heights: Default::default(),
                varps: Default::default(),
            },
            None,
            7,
            requested,
        );
        assert_eq!(npc.stamp, stamp);
        assert_eq!(npc.context.target.as_ref(), &target);
        assert_eq!(npc.context.presentation_generation, 7);
        assert_eq!(npc.context.render_path, path);
        let frame = PlayerFrame {
            token: 11,
            look: PlayerLook::default(),
            location: ActorLocation {
                tile: MapTile::new(3200, 3200),
                plane: PlaneIndex::new(0).ok_or("plane")?,
            },
            position: (409664, 409664),
            rotation: 0,
            ground_height: 0,
            sequence: None,
            cycles: 90,
        };
        let player = extract_player_frame(&mut definitions, &frame, None, 7, requested);
        assert_eq!(player.token, frame.token);
        assert_eq!(player.context, npc.context);
        assert!(player.errors.is_empty(), "{:?}", player.errors);
        assert!(
            player.geometry.vertex_count() > 0,
            "real default player geometry"
        );
        if requested {
            assert_eq!(
                Some(&player.geometry),
                reference_player.as_ref(),
                "Reference fallback preserves the actual player output"
            );
        } else {
            reference_player = Some(player.geometry);
        }
    }
    Ok(())
}
