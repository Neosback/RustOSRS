//! Native theme-pack dialogs and immutable library entries; portable codecs live in osrs-hd.
use osrs_hd::theme::ThemePack;
#[cfg(not(target_arch = "wasm32"))]
fn directory() -> Option<std::path::PathBuf> {
    Some(crate::settings::path()?.parent()?.join("themes"))
}
pub(super) fn load_library() -> Vec<ThemePack> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(directory) = directory() else {
            return Vec::new();
        };
        let Ok(entries) = std::fs::read_dir(directory) else {
            return Vec::new();
        };
        let mut paths: Vec<_> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|s| s == "osrstheme"))
            .collect();
        paths.sort();
        paths
            .into_iter()
            .filter_map(|p| std::fs::read(p).ok())
            .filter_map(|b| ThemePack::from_zip(&b).ok())
            .collect()
    }
    #[cfg(target_arch = "wasm32")]
    {
        Vec::new()
    }
}
pub(super) fn import() -> Result<Option<ThemePack>, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("RustOSRS theme", &["osrstheme"])
            .pick_file()
        else {
            return Ok(None);
        };
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        ThemePack::from_zip(&bytes).map(Some)
    }
    #[cfg(target_arch = "wasm32")]
    {
        Err("Theme file dialogs are not available in the browser yet".into())
    }
}
pub(super) fn export(pack: &ThemePack) -> Result<(), String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let bytes = pack.to_zip()?;
        let Some(path) = rfd::FileDialog::new()
            .add_filter("RustOSRS theme", &["osrstheme"])
            .set_file_name(format!("{}.osrstheme", pack.definition.id))
            .save_file()
        else {
            return Ok(());
        };
        atomic_write(&path, &bytes)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = pack;
        Err("Theme file dialogs are not available in the browser yet".into())
    }
}
pub(super) fn save_library(pack: &ThemePack) -> Result<(), String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let directory = directory().ok_or("No theme library directory")?;
        let bytes = pack.to_zip()?;
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let path = directory.join(format!("{}.osrstheme", pack.identity()?));
        if path.exists() {
            return Ok(());
        }
        atomic_write(&path, &bytes)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = pack;
        Err("Persistent theme library is not available in the browser yet".into())
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn atomic_write(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension("osrstheme.tmp");
    std::fs::write(&temp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&temp, path).map_err(|e| e.to_string())
}
