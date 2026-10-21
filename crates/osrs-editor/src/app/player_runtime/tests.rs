//! Actual arrival/update tests; synthetic triangles make accidental publication observable.
use super::*;
use crate::app::world_runtime::test_support::*;

fn result(token: u64) -> PlayerGeometryUpdate {
    PlayerGeometryUpdate {
        token,
        context: actor_context(),
        geometry: candidate().geometry,
        errors: vec!["obsolete".into()],
    }
}

#[test]
fn old_tokens_cannot_retire_the_current_player_request() {
    let mut player = PlayerRuntime {
        inflight: Some(9),
        ..Default::default()
    };
    player.arrived(result(8));
    assert_eq!(player.inflight, Some(9));
    assert!(player.pending.is_none());
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn gpu_player_publication_reconciles_hide_teleport_look_ground_and_source() {
    let Some(mut renderer) = actor_renderer() else {
        return;
    };
    for change in [
        "hide",
        "hidden_arrival",
        "teleport",
        "look",
        "ground",
        "missing_ground",
        "source",
        "presentation",
        "plane",
        "valid_lag",
        "empty",
    ] {
        let mut world = world();
        seed_installed(&mut world, &candidate());
        let map = MapDocument::default();
        let mut player = PlayerRuntime::default();
        player.source_opened(&mut renderer);
        let mut spec;
        {
            let mut host = PlayerHost {
                world: &world,
                map: &map,
                renderer: &mut renderer,
                dt: 0.0,
                spec: Some(PlayerSpec {
                    look: PlayerLook::default(),
                    tile: (3200, 3200),
                    plane: 0,
                }),
                running: false,
            };
            player.update(&mut host);
            let token = player
                .inflight
                .unwrap_or_else(|| panic!("initial request: {change}"));
            if change == "hidden_arrival" {
                host.spec = None;
                player.update(&mut host);
                assert_eq!(
                    player.inflight,
                    Some(token),
                    "hidden physical request must drain"
                );
                host.spec = Some(PlayerSpec {
                    look: PlayerLook::default(),
                    tile: (3200, 3200),
                    plane: 0,
                });
            }
            player.arrived(result(token));
            assert_eq!(host.renderer.resident_vertex_bytes(), 0, "staged: {change}");
            assert!(host.renderer.player_identity().is_none());
            spec = host.spec;
        }
        let issued = player.sent.as_ref().map(Arc::downgrade);
        let mut dt = 0.0;
        match change {
            "hide" => spec = None,
            "teleport" => player.teleport((3201, 3201, 0)),
            "look" => {
                let spec = spec.as_mut().unwrap_or_else(|| panic!("spec"));
                spec.look.gender = osrs_core::player::Gender::Female;
            }
            "ground" => {
                seed_info(&mut world, &candidate());
            }
            "missing_ground" => {
                world.remove_info(KEY);
            }
            "source" => player.source_opened(&mut renderer),
            "presentation" => {
                world.begin_rebuild_all(false, true);
            }
            "plane" => spec.as_mut().unwrap_or_else(|| panic!("spec")).plane = 4,
            "valid_lag" | "empty" => dt = 0.2,
            _ => {}
        }
        let mut host = PlayerHost {
            world: &world,
            map: &map,
            renderer: &mut renderer,
            dt,
            spec,
            running: false,
        };
        player.update(&mut host);
        if change == "valid_lag" || change == "empty" {
            assert!(host.renderer.resident_vertex_bytes() > 0);
            assert_eq!(player.error(), Some("obsolete"));
            assert!(player.inflight.is_some(), "later animation frame scheduled");
            let issued = issued.unwrap_or_else(|| panic!("issued input"));
            let request = issued
                .upgrade()
                .unwrap_or_else(|| panic!("GPU retains issued input"));
            let lines = host
                .renderer
                .player_identity()
                .unwrap_or_else(|| panic!("GPU identity"))
                .describe();
            assert!(lines[0].contains(&format!(
                "token {}; cycle {}",
                request.frame.token, request.frame.cycles
            )));
            assert!(lines[0].contains(&format!("ground version {}", request.key.infos_version)));
            assert!(
                lines
                    .iter()
                    .any(|line| line == &request.ground.describe()[0])
            );
            drop(request);
            if change == "empty" {
                let mut empty = result(player.inflight.unwrap_or_else(|| panic!("next token")));
                empty.geometry = SceneGeometry::default();
                player.arrived(empty);
                host.dt = 0.0;
                player.update(&mut host);
                assert!(host.renderer.player_identity().is_none());
                assert_eq!(host.renderer.resident_vertex_bytes(), 0);
                assert!(
                    issued.upgrade().is_none(),
                    "empty upload releases old request"
                );
            }
            host.spec = None;
            player.update(&mut host);
            assert_eq!(
                host.renderer.resident_vertex_bytes(),
                0,
                "hides installed layer"
            );
            assert!(host.renderer.player_identity().is_none());
            assert!(
                issued.upgrade().is_none(),
                "hide releases installed request"
            );
        } else {
            assert_eq!(
                host.renderer.resident_vertex_bytes(),
                0,
                "rejected: {change}"
            );
            assert!(player.error().is_none(), "stale diagnostics: {change}");
            assert!(
                host.renderer.player_identity().is_none(),
                "stale identity: {change}"
            );
            if change == "hidden_arrival"
                || change == "look"
                || change == "ground"
                || change == "presentation"
            {
                let token = player
                    .inflight
                    .unwrap_or_else(|| panic!("replacement request"));
                let mut update = result(token);
                update.context.presentation_generation = host.world.presentation_generation();
                player.arrived(update);
                player.update(&mut host);
                assert!(
                    host.renderer.resident_vertex_bytes() > 0,
                    "valid replacement: {change}"
                );
                assert!(host.renderer.player_identity().is_some());
            }
        }
    }
    eprintln!("GPU player projection lifecycle exercised");
}
