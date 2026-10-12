# Workspace v3 (2026-10-11): polish, one source of truth, editing tools

The user's list, grouped into the order it is built in (each group is one commit or more).

## 1. Polish pass
- [x] Transparent / "draws nothing" overlays show as a checkerboard, not dark purple.
- [x] Palette swatches larger (S / M / L), preview tile a little larger.
- [x] Channel tabs (Overlay, Underlay, Shape, Height, Mask) read as folder tabs: clicking a tab shows it, the
      round tick arms it; the selected tab joins the panel below. No extra words.
- [x] One **View** rail button: tabs View | Graphics (and Overlays) in one panel.
- [x] Tile-info HUD and statistics: one eye menu in the toolbar; the HUD never floats over the picker rail.
- [x] Remove the Teleport button (the T hotkey and world map stay); statistics overlay lives under View.
- [x] Clicking the minimap teleports there (as the world map already does).
- [x] Inspector: tile on the top half (visual), object on the bottom half (visual); inspection only.

## 2. One source of truth for colours (and sizes)
- [x] `style.rs`: named colours with defaults (grid, hover tile, selection, path preview, HUD, transparent
      checker...), user overrides with alpha in the settings, a Colors settings page; every viewport overlay and
      plugin reads them (grid line colour and opacity included).

## 3. Context menus and inspect details
- [x] Menus by what is under the cursor: object (varbit / varp transforms, ids, copy, wiki), tile, both.

## 4. Editing tools
- [x] Object edit layer in the world worker (remove / add loc), rebuilding the region.
- [x] Object tool: move, rotate, delete; Tile tool: select + act (copy, flatten, clear).
- [x] Map icons drawn as billboards in the 3D view (setting).

## 5. Extensibility and docking
- [x] Tool rail entries the user adds in-app (presets, links, macros); plugin API gets rail / menu hooks.
- [x] Dockable panels: move a panel to the other edge or pop it out into a window; saved in settings.

## 6. Rendering settings and cleanup
- [x] A short list of more rendering options for the different needs (not dozens).
- [x] Code cleanup: dead code, long files (`app.rs`, `settings_window.rs`), module docs.

## Built (2026-10-11)

* Everything above is in. Notes on what "done" means for the looser items:
  * **Rendering options**: one new one, **Resolution scale** (50 to 200%: supersampling or a cheaper frame);
    the renderer has no other cheap switches, and more need renderer work (texture filtering, objects off).
  * **Docking**: the panel can move to the other edge or pop out into a window you can place and resize (both
    saved); the rail stays on the edge. Splitting the panel into several docked panels is not built.
  * **Tile editing**: the Inspector shows tiles (inspection only); acting on tiles stays with the Paint and
    Height tools and the context menu (use as brush, clear overlay). A dedicated tile tool is the next step.
  * **Cleanup**: `app.rs` is 900 lines (from 1580): settings I/O, camera moves, streaming and viewport work
    live in their own modules; the env-variable hooks became script commands; dead code and the `.bak` file
    are gone.

## Modal windows and the Plugins menu (2026-10-11)

* **Plugins** is no longer a rail tab or an on/off panel: it is a **Plugins** entry in the menu bar that opens a
  window (My tools, then each plugin's options). Old settings files that name the Plugins rail tab open on the
  Inspector.
* **Every window is modal** (`app/modal.rs`, over `egui::Modal`): Settings, Plugins, Script, Add/Edit tool,
  Teleport, World map and the cache welcome card. The rest of the editor is dimmed and takes no clicks, scrolls or
  key bindings until the window is closed (close button or Esc; Esc closes only the topmost, and not while a combo
  box is open or a key binding is being captured; the cache card has no close because the editor is empty without a
  cache). Windows opened from a window (Add a tool or Script from Plugins) stack on top.
* `EditorApp::modal_open` / `keys_blocked` are the one place that decides the editor's key bindings stay quiet.
* Script: `window settings|plugins|script|map|teleport|none` opens a window (used for screenshots).

## egui-elegance, transform gizmo, floating shelf (2026-10-11)

* **egui-elegance** (`elegance` crate): `theme::set_theme` installs an Elegance theme built from the active
  editor theme, so its widgets match every theme. Used for the boolean switches (`theme::toggle`, replacing
  checkboxes in Settings, the View panel and plugin options), segmented controls for few-way choices
  (`theme::segmented`: camera mode, minimap size, context menu) and filled / outline buttons
  (`theme::primary_button` / `outline_button`: Add a tool, Save, Cancel, Run, Go, Choose cache folder). Four themes
  from the crate's palettes join the list: Slate, Charcoal, Frost, Paper. Menu items, tool arms and the viewport
  chrome keep the editor's own widgets.
* **transform-gizmo-egui** (`app/gizmo.rs`): with the Objects tool and a selection, a gizmo sits on the
  selection: arrows (east, north), a ground-plane handle and a turn ring, snapped to whole tiles and quarter turns.
  Dragging shows the footprints (the same ghost as drag-to-move); release commits one undo step (move, or turn).
  The gizmo's matrices are the viewport's projection re-expressed right-handed (a unit test checks they put a
  point where `pick::project` does). It has the pointer while hovered, so box select and drag-to-move stay out of
  its way. Setting: Interface > "Transform gizmo". The web build needs `getrandom`'s `wasm_js` backend
  (`.cargo/config.toml`), which `transform-gizmo` pulls in through `ahash`.
* **Floating brush shelf**: the shelf (brush, strength, path) has the same pop-out as the side panel: undock
  it into a window you can drag anywhere over the viewport (position kept), dock it back, or swap it to the
  right edge.
* Script: `focus` moves the camera to the selection.

## Ink theme, plugin switches, HUD flicker (2026-10-11)

* **Ink** is the new default theme (index 7 in `theme::ALL`; saved choices keep their index). The readability fix
  is shared by every theme: plain labels now use the full text colour (they used the dim one), only `weak()` text
  (hints, captions) is dim, and the dim colour itself is brighter in Ink. Body text is 14.5 pt, small 12 pt.
* **Plugins window** (menu bar, no icon): a switch per plugin, and nothing else of theirs. Off means gone: the
  plugin is not run, so no menu, toolbar button, overlay, hotkey, guide lines or per-frame work
  (`EditorApp::set_plugin_enabled`, `disabled_plugins` in the settings, `EditorPlugin::set_enabled` to undo what it
  left). `EditorPlugin::core()` marks plugins that cannot be switched off (they show "core" instead of a switch;
  none of the three is core yet). The plugins' own options moved to Settings > Plugin options (and the View panel's
  Overlays tab for the grid). My tools stays in the window.
* **Tile HUD flicker**: the HUD in the viewport's corner covered the pointer, the viewport stopped being hovered,
  the pick went empty, the HUD hid, the pointer was uncovered, repeat. The world under the HUD keeps being picked
  while the pointer is over it (`hud_rect`).

## Objects drawer, valid moves, camera with the gizmo, contrast (2026-10-11)

* **Objects drawer** in groups: Move one tile (west, north, south, east), Floor (up, down: `z` +/- 1, the view
  follows), Turn (both ways), Edit (copy, delete). Objects have no height setting in the map format (they sit on
  the terrain), so "up and down" is the floor. The gizmo's green square is the free-move handle across the ground.
* **Valid moves only**: a move (drag, gizmo, arrows, copy) is refused when it would leave the map, land on a floor
  that does not exist, or land in a square the cache has no map for (`move_blocked`); the footprints turn red while
  dragging and a notice says why.
* **Camera with the gizmo**: `GizmoExt::interact` lays a widget over the cursor, which took the pointer from the
  viewport, so right-drag orbit, pan and scroll zoom stopped while an object was selected in the Objects tool. The
  gizmo is now driven directly (`interact_gizmo`) and only the primary button reaches it.
* **Contrast**: the channel ticks draw a bold check in black or white, whichever the accent calls for
  (`theme::contrast_on`); tabs sit between the inset and the widget fill instead of black, armed tab labels are
  full strength; the picker rail is more opaque and its icons use the text colours.
