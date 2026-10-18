//! Role filters affect object membership independently of sparse terrain ownership.
use eframe::egui;
use osrs_scene::building_census::description::MemberRole;
#[derive(Clone)]
pub(super) struct Options {
    furnishings: bool,
    decoration: bool,
    ground_decoration: bool,
    roofs: bool,
    stairs: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            furnishings: true,
            decoration: true,
            ground_decoration: true,
            roofs: true,
            stairs: true,
        }
    }
}
impl Options {
    #[cfg(test)]
    pub(super) fn structure_only() -> Self {
        Self {
            furnishings: false,
            decoration: false,
            ground_decoration: false,
            roofs: false,
            stairs: false,
        }
    }

    pub(super) fn includes(&self, role: MemberRole) -> bool {
        match role {
            MemberRole::Boundary => true,
            MemberRole::Furniture => self.furnishings,
            MemberRole::Decoration => self.decoration,
            MemberRole::GroundDecoration => self.ground_decoration,
            MemberRole::Roof => self.roofs,
            MemberRole::Stair => self.stairs,
        }
    }
    pub(super) fn show(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        ui.strong("Building capture settings");
        ui.weak("Only reviewed room floors own ground. Outside-anchored walls are kept; their ground is excluded. Courtyards and gaps remain holes.");
        for (enabled, name) in [
            (&mut self.furnishings, "Furnishings and interior objects"),
            (
                &mut self.decoration,
                "Attached wall decoration (including outside anchors)",
            ),
            (
                &mut self.ground_decoration,
                "Ground decoration inside rooms",
            ),
            (&mut self.roofs, "Roof pieces and overhangs"),
            (&mut self.stairs, "Stairs and floor connections"),
        ] {
            changed |= ui.checkbox(enabled, name).changed();
        }
        changed
    }
}
