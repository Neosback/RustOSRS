//! Using a trained path model on the main map, through the Path tool: the points you click are
//! joined the way the model would draw them, and come back as ordinary path points you can still
//! move before laying (one undo, like any path).

use super::{PathsState, import};
use crate::app::EditorApp;
use osrs_lab::{PathModel, PathScene, path_waypoints};

/// How far from the clicked points the model looks, in tiles.
const MARGIN: i32 = 16;
/// The widest area a route may cover.
const MAX_SPAN: i32 = 128 - 2 * MARGIN;

/// What the Path tool's picker shows: the label of every model, yours first.
pub(in crate::app) fn labels(app: &mut EditorApp) -> Vec<String> {
    let state = &mut app.prefabs.workshop.paths;
    state.models.ensure_loaded();
    state
        .all_models()
        .into_iter()
        .map(|(_, m, starter)| {
            if starter {
                format!("{} (comes with the editor)", m.name)
            } else {
                m.name
            }
        })
        .collect()
}

/// Which of [`labels`] is in use.
pub(in crate::app) fn selected(app: &mut EditorApp) -> usize {
    let state = &mut app.prefabs.workshop.paths;
    let keys: Vec<String> = state.all_models().into_iter().map(|(k, ..)| k).collect();
    state
        .live_model
        .as_ref()
        .and_then(|k| keys.iter().position(|x| x == k))
        .unwrap_or(0)
}

pub(in crate::app) fn choose(app: &mut EditorApp, index: usize) {
    let state = &mut app.prefabs.workshop.paths;
    state.live_model = state.all_models().into_iter().nth(index).map(|(k, ..)| k);
}

fn model(state: &mut PathsState) -> Option<PathModel> {
    let all = state.all_models();
    let chosen = state
        .live_model
        .as_ref()
        .and_then(|k| all.iter().find(|(f, ..)| f == k));
    chosen.or_else(|| all.first()).map(|(_, m, _)| m.clone())
}

/// Join the path points on the map the way the chosen model would: it looks at the ground around
/// them (water, buildings, doors, slopes) and replaces the points with the route it prefers.
/// Returns what to tell the user.
pub(in crate::app) fn route(app: &mut EditorApp) -> Result<String, String> {
    let points: Vec<(f32, f32)> = app.interaction.spline_points().to_vec();
    if points.len() < 2 {
        return Err(
            "Click at least two points first: where the path starts and where it ends.".into(),
        );
    }
    let model = model(&mut app.prefabs.workshop.paths)
        .ok_or("There is no model to use. Train one in the Workshop (Paths tab).")?;
    let min = (
        points.iter().map(|p| p.0).fold(f32::MAX, f32::min).floor() as i32,
        points.iter().map(|p| p.1).fold(f32::MAX, f32::min).floor() as i32,
    );
    let max = (
        points.iter().map(|p| p.0).fold(f32::MIN, f32::max).floor() as i32,
        points.iter().map(|p| p.1).fold(f32::MIN, f32::max).floor() as i32,
    );
    if max.0 - min.0 > MAX_SPAN || max.1 - min.1 > MAX_SPAN {
        return Err(format!(
            "The points are too far apart: the AI can route over at most {MAX_SPAN} tiles."
        ));
    }
    let origin = (min.0 - MARGIN, min.1 - MARGIN);
    let (w, h) = (
        (max.0 - min.0 + 1 + 2 * MARGIN) as u16,
        (max.1 - min.1 + 1 + 2 * MARGIN) as u16,
    );
    let lesson = import::lesson_of(app, origin.0, origin.1, w, h, "live")?;
    let scene = PathScene::of(&lesson);
    let local: Vec<(usize, usize)> = points
        .iter()
        .map(|p| {
            (
                (p.0.floor() as i32 - origin.0) as usize,
                (p.1.floor() as i32 - origin.1) as usize,
            )
        })
        .collect();
    let mut tiles: Vec<(usize, usize)> = Vec::new();
    for pair in local.windows(2) {
        if pair[0] == pair[1] {
            continue;
        }
        let drawn = model.draw_between(&scene, &[vec![pair[0], pair[1]]]);
        let Some(mut segment) = drawn.routes.into_iter().next() else {
            return Err(
                "The AI could not find a way between those points (they may be inside a building)."
                    .into(),
            );
        };
        // The route is drawn from the second point back to the first: put it in walking order.
        segment.reverse();
        if tiles.last() == segment.first() {
            segment.remove(0);
        }
        tiles.extend(segment);
    }
    let routed: Vec<(f32, f32)> = path_waypoints(&tiles, 1.2)
        .into_iter()
        .map(|p| (origin.0 as f32 + p[0] as f32, origin.1 as f32 + p[1] as f32))
        .collect();
    let count = routed.len();
    app.interaction.replace_spline_points(routed);
    Ok(format!(
        "The AI routed the path through {count} points. Move any you like, then press Enter to lay it."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_choice_falls_back_to_the_first_model() {
        let mut state = PathsState::default();
        assert!(
            model(&mut state).is_some(),
            "the starter model is always there"
        );
        state.live_model = Some("gone.pathmodel.json".into());
        assert!(model(&mut state).is_some());
    }
}
