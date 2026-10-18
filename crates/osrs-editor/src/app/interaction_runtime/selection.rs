//! Selection and plane transitions owned by the interaction runtime.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum EscapeLayer {
    Menu,
    QuickHud,
    Marquee,
    Armed,
    Tool,
    Selection,
}
impl EscapeLayer {
    pub(in crate::app) fn choose(
        menu: bool,
        quick: bool,
        marquee: bool,
        armed: bool,
        selecting: bool,
    ) -> Self {
        if menu {
            Self::Menu
        } else if quick {
            Self::QuickHud
        } else if marquee {
            Self::Marquee
        } else if armed {
            Self::Armed
        } else if !selecting {
            Self::Tool
        } else {
            Self::Selection
        }
    }
}

impl InteractionRuntime {
    fn same_object(a: &ObjectPick, b: &ObjectPick) -> bool {
        a.region == b.region && a.index == b.index
    }

    pub(in crate::app) fn set_hover(&mut self, hover: Pick) {
        if let Some(tile) = hover.tile {
            self.last_hover_tile = Some(tile.tile);
        }
        self.hover = hover;
    }

    pub(in crate::app) fn clear_hover(&mut self) {
        self.hover = Pick::default();
    }

    pub(in crate::app) fn capture_context_pick(&mut self) {
        self.context_pick = self.hover.clone();
    }

    pub(in crate::app) fn is_primary_object(&self, object: &ObjectPick) -> bool {
        self.selected
            .as_ref()
            .is_some_and(|selected| Self::same_object(selected, object))
    }

    pub(in crate::app) fn clear_selection(&mut self) {
        self.drop_pending_reselect();
        self.selected = None;
        self.selection_objects.clear();
        self.selection_tiles.clear();
        self.inspect_tile = None;
    }

    /// A plane change the user made: the selection, hover, box and peeled roof belong to the plane
    /// they were made on (Fill, Height and Paste act on the viewed plane when they run).
    pub(in crate::app) fn switch_plane(&mut self, plane: u8) {
        if plane.min(3) == self.view_plane {
            return;
        }
        self.view_plane = plane.min(3);
        self.clear_selection();
        self.clear_hover();
        self.context_pick = Pick::default();
        self.last_hover_tile = None;
        self.cancel_marquee();
        self.peeled_roof = None;
        self.pending_reselect.clear();
    }

    pub(in crate::app) fn clear_objects(&mut self) {
        self.drop_pending_reselect();
        self.selected = None;
        self.selection_objects.clear();
    }

    pub(in crate::app) fn clear_tiles(&mut self) {
        self.drop_pending_reselect();
        self.selection_tiles.clear();
    }

    pub(in crate::app) fn select_only_object(
        &mut self,
        object: ObjectPick,
        inspect_tile: Option<(i32, i32)>,
    ) {
        self.drop_pending_reselect();
        if let Some(tile) = inspect_tile {
            self.inspect_tile = Some(tile);
        }
        self.selection_objects = vec![object.clone()];
        self.selected = Some(object);
    }

    pub(in crate::app) fn replace_objects(&mut self, objects: Vec<ObjectPick>) {
        self.drop_pending_reselect();
        self.selected = objects.last().cloned();
        self.selection_objects = objects;
    }

    pub(in crate::app) fn add_object(
        &mut self,
        object: ObjectPick,
        inspect_tile: Option<(i32, i32)>,
    ) {
        self.drop_pending_reselect();
        if let Some(tile) = inspect_tile {
            self.inspect_tile = Some(tile);
        }
        if !self
            .selection_objects
            .iter()
            .any(|selected| Self::same_object(selected, &object))
        {
            self.selection_objects.push(object.clone());
        }
        self.selected = Some(object);
    }

    pub(in crate::app) fn toggle_object(
        &mut self,
        object: ObjectPick,
        inspect_tile: Option<(i32, i32)>,
    ) {
        self.drop_pending_reselect();
        if let Some(index) = self
            .selection_objects
            .iter()
            .position(|selected| Self::same_object(selected, &object))
        {
            self.selection_objects.remove(index);
            self.selected = self.selection_objects.last().cloned();
        } else {
            self.add_object(object, inspect_tile);
        }
    }

    pub(in crate::app) fn add_tile(&mut self, tile: (i32, i32)) {
        self.drop_pending_reselect();
        self.selection_tiles.insert(tile);
    }

    pub(in crate::app) fn select_tile(&mut self, tile: (i32, i32)) {
        self.drop_pending_reselect();
        self.selection_tiles.insert(tile);
        self.inspect_tile = Some(tile);
    }

    pub(in crate::app) fn toggle_tile(&mut self, tile: (i32, i32)) {
        self.drop_pending_reselect();
        if !self.selection_tiles.remove(&tile) {
            self.select_tile(tile);
        }
    }

    pub(in crate::app) fn selection_counts(&self) -> (usize, usize) {
        (self.selection_objects.len(), self.selection_tiles.len())
    }

    pub(in crate::app) fn set_pending_reselect(&mut self, specs: Vec<osrs_world::LocSpec>) {
        self.pending_reselect = specs;
    }

    /// The user chose something else: the selection an edit queued no longer applies.
    pub(in crate::app) fn drop_pending_reselect(&mut self) {
        self.pending_reselect.clear();
    }

    pub(in crate::app) fn pending_reselect(&self) -> &[osrs_world::LocSpec] {
        &self.pending_reselect
    }

    pub(in crate::app) fn complete_reselect(&mut self, objects: Vec<ObjectPick>) -> bool {
        if self.pending_reselect.is_empty() || objects.len() != self.pending_reselect.len() {
            return false;
        }
        self.replace_objects(objects);
        self.pending_reselect.clear();
        true
    }

    pub(in crate::app) fn begin_marquee(
        &mut self,
        position: egui::Pos2,
        additive: bool,
        toggle: bool,
    ) {
        self.marquee = Some(Marquee {
            start: position,
            current: position,
            additive,
            toggle,
            start_tile: None,
            current_tile: None,
        });
    }

    /// Record the ground tiles under the press and under the pointer.
    pub(in crate::app) fn track_marquee_tiles(
        &mut self,
        start: Option<(i32, i32)>,
        current: Option<(i32, i32)>,
    ) {
        if let Some(marquee) = &mut self.marquee {
            if marquee.start_tile.is_none() {
                marquee.start_tile = start;
            }
            if current.is_some() {
                marquee.current_tile = current;
            }
        }
    }

    pub(in crate::app) fn update_marquee(
        &mut self,
        position: Option<egui::Pos2>,
        bounds: egui::Rect,
        additive: bool,
        toggle: bool,
    ) {
        let Some(marquee) = &mut self.marquee else {
            return;
        };
        if let Some(position) = position {
            marquee.current = position.clamp(bounds.min, bounds.max);
        }
        marquee.additive |= additive;
        marquee.toggle |= toggle;
    }

    pub(in crate::app) fn cancel_marquee(&mut self) {
        self.marquee = None;
    }

    pub(in crate::app) fn finish_marquee(&mut self) -> Option<Marquee> {
        self.marquee.take()
    }

    pub(in crate::app) fn marquee(&self) -> Option<&Marquee> {
        self.marquee.as_ref()
    }
}
