//! The widget of each setting: the only place a row's control exists.

use super::EditorApp;
use super::catalog::Row;
use crate::camera::CameraMode;
use crate::settings::{
    ColorblindChoice, ContextMenuStyle, MinimapSize, ProfileChoice, RoofChoice, Settings,
};
use eframe::egui;
use osrs_hd::{
    environment::{DefaultSky, FogDepthMode},
    shadows::{ShadowFiltering, ShadowMode, ShadowResolution},
};

impl EditorApp {
    /// The widget of one row. This is the only place a setting's control exists.
    pub(super) fn draw_row(&mut self, ui: &mut egui::Ui, row: Row) {
        let handled = self.draw_interface_row(ui, row)
            || self.draw_world_row(ui, row)
            || self.draw_reference_row(ui, row)
            || self.draw_hd_row(ui, row);
        debug_assert!(handled, "a settings row has no widget");
    }

    /// Appearance, panels, menus, and hints.
    #[allow(clippy::too_many_lines)]
    fn draw_interface_row(&mut self, ui: &mut egui::Ui, row: Row) -> bool {
        let prefs: &mut Settings = &mut self.settings.prefs;
        match row {
            Row::Theme => {
                egui::ComboBox::from_id_salt("setting_theme")
                    .selected_text(crate::theme::NAMES[prefs.appearance])
                    .show_ui(ui, |ui| {
                        for (index, name) in crate::theme::NAMES.iter().enumerate() {
                            if ui
                                .selectable_value(&mut prefs.appearance, index, *name)
                                .clicked()
                            {
                                crate::theme::set_theme(ui.ctx(), crate::theme::ALL[index]);
                            }
                        }
                    });
            }
            Row::SidePanel => {
                crate::theme::toggle(ui, &mut prefs.properties_panel, "Show");
            }
            Row::DockRight => {
                crate::theme::toggle(ui, &mut prefs.dock_right, "Right (off: left)");
            }
            Row::StatsOverlay => {
                crate::theme::toggle(ui, &mut prefs.stats_overlay, "Show");
            }
            Row::ShowMinimap => {
                crate::theme::toggle(ui, &mut prefs.show_minimap, "Show");
            }
            Row::MinimapSize => {
                let mut at = MinimapSize::ALL
                    .iter()
                    .position(|size| *size == prefs.minimap_size)
                    .unwrap_or(0);
                let labels: Vec<&str> = MinimapSize::ALL.iter().map(|size| size.label()).collect();
                if crate::theme::segmented(ui, "minimap_size", &mut at, &labels) {
                    prefs.minimap_size = MinimapSize::ALL[at];
                }
            }
            Row::ContextMenu => {
                let mut at = ContextMenuStyle::ALL
                    .iter()
                    .position(|style| *style == prefs.context_menu)
                    .unwrap_or(0);
                let labels: Vec<&str> = ContextMenuStyle::ALL
                    .iter()
                    .map(|style| style.short_label())
                    .collect();
                if crate::theme::segmented(ui, "context_menu_style", &mut at, &labels) {
                    prefs.context_menu = ContextMenuStyle::ALL[at];
                }
            }
            Row::MouseTooltips => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.mouse_tooltips,
                    "Show the top menu entry at the cursor",
                );
            }
            Row::HoverTile => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.hover_tile,
                    "Outline the tile under the cursor",
                );
            }
            Row::TileInfoHud => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.tile_info_hud,
                    "Show the tile under the cursor",
                );
            }
            Row::MapIcons3d => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.map_icons_3d,
                    "Draw map icons over their tiles",
                );
            }
            Row::SoundOverlay => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.sound_overlay,
                    "Mark objects that play a sound, even with the Sound panel closed",
                );
            }
            Row::NpcSpawns => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.npc.spawns,
                    "Load and show the NPCs OpenRune places",
                );
            }
            Row::NpcDistance => {
                let mut value = i32::from(prefs.npc.distance);
                ui.add(egui::Slider::new(&mut value, 8..=184).suffix(" tiles"));
                prefs.npc.distance = value as u16;
            }
            Row::NpcMaxDrawn => {
                let mut value = i32::from(prefs.npc.max_drawn);
                ui.add(egui::Slider::new(&mut value, 10..=2000).logarithmic(true));
                prefs.npc.max_drawn = value as u16;
            }
            Row::NpcFootprints => {
                crate::theme::toggle(ui, &mut prefs.npc.footprints, "Outline each spawn's tiles");
            }
            Row::NpcModels => {
                crate::theme::toggle(ui, &mut prefs.npc.models, "Draw the NPC models");
            }
            Row::ObjectGizmo => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.object_gizmo,
                    "Arrows and a turn ring on selected objects",
                );
            }
            _ => return false,
        }
        true
    }

    /// Camera, world streaming, and shared graphics.
    #[allow(clippy::too_many_lines)]
    fn draw_world_row(&mut self, ui: &mut egui::Ui, row: Row) -> bool {
        let prefs: &mut Settings = &mut self.settings.prefs;
        match row {
            Row::RenderScale => {
                ui.add(
                    egui::Slider::new(&mut prefs.render_scale, 0.5..=2.0)
                        .step_by(0.25)
                        .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                        .custom_parser(|text| {
                            text.trim_end_matches('%').trim().parse::<f64>().ok().map(|v| v / 100.0)
                        }),
                )
                .on_hover_text("Above 100% renders more pixels than the window has and smooths edges; below saves GPU time");
            }
            Row::CameraMode => {
                let mut at = usize::from(self.interaction.camera().mode == CameraMode::Orbit);
                if crate::theme::segmented(ui, "camera_mode", &mut at, &["Fly", "Orbit"]) {
                    let mode = if at == 0 {
                        CameraMode::Fly
                    } else {
                        CameraMode::Orbit
                    };
                    self.interaction.set_camera_mode(mode);
                }
            }
            Row::FieldOfView => {
                let mut value = self.interaction.camera().fov_degrees;
                ui.add(egui::Slider::new(&mut value, 30.0..=100.0).suffix("°"));
                self.interaction.set_camera_settings(
                    value,
                    self.interaction.camera().speed,
                    self.interaction.camera().mode,
                );
            }
            Row::FlySpeed => {
                let mut value = self.interaction.camera().speed;
                ui.add(
                    egui::Slider::new(&mut value, 50.0..=20_000.0)
                        .logarithmic(true)
                        .fixed_decimals(0)
                        .suffix(" u/s"),
                );
                self.interaction.set_camera_settings(
                    self.interaction.camera().fov_degrees,
                    value,
                    self.interaction.camera().mode,
                );
            }
            Row::LoadRadius => {
                ui.add(
                    egui::Slider::new(&mut prefs.load_radius, 0..=4).custom_formatter(
                        |radius, _| {
                            let side = radius as u32 * 2 + 1;
                            format!("{side}\u{d7}{side} regions")
                        },
                    ),
                );
            }
            Row::DrawDistance => {
                ui.add(egui::Slider::new(&mut prefs.draw_distance_tiles, 8..=184).suffix(" tiles"));
            }
            Row::AnimateObjects => {
                crate::theme::toggle(ui, &mut prefs.animate_objects, "Play object animations");
            }
            Row::Roofs => {
                egui::ComboBox::from_id_salt("setting_roofs")
                    .selected_text(prefs.roofs.label())
                    .show_ui(ui, |ui| {
                        for choice in RoofChoice::ALL {
                            ui.selectable_value(&mut prefs.roofs, choice, choice.label());
                        }
                    });
            }
            Row::SmoothTerrain => {
                if ui
                    .checkbox(&mut prefs.smooth_terrain, "Blend terrain heights")
                    .changed()
                {
                    self.rebuild_all();
                }
            }
            Row::SkyColor => {
                ui.horizontal(|ui| {
                    ui.color_edit_button_rgb(&mut prefs.sky_color);
                    ui.weak("Reference profile; HD: Default sky > Editor sky colour");
                });
            }
            _ => return false,
        }
        true
    }

    /// The Reference profile's graphics and colour options.
    #[allow(clippy::too_many_lines)]
    fn draw_reference_row(&mut self, ui: &mut egui::Ui, row: Row) -> bool {
        let prefs: &mut Settings = &mut self.settings.prefs;
        match row {
            Row::Profile => {
                egui::ComboBox::from_id_salt("setting_profile")
                    .selected_text(prefs.render_profile.label())
                    .show_ui(ui, |ui| {
                        for profile in ProfileChoice::ALL {
                            ui.selectable_value(
                                &mut prefs.render_profile,
                                profile,
                                profile.label(),
                            );
                        }
                    });
            }
            Row::ReferenceBrightness => {
                ui.add(egui::Slider::new(&mut prefs.brightness, 0.5..=1.0));
            }
            Row::ReferenceFog => {
                ui.add(egui::Slider::new(&mut prefs.fog_depth_tiles, 0..=100).suffix(" tiles"));
            }
            Row::ColorBanding => {
                crate::theme::toggle(ui, &mut prefs.remove_color_banding, "On");
            }
            Row::BrightTextures => {
                crate::theme::toggle(ui, &mut prefs.bright_textures, "On");
            }
            Row::ColorblindMode => {
                egui::ComboBox::from_id_salt("setting_colorblind")
                    .selected_text(format!("{:?}", prefs.colorblind_mode))
                    .show_ui(ui, |ui| {
                        for mode in ColorblindChoice::ALL {
                            ui.selectable_value(
                                &mut prefs.colorblind_mode,
                                mode,
                                format!("{mode:?}"),
                            );
                        }
                    });
            }
            Row::ColorblindStrength => {
                ui.add(egui::Slider::new(
                    &mut prefs.colorblind_intensity,
                    0.0..=100.0,
                ));
            }
            _ => return false,
        }
        true
    }

    /// The 117 HD profile's sky, lighting, shadows, and water.
    #[allow(clippy::too_many_lines)]
    fn draw_hd_row(&mut self, ui: &mut egui::Ui, row: Row) -> bool {
        let prefs: &mut Settings = &mut self.settings.prefs;
        match row {
            Row::HdBrightness => {
                ui.add(egui::Slider::new(&mut prefs.hd_brightness, 25.0..=400.0).suffix(" %"))
                    .on_hover_text("A gamma on the finished image; 100 % = as authored.");
            }
            Row::AtmosphericLighting => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.hd_environment.atmospheric_lighting,
                    "Light each area with its own environment",
                );
            }
            Row::NormalMapping => {
                crate::theme::toggle(ui, &mut prefs.hd_normal_mapping, "On");
            }
            Row::Parallax => {
                crate::theme::toggle(ui, &mut prefs.hd_parallax_mapping, "On (higher GPU cost)");
            }
            Row::Lightning => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.hd_environment.flashing_effects,
                    "Flash in storm areas",
                );
            }
            Row::CurrentArea => {
                ui.label(
                    self.hd
                        .current_name()
                        .unwrap_or_else(|| "(loading)".to_owned()),
                );
            }
            Row::DistanceFog => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.hd_environment.distance_fog,
                    "On (off removes all fog)",
                );
            }
            Row::FogDepthMode => {
                egui::ComboBox::from_id_salt("setting_fog_mode")
                    .selected_text(prefs.hd_environment.fog_depth_mode.label())
                    .show_ui(ui, |ui| {
                        for mode in FogDepthMode::ALL {
                            ui.selectable_value(
                                &mut prefs.hd_environment.fog_depth_mode,
                                mode,
                                mode.label(),
                            );
                        }
                    });
            }
            Row::StaticFogDepth => {
                ui.add(
                    egui::Slider::new(&mut prefs.hd_environment.static_fog_depth, 0.0..=100.0)
                        .suffix(" tiles"),
                );
            }
            Row::GroundFog => {
                crate::theme::toggle(ui, &mut prefs.hd_environment.ground_fog, "On");
            }
            Row::DefaultSky => {
                egui::ComboBox::from_id_salt("setting_default_sky")
                    .selected_text(prefs.hd_environment.default_sky.label())
                    .show_ui(ui, |ui| {
                        for sky in DefaultSky::ALL {
                            ui.selectable_value(
                                &mut prefs.hd_environment.default_sky,
                                sky,
                                sky.label(),
                            );
                        }
                    });
            }
            Row::OverrideSky => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.hd_environment.override_sky,
                    "Use the default sky in every area",
                );
            }
            Row::DecoupleSky => {
                crate::theme::toggle(ui, &mut prefs.hd_environment.decouple_sky_and_water, "On");
            }
            Row::PohThemes => {
                crate::theme::toggle(ui, &mut prefs.hd_environment.poh_themes, "On");
            }
            Row::LegacyTob => {
                crate::theme::toggle(ui, &mut prefs.hd_environment.legacy_tob, "On");
            }
            Row::ShadowMode => {
                egui::ComboBox::from_id_salt("setting_shadow_mode")
                    .selected_text(prefs.hd_shadows.mode.label())
                    .show_ui(ui, |ui| {
                        for mode in ShadowMode::ALL {
                            ui.selectable_value(&mut prefs.hd_shadows.mode, mode, mode.label());
                        }
                    });
            }
            Row::ShadowQuality => {
                egui::ComboBox::from_id_salt("setting_shadow_quality")
                    .selected_text(prefs.hd_shadows.resolution.label())
                    .show_ui(ui, |ui| {
                        for resolution in ShadowResolution::ALL {
                            ui.selectable_value(
                                &mut prefs.hd_shadows.resolution,
                                resolution,
                                resolution.label(),
                            );
                        }
                    });
            }
            Row::ShadowFiltering => {
                egui::ComboBox::from_id_salt("setting_shadow_filtering")
                    .selected_text(prefs.hd_shadows.filtering.label())
                    .show_ui(ui, |ui| {
                        for filtering in ShadowFiltering::ALL {
                            ui.selectable_value(
                                &mut prefs.hd_shadows.filtering,
                                filtering,
                                filtering.label(),
                            );
                        }
                    });
            }
            Row::ShadowDistance => {
                ui.add(
                    egui::Slider::new(&mut prefs.hd_shadows.distance_tiles, 4..=184)
                        .suffix(" tiles"),
                );
            }
            Row::ShadowTransparency => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.hd_shadows.transparency,
                    "Take model transparency into account",
                );
            }
            Row::TerrainShadows => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.hd_shadows.terrain_shadows,
                    "Hills cast shadows",
                );
            }
            Row::RoofShadows => {
                crate::theme::toggle(
                    ui,
                    &mut prefs.hd_shadows.roof_shadows,
                    "Hidden roofs still cast shadows",
                );
            }
            Row::ShadowMemory => {
                ui.weak(format!(
                    "{:.0} MiB",
                    self.renderer.hd_shadow_map_bytes() as f64 / 1_048_576.0
                ));
            }
            Row::Caustics => {
                crate::theme::toggle(ui, &mut prefs.hd_underwater_caustics, "On");
            }
            Row::Exposure => {
                ui.add(egui::Slider::new(&mut prefs.hd_exposure, 0.25..=4.0))
                    .on_hover_text("A multiplier on the finished image; the plugin has none.");
            }
            Row::Vignette => {
                ui.add(egui::Slider::new(&mut prefs.hd_vignette, 0.0..=1.0));
            }
            _ => return false,
        }
        true
    }
}
