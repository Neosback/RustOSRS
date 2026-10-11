# Workspace v2: layout, settings, editing model, context menus

Status: **design + first slice built** (settings window, side-panel Settings tab, editor/game context menus,
2026-10-11). Everything below the "Built" section is the plan the next slices follow. It extends
`UI-UX.md` (interaction model, shortcuts) and `ARCHITECTURE.md` (one home per fact, commands for every edit).

Reference: OpenRune Studio / RSPSi (`~/Documents/ChatGPT/RSPSi-master`), whose `MapEditorView` is "faithful to
Displee's Map Editor UI layout". We keep what makes that layout fast to learn and drop what costs space or
duplicates state.

## 1. What we take from the Displee / RSPSi layout, and what we do better

| Displee / RSPSi has | We keep | We change |
|---|---|---|
| Menu bar, workspace tab bar, status bar | menu bar, status bar | tabs only when more than one map/project is open |
| Bottom **tool rail** + **context drawer** above it (palette, library of the active tool) | the drawer: wide palettes need width | the rail is a **left vertical rail** (Blender-like): bottom space stays free for the drawer; one key per tool |
| Left **brush rail** (shape, radius, falloff, strength), appears for brush tools | the idea that brush mechanics are shared | no second left column: brush options are a **pinnable popover** by the tool rail, `[` `]` resize, Alt + wheel strength |
| Right **icon rail + panel host** (map settings, minimap, object viewer, outliner) | right dock with panels | settings are not a panel: **Settings tab (frequent) + settings window (all, searchable)** |
| Viewport HUDs: floating toolbar, minimap, tile info | minimap HUD, tile info HUD (movable, pinnable) | HUDs read one inspection snapshot shared with the inspector; none holds a settings form |
| Command palette | **Cmd+K** palette over every action (the keymap registry already lists them) | actions are the same ids the shortcuts use |
| Tile painter console with six tabs (overlay, underlay, shape, height, flags, rotation) and an apply-checkbox per channel | per-channel enable, live topology preview, eyedropper | one **channel strip** instead of six tabs, plus per-channel *modes*, presets and region operations (section 4) |
| `Preferences` window, plugin manager | preferences window | one registry drives window, search, quick tab (section 2) |

Layout (target):

```
+------------------------------------------------------------------------------------+
| File Edit View Go Help                                  Cmd+K command palette       |
| Plane 0 1 2 3 | Fly Orbit | Undo Redo | selection mode | grid |      Teleport  Gear  |
+------+-----------------------------------------------------------+-----------------+
| tool |                                                           | Object|Settings |
| rail |                       viewport                            |  |Tools         |
|      |      HUDs (minimap, tile info, brush) float on it         |  inspector /    |
| Sel  |                                                           |  quick settings |
| Paint|                                                           |  / plugin tabs  |
| Hgt  +-----------------------------------------------------------+                 |
| Flag |  context drawer: the active tool's palette / library      |                 |
| Obj  |                                                           |                 |
+------+-----------------------------------------------------------+-----------------+
| status: hover info, hints, RAM/GPU, dirty regions, undo depth                       |
+------------------------------------------------------------------------------------+
```
Rules: one active tool; the drawer shows only the active tool's content and collapses when it has none; the
inspector never mutates render or cache state directly (it emits commands); every panel's state is part of
`Settings`/workspace state with a "reset layout" escape hatch.

## 2. Settings (built)

* **One registry** (`app/settings_window.rs`, table `ROWS`): each setting is declared once with page, section,
  label, search keywords and the render profile it acts on; `draw_row` is the only place its widget exists.
  The window, the search, the profile greying and the side-panel quick tab all read the table.
* **Settings window** (gear in the toolbar, Edit > Settings, Cmd+,): pages Interface, Camera and world, Graphics,
  HD lighting and sky, HD shadows and water, Controls (mouse + editable shortcuts), Advanced (resets, file
  path). Search matches label, section and keywords across all pages.
* **Side panel Settings tab**: the frequently used rows (camera mode, FOV, fly speed, draw distance, load radius,
  animate objects, render profile + brightness, fog, sky, shadows, context menu style, minimap, stats), stacked
  for a narrow panel, only those that act on the current profile. "All settings..." opens the window.
* Adding a setting: one `Row` variant, one table entry, one `draw_row` arm, optionally one entry in `QUICK`.
  Tool state (brush, armed values, presets) is **not** a global setting; it lives with the tool and persists
  under `Settings::plugins`.

## 3. Context menus (built)

Two menus, never mixed; Shift + right click opens the one chosen in Settings > Interface (Alt+M switches):

* **Editor menu** (default): header naming the object / tile, Select, Focus camera, Teleport here, Copy.
  Editing actions (Paint here, Move, Duplicate, Delete, Replace...) join this menu as tools land, filtered by
  what is under the cursor and by the active tool.
* **Game menu**: exactly the client's menu (`Choose Option`, the object's actions, `Examine`, `Walk here`,
  `Cancel`) from `interact.rs`, with nothing of the editor in it. The mouse tooltip (RuneLite's hover text) has
  its own switch.

The game menu is parity/reference behaviour and must stay read-only in look; the client branch reuses it.

## 4. Tile painting

What a tile is: underlay id, overlay id + shape (0..11) + rotation (0..3), four corner heights, a flag byte
(blocked, bridge, hidden-below / roof rules, ...), and the objects standing on it. People paint in different
ways: "this material, keep my heights", "carve a path of overlay 5 over whatever is there", "raise this hill but
leave textures", "mark these tiles blocked". So the brush does not paint "a tile"; it paints **channels**:

```
 Underlay [x] 50   keep | set | clear
 Overlay  [x] 6    keep | set | clear      shape [Auto|Full..] rotation [Auto|0..3]
 Height   [ ] 0    keep | set | raise | lower | flatten | smooth
 Flags    [ ] 0x01 keep | set | add | remove | toggle
 [ preview: result on the tile under the cursor, kept channels ghosted ]   [eyedropper] [presets v]
```
* **Default is keep**: an unchecked channel is never touched, so "use what is already there except the overlay
  I set" is the normal case, not a mode. Checking a channel arms it.
* **Overlay carries its shape**: setting an overlay on a tile without one needs a shape; `Auto` uses the
  auto-tiler (`AUTO-TILER.md`: sub-tile mask, portals, census grammar) so strokes produce valid edges;
  `Full`/fixed shapes are available; on a tile that already has an overlay, `Keep shape` is the default.
  Clearing the overlay resets shape and rotation (the client ignores them).
* **Eyedropper** (I / Alt + click): samples the tile under the cursor into the armed channels; holding a chip
  samples only that channel. **Presets**: save the armed set as a named preset (dirt path, shoreline,
  cobbles), a favourites row in the drawer, recent values per channel.
* **Brush shapes and operations**: brush (circle / square, radius, falloff, strength), line, rectangle,
  flood fill (same underlay+overlay region), **replace** (A -> B inside the selection or region), erase (clear
  armed channels), with `Shift` = straight line, `Ctrl` = erase, `Alt` = sample. Operations run on the brush
  footprint *or on the current tile selection*.
* **Flags and heights as views**: while a channel is armed its overlay shows (collision flags, height
  contours, hidden tiles); the heights tool owns sculpting (raise, lower, flatten, smooth, slope limit,
  falloffs from Terraini); the paint tool only *sets* height for deliberate flat patches.
* **Safety**: hover shows a ghost of the result (change plan: calculate, validate, preview, commit); one
  stroke is one undo step; edits mark dirty rectangles padded by 6 tiles (blend and normal windows); the
  derived render, picker, world map and minimap data rebuild from the same pass.
* Region rules checked before commit: not editing outside loaded regions, bridge/plane linkage, hidden-tile
  colour 12345678, underlay ids that exist in the cache.

## 5. Objects: select, move, place

* **Selection**: click, shift = add, ctrl = toggle, drag = box, modifiers replace/add/subtract; selecting by
  type filter (walls, game objects, decorations, ground decorations) and "same id nearby". The selection is
  the scope tools act on; the outliner lists it.
* **Move**: three equivalent ways, all emitting the same `MoveObjects` command: (1) drag on the ground plane,
  tile-snapped (objects live on tiles; sub-tile offsets exist only through decoration displacement and
  diagonal types, shown as such); (2) arrow keys nudge 1 tile (Shift = 8, PageUp/Down = plane); (3) exact
  x, y, plane, orientation, type fields in the inspector. A translate gizmo (Bevy's
  `transform_gizmo_render.rs` as reference) is optional on top.
* **Rotate**: R or the orientation field (0..3); a multi-selection rotates around its centre (positions and
  orientations together); mirror and diagonal types follow the definition's rules.
* **Place**: library drawer (search by name / id / category, model thumbnails from the cache), ghost
  preview on the hovered tile with the **footprint outline** (width x depth by orientation), red when the slot
  is invalid; loc type picker (0..22, defaulting from the definition); scatter brush for trees and
  flowers (density, minimum spacing, random from a set, random orientation).
* **Validation = the scene builder's own rules** (`PlacementPlan`: slot per loc type, footprint, plane,
  bridge handling), reported like Terraini's placement result ("skipped 3: occupied").
* **Copy / paste as map piece** (Ctrl+C / V): ghost with anchor (origin / centre), height mode (relative /
  absolute / keep target), conflict policy (merge / skip occupied / replace).
* **Query operations**: replace id A with B, delete by id, in the selection / region / whole map, with a count
  preview before commit.

## 6. Architecture the slices share

* `osrs-edit` (pure): `WorldDocument` (edit layers over the cache), `Command` + history (coalescing per stroke),
  `ChangePlan` (preview before commit), `SelectionModel`, `ToolCapabilities`. No egui, no GPU.
* Editor shell: `Tool` trait with capabilities (brush options, drawer, quick palette, HUD data, selection
  kind, wheel ownership, preview); the shell decides where each shows (the capability model from
  `UI_WORKSPACE_CONTRACT.md`). Tools register through the same plugin seam as the grid and wiki tools.
* Every tool action is a keymap action id (palette, shortcuts and menus share it); tool options persist per
  tool; the context menu asks the active tool for its entries.

## 7. Order of work

1. **Built**: settings registry, window, quick tab, editor/game context menus (this document's section 2-3).
2. `osrs-edit` foundation + tests (document, commands, undo, selection, dirty rectangles, derivation hook).
3. Workspace shell: `Tool` trait, left rail, context drawer, brush popover, undo/redo in the toolbar,
   Cmd+K palette, selection modes in the status/toolbar.
4. Select tool + object move / rotate / delete / duplicate (first real editing, smallest data model).
5. Tile paint tool: channel strip, presets, eyedropper, brush / line / rect / fill / replace, auto shape.
6. Heights and flags tools with their overlays; map pieces (copy / paste).
7. Place tool and library; scatter brush; query operations.
8. Save/export: project overlay file, then cache write-back behind an explicit export.

Acceptance for each slice: unit tests in `osrs-edit` for the commands, a golden round trip (edit then undo is
byte-identical), the Reference pixel goldens unchanged, and the shortcut / palette / menu entry for every
new action.
