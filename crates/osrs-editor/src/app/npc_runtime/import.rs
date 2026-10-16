//! OpenRune installation reading and spawn import on a background thread (native only: the
//! browser has no installation tree, so it exchanges scenario sidecars and exports instead).
// The browser has no OpenRune installation to read, so these are only constructed natively.
#![cfg_attr(target_arch = "wasm32", allow(dead_code))]
use osrs_cache::openrune::{GamevalSymbols, ImportReport, SourceText};
use osrs_core::scenario::ScenarioDefinition;
use std::{path::PathBuf, sync::Arc};

/// What was read from an installed OpenRune checkout. Nothing here was written or assigned.
pub(in crate::app) struct InstallationData {
    pub root: PathBuf,
    pub revision: u32,
    pub commit: Option<String>,
    /// Fingerprint of the installation's own LIVE cache, which a scenario binds to.
    pub cache_fingerprint: String,
    pub symbols: Arc<GamevalSymbols>,
    /// First OpenRune symbol of each NPC ID, for placing new spawns that can be exported.
    pub npc_symbols: std::collections::HashMap<u32, String>,
    pub spawn_files: Arc<Vec<SourceText>>,
    pub server_files: Arc<Vec<SourceText>>,
}

/// A parsed selection. Nothing replaces the active scenario until it is applied.
pub(in crate::app) struct ImportPreview {
    pub files: usize,
    pub report: ImportReport,
}

impl ImportPreview {
    /// Validate and build the scenario; `Err` carries every reason it cannot be installed.
    pub fn into_scenario(
        self,
        build: u32,
        fingerprint: String,
        seed: u64,
    ) -> Result<ScenarioDefinition, Vec<String>> {
        self.report.into_scenario(build, fingerprint, seed)
    }
}

pub(in crate::app) enum ImportJob {
    Reading(std::sync::mpsc::Receiver<Result<InstallationData, String>>),
    Parsing(std::sync::mpsc::Receiver<ImportPreview>),
}

pub(in crate::app) enum ImportEvent {
    Installation(Result<InstallationData, String>),
    Preview(ImportPreview),
}

impl ImportJob {
    pub fn poll(&mut self) -> Option<ImportEvent> {
        match self {
            Self::Reading(rx) => match rx.try_recv() {
                Ok(result) => Some(ImportEvent::Installation(result)),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(_) => Some(ImportEvent::Installation(Err(
                    "the installation reader stopped unexpectedly".into(),
                ))),
            },
            Self::Parsing(rx) => match rx.try_recv() {
                Ok(preview) => Some(ImportEvent::Preview(preview)),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(_) => Some(ImportEvent::Preview(ImportPreview {
                    files: 0,
                    report: ImportReport {
                        spawns: Vec::new(),
                        errors: vec!["the importer stopped unexpectedly".into()],
                        warnings: Vec::new(),
                    },
                })),
            },
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(in crate::app) fn read_installation(root: PathBuf) -> ImportJob {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = (|| {
            let installation = osrs_cache::openrune::installation::Installation::read(&root)?;
            let profile = installation.profile(false)?;
            let cache_fingerprint = profile
                .cache_fingerprint
                .value
                .ok_or("the installed cache has no fingerprint")?;
            let mut npc_symbols = std::collections::HashMap::new();
            for (id, name) in installation.symbols.table_symbols("npc") {
                npc_symbols.entry(id).or_insert_with(|| name.to_owned());
            }
            Ok(InstallationData {
                root: installation.root,
                revision: installation.revision,
                commit: installation.source_commit,
                cache_fingerprint,
                symbols: Arc::new(installation.symbols),
                npc_symbols,
                spawn_files: Arc::new(installation.spawn_files),
                server_files: Arc::new(installation.server_files),
            })
        })();
        let _ = tx.send(result);
    });
    ImportJob::Reading(rx)
}

/// Parse the chosen spawn files (every file when `selected` is `None`).
#[cfg(not(target_arch = "wasm32"))]
pub(in crate::app) fn preview_import(
    data: &InstallationData,
    selected: Option<Vec<usize>>,
) -> ImportJob {
    let (tx, rx) = std::sync::mpsc::channel();
    let (symbols, server) = (Arc::clone(&data.symbols), Arc::clone(&data.server_files));
    let files: Vec<SourceText> = match selected {
        None => data.spawn_files.iter().cloned().collect(),
        Some(indices) => indices
            .into_iter()
            .filter_map(|i| data.spawn_files.get(i).cloned())
            .collect(),
    };
    std::thread::spawn(move || {
        let count = files.len();
        let report = osrs_cache::openrune::import_spawns(&files, &server, &symbols);
        let _ = tx.send(ImportPreview {
            files: count,
            report,
        });
    });
    ImportJob::Parsing(rx)
}
