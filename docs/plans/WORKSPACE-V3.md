# Workspace v3 (2026-10-11): polish, one source of truth, editing tools

The user's list, grouped into the order it is built in (each group is one commit or more).

## 1. Polish pass
- [ ] Transparent / "draws nothing" overlays show as a checkerboard, not dark purple.
- [ ] Palette swatches larger (S / M / L), preview tile a little larger.
- [ ] Channel tabs (Overlay, Underlay, Shape, Height, Mask) read as folder tabs: clicking a tab shows it, the
      round tick arms it; the selected tab joins the panel below. No extra words.
- [ ] One **View** rail button: tabs View | Graphics (and Overlays) in one panel.
- [ ] Tile-info HUD and statistics: one eye menu in the toolbar; the HUD never floats over the picker rail.
- [ ] Remove the Teleport button (the T hotkey and world map stay); statistics overlay lives under View.
- [ ] Clicking the minimap teleports there (as the world map already does).
- [ ] Inspector: tile on the top half (visual), object on the bottom half (visual); inspection only.

## 2. One source of truth for colours (and sizes)
- [ ] `style.rs`: named colours with defaults (grid, hover tile, selection, path preview, HUD, transparent
      checker...), user overrides with alpha in the settings, a Colors settings page; every viewport overlay and
      plugin reads them (grid line colour and opacity included).

## 3. Context menus and inspect details
- [ ] Menus by what is under the cursor: object (varbit / varp transforms, ids, copy, wiki), tile, both.

## 4. Editing tools
- [ ] Object edit layer in the world worker (remove / add loc), rebuilding the region.
- [ ] Object tool: move, rotate, delete; Tile tool: select + act (copy, flatten, clear).
- [ ] Map icons drawn as billboards in the 3D view (setting).

## 5. Extensibility and docking
- [ ] Tool rail entries the user adds in-app (presets, links, macros); plugin API gets rail / menu hooks.
- [ ] Dockable panels: move a panel to the other edge or pop it out into a window; saved in settings.

## 6. Rendering settings and cleanup
- [ ] A short list of more rendering options for the different needs (not dozens).
- [ ] Code cleanup: dead code, long files (`app.rs`, `settings_window.rs`), module docs.
