//! `inspect X Y`: print the view and document values of one tile (smoke tests).

use crate::app::EditorApp;
use std::str::SplitWhitespace;

pub(super) fn run(app: &mut EditorApp, words: &mut SplitWhitespace<'_>) -> Result<(), String> {
    let x = super::number::<i32>(words.next(), "x")?;
    let y = super::number::<i32>(words.next(), "y")?;
    let plane = app.interaction.view_plane();
    let semantic = osrs_core::coords::SourcePlane::new(plane).map(|plane| {
        app.session
            .document()
            .terrain_at(osrs_edit::MapPosition::new(
                osrs_core::coords::MapTile::new(x, y),
                plane,
            ))
    });
    eprintln!(
        "smoke: inspect ({x},{y}) plane {plane}: view {:?}, document {semantic:?}",
        app.tile_now((x, y))
    );
    Ok(())
}
