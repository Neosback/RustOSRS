//! Theme Studio widgets own drafts only. Frame integration routes authored changes to EditRuntime.
mod controls;
mod files;
mod integration;
mod rules;
use eframe::egui;
pub(super) use integration::{frame, panel};
use osrs_hd::theme::{ThemeDefinition, ThemePack};

pub(in crate::app) enum Action {
    Apply(Box<ThemePack>),
    Import,
    Export,
    SaveLibrary,
    LoadLibrary(usize),
    CancelPreparation,
}

#[derive(Default)]
pub(in crate::app) struct ThemeStudio {
    draft: Option<ThemePack>,
    baseline: Option<ThemePack>,
    pub(super) action: Option<Action>,
    pub(super) preparing: Option<(ThemePack, u64, u64, bool)>,
    pub(super) error: Option<String>,
    pub(super) library: Vec<ThemePack>,
    library_loaded: bool,
    search: String,
    pub(super) compare: bool,
    pub(super) preview_time: Option<f32>,
    pub(super) shader_edit: bool,
}
impl ThemeStudio {
    pub(in crate::app) fn preview(&self) -> Option<&ThemePack> {
        if self.compare {
            self.baseline.as_ref()
        } else if self.shader_edit || self.error.is_some() {
            None
        } else {
            self.draft.as_ref()
        }
    }
    pub(in crate::app) fn draw(
        &mut self,
        ui: &mut egui::Ui,
        current: &ThemePack,
        tile: osrs_core::coords::MapTile,
        plane: osrs_core::coords::PlaneIndex,
    ) {
        if !self.library_loaded {
            self.library = files::load_library();
            self.library_loaded = true;
        }
        if self.draft.is_none() || self.baseline.as_ref().is_some_and(|base| base != current) {
            self.draft = Some(current.clone());
            self.baseline = Some(current.clone());
        }
        ui.heading("Theme Studio");
        ui.label("Give this world its own look.");
        ui.horizontal_wrapped(|ui| {
            if ui.button("Stock 117HD").clicked() {
                self.action = Some(Action::Apply(Box::default()));
            }
            ui.add_enabled(false, egui::Button::new("2008 HD · coming next"))
                .on_disabled_hover_text(
                    "The 530 texture importer and material mappings are still being implemented.",
                );
            if ui.button("New Theme").clicked() {
                self.action = Some(Action::Apply(Box::new(new_theme())));
            }
        });
        let can_export =
            self.preparing.is_none() && self.draft.as_ref().is_none_or(|draft| draft == current);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Duplicate").clicked() {
                let mut pack = current.clone();
                pack.definition = pack
                    .definition
                    .duplicate(custom_id(), format!("{} copy", pack.definition.name));
                self.action = Some(Action::Apply(Box::new(pack)));
            }
            if ui.button("Import…").clicked() {
                self.action = Some(Action::Import);
            }
            if ui
                .add_enabled(can_export, egui::Button::new("Export…"))
                .on_disabled_hover_text("Apply or cancel your preview changes first.")
                .clicked()
            {
                self.action = Some(Action::Export);
            }
            if ui
                .add_enabled(can_export, egui::Button::new("Save to library"))
                .on_disabled_hover_text("Apply or cancel your preview changes first.")
                .clicked()
            {
                self.action = Some(Action::SaveLibrary);
            }
        });
        if !self.library.is_empty() {
            egui::ComboBox::from_id_salt("theme-library")
                .selected_text("Theme library…")
                .show_ui(ui, |ui| {
                    for (index, pack) in self.library.iter().enumerate() {
                        if ui
                            .selectable_label(
                                false,
                                format!("{} · v{}", pack.definition.name, pack.definition.revision),
                            )
                            .clicked()
                        {
                            self.action = Some(Action::LoadLibrary(index));
                        }
                    }
                });
        }
        if self.preparing.is_some() {
            ui.spinner();
            ui.label("Preparing appearance…");
            if ui.button("Cancel preparation").clicked() {
                self.action = Some(Action::CancelPreparation);
            }
            return;
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.separator();
        ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Find a control…"));
        let Some(draft) = &mut self.draft else {
            return;
        };
        let before = draft.clone();
        ui.label("Theme name");
        ui.text_edit_singleline(&mut draft.definition.name);
        controls::draw(
            ui,
            &mut draft.definition,
            &self.search,
            &mut self.shader_edit,
            tile,
            plane,
        );
        if *draft != before && draft.definition.id == "stock-117hd" {
            draft.definition.id = custom_id();
            if draft.definition.name == before.definition.name {
                draft.definition.name = "My HD theme".into();
            }
        }
        let dirty = draft != current;
        ui.separator();
        let mut cancel = false;
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut self.compare, "Compare before edit");
            if ui.add_enabled(dirty, egui::Button::new("Apply")).clicked() {
                self.action = Some(Action::Apply(Box::new(draft.clone())));
            }
            if ui
                .add_enabled(dirty, egui::Button::new("Cancel changes"))
                .clicked()
            {
                cancel = true;
            }
        });
        if cancel {
            self.settled();
            return;
        }
        // A continuous gesture previews from a draft and commits once when released.
        if dirty
            && !self.shader_edit
            && !ui.ctx().input(|i| i.pointer.any_down())
            && !ui.ctx().egui_wants_keyboard_input()
            && let Some(draft) = &self.draft
        {
            self.action
                .get_or_insert_with(|| Action::Apply(Box::new(draft.clone())));
        }
        ui.small("Appearance changes stay in your project and theme packs. Map exports keep their original game semantics.");
    }
    pub(in crate::app) fn source_opened(&mut self) {
        self.settled();
        self.preparing = None;
        self.action = None;
        self.error = None;
    }
    pub(super) fn settled(&mut self) {
        self.error = None;
        self.draft = None;
        self.baseline = None;
        self.compare = false;
        self.shader_edit = false;
        self.preview_time = None;
    }
}
fn custom_id() -> String {
    format!(
        "custom-{}",
        web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}
fn new_theme() -> ThemePack {
    ThemePack {
        definition: ThemeDefinition::stock().duplicate(custom_id(), "New theme".into()),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelling_preview_keeps_authority_and_restores_the_visible_base() {
        let authority = crate::app::edit_runtime::EditRuntime::default();
        let before = authority.clone();
        let mut draft = authority.appearance().clone();
        draft.definition.name = "Unsaved preview".into();
        let mut studio = ThemeStudio {
            draft: Some(draft),
            baseline: Some(authority.appearance().clone()),
            ..Default::default()
        };
        assert_ne!(studio.preview(), Some(authority.appearance()));
        studio.compare = true;
        assert_eq!(studio.preview(), Some(authority.appearance()));
        studio.settled();
        assert!(studio.preview().is_none());
        assert_eq!(authority, before);
    }

    #[test]
    fn studio_controls_fit_dock_widths_and_display_scales() {
        for width in [240.0, 320.0, 480.0] {
            for scale in [1.0, 1.5, 2.0] {
                let ctx = crate::theme::layout_test_context(scale);
                let mut studio = ThemeStudio {
                    library_loaded: true,
                    ..Default::default()
                };
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 900.0),
                    )),
                    ..Default::default()
                };
                let mut output = ctx.run_ui(input, |ui| {
                    let right = ui.max_rect().right();
                    let response = ui.scope(|ui| {
                        studio.draw(
                            ui,
                            &ThemePack::default(),
                            osrs_core::coords::MapTile::new(3200, 3200),
                            osrs_core::coords::PlaneIndex::new(0)
                                .unwrap_or_else(|| unreachable!("valid plane")),
                        )
                    });
                    assert!(
                        response.response.rect.right() <= right + 0.5,
                        "width {width}, scale {scale}: {:?}",
                        response.response.rect
                    );
                });
                output.textures_delta.clear();
            }
        }
    }
}
