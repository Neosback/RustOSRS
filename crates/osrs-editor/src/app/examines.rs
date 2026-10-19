//! Authentic OSRS object and NPC examine descriptions from cache telemetry.
//!
//! Provides instant, zero-allocation lookup of examine texts for 30,239 objects
//! and 12,694 NPCs, embedded statically from `data/examines/locs.csv` and
//! `data/examines/npcs.csv`.

use eframe::egui;
use std::collections::HashMap;
use std::sync::OnceLock;

static EXAMINES: OnceLock<Examines> = OnceLock::new();

const LOCS_CSV: &str = include_str!("../../data/examines/locs.csv");
const NPCS_CSV: &str = include_str!("../../data/examines/npcs.csv");

pub(in crate::app) struct Examines {
    locs: HashMap<u32, &'static str>,
    npcs: HashMap<u32, &'static str>,
}

impl Examines {
    fn init() -> Self {
        let mut locs = HashMap::with_capacity(30_500);
        for line in LOCS_CSV.lines() {
            let Some((id_str, text)) = line.split_once(',') else {
                continue;
            };
            let Ok(id) = id_str.trim().parse::<u32>() else {
                continue;
            };
            let cleaned = text.trim();
            let unquoted = cleaned
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .unwrap_or(cleaned);
            locs.insert(id, unquoted);
        }

        let mut npcs = HashMap::with_capacity(12_700);
        for line in NPCS_CSV.lines() {
            let Some((id_str, text)) = line.split_once(',') else {
                continue;
            };
            let Ok(id) = id_str.trim().parse::<u32>() else {
                continue;
            };
            let cleaned = text.trim();
            let unquoted = cleaned
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .unwrap_or(cleaned);
            npcs.insert(id, unquoted);
        }

        Self { locs, npcs }
    }

    pub fn get() -> &'static Examines {
        EXAMINES.get_or_init(Self::init)
    }

    pub fn loc(&self, id: u32) -> Option<&'static str> {
        self.locs.get(&id).copied()
    }

    pub fn npc(&self, id: u32) -> Option<&'static str> {
        self.npcs.get(&id).copied()
    }
}

/// Helper function to get an object examine string.
pub(in crate::app) fn loc(id: u32) -> Option<&'static str> {
    Examines::get().loc(id)
}

/// Helper function to get an NPC examine string.
pub(in crate::app) fn npc(id: u32) -> Option<&'static str> {
    Examines::get().npc(id)
}

/// Render an examine text callout in the inspector.
pub(in crate::app) fn show_object(ui: &mut egui::Ui, id: u32) {
    if let Some(text) = loc(id).filter(|t| !t.is_empty()) {
        ui.label(
            egui::RichText::new(format!("\"{text}\""))
                .italics()
                .color(ui.visuals().weak_text_color()),
        );
        ui.add_space(2.0);
    }
}

/// Render an examine text callout in the NPC inspector.
pub(in crate::app) fn show_npc(ui: &mut egui::Ui, id: u32) {
    if let Some(text) = npc(id).filter(|t| !t.is_empty()) {
        ui.label(
            egui::RichText::new(format!("\"{text}\""))
                .italics()
                .color(ui.visuals().weak_text_color()),
        );
        ui.add_space(2.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_object_examines() {
        let examines = Examines::get();
        assert_eq!(
            examines.loc(1),
            Some("Big mysterious crate. You wonder what could be inside.")
        );
        assert_eq!(examines.loc(3), Some("The door is closed."));
        assert_eq!(
            examines.loc(5),
            Some("A powerful ranging device, unfortunately not working.")
        );
    }

    #[test]
    fn parses_known_npc_examines() {
        let examines = Examines::get();
        assert_eq!(examines.npc(0), Some("Looks after your farming tools."));
        assert_eq!(examines.npc(1), Some("A strange mole-like being."));
        assert_eq!(examines.npc(17), Some("A carpet merchant."));
    }
}
