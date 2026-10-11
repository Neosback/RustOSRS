# Workspace v2: layout, settings, editing model, context menus

Status: **design + first slice built** (settings window, side-panel Settings tab, editor/game context menus,
2026-10-11). Everything below the "Built" section is the plan the next slices follow. It extends
`UI-UX.md` (interaction model, shortcuts) and `ARCHITECTURE.md` (one home per fact, commands for every edit).

## Status: workspace slice (built)

The layout now follows Displee's: a **category rail** on the dock edge (Inspector, View, Graphics, Plugins, and
the settings gear) whose icons swap the **dock panel** (the Inspector has Object | Tile tabs); a bottom **tool
strip** (Select, Paint, Height, Flags, brush size and footprint) and, above it, the active tool's **drawer**: the
tile painter with one tab per channel (Overlay, Underlay, Shape, Height, Mask), each with an *armed* checkbox,
real floor-colour swatches from the cache, shape and rotation thumbnails drawn from the terrain tables, and a
preview. Unarmed channels keep the tile's own values. **Space** opens the quick tools HUD (tools, armed
channels, brush, plane, camera); `V B H X` pick tools, `I` or Alt+click is the eyedropper, `[` `]` resize the
brush, Tab toggles the drawer. The editor context menu has a tool row and, with a brush tool, "Paint here" and
"Eyedrop tile". Context awareness: no drawer for Select, brush controls only for brushing tools, the tile HUD
only over terrain, undo/redo buttons only once a brush tool is active or there is history, game hover text only
with Select. Below 1100 points the toolbar and tool strip collapse to icons; the stats overlay shrinks in narrow
viewports.

**Painting is real for the colour channels**: strokes (left drag) write into an edit layer in the world worker
(`osrs-world::TerrainEdits`, applied right after the cache decodes a region, so the scene, the picker and the HD
stream all see it), rebuild the regions within 6 tiles, and are undone and redone per stroke (Cmd+Z,
Cmd+Shift+Z). Not yet: height edits, flags in the minimap and world map (they read the cache), objects, saving or
exporting. `osrs-edit` (the pure document crate of section 6) is still to be extracted from `paint.rs`,
`app/editing.rs` and `terrain_edits.rs`.

## Status: second workspace pass (built)

* **Always in Select.** The bottom tool strip is gone. A **floating toolbar** on the viewport's left edge shows the
  current tool and switches it (Select, Paint, Spline, Height); in Select it also sets what a click or box
  selects (objects, tiles, or both). Switching is friction-free: tap a tool key (V B P H) to switch, tap it again
  or press `Esc` to return to Select, **hold** a key to peek at a tool and return on release; the quick HUD
  (Space), the context menu's tool row and the toolbar buttons do the same. Brush size lives in the drawer header.
* **Multi-select.** Click, Shift adds, Cmd toggles, drag a box. Tiles and objects have their own sets; the
  Selection tab summarises them; Paint can "Fill N selected", Height can level or smooth them.
* **Tile painter.** Channels are plain values now: overlay, underlay, shape + rotation, **one height** (a slider and
  a field, up is positive) and **one set of flags** (checkboxes). Unticked = the tile keeps its own. The palettes
  list the floors the real map uses (a one-second census at start: 476 of ~650 overlays, 221 underlays are used),
  Common / All used / Textured, bigger swatches, **textures drawn from the cache with their scroll animation**.
  Placeholder (magenta) overlays are listed only under All used. The eyedropper has no button; `I`, Alt + click
  and the context menu's "Copy tile to brush" remain.
* **Spline tool** (replaces Flags). Click control points on the terrain (drag to move, Cmd + click removes,
  Backspace removes the last), set the width and edge style, Enter lays it as one undo step. The path is a
  Catmull-Rom spline; `autotile.rs` rasterises the band per tile at 8 x 8 and picks the (shape, rotation) whose
  overlay from the real face tables has the smallest Hamming error, then runs portal passes so neighbouring
  edges agree. Preview shows the exact shapes before committing.
* **Height tool** (dedicated): Raise, Lower, Smooth, Flatten (to the height under the cursor or a set height),
  Roughen; strength, falloff, brush size; applies to the selection too. Heights are stored as the map file does
  (a byte, relative to the plane below, a stored 1 reads as 0) through the edit layer, and the regions rebuild.

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
Docking: the **right dock** and the **bottom drawer** are each an `egui_tiles::Tree` (crate 0.17.1, depends on
egui ^0.36, same as ours). Panes are ours (inspector, quick settings, outliner, object viewer, minimap, tool palette,
object library); egui_tiles only provides tabs, splits and drag to rearrange, plus a serialisable tree for "reset
layout". The viewport stays the central panel (not a tile: it cannot be closed or tabbed away, and its input rect
stays simple), and so do the menu bar, tool rail and status bar. See section 8.

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
  stroke is one undo step; edits mark **two** dirty rectangles (see "Invalidation" below); the derived render,
  picker, world map and minimap data rebuild from the same pass.
* **Channel types** (`osrs-edit`): `ChannelAction<T> { Keep, Set(T), Clear }` for underlay, overlay and height;
  `FlagAction { Keep, Set(u8), Add(u8), Remove(u8), Toggle(u8) }`; `shape_mode: Auto | Full | Fixed(shape, rot)`.
  A tile edit is a sparse per-tile `(before, after)` record, so undo is exact.
* **Invalidation** (checked against `terrain_build.rs`): underlay ids feed an 11 x 11 sliding window
  (`BLEND_RADIUS = 5`, only ids > 0 contribute), so a changed underlay dirties colour **+-5**. Heights change
  the slope-light and normals of the tile and its neighbours (**+-1**) and the models standing there; they do not
  feed the underlay blend (the window sums underlay definitions, not shaded colour). Overlay id / shape / rotation
  are **+-0**. `dirty_colour_rect` and `dirty_geometry_rect` stay separate; the rebuild takes the union of what
  its edits touched. Unit tests pin each radius against a full rebuild (partial result == full result).
* **Underlay under an overlay**: a tile with underlay 0 contributes nothing to its neighbours, so painting a full
  overlay on it does not bleed anything. A tile that *has* an underlay keeps feeding the blend under its overlay,
  which is why `Keep` is correct. The editor never rewrites a kept channel silently; when an overlay lands on a
  tile with underlay 0 next to textured ground it only *offers* "copy the dominant neighbouring underlay"
  (a checkbox on the stroke, off by default).
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
* **Slots** (from `SceneGrid::insert_placement`): boundary (types 0-3), wall decoration (4-8), floor decoration
  (22) are one slot each and replace; **game objects (types 9-21, including diagonal walls and roofs)** are a
  list of up to five per tile, with per-tile edge masks, inserted atomically over the whole footprint. So a rug
  (22) and a bench (10) share a tile, and two game objects can too while capacity allows. Validation asks
  "is this slot taken / is there capacity", never "has the tile any loc".
* **Rotating in place**: the footprint swaps width and depth for orientations 1 and 3. The default keeps the
  south-west anchor tile (what the client does when only the orientation changes); "keep centre" re-derives the
  anchor from the rotated footprint rectangle and is only offered when the result lands on the tile grid (even
  differences of width and depth). The ghost uses the same function as the commit.
* **Multi-selection rotation**: integer arithmetic only. The pivot is the south-west corner of the primary
  object (the first selected or the hovered one), or `(floor cx, floor cy)` of the selection; every object's
  footprint *rectangle* is rotated about the pivot and its new anchor is the rectangle's minimum corner (rotating
  only the anchor tile would land the wrong corner for footprints larger than 1 x 1).
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

### Stroke transactions

Pointer down opens a `StrokeTransaction` holding a sparse scratch overlay of the touched tiles or objects (never a
copy of the document). Drags mutate the scratch and refresh the preview and dirty rectangles; nothing reaches the
undo stack. Pointer up diffs scratch against the base: empty -> dropped, otherwise one atomic `ChangePlan` is
pushed (one stroke = one undo step). Escape aborts and restores the preview. Nudge-key repeats and gizmo drags
use the same transaction.

### Scatter determinism

Scatter placement is a pure function `hash(tile.x, tile.y, stroke_seed, salt)` (a small integer mixer such as
splitmix64; no dependency, no `thread_rng`). `stroke_seed` is fixed from pointer down to up, so the ghost does
not flicker while the cursor moves or the radius changes and the committed result equals the preview.

### Bridge tiles

Tile flag bit 2 on **plane 1** is the bridge bit (`terrain_build::link_bridge_tiles`: plane 1 content is linked
below, locs on a bridge tile render one plane lower via `tile_min_plane`); bit 8 forces render plane 0. Rules:
locs on a bridge tile stay stored on plane 1 even though they are drawn and picked as plane 0; copying plane 1
without plane 0 warns "bridge without support"; the place and paint tools refuse to set the bridge bit when the
plane 0 tile below is not loaded, and warn when it is blocked or empty.

## 7. Order of work

1. **Built**: settings registry, window, quick tab, editor/game context menus (this document's section 2-3).
2. `osrs-edit` foundation + tests (document, commands, undo, selection, dirty rectangles, derivation hook).
3. Workspace shell: `Tool` trait, left rail, context drawer, brush popover, undo/redo in the toolbar,
   Cmd+K palette, selection modes in the status/toolbar.
4. Tile paint tool: channel strip, presets, eyedropper, brush / line / rect / fill / replace, auto shape. First
   because tile paint is the first thing a mapper does, and its data (a per-tile channel record) is the smallest
   self-contained case to prove the `Tool` trait, transactions, undo and the partial-rebuild hook.
5. Select tool + object move / rotate / delete / duplicate.
6. Heights and flags tools with their overlays; map pieces (copy / paste, bridge rules).
7. Place tool and library; scatter brush; query operations.
8. Save/export: project overlay file, then cache write-back behind an explicit export.

Acceptance for each slice: unit tests in `osrs-edit` for the commands, a golden round trip (edit then undo is
byte-identical), the Reference pixel goldens unchanged, and the shortcut / palette / menu entry for every
new action.

## 8. Review of outside comments (2026-10-11), checked against the code

Adopted as written: the `ChannelAction` / `FlagAction` model; stroke transactions; deterministic scatter; the
bridge hazards; tile paint before objects (for the reason given in section 7, not for a cache-metadata
dependency: loc definitions are already decoded and used by the picker and scene builder); separate colour and
geometry dirty rectangles.

Corrected before adopting:
* "Uninitialised underlays bleed 5 tiles": underlay id 0 is skipped by the blend, so there is nothing to bleed.
  A real underlay under a full overlay does bleed, which is correct client behaviour. Silent auto-seeding would
  break `Keep`; it is an opt-in checkbox instead.
* Height invalidation +-7: the blend window sums underlay definitions, not shaded colours, so heights need +-1
  (normals and slope light), not +-7. Colour is +-5, overlay is +-0.
* Slot table: game objects are types **9-21**, not 9-11, and several (up to five) may share a tile; only
  boundary, wall decoration and floor decoration are single-slot.
* The rotation anchor formula is an editor policy, not an OSRS rule, and rotating only anchor tiles of a
  multi-selection is wrong for larger footprints; replaced by the rules in section 5.
* "Camera plane is adjusted down" for bridges is not part of map data; the bridge rule is the link-below and
  render-level one above.

Crates. `egui_tiles` is adopted for the two docks (section 1). It is a tile tree with panes we write, not a
ready-made outliner or inspector. Not adopted now: `egui_gizmo` / a gizmo (moves are tile-snapped; the arrow
keys and inspector fields cover precision), `parry3d` (the picker and terrain ray march already exist),
`fixedbitset`, `smallvec`, `postcard`, `flume`, `rhai` (add when a profile or a slice needs it). `bitflags` for the
tile flag byte is fine to add with the paint tool; `rayon` only when a bulk operation (replace over a region,
census) is measured to need it.
