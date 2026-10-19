//! `select object|tiles|building ...`: set the selection from a script (smoke tests).

use super::{Step, number};
use crate::app::{EditorApp, building_pick};
use std::str::SplitWhitespace;

pub(super) fn run(app: &mut EditorApp, words: &mut SplitWhitespace<'_>) -> Result<Step, String> {
    match words.next() {
        Some("object") => {
            let x: i32 = number(words.next(), "x")?;
            let y: i32 = number(words.next(), "y")?;
            Ok(if app.select_nearest_object((x, y)) {
                Step::Done
            } else {
                Step::Retry
            })
        }
        Some("tiles") => {
            let x0: i32 = number(words.next(), "x0")?;
            let y0: i32 = number(words.next(), "y0")?;
            let x1: i32 = number(words.next(), "x1")?;
            let y1: i32 = number(words.next(), "y1")?;
            for x in x0.min(x1)..=x0.max(x1) {
                for y in y0.min(y1)..=y0.max(y1) {
                    app.interaction.add_tile((x, y));
                }
            }
            Ok(Step::Done)
        }
        Some("building") => {
            let x: i32 = number(words.next(), "x")?;
            let y: i32 = number(words.next(), "y")?;
            Ok(match building_pick::select_building(app, (x, y), false) {
                Some(message) => {
                    eprintln!("smoke: {message}");
                    Step::Done
                }
                None => Step::Retry,
            })
        }
        Some("review") => {
            building_pick::review(app);
            Ok(Step::Done)
        }
        Some("pointer") => {
            use crate::app::interaction_runtime::ScriptedPointer;
            let pointer = match words.next() {
                Some("off") => None,
                Some("center") => Some(ScriptedPointer::Centre),
                other => {
                    let x: f32 = number(other, "x")?;
                    let y: f32 = number(words.next(), "y")?;
                    Some(ScriptedPointer::At(x, y))
                }
            };
            app.interaction.set_scripted_pointer(pointer);
            Ok(Step::Done)
        }
        Some("hover") => {
            // What the pipeline picked under the (scripted) pointer: proof that picking runs
            // with nothing selected.
            let hover = app.interaction.hover();
            let object = hover.object.as_ref().map(|o| (o.region, o.index));
            let tile = hover.tile.map(|t| t.tile);
            eprintln!(
                "smoke: hover tile {tile:?} object {object:?} (selected objects {}, tiles {})",
                app.interaction.selection_objects().len(),
                app.interaction.selection_tiles().len()
            );
            Ok(Step::Done)
        }
        Some("report") => {
            // How many selected objects have their geometry ready to draw, by plane.
            let mut by_plane = std::collections::BTreeMap::<u8, (usize, usize)>::new();
            for object in app.interaction.selection_objects() {
                let plane = app
                    .world
                    .infos()
                    .get(&object.region)
                    .and_then(|info| info.locs.get(object.index))
                    .map_or(9, |loc| loc.plane);
                let ready = app.world.outline(&(object.region, object.index)).is_some();
                let entry = by_plane.entry(plane).or_default();
                entry.0 += 1;
                entry.1 += usize::from(ready);
            }
            eprintln!("smoke: selection (plane: selected, geometry ready) {by_plane:?}");
            Ok(Step::Done)
        }
        other => Err(format!(
            "select object, tiles, building, pointer, hover or report, not {other:?}"
        )),
    }
}
