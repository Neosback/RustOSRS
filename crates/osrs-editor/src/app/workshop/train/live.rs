//! Using a trained building model on the main map, through the Build page: the model's learned
//! style becomes a kit in the Kit list ("... (trained model)"), so the site, ghost preview, apply
//! and undo are the Build page's own; and the model's judge can pick a good seed for the site.

use crate::app::EditorApp;
use osrs_lab::{Brief, build_with_model};

const SUFFIX: &str = " (trained model)";

/// Put every trained model's style in the Build page's kit list (once each).
pub(in crate::app) fn offer_kits(app: &mut EditorApp) {
    let train = &mut app.prefabs.workshop.train;
    train.models.ensure_loaded();
    let new: Vec<(String, String)> = train
        .models
        .entries
        .iter()
        .filter(|(file, _)| !train.offered.contains(file))
        .filter_map(|(file, model)| {
            let mut kit = model.kit.clone();
            kit.name = format!("{}{SUFFIX}", model.name);
            serde_json::to_string(&kit)
                .ok()
                .map(|text| (file.clone(), text))
        })
        .collect();
    for (file, text) in new {
        app.prefabs.workshop.train.offered.insert(file.clone());
        app.prefabs.build.add_kit_text(&file, &text);
    }
}

/// The name of the trained model behind the chosen kit, when it is one.
pub(in crate::app) fn chosen_model(app: &EditorApp) -> Option<String> {
    let kit = app.prefabs.build.kits().get(app.prefabs.build.form.kit)?;
    kit.name.strip_suffix(SUFFIX).map(str::to_owned)
}

/// Try several seeds for the form's kind and size with the chosen model's judge, and set the form
/// to the one it likes best. Returns what to tell the user.
pub(in crate::app) fn best_seed(app: &mut EditorApp) -> Result<String, String> {
    let name = chosen_model(app).ok_or("Choose a kit that ends in (trained model) first.")?;
    let model = app
        .prefabs
        .workshop
        .train
        .models
        .entries
        .iter()
        .find(|(_, m)| m.name == name)
        .map(|(_, m)| m.clone())
        .ok_or("That model is no longer saved.")?;
    let form = app.prefabs.build.form.clone();
    let (seed, realism) = pick_seed(&model, &form)?;
    app.prefabs.build.form.seed = seed;
    Ok(format!(
        "The AI looked at 16 seeds and kept number {seed} (ranking score {realism:.3}; uncalibrated).",
    ))
}

/// The seed the model's judge likes best among 16 for the form's kind and size, and how real it
/// thinks that building looks.
fn pick_seed(
    model: &osrs_lab::Model,
    form: &crate::app::prefab_runtime::BuildForm,
) -> Result<(u64, f32), String> {
    let kind = crate::app::prefab_runtime::ARCHETYPES
        .iter()
        .find(|(a, _)| *a == form.archetype)
        .map_or("house".to_owned(), |(_, label)| label.to_lowercase());
    let brief = Brief {
        sketch: None,
        kind: if ["house", "shop", "tavern", "bank", "church", "warehouse"].contains(&kind.as_str())
        {
            kind
        } else {
            "house".to_owned()
        },
        width: form.width.clamp(8, 24),
        height: form.height.clamp(8, 24),
        floors: form.floors.clamp(1, 2),
        seed: form.seed,
        variation: form.variation,
        rooms: None,
    };
    let built = build_with_model(model, &brief, 16)?;
    let best = built
        .tried
        .first()
        .ok_or("none of the tries gave a usable building")?;
    Ok((best.seed, best.realism))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::prefab_runtime::BuildForm;

    #[test]
    fn a_trained_models_style_joins_the_kit_list_and_its_judge_picks_a_seed() {
        let model = super::super::tests::quick_model();
        let mut kit = model.kit.clone();
        kit.name = format!("{}{SUFFIX}", model.name);
        let text = serde_json::to_string(&kit).unwrap_or_else(|e| panic!("{e}"));
        let mut build = crate::app::prefab_runtime::BuildState::default();
        let before = build.kits().len();
        assert!(
            build.add_kit_text("trained", &text),
            "{:?}",
            build.kit_problems()
        );
        assert_eq!(build.kits().len(), before + 1);
        assert!(
            build
                .kits()
                .last()
                .is_some_and(|k| k.name.ends_with(SUFFIX))
        );
        let form = BuildForm {
            width: 12,
            height: 10,
            seed: 5,
            ..BuildForm::default()
        };
        let (seed, realism) = pick_seed(&model, &form).unwrap_or_else(|e| panic!("{e}"));
        assert!(
            (5..21).contains(&seed),
            "a seed from the sixteen tried: {seed}"
        );
        assert!((0.0..=1.0).contains(&realism));
        assert_eq!(
            pick_seed(&model, &form),
            Ok((seed, realism)),
            "the same form gives the same pick"
        );
    }
}
