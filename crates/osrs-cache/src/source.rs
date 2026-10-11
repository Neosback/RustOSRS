//! Where a cache comes from: a directory on disk, or the files already read into memory (the browser
//! build, where the page reads the folder through the File System Access API).

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

/// The files of an OSRS cache held in memory: `main_file_cache.dat2`, `main_file_cache.idx255` (the
/// reference table) and every other `main_file_cache.idxN`.
#[derive(Debug, Default)]
pub struct CacheFiles {
    pub dat2: Arc<Vec<u8>>,
    pub reference: Vec<u8>,
    pub indices: Vec<(u8, Vec<u8>)>,
    /// The fingerprint of these exact bytes, computed by the first open (every later open of the
    /// same files, from the streamer, the palette and the asset loader, reuses it).
    pub(crate) fingerprint: OnceLock<[u8; 32]>,
}

impl CacheFiles {
    /// Add one file by its name (`main_file_cache.dat2`, `main_file_cache.idx12`, ...). Other names
    /// (`.dat`, `.idx` of other formats, `info`, ...) are ignored; the result says whether the file
    /// was one the cache needs.
    pub fn add_file(&mut self, name: &str, bytes: Vec<u8>) -> bool {
        let Some(extension) = name.rsplit('.').next() else {
            return false;
        };
        if extension == "dat2" {
            self.dat2 = Arc::new(bytes);
            self.fingerprint = OnceLock::new();
            return true;
        }
        match extension
            .strip_prefix("idx")
            .and_then(|id| id.parse::<u8>().ok())
        {
            Some(255) => {
                self.reference = bytes;
                true
            }
            Some(id) => {
                self.indices.retain(|(existing, _)| *existing != id);
                self.indices.push((id, bytes));
                true
            }
            None => false,
        }
    }

    /// Whether the files needed to open a cache are all there.
    pub fn is_complete(&self) -> bool {
        !self.dat2.is_empty() && !self.reference.is_empty() && !self.indices.is_empty()
    }

    /// Size of everything held, in bytes.
    pub fn total_bytes(&self) -> usize {
        self.dat2.len()
            + self.reference.len()
            + self.indices.iter().map(|(_, b)| b.len()).sum::<usize>()
    }
}

/// A cache to open. Cheap to clone (memory sources share their bytes).
#[derive(Debug, Clone)]
pub enum CacheSource {
    Dir(PathBuf),
    Memory(Arc<CacheFiles>),
}

impl CacheSource {
    /// A short label for messages.
    pub fn label(&self) -> String {
        match self {
            Self::Dir(path) => path.display().to_string(),
            Self::Memory(files) => format!("memory ({} MiB)", files.total_bytes() / (1024 * 1024)),
        }
    }
}

impl From<PathBuf> for CacheSource {
    fn from(path: PathBuf) -> Self {
        Self::Dir(path)
    }
}
impl From<&PathBuf> for CacheSource {
    fn from(path: &PathBuf) -> Self {
        Self::Dir(path.clone())
    }
}
impl From<&Path> for CacheSource {
    fn from(path: &Path) -> Self {
        Self::Dir(path.to_path_buf())
    }
}
impl From<&str> for CacheSource {
    fn from(path: &str) -> Self {
        Self::Dir(PathBuf::from(path))
    }
}
impl From<String> for CacheSource {
    fn from(path: String) -> Self {
        Self::Dir(PathBuf::from(path))
    }
}
impl From<&String> for CacheSource {
    fn from(path: &String) -> Self {
        Self::Dir(PathBuf::from(path))
    }
}
impl From<&CacheSource> for CacheSource {
    fn from(source: &CacheSource) -> Self {
        source.clone()
    }
}
impl From<Arc<CacheFiles>> for CacheSource {
    fn from(files: Arc<CacheFiles>) -> Self {
        Self::Memory(files)
    }
}
