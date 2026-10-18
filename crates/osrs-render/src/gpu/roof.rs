//! Which upper floors and roofs a frame draws.

use std::collections::HashSet;

/// How upper floors (everything with a minimum plane above the viewed plane) are shown.
///
/// The pinned client draws up to plane 3 unless the tile under the camera has a roof above it
/// (`topLevel`), and RuneLite's roof removal hides single roof volumes (`hideRoofIds`). The editor
/// keeps the viewed plane for editing and decides separately how much of what lies above it to
/// draw.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RoofView {
    /// Highest minimum plane drawn; `None` draws nothing above the viewed plane.
    pub top_plane: Option<u8>,
    /// Roof volumes to leave out even when they lie at or below `top_plane`.
    pub hidden: HashSet<u32>,
}

impl RoofView {
    /// Nothing above the viewed plane (the editor's long-standing behaviour).
    pub fn hidden_above() -> Self {
        Self::default()
    }

    /// Every plane, minus the roof volumes in `hidden`.
    pub fn all_but(hidden: HashSet<u32>) -> Self {
        Self {
            top_plane: Some(3),
            hidden,
        }
    }

    /// Whether geometry with `min_plane` under roof volume `roof` is drawn when `view_plane` is
    /// viewed. Geometry at or below the viewed plane is always drawn.
    pub fn shows(&self, min_plane: u8, roof: u32, view_plane: u8) -> bool {
        if min_plane <= view_plane {
            return true;
        }
        self.top_plane.is_some_and(|top| min_plane <= top)
            && !(roof != 0 && self.hidden.contains(&roof))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upper_floors_follow_the_top_plane_and_the_hidden_roofs() {
        let hidden_above = RoofView::hidden_above();
        assert!(hidden_above.shows(0, 0, 0));
        assert!(!hidden_above.shows(1, 0, 0));

        let all = RoofView::all_but(HashSet::from([7]));
        assert!(all.shows(1, 0, 0), "roofs without an id stay");
        assert!(all.shows(1, 9, 0));
        assert!(!all.shows(1, 7, 0), "the hidden volume goes");
        assert!(all.shows(1, 7, 1), "the viewed plane is never hidden");
        assert!(all.shows(3, 0, 0));
    }
}
