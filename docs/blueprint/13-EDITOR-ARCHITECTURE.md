# RustOSRS Editor Architecture

Status: **Checkpoint 6 normative editor blueprint**  
Decision class: `PROJECT_DECISION`

This document defines the native RustOSRS map-editor product layer. The editor composes `osrs-cache`, `osrs-core`, `osrs-scene`, and `osrs-render`; it does not redefine their semantics.

The editor's primary design rule is:

> Every persistent edit changes semantic document state first. Renderer state, selection visuals, previews, caches, and UI state are derived and disposable.

## 1. Product goals

The editor must provide:

- a native desktop application built with `eframe`/`egui`;
- a Catppuccin-based visual system;
- one or more high-performance wgpu-backed 3D viewports;
- deterministic editing of OSRS terrain and location semantics;
- multi-region and multi-plane workflows;
- stable object/tile selection and inspection;
- semantic-safe placement, move, orientation, duplicate, delete, and terrain tools;
- command/transaction-based undo and redo;
- project persistence, autosave, crash recovery, and explicit export/save workflows;
- preview-only morph, animation, render-profile, time/tick, roof, and diagnostic controls;
- provenance/error surfaces that make unsupported or revision-gated behavior visible;
- a UX suitable for large editing sessions without making the GPU representation authoritative.

## 2. Non-goals

The initial editor does not require:

- networking, login, packet processing, or official-world connectivity;
- CS2/game-widget execution;
- player/NPC simulation;
- a plugin sandbox;
- arbitrary free-form mesh transforms that cannot be represented by the target OSRS semantic/export format;
- editor UI types inside reusable `osrs-*` crates;
- mandatory wasm compatibility.

Reusable lower-level crates remain future-client capable as defined by ADR-0001.

## 3. Application composition

The application-level dependency flow is:

```text
OS/cache source
    |
    v
osrs-cache
    |
    v
canonical definitions/assets
    |
    v
EditorProject / SemanticDocument
    |
    +--> osrs-scene materialization
    |       |
    |       v
    |   semantic SceneSnapshot
    |       |
    |       v
    |   osrs-render extraction -> viewport texture
    |
    +--> command history
    +--> autosave/recovery
    +--> export/save pipeline
```

UI state observes or commands this model. It never owns an alternate map representation.

## 4. Application state partitions

`osrs-editor` should separate state by lifetime and authority.

### 4.1 Persistent project state

Saved with the project:

- project schema version;
- target profile/revision identity;
- source-cache fingerprint and source paths/references as appropriate;
- loaded workspace/region set;
- semantic edits or project-owned canonical map state;
- project metadata;
- optional named camera bookmarks;
- explicit editor annotations that are not exported as OSRS map data;
- export configuration when project-owned.

Persistent project state must not contain raw GPU handles or egui widget state.

### 4.2 Semantic working document

Authoritative editable state for the open project:

- terrain semantic fields;
- loc placements and semantic identifiers;
- source/working region ownership;
- target-profile information;
- edit generation;
- dirty/export status;
- semantic diagnostics/errors.

The exact lower-level representation may be compact, patch-based, or materialized, but all commands must address stable semantic identity.

### 4.3 Session/editor state

May persist as user preferences but is not OSRS data:

- open panels and docking layout;
- active tool;
- current selection and hover;
- camera state;
- active plane visibility;
- render profile;
- diagnostic mode;
- morph/varbit/varp preview state;
- animation/tick timeline state;
- filters/search terms;
- recent files/workspaces.

### 4.4 Derived runtime state

Disposable:

- `osrs-scene` materializations;
- render extraction snapshots;
- GPU zones/resources;
- pick-ID maps;
- thumbnails;
- search indexes;
- temporary previews/ghost geometry;
- asynchronous compile/load results.

## 5. eframe / egui shell

The native application shell uses `eframe` and `egui`.

Responsibilities:

- native window lifecycle;
- menus, panels, dialogs, status bars, popovers, inspectors;
- input routing;
- accessibility and keyboard focus;
- viewport allocation and rendering callback registration;
- persistence of editor-layout/user preferences;
- device-loss/safe-mode presentation.

The editor should use the wgpu renderer already owned by the eframe integration where practical. A viewport must not create an unrelated second graphics device merely to render the scene.

`osrs-render` receives renderer/device integration through a narrow adapter owned by `osrs-editor`; `osrs-render` itself remains egui-independent.

## 6. Viewport composition

The central viewport is a normal egui region whose image/render callback is produced by `osrs-render`.

Conceptually:

```text
egui layout
  -> allocate viewport rect
  -> camera/input update
  -> request renderer frame for rect + project generation
  -> renderer writes viewport target
  -> egui composites viewport target with UI overlays
```

World-space editing overlays should be rendered through the scene renderer when they require depth/camera coherence. Screen-space labels and controls belong to egui.

Examples of world-space overlays:

- selected tile/loc outlines;
- footprints;
- placement ghosts;
- transform/orientation gizmos;
- brush extents;
- normals/priority/depth diagnostics;
- region and zone boundaries;
- collision/occlusion visualization.

Examples of screen-space egui overlays:

- tool HUD;
- coordinate readout;
- active plane/profile badge;
- warning banners;
- hover tooltip;
- camera controls.

## 7. Theme system

Catppuccin is the baseline editor theme family.

Requirements:

- initial default: Catppuccin Mocha;
- theme application centralized in `osrs-editor::theme`;
- semantic status colors must not be encoded only by hue; include icon/text/shape distinctions for accessibility;
- viewport diagnostic palettes are separate from chrome theme colors;
- project data never stores raw egui colors as semantic meaning.

Supporting other Catppuccin flavors or a system/light theme is editor policy and may be added without affecting lower layers.

## 8. Docking and panel model

The editor uses a dockable workspace abstraction. `egui_dock` is an acceptable initial implementation, but the internal panel identity/layout model must not expose third-party dock types throughout the application.

Stable panel IDs should include at least:

- `Viewport`;
- `Toolbox`;
- `Inspector`;
- `AssetBrowser`;
- `Regions`;
- `LayersPlanes`;
- `History`;
- `Diagnostics`;
- `Problems`;
- `ConsoleLog`;
- `TimelinePreview`;
- `ProjectSettings`.

A default layout should prioritize a large viewport, left-side tools/assets, right-side inspector/layers, and a collapsible bottom diagnostics/history/timeline area.

Layout persistence belongs to user/session preferences, not the semantic project format unless explicitly saved as an optional workspace profile.

## 9. Input routing

Input is resolved in strict ownership order:

1. modal dialogs / menus;
2. focused text/numeric editor;
3. egui chrome/panels;
4. interactive screen-space overlay;
5. interactive world-space gizmo/tool;
6. viewport selection/brush;
7. viewport camera controls.

A layer that does not intentionally handle an event must not consume it.

This prevents common editor failures such as a transparent overlay blocking tile selection or a camera drag changing values in an inspector.

## 10. Camera UX

Camera behavior is `EDITOR_POLICY` and remains separate from OSRS semantic orientation.

The editor should support:

- orbit camera around a focus point;
- pan;
- zoom/dolly;
- optional fly/free-camera mode;
- frame selection;
- focus selected tile/object;
- camera bookmarks;
- perspective baseline, with orthographic/top-down views available later if useful.

Camera state uses renderer coordinate conventions but never writes model/loc orientation values.

Input bindings must be configurable through the command/shortcut system rather than hardcoded in individual widgets.

## 11. Selection model

Selection uses stable semantic handles, never raw GPU indices.

Selection state includes:

- primary selection;
- optional multi-selection set;
- current hover candidate;
- selection kind (`Tile`, `Loc`, `Region`, later semantic entities if added);
- selection generation/context.

Picking flow:

```text
pointer request
  -> renderer pick ID
  -> generation check
  -> renderer ID -> stable semantic handle
  -> tool-level acceptance/filtering
  -> selection state
```

Stale pick results are ignored.

Selection remains valid across zone rebuilds. If the underlying semantic entity is deleted or replaced, the selection model resolves or clears it explicitly.

## 12. Multi-selection

Multi-selection is supported where semantic operations are well-defined.

Examples:

- delete multiple loc placements;
- rotate multiple compatible locs in 90-degree semantic steps;
- move a group by whole-tile deltas when every placement remains valid;
- apply a terrain operation to a selected tile area.

The editor must reject or split an operation when a selection contains entities that cannot share the requested semantic edit.

## 13. Layers and plane workflow

The layers/planes panel must distinguish concepts proven separate by `PLANES-*`:

- source/encoded plane;
- scene/storage plane where relevant;
- render/height level;
- collision plane;
- linked-below/bridge relationship.

The UI may present a simplified working concept such as `Editing plane`, but detailed inspection must expose the underlying distinctions when a bridge or derived relation exists.

Required controls:

- active editing plane;
- visibility per plane;
- ghost/fade other planes;
- isolate current plane;
- bridge/link-below indicator;
- roof visibility/removal controls as derived renderer/editor state, not cache semantics.

## 14. Multi-region workspace

The editor must support a workspace larger than one 64x64 region.

Requirements:

- region set identified explicitly in project state;
- stable world/tile coordinates across region boundaries;
- neighboring-region context loadable for algorithms and visual continuity;
- selection and commands may cross region boundaries when all affected regions are writable/loaded;
- unloaded neighbor state must be visibly distinct from empty map state;
- region-border operations must not silently substitute missing context;
- dirty/export status tracked per source region as well as globally.

The editor can stream/render zones independently of region file ownership.

## 15. Inspectors

### Tile inspector

Expose semantic values such as:

- world and region-local coordinate;
- source plane / derived plane information;
- corner heights;
- underlay;
- overlay;
- terrain shape/rotation;
- tile settings/flags with named interpretations where proven;
- locs occupying or anchored to the tile;
- semantic diagnostics/provenance.

Derived render values are shown in a separate diagnostics subsection and are not directly editable unless they map to an explicit semantic field.

### Loc instance inspector

Expose:

- object definition ID/name;
- loc type;
- orientation;
- anchor tile;
- footprint;
- source plane and relevant derived plane information;
- active morph result under preview state;
- animation preview state;
- model/material provenance;
- definition flags relevant to collision/clipping/placement;
- semantic warnings.

Instance-editable fields must be visually distinguished from read-only definition fields.

### Definition/material inspectors

Object/model/texture definitions are browsable and inspectable. Editing shared cache definitions, if ever supported, is a separate advanced feature and must not be confused with editing one loc instance.

## 16. Asset browser

The asset browser should provide:

- object definition search by ID/name;
- model/texture metadata filters;
- thumbnail/preview support;
- type/footprint/animation/morph indicators;
- recent/favorite assets as editor metadata;
- drag or explicit action to begin placement.

Asset thumbnails are derived cache/render artifacts and may be regenerated.

## 17. Preview state is non-destructive

The following are preview/session inputs unless an explicit semantic command changes project data:

- varbit values;
- varp values;
- selected morph branch;
- animation sequence time/frame;
- deterministic render tick;
- roof hiding/removal state;
- plane visibility;
- reference/enhanced render profile;
- diagnostic render mode;
- camera state.

Preview changes must not mark the project dirty.

## 18. Problems and provenance surfaces

The editor must make unsupported/revision-sensitive behavior visible instead of silently approximating it.

The Problems panel should group issues by:

- project/cache compatibility;
- decoder/definition problem;
- semantic scene problem;
- export blocker;
- renderer/device problem;
- stale/recovered autosave problem.

Each problem should retain IDs, region/tile/plane, target revision, owning spec/ADR when known, and an actionable diagnostic path.

Examples:

- terrain color builder is revision-gated for selected target;
- missing model for exact requested loc type;
- invalid/unsupported texture format;
- missing neighboring region required for deterministic border operation;
- project source cache fingerprint changed;
- export would lose an editor-only unsupported transform.

## 19. Status bar

The primary status bar should expose at minimum:

- project dirty state;
- autosave/recovery state;
- active target profile;
- world/tile coordinate under cursor;
- editing plane;
- active tool;
- selection summary;
- render profile;
- compile/render activity;
- warning/error count.

## 20. Safe mode and recovery UX

The editor should support a safe-start mode that can:

- use conservative renderer settings;
- disable optional enhanced features;
- reset corrupted layout/preferences;
- skip automatic reopening of a problematic project when requested;
- expose detailed GPU/capability diagnostics.

Safe mode does not change semantic project data.

## 21. Crate/module ownership

Suggested `osrs-editor` modules:

```text
app/
project/
document/
commands/
history/
tools/
selection/
viewport/
camera/
panels/
inspectors/
assets/
preview/
autosave/
recovery/
export/
shortcuts/
theme/
problems/
settings/
```

These are product modules, not new semantic homes.

## 22. Acceptance conditions for the editor architecture

Checkpoint 6 architecture is satisfied when implementation can eventually prove:

1. a semantic edit succeeds without any GPU object being authoritative;
2. undo/redo restores exact semantic values;
3. viewport/device recreation preserves the document;
4. preview state changes visuals without dirtying the document;
5. selection survives zone recompilation;
6. multi-region coordinates remain stable at borders;
7. bridge/plane detail is inspectable without collapsing distinct plane concepts;
8. unsupported/revision-sensitive semantics produce visible problems rather than silent correction;
9. layout/theme/preferences can reset without affecting project semantics;
10. export reads semantic document state, never render artifacts.
