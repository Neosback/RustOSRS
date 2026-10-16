//! Native discovery for an installed OpenRune checkout. All semantic parsing
//! lives in the byte/text APIs alongside this adapter.
use super::{GamevalSymbols, SourceText};
use crate::{profile::TargetProfile, registry::bundled_profiles, transport::CacheProbe};
use std::path::{Path, PathBuf};

pub struct Installation {
    pub root: PathBuf,
    pub revision: u32,
    pub source_commit: Option<String>,
    pub symbols: GamevalSymbols,
    pub spawn_files: Vec<SourceText>,
    pub server_files: Vec<SourceText>,
}
impl Installation {
    /// Read installed mappings before declarations, exactly as GameValProvider.
    /// Does not run install, assign missing IDs, or write external files.
    pub fn read(root: impl AsRef<Path>) -> Result<Self, String> {
        let root = root.as_ref();
        let manifest: toml::Value =
            toml::from_str(&read(root.join(".data/gamevals-binary/max-ids.toml"))?)
                .map_err(|e| e.to_string())?;
        let revision = manifest
            .get("revision")
            .and_then(toml::Value::as_integer)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or("missing manifest revision")?;
        let mut symbols = GamevalSymbols::default();
        for (file, base) in [("gamevals.dat", true), ("gamevals_generated.dat", false)] {
            let path = root.join(".data/gamevals-binary").join(file);
            if base || path.exists() {
                symbols.load_dat(
                    &std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?,
                    base,
                )?;
            }
        }
        let max = manifest
            .get("max_ids")
            .and_then(|v| v.get("npc"))
            .and_then(toml::Value::as_integer)
            .ok_or("missing NPC reserved maximum")?;
        if symbols.base_max("npc").map(i64::from) != Some(max) {
            return Err("NPC base mapping disagrees with max-ids manifest".into());
        }
        let mut declarations = Vec::new();
        for directory in ["content", "api"] {
            walk(&root.join(directory), &mut declarations)?;
        }
        declarations.sort();
        let mut server_files = Vec::new();
        for path in declarations {
            if path.file_name().is_some_and(|n| n == "gamevals.toml") {
                symbols
                    .load_toml(&read(&path)?)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
            }
            if path.extension().is_some_and(|n| n == "toml")
                && path.components().any(|c| c.as_os_str() == "configs")
            {
                let text = read(&path)?;
                if text.contains("[[npc]]") {
                    server_files.push(source(root, &path, text)?);
                }
            }
        }
        let mut rscm = Vec::new();
        walk(&root.join(".data/gamevals"), &mut rscm)?;
        rscm.sort();
        for path in rscm {
            if let Some(table) = path.file_stem().and_then(|n| n.to_str()) {
                symbols.load_rscm(table, &read(&path)?)?;
            }
        }
        let spawn_dir = root.join(".data/raw-cache/map/npcs");
        let mut paths: Vec<_> = std::fs::read_dir(&spawn_dir)
            .map_err(|e| e.to_string())?
            .map(|r| r.map(|e| e.path()).map_err(|e| e.to_string()))
            .collect::<Result<_, _>>()?;
        paths.sort();
        let spawn_files = paths
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e == "toml"))
            .map(|p| source(root, &p, read(&p)?))
            .collect::<Result<_, _>>()?;
        let server = root.join(".data/raw-cache/server/npcs.toml");
        server_files.insert(0, source(root, &server, read(&server)?)?);
        let source_commit = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_owned());
        Ok(Self {
            source_commit,
            root: root.to_owned(),
            revision,
            symbols,
            spawn_files,
            server_files,
        })
    }
    /// Fingerprint the installed cache and bind its manifest-declared build.
    /// This records an installed artifact, not an independent proof of its revision.
    pub fn profile(&self, server: bool) -> Result<TargetProfile, String> {
        let kind = if server { "SERVER" } else { "LIVE" };
        let path = self.root.join(".data/cache").join(kind);
        let probe = CacheProbe::of(&path)?;
        let mut profile = bundled_profiles()
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|p| p.cache_source.build == self.revision)
            .ok_or_else(|| format!("unsupported installed revision {}", self.revision))?;
        profile.id = format!(
            "openrune-installed-{}-{}-{}",
            self.revision,
            kind.to_lowercase(),
            &probe.fingerprint[..12]
        );
        profile.cache_source.provider = "openrune-installed".into();
        profile.cache_source.cache_id = 0;
        profile.cache_source.uri = format!("openrune-installation:{kind}");
        profile.cache_source.timestamp = "installed-artifact-time-unspecified".into();
        profile.cache_source.groups_present = probe.groups_total as u64;
        profile.cache_source.groups_total = probe.groups_total as u64;
        profile.cache_source.physical_content_indices = probe.content_indices.len() as u32;
        profile.cache_source.empty_logical_indices.clear();
        profile.cache_source.logical_archive_slots_present = probe.content_indices.len() as u32;
        profile.cache_source.logical_archive_slots_total = probe.content_indices.len() as u32;
        profile.cache_fingerprint.value = Some(probe.fingerprint);
        profile.cache_fingerprint.state = "installed-artifact-manifest-declared-build".into();
        profile.cache_source.reported_size_mib = std::fs::read_dir(&path)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .filter_map(|e| e.metadata().ok())
            .filter(|m| m.is_file())
            .map(|m| m.len())
            .sum::<u64>()
            .div_ceil(1024 * 1024);
        profile.semantic_sources.insert("installed_openrune".into(),crate::profile::SemanticSource {repository:Some("OpenRune/OpenRune-Server".into()),commit:self.source_commit.clone(),tree:None,status:"installed_checkout; source/data hashes recorded separately; manifest declares build".into()});
        profile.transport_evidence = None;
        profile.xtea.provider_id = "openrune-installed".into();
        profile.validate().map_err(|e| e.to_string())?;
        Ok(profile)
    }
}
fn read(path: impl AsRef<Path>) -> Result<String, String> {
    std::fs::read_to_string(path.as_ref()).map_err(|e| format!("{}: {e}", path.as_ref().display()))
}
fn source(root: &Path, path: &Path, text: String) -> Result<SourceText, String> {
    Ok(SourceText {
        path: path
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/"),
        text,
    })
}
fn walk(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(path).map_err(|e| format!("{}: {e}", path.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().map_err(|e| e.to_string())?.is_symlink() {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            if !["build", "target", "out", ".git", ".gradle"]
                .iter()
                .any(|n| path.file_name().is_some_and(|p| p == *n))
            {
                walk(&path, files)?;
            }
        } else if path
            .extension()
            .is_some_and(|p| p == "toml" || p == "rscm" || p == "rscm2")
        {
            files.push(path);
        }
    }
    Ok(())
}
