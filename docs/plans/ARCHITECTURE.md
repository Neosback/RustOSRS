# Architecture: the base everything else sits on

This consolidates what the code, the earlier plans and the reference projects (OpenRune Studio under
`ChatGPT/RSPSi-master`, Terraini, rspsi-imgui, RuneLite, Bevy) say about how to build a RuneScape map editor
that stays easy to extend, debug, fast, and not a pile of parallel systems.

## 1. The reference projects, and what each teaches

| Project | Teaches |
|---|---|
| **OpenRune Studio** (`RSPSi-master`) | The closest cousin: Java/Kotlin + Dear ImGui. *Modular monolith with one canonical world model, one command/change-plan boundary, one place each concern lives.* Its `UI_WORKSPACE_CONTRACT.md` ("Contextual Multi-Rail Workspace") and `EDITOR_DEVELOPMENT_ARCHITECTURE.md` are the best written design to follow. Roadmap items we share: query engine, ChangePlan, WorldFragment transforms, **AutoTile service**, linear-feature service, deterministic modifiers. |
| **Terraini** (decompiled) | Feature depth: overlay-shape atlas with sub-tile masks and edge "portals", road grammar from real-map frequencies, A* road routing, island/coast generators, non-destructive layers, map-piece stamps, height brushes. See `TERRAINI-IDEAS.md`. |
| **rspsi-imgui** | Dear ImGui docking; one `EditorTab` per open map with its own client, undo stack and options; floating tool bar, radial brush menu, swatch panels, minimap HUD overlay, console + performance panels; property classes (`Bool/Int/Str/ObjProperty`, with per-tab proxies) so a setting is declared once. No dedicated auto-tile; overlay/underlay swatches and prefab/ramp/spline tools instead. |
| **RuneLite** | Ground truth for rendering and a model of plugins: small features hooked on well-defined extension points (our `EditorPlugin`). |
| **Bevy** | Slab allocators, GPU timing, camera controllers, picking events, gizmos: reference implementations on the same wgpu version. |

## 2. Principles (non-negotiable)
1. **One home per fact.** Settings: one `Settings` struct (`settings.rs`) the UI edits in place and the file
   serialises. Shortcuts: one registry (`keymap.rs`), plugins add actions to it. Icons: one table
   (`icons.rs`). Theme/fonts: one module each. Widgets that appear in two places call the same function.
   (Started: the duplicated field-per-setting copies in `app.rs` were removed this round.)
2. **Canonical world, derived everything.** The authored region data is the only mutable truth; render
   geometry, picking data, world-map tiles and minimap are *derived* by pure functions of (region data,
   edits). Never edit derived data.
3. **Commands for every edit.** Do/undo, serialisable, validated by a change plan (calculate -> validate ->
   preview -> commit -> undo). Tools and scripts only emit commands.
4. **Pure core, thin shell.** `osrs-core`/`osrs-scene` have no IO or UI; `osrs-world` adds the cache; the
   editor adds presentation. Parity-critical maths stays integer/fixed-point and never learns about egui.
5. **Capabilities, not coordinates.** A tool declares what it needs (brush, drawer, quick palette, HUD,
   preview, selection kind); the shell decides where that appears (OpenRune Studio's tool-capability model).
6. **Work off the render thread, upload on it.** Facades per worker (`RegionStreamer`, `AnimationWorker`,
   `AssetWorker`) with generation-tagged results, bounded per-frame uploads.
7. **Measure before optimising; test before refactoring.** Timings in the overlay, pixel-diff goldens,
   oracle diffs, `cargo test` on every layer.

## 3. Two UIs without two codebases
The UI is a *profile*, not a fork. Everything the profile can change sits behind `Settings` + `theme.rs`
+ layout flags:

| | **Pro** (default) | **Classic OSRS** |
|---|---|---|
| Theme | Catppuccin Mocha etc. | `OSRS Classic` theme (brown stone panels, parchment text, orange accent, square corners) - exists |
| Layout | menu bar, toolbar, tabbed side panel (dockable left/right), status bar | same panels re-skinned; side panel left like the game's key list; game fonts/sprites for chrome |
| Icons | Phosphor | game sprites where they exist (wiki, compass, orbs) via the asset worker |
| Density | compact | larger hit targets, fixed-width panel like the in-game interface |

What exists: theme list with the Classic palette, the asset worker that can load any interface sprite
from the cache. What a full Classic skin still needs (all cosmetic): 9-slice panel frames and buttons from
sprites, the game's bitmap fonts (cache index 13) for headings, and a layout preset (`UiProfile`). Because
all panels already read `Settings` and the keymap, none of this duplicates logic.

## 4. Web (third option)
Is the project "good to go for web"? **Mostly yes at the architecture level, not yet in the build.**
* egui, wgpu 30 and eframe all target WebAssembly (WebGPU, WebGL2 fallback), so the renderer and UI port.
* Blockers, all already isolated in a few places:
  - **Threads**: `RegionStreamer`, `AnimationWorker`, `AssetWorker` use `std::thread` + channels. On the
    web use Web Workers or a single-threaded executor; keep the facades, swap the implementation.
  - **Filesystem**: the cache is read with `std::fs` in `osrs-cache`. Put a `CacheSource` trait under it
    (native file, in-memory, HTTP range, File System Access API / IndexedDB). The cache is ~0.5 GB of
    Jagex data, so on the web the user must supply their own cache locally; do not host it.
  - Small things: `std::time::Instant` -> `web-time`, `memory-stats` / `open` / system font loading are
    native-only (feature-gate), settings path -> `localStorage`, `pollster` headless path unused on web.
* Recommendation: do the `CacheSource` trait now (it also enables server caches and projects), keep
  everything else native, and revisit web when the editor has a usable tool set. Do not design anything
  that needs `std::thread::spawn` in `osrs-core`/`osrs-scene`.

## 5. Crate and module map (target)
```
osrs-core ─ osrs-cache(CacheSource) ─ osrs-scene ─ osrs-world ─ osrs-render
                                               └──────── osrs-edit (NEW: world document, commands,
                                                          change plans, selection, query, auto-tile,
                                                          linear features, fragments)
osrs-editor: shell (workspace, panels, tools UI), plugins, settings, keymap, theme/fonts/icons
osrs-reference: oracle + fixtures
```
`osrs-edit` is the one new crate: it holds everything that is *not* presentation and *not* the cache, so
tools, scripts (Lua later) and tests share it.

## 6. Order of work that avoids crashing and burning
1. `CacheSource` trait + `osrs-edit` skeleton: `WorldDocument` (region overlay over the cache),
   `Command` + history, `ChangePlan`, `SelectionModel`. No tools yet; tests only.
2. Derivation pipeline: dirty-rectangle rebuild (edit +- 6 tiles) feeding render geometry, picking data,
   world-map/minimap tiles from one pass.
3. Workspace shell per `UI_WORKSPACE_CONTRACT`: primary tool rail, context drawer, brush shelf, inspector,
   HUD layer; all driven by `WorkspaceContext` (active tool, capabilities, selection).
4. Auto-tile service (sub-tile atlas, portals, frequency grammar) as a pure `osrs-edit` module with the
   census harness; then tile paint tool on top.
5. Tools one at a time (select, paint, height, objects, roads) each thin over services.
6. Lua/scripting and the HD profile only once the command and query APIs have stopped moving.

## 7. Failure modes to design against
* Two sources of truth (a UI copy of a setting, a cached derived value edited by hand) -> rule 1 and 2.
* Tools that reach into rendering or the cache -> capabilities + commands.
* Edits that change region data but not the derived tiles (world map, minimap, picker) -> one derivation
  pipeline with generation numbers; UI shows stale/fresh state.
* Parity regressions from "improvements" -> every non-reference behaviour is a named, off-by-default or
  clearly labelled option; pixel-diff goldens guard the Reference profile.
* Settings and keymap files that brick the app -> `Settings::sanitized`, unknown fields ignored, defaults
  for missing ones, a "reset layout" escape hatch (OpenRune Studio's panel-persistence rule).
