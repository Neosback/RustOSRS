//! Portable, deterministic ZIP packs. Entries stay in memory; importing never extracts paths.
use super::ThemeDefinition;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Cursor, Read, Write},
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

const MAX_PACK_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ENTRY_BYTES: u64 = 32 * 1024 * 1024;
const MAX_ENTRIES: usize = 4096;
const MANIFEST: &str = "theme.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeAsset {
    pub sha256: String,
    pub bytes: std::sync::Arc<[u8]>,
}
impl ThemeAsset {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            sha256: digest(&bytes),
            bytes: bytes.into(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ThemePack {
    pub definition: ThemeDefinition,
    pub assets: BTreeMap<String, ThemeAsset>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    definition: ThemeDefinition,
    assets: BTreeMap<String, String>,
}
pub(super) fn validate_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.len() > 512
        || path.contains(['\\', ':', '\0'])
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(format!("Invalid asset path: {path:?}"));
    }
    Ok(())
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn failure(error: impl std::fmt::Display) -> String {
    error.to_string()
}
impl ThemePack {
    pub fn validate(&self) -> Result<(), String> {
        self.definition.validate()?;
        if self.assets.len() >= MAX_ENTRIES {
            return Err("Too many theme assets".into());
        }
        let mut total = 0_u64;
        for (path, asset) in &self.assets {
            validate_path(path)?;
            if path == MANIFEST {
                return Err("theme.json is reserved".into());
            }
            total = total.saturating_add(asset.bytes.len() as u64);
            if asset.bytes.len() as u64 > MAX_ENTRY_BYTES || total > MAX_PACK_BYTES {
                return Err("Theme asset size limit exceeded".into());
            }
            if digest(&asset.bytes) != asset.sha256 {
                return Err(format!("Asset checksum mismatch: {path}"));
            }
        }
        for material in &self.definition.materials {
            for path in material.asset_paths() {
                if !self.assets.contains_key(path) {
                    return Err(format!("Missing material asset: {path}"));
                }
            }
        }
        Ok(())
    }
    /// Hash includes the complete definition and asset identities, independent of ZIP encoding.
    pub fn identity(&self) -> Result<String, String> {
        self.validate()?;
        Ok(digest(
            &serde_json::to_vec(&self.manifest()).map_err(failure)?,
        ))
    }
    fn manifest(&self) -> Manifest {
        Manifest {
            definition: self.definition.clone(),
            assets: self
                .assets
                .iter()
                .map(|(p, a)| (p.clone(), a.sha256.clone()))
                .collect(),
        }
    }
    pub fn to_zip(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::default())
            .unix_permissions(0o644);
        writer.start_file(MANIFEST, options).map_err(failure)?;
        writer
            .write_all(&serde_json::to_vec_pretty(&self.manifest()).map_err(failure)?)
            .map_err(failure)?;
        for (path, asset) in &self.assets {
            writer.start_file(path, options).map_err(failure)?;
            writer.write_all(&asset.bytes).map_err(failure)?;
        }
        Ok(writer.finish().map_err(failure)?.into_inner())
    }
    pub fn from_zip(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() as u64 > MAX_PACK_BYTES {
            return Err("Theme pack exceeds 256 MiB".into());
        }
        let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(failure)?;
        if archive.len() > MAX_ENTRIES {
            return Err("Too many theme entries".into());
        }
        let mut entries = BTreeMap::new();
        let mut total = 0_u64;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(failure)?;
            let path = file.name().to_owned();
            validate_path(&path)?;
            if file.is_dir() || file.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
                return Err("Theme packs contain regular files only".into());
            }
            total = total.saturating_add(file.size());
            if file.size() > MAX_ENTRY_BYTES || total > MAX_PACK_BYTES {
                return Err("Theme expansion size limit exceeded".into());
            }
            let mut data = Vec::new();
            (&mut file)
                .take(MAX_ENTRY_BYTES + 1)
                .read_to_end(&mut data)
                .map_err(failure)?;
            if data.len() as u64 > MAX_ENTRY_BYTES {
                return Err("Theme entry too large".into());
            }
            if entries.insert(path, data).is_some() {
                return Err("Duplicate theme entry".into());
            }
        }
        let manifest: Manifest =
            serde_json::from_slice(&entries.remove(MANIFEST).ok_or("Missing theme.json")?)
                .map_err(failure)?;
        let mut assets = BTreeMap::new();
        for (path, expected) in manifest.assets {
            let asset = ThemeAsset::new(
                entries
                    .remove(&path)
                    .ok_or_else(|| format!("Missing asset: {path}"))?,
            );
            if asset.sha256 != expected {
                return Err(format!("Asset checksum mismatch: {path}"));
            }
            assets.insert(path, asset);
        }
        if !entries.is_empty() {
            return Err("Pack contains undeclared assets".into());
        }
        let pack = Self {
            definition: manifest.definition,
            assets,
        };
        pack.validate()?;
        Ok(pack)
    }
}
