//! Reject authored features whose visual projection has not landed, rather than silently ignoring them.
use osrs_hd::theme::{ThemePack, ThemeTarget};
pub(crate) fn validate_appearance_pack(pack: &ThemePack) -> Result<(), String> {
    pack.validate()?;
    if !pack.definition.materials.is_empty()
        || pack.definition.overrides.material.is_some()
        || pack
            .definition
            .rules
            .iter()
            .any(|r| r.values.material.is_some())
    {
        return Err("This theme uses image material replacements, which this editor checkpoint cannot render yet".into());
    }
    if pack
        .definition
        .rules
        .iter()
        .any(|r| !matches!(r.target, ThemeTarget::Global | ThemeTarget::Area { .. }))
    {
        return Err("This theme uses terrain or object appearance selectors, which this editor checkpoint cannot render yet".into());
    }
    Ok(())
}
