//! Browser support: reading the cache folder through the File System Access API
//! (`showDirectoryPicker`, Chromium), or from URLs for development, plus the page glue.

use osrs_cache::source::CacheFiles;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r#"
const WANTED = /^main_file_cache\.(dat2|idx\d+)$/;

async function readInto(file, name, progress) {
  progress("Reading " + name + " (" + Math.round(file.size / 1048576) + " MiB)...");
  return [name, new Uint8Array(await file.arrayBuffer())];
}

// Ask for a folder and read the cache files in it.
export async function pick_cache_folder(progress) {
  if (!("showDirectoryPicker" in window)) {
    throw new Error("This browser has no File System Access API. Use Chrome or Edge.");
  }
  const dir = await window.showDirectoryPicker({ id: "osrs-cache", mode: "read" });
  const files = [];
  for await (const [name, handle] of dir.entries()) {
    if (handle.kind === "file" && WANTED.test(name)) {
      files.push(await readInto(await handle.getFile(), name, progress));
    }
  }
  return { name: dir.name, files };
}

// Development: read the same files from a URL prefix on the page's own server.
export async function fetch_cache_folder(base, progress) {
  const names = ["main_file_cache.dat2", "main_file_cache.idx255"];
  for (let i = 0; i < 40; i++) names.push("main_file_cache.idx" + i);
  const files = [];
  for (const name of names) {
    const response = await fetch(base + name);
    if (!response.ok) {
      if (name.endsWith("dat2") || name.endsWith("idx255")) {
        throw new Error("could not fetch " + base + name + ": " + response.status);
      }
      continue;
    }
    progress("Reading " + name + "...");
    files.push([name, new Uint8Array(await response.arrayBuffer())]);
  }
  return { name: base, files };
}

export function hide_loading() {
  const el = document.getElementById("loading");
  if (el) el.remove();
}

export function page_query(name) {
  return new URLSearchParams(window.location.search).get(name);
}

// A small text file from a URL (the viewer's spawn list).
export async function fetch_text(url) {
  const response = await fetch(url);
  if (!response.ok) throw new Error("could not fetch " + url + ": " + response.status);
  return await response.text();
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn pick_cache_folder(progress: &js_sys::Function) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch)]
    async fn fetch_cache_folder(
        base: &str,
        progress: &js_sys::Function,
    ) -> Result<JsValue, JsValue>;
    fn page_query(name: &str) -> Option<String>;
    #[wasm_bindgen(catch)]
    async fn fetch_text(url: &str) -> Result<JsValue, JsValue>;
    pub fn hide_loading();
}

/// A cache folder read into memory.
pub struct PickedCache {
    pub name: String,
    pub files: CacheFiles,
}

/// What the page is doing about loading a cache, shared with the async task.
pub enum PickState {
    Reading(String),
    Done(Result<PickedCache, String>),
}

pub type SharedPick = Rc<RefCell<PickState>>;

/// The `?cache=<url prefix>` the page was opened with, if any.
pub fn cache_url_param() -> Option<String> {
    page_query("cache").filter(|value| !value.is_empty())
}

/// The page was opened as the mobile viewer (`?view=mobile`): no editor chrome, a fixed demo.
pub fn viewer_requested() -> bool {
    page_query("view").is_some_and(|view| view == "mobile")
}

/// `?npcs=<url>`: a spawn-edits file for the viewer to show (the browser cannot read OpenRune).
pub fn npcs_url_param() -> Option<String> {
    page_query("npcs").filter(|value| !value.is_empty())
}

/// `?at=x,y[,plane]`: where the viewer's player stands.
pub fn at_param() -> Option<(i32, i32, u8)> {
    let value = page_query("at")?;
    let mut parts = value.split(',').map(|part| part.trim().parse::<i32>().ok());
    let x = parts.next()??;
    let y = parts.next()??;
    let plane = parts.next().flatten().unwrap_or(0).clamp(0, 3) as u8;
    Some((x, y, plane))
}

/// A text file read by the page's `fetch`, shared with the async task that reads it.
pub type SharedText = Rc<RefCell<Option<Result<String, String>>>>;

pub fn start_fetch_text(url: String, repaint: impl Fn() + 'static) -> SharedText {
    let state: SharedText = Rc::new(RefCell::new(None));
    let shared = Rc::clone(&state);
    wasm_bindgen_futures::spawn_local(async move {
        let result = fetch_text(&url)
            .await
            .map_err(|e| describe(&e))
            .and_then(|value| value.as_string().ok_or_else(|| "not text".to_owned()));
        *shared.borrow_mut() = Some(result);
        repaint();
    });
    state
}

fn describe(error: &JsValue) -> String {
    error
        .as_string()
        .or_else(|| {
            js_sys::Reflect::get(error, &"message".into())
                .ok()
                .and_then(|message| message.as_string())
        })
        .unwrap_or_else(|| format!("{error:?}"))
}

fn unpack(result: JsValue) -> Result<PickedCache, String> {
    let name = js_sys::Reflect::get(&result, &"name".into())
        .ok()
        .and_then(|name| name.as_string())
        .unwrap_or_default();
    let files = js_sys::Reflect::get(&result, &"files".into()).map_err(|e| describe(&e))?;
    let mut cache = CacheFiles::default();
    for entry in js_sys::Array::from(&files).iter() {
        let pair = js_sys::Array::from(&entry);
        let file_name = pair.get(0).as_string().unwrap_or_default();
        let bytes = js_sys::Uint8Array::new(&pair.get(1)).to_vec();
        cache.add_file(&file_name, bytes);
    }
    if !cache.is_complete() {
        return Err(format!(
            "{name} has no main_file_cache.dat2 / .idx files. Choose the folder that contains them."
        ));
    }
    Ok(PickedCache { name, files: cache })
}

/// Start reading a cache; the returned state is `Reading` until the task finishes, then `Done`.
/// `base` reads from URLs (development) instead of asking for a folder.
pub fn start_pick(base: Option<String>, repaint: impl Fn() + 'static) -> SharedPick {
    let state: SharedPick = Rc::new(RefCell::new(PickState::Reading(
        "Waiting for the folder...".to_owned(),
    )));
    let shared = Rc::clone(&state);
    let repaint = Rc::new(repaint);
    wasm_bindgen_futures::spawn_local(async move {
        let progress_state = Rc::clone(&shared);
        let progress_repaint = Rc::clone(&repaint);
        let progress = Closure::<dyn Fn(String)>::new(move |message: String| {
            *progress_state.borrow_mut() = PickState::Reading(message);
            progress_repaint();
        });
        let function: &js_sys::Function = progress.as_ref().unchecked_ref();
        let outcome = match &base {
            Some(base) => fetch_cache_folder(base, function).await,
            None => pick_cache_folder(function).await,
        };
        let result = outcome.map_err(|e| describe(&e)).and_then(unpack);
        *shared.borrow_mut() = PickState::Done(result);
        repaint();
        drop(progress);
    });
    state
}

pub fn open_url(url: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or("no window")?;
    window
        .open_with_url_and_target(url, "_blank")
        .map(|_| ())
        .map_err(|e| describe(&e))
}

/// Linear memory in use by this module, in MiB.
pub fn wasm_memory_mib() -> Option<f32> {
    let memory = wasm_bindgen::memory();
    let buffer = js_sys::Reflect::get(&memory, &"buffer".into()).ok()?;
    let length = js_sys::Reflect::get(&buffer, &"byteLength".into()).ok()?;
    Some(length.as_f64()? as f32 / (1024.0 * 1024.0))
}
