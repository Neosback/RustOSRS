//! Project/source identity. Owns the active source and delegates native save/replacement policy
//! to a private persistence adapter. Edited map values remain exclusively in `EditRuntime`.
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub(super) use native::{LifecycleChoice, LifecycleTransition};
use osrs_cache::source::CacheSource;
use std::path::PathBuf;

pub(super) struct ProjectRuntime {
    source: CacheSource,
    generation: u64,
    label: PathBuf,
    build: Option<(u32, String)>,
    error: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    persistence: native::ProjectPersistence,
}
impl Default for ProjectRuntime {
    fn default() -> Self {
        Self::new(CacheSource::Dir(PathBuf::new()), PathBuf::new(), None)
    }
}
impl ProjectRuntime {
    pub(super) fn new(source: CacheSource, label: PathBuf, error: Option<String>) -> Self {
        Self {
            source,
            generation: 1,
            label,
            build: None,
            error,
            #[cfg(not(target_arch = "wasm32"))]
            persistence: native::ProjectPersistence::default(),
        }
    }
    pub(super) fn generation(&self) -> u64 {
        self.generation
    }
    pub(super) fn cache_label(&self) -> &PathBuf {
        &self.label
    }
    pub(super) fn cache_build(&self) -> &Option<(u32, String)> {
        &self.build
    }
    pub(super) fn cache_error(&self) -> &Option<String> {
        &self.error
    }
    pub(super) fn set_cache_error(&mut self, error: String) {
        self.error = Some(error);
    }
    pub(super) fn source_opened(&mut self, build: u32, profile: String) {
        self.build = Some((build, profile));
    }
    pub(super) fn replace_source(&mut self, source: CacheSource, label: PathBuf) {
        self.generation = self.generation.wrapping_add(1);
        self.source = source;
        self.label = label;
        self.build = None;
        self.error = None;
    }
}
