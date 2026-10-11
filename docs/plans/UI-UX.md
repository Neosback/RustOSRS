# Editor UI/UX: layout, interaction model, shortcuts

## 1. What the current layout is (implemented)

```
+--------------------------------------------------------------------------+
| File  Edit  View  Go  Help                                    (menu bar) |
| Plane 0 1 2 3 | Fly Orbit | Grid v |                 Teleport Stats Panel |  toolbar
+---------------------------------------------------------+----------------+
|                                                         | Object|View|   |
|   3D viewport                                           | Shading|Tools  |  side panel
|   (stats overlay top-left, hover info bottom-left)      | (dock L or R)  |  (tabs)
|                                                         |                |
+---------------------------------------------------------+----------------+
| context hints                          RAM / GPU | regions, vertices     |  status bar
+--------------------------------------------------------------------------+
```
Principles taken from Blender, Unreal and the community RuneScape map editors (RSPSi, Terraini):
* The viewport owns the screen; chrome is thin. One menu bar, one toolbar row, one status bar.
* Anything used every minute is on the toolbar (plane, camera mode, grid, teleport); everything is also in
  a menu and has a rebindable shortcut. The side panel is for *properties of the current thing*, in tabs,
  so it never needs scrolling through unrelated options.
* Overlays (stats) live *under* floating windows; windows (teleport, shortcuts) never get covered.
* Docking: a full docking system (egui_dock) is not worth it yet: there are four tabs and one dock. The
  panel can sit left or right (View menu). A real dock becomes useful when the outliner, asset browser,
  and console exist; the `EditorPlugin` trait already lets a plugin add a tab/section, which is the seam to
  extend (add `PanelTab::Plugin(name)` or move to `egui_dock` then).

## 2. Interaction model (what the user asked for)

| Input | Action |
|---|---|
| Right drag | look (fly) / orbit (orbit camera) |
| Shift + right drag, middle drag | pan |
| Shift + right click | context menu (copy object / tile / neighbourhood) |
| Left click | select (left drag is free for box select and brushes) |
| Scroll | fly speed (fly) / zoom (orbit) |
| Shift + movement keys | boost |

Right-drag-to-move is what OSRS players and RSPSi users expect, so it stays primary. The context menu
moved behind Shift so that releasing a right drag can never open a menu. All keys are actions in
`keymap.rs`; the *Controls and shortcuts* window (Edit / Help menu) lists them, lets you click a binding
and press the new keys, shows conflicts, resets one or all, and persists overrides in `editor.json`.
Plugins register their own actions (`EditorPlugin::actions`), so they appear there automatically.

## 3. How Jagex-style tooling would likely work (design stance, not documented fact)

Jagex's internal map tools are proprietary, so this is inference from the client's data model and the
public community editors:
* **Region-centric**: the unit of work is the 64x64 region with 4 planes; editing happens per region and the
  loader rebuilds only dirty regions (the blend/normal windows are +-6 tiles, so dirty rectangles must be
  padded).
* **Layered data**: terrain (heights, underlay, overlay+shape+rotation, flags), locs (objects with type and
  orientation), and later spawns/areas. Tools act on one layer at a time with an active-layer indicator.
* **Selection first**: rectangle/brush selection with replace/add/subtract/toggle (Terraini), then
  operations on the selection (flatten, fill, retile, copy as map piece).
* **Preview = the real renderer**: no separate "editor look"; what you see is the game scene with overlays
  (grid, collision flags, selection), which is why the picker/outline data is built from the same pipeline.
* **Safe by default**: every edit is a command with undo; write-back is explicit (export to cache/server).

## 4. Next UI steps (structure only, in order)
1. Outliner tab (regions loaded, objects in selection, layers) as another `PanelTab`.
2. Viewport tool trait (activate / handle input / draw overlay / commit) and a **tool shelf** column on the
   left: Select, Terrain paint, Height, Objects, Measure. Tools register through plugins.
3. Selection model shared by picker and tools (see `TERRAINI-IDEAS.md`).
4. Command log / undo history panel.
5. Layouts: save the panel state per workspace (Edit, Inspect, Shading) once more than one panel exists.

## 5. Bevy: UI/UX and shader pieces that suit an OSRS editor
Bevy renders with wgpu 30 like we do, so these are copyable (MIT/Apache):
* `bevy_dev_tools`: `infinite_grid` (done, adapted to draped tile lines), `frame_time_graph` (a graph
  instead of one number), `picking_debug`, `easy_screenshot` (screenshot hotkey).
* `bevy_gizmos_render` + `transform_gizmo_render.rs`: a translate/rotate gizmo. This is the reference for
  moving objects with handles once object editing exists (screen-space-constant size, axis picking).
* `bevy_camera_controller`: its pan/orbit and free-camera docs define "responsive, robust, satisfying"
  rules we followed (first-order input, no drift while panning).
* `bevy_picking` (backend / hover / pointer model): clean split between pointer events and picking
  backends; our picker could expose the same shape so tools get `Over/Out/Click/Drag` events.
* `bevy_feathers` / `bevy_ui_widgets`: ideas for consistent editor widgets (numeric drag fields, colour
  swatches). We stay on egui; borrow behaviour (drag-to-scrub numbers, Alt-click to reset).
* Shaders for the HD port: `bevy_pbr/src/atmosphere` (sky), `volumetric_fog`, `ssao`, `bevy_anti_alias`
  (SMAA/FXAA/TAA), `bevy_post_process` (bloom, tonemapping). Do not use them for the Reference profile.
