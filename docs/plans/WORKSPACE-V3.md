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
