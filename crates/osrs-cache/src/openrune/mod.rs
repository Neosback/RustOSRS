//! OpenRune scenario interchange. Parsing is headless and accepts owned source
//! bytes; native directory discovery is an adapter, not import authority.
mod export;
mod gamevals;
mod import;
#[cfg(not(target_arch = "wasm32"))]
pub mod installation;
pub use export::{OpenRuneExport, export_scenario};
pub use gamevals::GamevalSymbols;
pub use import::{ImportReport, SourceText, import_spawns};
