//! Dedicated Character & Player Studio window: detailed look/appearance customization,
//! equipment paperdoll with authentic OSRS presets, skill matrix, and named VarPlayer/Varbit inspector.

use super::EditorApp;
use crate::icons;
use crate::player::{COLOUR_SLOTS, EQUIPMENT_SLOTS, KIT_SLOTS, MAX_LEVEL, SKILLS};
use eframe::egui::{self, Color32, Stroke, Vec2};
use osrs_core::var_names::{search_varbits, search_varps, varbit_info, varp_info};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Look,
    Equipment,
    Skills,
    Variables,
}

/// Authentic OSRS player skin tones (0..7).
const SKIN_TONES: [Color32; 8] = [
    Color32::from_rgb(222, 178, 142), // 0: Fair
    Color32::from_rgb(198, 150, 114), // 1: Tan
    Color32::from_rgb(166, 118, 86),  // 2: Olive
    Color32::from_rgb(134, 88, 58),   // 3: Brown
    Color32::from_rgb(94, 58, 34),    // 4: Dark Brown
    Color32::from_rgb(230, 205, 185), // 5: Pale
    Color32::from_rgb(112, 160, 112), // 6: Green
    Color32::from_rgb(130, 145, 180), // 7: Blue
];

/// Authentic OSRS hair color swatches (0..11 common selections).
const HAIR_COLORS: [(u8, Color32); 12] = [
    (0, Color32::from_rgb(104, 72, 48)),   // Dark Brown
    (1, Color32::from_rgb(200, 200, 200)), // White/Silver
    (2, Color32::from_rgb(120, 120, 120)), // Grey
    (3, Color32::from_rgb(45, 45, 45)),    // Black
    (4, Color32::from_rgb(190, 50, 40)),   // Red
    (5, Color32::from_rgb(220, 180, 80)),  // Blonde
    (6, Color32::from_rgb(150, 100, 50)),  // Light Brown
    (7, Color32::from_rgb(160, 140, 100)), // Ash Brown
    (8, Color32::from_rgb(60, 130, 180)),  // Blue
    (9, Color32::from_rgb(70, 150, 80)),   // Green
    (10, Color32::from_rgb(180, 80, 160)), // Purple
    (11, Color32::from_rgb(220, 120, 50)), // Orange
];

pub(super) fn draw_player_window(app: &mut EditorApp, ctx: &egui::Context) {
    if !app.ui.player_window_open {
        return;
    }

    let mut open = app.ui.player_window_open;
    egui::Window::new("Character & Player Studio")
        .open(&mut open)
        .default_size(egui::vec2(580.0, 560.0))
        .min_width(460.0)
        .min_height(400.0)
        .resizable(true)
        .show(ctx, |ui| {
            let key = ui.make_persistent_id("player_window_tab");
            let mut current_tab = ui.data_mut(|d| d.get_temp::<Tab>(key)).unwrap_or_default();

            ui.horizontal(|ui| {
                ui.selectable_value(&mut current_tab, Tab::Look, "Look & Style");
                ui.selectable_value(&mut current_tab, Tab::Equipment, "Equipment");
                ui.selectable_value(&mut current_tab, Tab::Skills, "Skills & Status");
                ui.selectable_value(
                    &mut current_tab,
                    Tab::Variables,
                    "Variables (Varps/Varbits)",
                );
            });
            ui.separator();

            ui.data_mut(|d| d.insert_temp(key, current_tab));

            egui::ScrollArea::vertical().show(ui, |ui| match current_tab {
                Tab::Look => look_tab(app, ui),
                Tab::Equipment => equipment_tab(app, ui),
                Tab::Skills => skills_tab(app, ui),
                Tab::Variables => variables_tab(app, ui),
            });
        });
    app.ui.player_window_open = open;
}

fn look_tab(app: &mut EditorApp, ui: &mut egui::Ui) {
    let player = &mut app.settings.prefs.player;

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Gender:").strong());
        if ui.selectable_label(!player.female, "Male").clicked() {
            player.female = false;
        }
        if ui.selectable_label(player.female, "Female").clicked() {
            player.female = true;
        }

        ui.separator();
        ui.label(egui::RichText::new("Presets:").weak());
        if ui.small_button("Default Male").clicked() {
            player.female = false;
            player.kits = [
                Some(0),
                Some(14),
                Some(18),
                Some(26),
                Some(33),
                Some(36),
                Some(42),
            ];
        }
        if ui.small_button("Default Female").clicked() {
            player.female = true;
            player.kits = [
                Some(45),
                None,
                Some(56),
                Some(61),
                Some(67),
                Some(70),
                Some(79),
            ];
        }
        if ui.small_button("Bald / Monk").clicked() {
            player.kits[0] = None;
            player.kits[1] = if player.female { None } else { Some(14) };
        }
    });

    ui.separator();
    ui.label(egui::RichText::new("Body Kit Slots").heading());
    ui.weak("Cycle styles with arrows or edit identity kit IDs directly.");

    egui::Grid::new("kits_grid")
        .num_columns(4)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            for (slot, name) in KIT_SLOTS.iter().enumerate() {
                ui.label(egui::RichText::new(*name).strong());
                let current = player.kits[slot];
                if ui.small_button("<").clicked() {
                    player.kits[slot] = match current {
                        Some(0) | None => None,
                        Some(n) => Some(n - 1),
                    };
                }
                let mut raw = current.map_or(-1, i32::from);
                if ui
                    .add(
                        egui::DragValue::new(&mut raw)
                            .range(-1..=255)
                            .custom_formatter(|n, _| {
                                if n < 0.0 {
                                    "none".into()
                                } else {
                                    format!("Style {n}")
                                }
                            }),
                    )
                    .changed()
                {
                    player.kits[slot] = u16::try_from(raw).ok();
                }
                if ui.small_button(">").clicked() {
                    player.kits[slot] = Some(current.map_or(0, |n| n + 1));
                }
                ui.end_row();
            }
        });

    ui.separator();
    ui.label(egui::RichText::new("Color Palette").heading());

    ui.label("Skin Tone:");
    ui.horizontal_wrapped(|ui| {
        for (tone_idx, color) in SKIN_TONES.iter().enumerate() {
            let selected = player.colours[4] == tone_idx as u8;
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(24.0), egui::Sense::click());
            ui.painter().rect_filled(rect, 3.0, *color);
            if selected {
                ui.painter().rect_stroke(
                    rect,
                    3.0,
                    Stroke::new(2.0, Color32::WHITE),
                    egui::StrokeKind::Inside,
                );
            }
            if resp.clicked() {
                player.colours[4] = tone_idx as u8;
            }
        }
    });

    ui.add_space(4.0);
    ui.label("Hair Color:");
    ui.horizontal_wrapped(|ui| {
        for (color_idx, color) in HAIR_COLORS {
            let selected = player.colours[0] == color_idx;
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(22.0), egui::Sense::click());
            ui.painter().rect_filled(rect, 3.0, color);
            if selected {
                ui.painter().rect_stroke(
                    rect,
                    3.0,
                    Stroke::new(2.0, Color32::WHITE),
                    egui::StrokeKind::Inside,
                );
            }
            if resp.clicked() {
                player.colours[0] = color_idx;
            }
        }
    });

    ui.add_space(6.0);
    egui::Grid::new("clothes_colors")
        .num_columns(2)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            for (idx, name) in COLOUR_SLOTS[1..4].iter().enumerate() {
                let slot = idx + 1;
                ui.label(format!("{name} color ID:"));
                ui.add(egui::Slider::new(&mut player.colours[slot], 0..=28));
                ui.end_row();
            }
        });
}

fn equipment_tab(app: &mut EditorApp, ui: &mut egui::Ui) {
    let player = &mut app.settings.prefs.player;

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Armor Sets:").strong());
        if ui.button("Rune Knight").clicked() {
            player.equipment.insert(0, 1163); // Rune full helm
            player.equipment.insert(1, 6568); // Obsidian cape
            player.equipment.insert(2, 1704); // Amulet of glory
            player.equipment.insert(3, 1333); // Rune scimitar
            player.equipment.insert(4, 1127); // Rune platebody
            player.equipment.insert(5, 1201); // Rune kiteshield
            player.equipment.insert(7, 1079); // Rune platelegs
            player.equipment.insert(9, 7462); // Barrows gloves
            player.equipment.insert(10, 4131); // Rune boots
        }
        if ui.button("Dragon Slayer").clicked() {
            player.equipment.insert(0, 1149); // Dragon med helm
            player.equipment.insert(1, 6568); // Obsidian cape
            player.equipment.insert(3, 1305); // Dragon longsword
            player.equipment.insert(4, 3140); // Dragon chainbody
            player.equipment.insert(5, 1187); // Dragon sq shield
            player.equipment.insert(7, 4087); // Dragon platelegs
            player.equipment.insert(10, 11732); // Dragon boots
        }
        if ui.button("Mystic Mage").clicked() {
            player.equipment.insert(0, 4099); // Mystic hat
            player.equipment.insert(3, 1393); // Fire staff
            player.equipment.insert(4, 4101); // Mystic robe top
            player.equipment.insert(7, 4103); // Mystic robe bottom
            player.equipment.insert(9, 4113); // Mystic gloves
            player.equipment.insert(10, 4115); // Mystic boots
        }
        if ui.button("Clear All").clicked() {
            player.equipment.clear();
        }
    });

    ui.separator();
    ui.label(egui::RichText::new("Equipment Slots").heading());
    ui.weak("Item ID per slot (0 = empty).");

    egui::Grid::new("equipment_table")
        .num_columns(3)
        .spacing([16.0, 6.0])
        .show(ui, |ui| {
            for (slot, name) in EQUIPMENT_SLOTS {
                ui.label(egui::RichText::new(name).strong());
                let mut item = player.equipment.get(&slot).copied().unwrap_or(0);
                if ui
                    .add(egui::DragValue::new(&mut item).custom_formatter(|n, _| {
                        if n == 0.0 {
                            "Empty".into()
                        } else {
                            format!("Item {n}")
                        }
                    }))
                    .changed()
                {
                    player.wear(slot, (item != 0).then_some(item));
                }
                if item != 0 && ui.small_button(egui_phosphor::regular::TRASH).clicked() {
                    player.equipment.remove(&slot);
                }
                ui.end_row();
            }
        });
}

fn skills_tab(app: &mut EditorApp, ui: &mut egui::Ui) {
    let player = &mut app.settings.prefs.player;

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("Combat Level: {}", player.combat_level())).heading());
        ui.separator();
        if ui.button("All 99").clicked() {
            player.max_levels();
        }
        if ui.button("Reset 1").clicked() {
            player.levels = [1; 23];
            player.levels[3] = 10; // HP starts at 10
        }
    });

    ui.separator();
    ui.label(egui::RichText::new("Skills").strong());

    egui::Grid::new("skills_matrix")
        .num_columns(6)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            for (idx, name) in SKILLS.iter().enumerate() {
                ui.label(*name);
                let mut level = player.levels[idx];
                if ui
                    .add(egui::DragValue::new(&mut level).range(1..=MAX_LEVEL))
                    .changed()
                {
                    player.set_level(idx, level);
                }
                if idx % 3 == 2 {
                    ui.end_row();
                }
            }
        });

    ui.separator();
    ui.label(egui::RichText::new("Vitals & Energy").strong());
    let max_hp = u16::from(player.levels[3]);
    let max_pray = u16::from(player.levels[5]);
    ui.horizontal(|ui| {
        ui.add(
            egui::DragValue::new(&mut player.hitpoints)
                .range(0..=max_hp)
                .prefix("HP: "),
        );
        ui.add(
            egui::DragValue::new(&mut player.prayer)
                .range(0..=max_pray)
                .prefix("Prayer: "),
        );
        ui.checkbox(&mut player.running, "Running");
        ui.add(egui::Slider::new(&mut player.run_energy, 0..=100).text("Run %"));
    });
}

fn variables_tab(app: &mut EditorApp, ui: &mut egui::Ui) {
    let search = &mut app.ui.player_var_search;
    ui.horizontal(|ui| {
        ui.label(icons::SEARCH);
        ui.add(egui::TextEdit::singleline(search).hint_text("Search varps & varbits by name..."));
    });

    let q = search.trim();
    let matching_varps = search_varps(q);
    let matching_varbits = search_varbits(q);

    let player = &mut app.settings.prefs.player;

    ui.separator();
    ui.label(
        egui::RichText::new(format!(
            "Known VarPlayers ({} matching)",
            matching_varps.len()
        ))
        .strong(),
    );
    egui::Grid::new("varps_found")
        .num_columns(4)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            for info in matching_varps.iter().take(12) {
                ui.label(format!("#{}", info.id));
                ui.label(egui::RichText::new(info.name).strong());
                ui.weak(info.description);
                let mut val = player.varps.get(&info.id).copied().unwrap_or(0);
                if ui
                    .add(egui::DragValue::new(&mut val).prefix("= "))
                    .changed()
                {
                    player.varps.insert(info.id, val);
                }
                ui.end_row();
            }
        });

    ui.separator();
    ui.label(
        egui::RichText::new(format!(
            "Known Varbits ({} matching)",
            matching_varbits.len()
        ))
        .strong(),
    );
    egui::Grid::new("varbits_found")
        .num_columns(4)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            for info in matching_varbits.iter().take(12) {
                ui.label(format!("#{}", info.id));
                ui.label(egui::RichText::new(info.name).strong());
                ui.weak(info.description);
                let mut val = player.varbits.get(&info.id).copied().unwrap_or(0);
                if ui
                    .add(egui::DragValue::new(&mut val).prefix("= "))
                    .changed()
                {
                    player.varbits.insert(info.id, val);
                }
                ui.end_row();
            }
        });

    ui.separator();
    ui.label(
        egui::RichText::new(format!(
            "Active Active Player Varps ({})",
            player.varps.len()
        ))
        .strong(),
    );
    let mut remove_varp = None;
    for (&id, val) in &mut player.varps {
        ui.horizontal(|ui| {
            let name = varp_info(id).map_or("Custom", |v| v.name);
            ui.label(format!("Varp #{id} ({name}):"));
            ui.add(egui::DragValue::new(val));
            if ui.small_button(egui_phosphor::regular::TRASH).clicked() {
                remove_varp = Some(id);
            }
        });
    }
    if let Some(id) = remove_varp {
        player.varps.remove(&id);
    }

    ui.separator();
    ui.label(
        egui::RichText::new(format!("Active Player Varbits ({})", player.varbits.len())).strong(),
    );
    let mut remove_varbit = None;
    for (&id, val) in &mut player.varbits {
        ui.horizontal(|ui| {
            let name = varbit_info(id).map_or("Custom", |v| v.name);
            ui.label(format!("Varbit #{id} ({name}):"));
            ui.add(egui::DragValue::new(val));
            if ui.small_button(egui_phosphor::regular::TRASH).clicked() {
                remove_varbit = Some(id);
            }
        });
    }
    if let Some(id) = remove_varbit {
        player.varbits.remove(&id);
    }
}
