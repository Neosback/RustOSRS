//! Which selected objects need their real geometry fetched for the selection ghost.

use super::*;

/// Most selected objects whose geometry is drawn as a ghost (a big building is a few hundred).
pub(super) const MAX_OUTLINED: usize = 1500;

/// The outline requests a selection needs: every selected object (up to the cap), once.
fn selection_outline_keys(selection: &[ObjectPick]) -> Vec<((i32, i32), usize)> {
    selection
        .iter()
        .take(MAX_OUTLINED)
        .map(|object| (object.region, object.index))
        .collect()
}

/// A selection of many objects (a whole building) is drawn from their real geometry, so every
/// one of them needs its outline. Runs once, before any pointer work: picking must not depend
/// on whether anything is selected.
pub(super) fn queue_selection_outlines(app: &mut EditorApp) {
    for key in selection_outline_keys(app.interaction.selection_objects()) {
        app.world.queue_outline(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pick(region: (i32, i32), index: usize) -> ObjectPick {
        ObjectPick { region, index }
    }

    #[test]
    fn every_selected_object_is_requested_once_and_an_empty_selection_requests_nothing() {
        assert!(selection_outline_keys(&[]).is_empty());
        let keys = selection_outline_keys(&[pick((50, 50), 3), pick((50, 51), 7)]);
        assert_eq!(keys, vec![((50, 50), 3), ((50, 51), 7)]);
    }

    #[test]
    fn a_huge_selection_is_capped() {
        let many: Vec<ObjectPick> = (0..MAX_OUTLINED + 10).map(|i| pick((0, 0), i)).collect();
        assert_eq!(selection_outline_keys(&many).len(), MAX_OUTLINED);
    }
}
