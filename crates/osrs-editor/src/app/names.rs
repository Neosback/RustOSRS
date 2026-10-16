//! Readable names for things the cache leaves unnamed: an OpenRune installation's gameval
//! symbols give almost every object, NPC, item, animation and some sounds an internal name
//! (`loc.castledoubledoorl`). They are read in the background when the open cache is that
//! installation's own LIVE cache, and shown wherever the cache has no name of its own. They are
//! hints for people, not identifiers: nothing keys on them.
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, mpsc},
};

/// Gameval tables worth naming, by table.
#[cfg(not(target_arch = "wasm32"))]
const TABLES: [&str; 5] = ["loc", "npc", "obj", "seq", "synth"];

#[derive(Default)]
pub(super) struct NameBook {
    tables: HashMap<&'static str, HashMap<u32, String>>,
}

impl NameBook {
    fn get(&self, table: &'static str, id: u32) -> Option<&str> {
        self.tables.get(table)?.get(&id).map(String::as_str)
    }

    pub(super) fn object(&self, id: u32) -> Option<&str> {
        self.get("loc", id)
    }

    pub(super) fn sound(&self, id: u32) -> Option<&str> {
        self.get("synth", id)
    }
}

/// `loc.castle_door_l` as `castle door l`.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn readable(symbol: &str) -> String {
    let name = symbol.split_once('.').map_or(symbol, |(_, rest)| rest);
    name.replace('_', " ")
}

#[derive(Default)]
pub(super) struct NameRuntime {
    book: Option<Arc<NameBook>>,
    job: Option<mpsc::Receiver<Result<NameBook, String>>>,
    /// Why names are missing, for the one place that explains it.
    note: Option<String>,
}

/// The OpenRune checkout a cache folder belongs to, when it is the usual `.data/cache/LIVE`.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn installation_of(cache: &Path) -> Option<std::path::PathBuf> {
    cache
        .ends_with(".data/cache/LIVE")
        .then(|| cache.ancestors().nth(3).map(Path::to_path_buf))
        .flatten()
}

impl NameRuntime {
    /// A cache was opened: forget the old names and read the installation's, if there is one.
    pub(super) fn source_opened(&mut self, cache: &Path, build: u32) {
        *self = Self::default();
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(root) = installation_of(cache) {
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(load(&root, build));
            });
            self.job = Some(rx);
        }
        #[cfg(target_arch = "wasm32")]
        let _ = (cache, build);
    }

    /// Pick up finished names. Returns true when something changed.
    pub(super) fn poll(&mut self) -> bool {
        let Some(job) = &self.job else {
            return false;
        };
        let result = match job.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => Err("the name reader stopped".into()),
        };
        self.job = None;
        match result {
            Ok(book) => self.book = Some(Arc::new(book)),
            Err(error) => self.note = Some(error),
        }
        true
    }

    pub(super) fn book(&self) -> Option<&NameBook> {
        self.book.as_deref()
    }

    pub(super) fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load(root: &Path, build: u32) -> Result<NameBook, String> {
    use osrs_cache::openrune::installation::Installation;
    let (revision, symbols, _) = Installation::read_symbols(root)?;
    if revision != build {
        return Err(format!(
            "gameval names are for revision {revision}, the cache is build {build}"
        ));
    }
    let mut book = NameBook::default();
    for table in TABLES {
        let mut names = HashMap::new();
        for (id, symbol) in symbols.table_symbols(table) {
            names.entry(id).or_insert_with(|| readable(symbol));
        }
        book.tables.insert(table, names);
    }
    Ok(book)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn symbols_read_as_plain_words() {
        assert_eq!(readable("loc.castle_door_l"), "castle door l");
        assert_eq!(readable("synth.door_open"), "door open");
        assert_eq!(readable("plain"), "plain");
    }

    #[test]
    fn the_installation_is_found_from_its_live_cache_only() {
        let live = Path::new("/home/me/OpenRune-Server/.data/cache/LIVE");
        assert_eq!(
            installation_of(live),
            Some(Path::new("/home/me/OpenRune-Server").to_path_buf())
        );
        assert_eq!(installation_of(Path::new("/home/me/cache/b241")), None);
        assert_eq!(
            installation_of(Path::new("/x/.data/cache/SERVER")),
            None,
            "only the client cache is named by these tables"
        );
    }
}
