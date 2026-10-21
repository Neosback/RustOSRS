use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) struct Staging {
    dir: PathBuf,
}

fn empty_destination(out: &Path) -> Result<(), String> {
    if out.exists()
        && (out
            .symlink_metadata()
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
            || !out.is_dir()
            || std::fs::read_dir(out)
                .map_err(|e| e.to_string())?
                .next()
                .is_some())
    {
        return Err(
            "Choose an empty destination folder; existing files will not be replaced.".to_owned(),
        );
    }
    Ok(())
}

impl Staging {
    pub(super) fn new(source: &Path, out: &Path) -> Result<Self, String> {
        empty_destination(out)?;
        let source = source.canonicalize().map_err(|e| e.to_string())?;
        let parent = out
            .parent()
            .ok_or("Export destination has no parent.")?
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let destination = parent.join(
            out.file_name()
                .ok_or("Export destination has no folder name.")?,
        );
        let destination = if destination.exists() {
            destination.canonicalize().map_err(|e| e.to_string())?
        } else {
            destination
        };
        if destination.starts_with(&source) || source.starts_with(&destination) {
            return Err(
                "Export destination must not overlap the original cache folder.".to_owned(),
            );
        }
        Self::allocate(&parent)
    }

    fn allocate(parent: &Path) -> Result<Self, String> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..100 {
            let dir = parent.join(format!(
                ".rustosrs-export-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&dir) {
                Ok(()) => return Ok(Self { dir }),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(format!("Creating export staging directory: {e}")),
            }
        }
        Err("Could not allocate an export staging directory.".to_owned())
    }

    pub(super) fn path(&self) -> &Path {
        &self.dir
    }

    pub(super) fn publish(self, out: &Path) -> Result<(), String> {
        empty_destination(out)?;
        #[cfg(target_os = "windows")]
        if out.exists() {
            std::fs::remove_dir(out).map_err(|e| e.to_string())?;
        }
        std::fs::rename(&self.dir, out).map_err(|e| format!("Publishing verified export: {e}"))?;
        Ok(())
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

pub(super) fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|e| format!("Writing {}: {e}", path.display()))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("Writing {}: {e}", path.display()))
}

pub(super) fn copy_cache(source: &Path, out: &Path) -> Result<(), String> {
    for entry in std::fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let text = name.to_string_lossy();
        if text != "main_file_cache.dat2"
            && !text
                .strip_prefix("main_file_cache.idx")
                .is_some_and(|s| s.parse::<u8>().is_ok())
        {
            continue;
        }
        if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
            return Err("Source cache files must be regular files.".to_owned());
        }
        std::fs::copy(entry.path(), out.join(name)).map_err(|e| format!("Copying cache: {e}"))?;
    }
    Ok(())
}

/// Publish a fully synced new artifact atomically; an existing destination is never replaced.
pub(crate) fn publish_new_file(out: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()
        .map_err(|e| format!("Artifact destination: {e}"))?;
    let destination = parent.join(
        out.file_name()
            .ok_or("Artifact destination has no file name")?,
    );
    let staging = Staging::allocate(&parent)?;
    let staged = staging.path().join("artifact");
    write_file(&staged, bytes)?;
    // The sibling staging directory guarantees one filesystem. Linking publishes only complete
    // bytes and fails on every existing destination; dropping staging retires the temporary link.
    std::fs::hard_link(&staged, &destination)
        .map_err(|e| format!("Publishing {}: {e}", destination.display()))
}
