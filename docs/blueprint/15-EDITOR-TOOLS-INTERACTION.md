# RustOSRS Editor Tools, Interaction, and Workflow

Status: **Checkpoint 6 normative editor blueprint**  
Decision class: `PROJECT_DECISION`

This document defines the user-facing editing tools and interaction contracts for the RustOSRS map editor.

## 1. Tool architecture

Tools are state machines over semantic selections and transactions.

A tool may:

- request picks from the renderer;
- produce non-destructive preview overlays;
- validate a candidate semantic operation;
- begin/update/commit/cancel a transaction;
- update editor-only hover/status information.

A tool may not:

- directly mutate GPU resources as authoritative state;
- invent unsupported semantic fields;
- silently clamp an invalid semantic operation into something else;
- mutate shared cache definitions when the user intends to edit one scene instance.

Conceptually:

```rust
trait EditorTool {
    fn id(&self) -> ToolId;
    fn begin(&mut self, ctx: &mut ToolContext, input: ToolInput);
    fn update(&mut self, ctx: &mut ToolContext, input: ToolInput);
    fn cancel(&mut self, ctx: &mut ToolContext);
    fn commit(&mut self, ctx: &mut ToolContext) -> Result<(), ToolError>;
    fn overlays(&self, out: &mut OverlayCommands);
}
```

Exact implementation may differ, but lifecycle boundaries must remain explicit.

## 2. Initial tool set

The first-class tool set should include:

- Select;
- Place Loc;
- Move Loc;
- Rotate/Orient Loc;
- Duplicate/Paste;
- Delete;
- Terrain Height/Sculpt;
- Terrain Flatten;
- Underlay Paint;
- Overlay Paint;
- Terrain Shape/Rotation;
- Tile Flag/Settings editor;
- Rect/area selection;
- Eyedropper/sample;
- Measure/inspect diagnostics.

Future tools may compose these primitives rather than introducing alternate semantic mutation paths.

## 3. Select tool

### Click selection

A click resolves the nearest acceptable semantic pick under the active filter.

Selection filters can include:

- locs;
- terrain tile/surface;
- specific plane;
- hidden/ghosted planes excluded by default;
- diagnostic-only renderer geometry excluded from semantic picking.

Repeated click cycling over overlapping candidates is desirable when multiple semantic objects occupy the same screen position.

### Box/area selection

A drag rectangle may select loc anchors/footprints and/or tiles depending on mode.

Selection semantics must be stable in world space, not dependent on the number of pixels an object occupies after camera changes.

## 4. Hover

Hover is distinct from selection.

Hover can drive:

- outline/highlight;
- coordinate and object tooltip;
- tool candidate preview;
- status-bar information.

Hover does not dirty the document and should not clear selection merely because the pointer leaves the viewport.

## 5. Place Loc tool

Placement workflow:

1. choose an object definition from asset browser/eyedropper;
2. choose loc type compatible with the definition/target behavior;
3. choose orientation;
4. hover/pick anchor tile and plane;
5. show footprint and model ghost;
6. validate placement context;
7. click/confirm to create one semantic `PlaceLoc` transaction;
8. optionally remain armed for repeated placement.

Preview should show:

- exact anchor tile;
- footprint extent after orientation;
- model/loc-type result or explicit no-model state;
- collision/overlap warnings where available;
- target plane;
- bridge/plane warnings;
- export blocker warnings.

If the exact requested type produces no model under `MODEL-BUILD-001`, the editor must display the absence/problem rather than preview a fallback model.

## 6. Loc type and orientation control

The UI should present meaningful loc categories where known but retain the raw loc type value.

Orientation commits use canonical semantic orientation values.

Convenient controls may include:

- rotate clockwise/counterclockwise by one semantic step;
- direct 0/1/2/3 orientation selection;
- orientation compass/wall-edge visualization;
- type-specific decoration/wall footprint visualization.

Type-2 walls and dual-decoration types must preview both semantic arms where the scene spec requires them.

## 7. Move Loc tool

Persistent movement is tile-semantic, not arbitrary floating-point translation.

Workflow:

- select one or more locs;
- drag from anchor or use keyboard nudge;
- show snapped candidate anchor(s);
- preserve loc definition/type/orientation unless command explicitly changes them;
- validate region/plane ownership and footprint;
- commit one transaction.

A gizmo may move continuously on screen during the gesture, but commit snaps to representable tile anchors.

Cross-region movement transfers source-region ownership correctly and dirties both origin and destination regions.

## 8. Rotate/orient tool

Loc orientation changes are discrete semantic steps.

For multi-selection:

- rotate every compatible loc about its own anchor by default;
- optional group rotation about a pivot may be offered only if resulting anchors/orientations are valid semantic placements;
- reject ambiguous operations rather than invent free rotations.

The preview must use semantic model reconstruction, not a visual GPU-only transform that bypasses `MODEL-BUILD-*`.

## 9. Duplicate, copy, and paste

Clipboard payloads are editor/project semantic data, not serialized GPU meshes.

A clipboard selection should preserve:

- relative world/tile offsets;
- loc definition IDs;
- types;
- orientations;
- source planes as relative/explicit placement data;
- selected terrain semantic fields when terrain copy is supported.

Paste uses a visible anchor/pivot ghost and creates new stable semantic IDs on commit.

External clipboard serialization, if supported, must be versioned and cannot silently omit unsupported semantic fields.

## 10. Delete

Delete removes semantic instances/fields through one transaction.

Delete confirmation should be reserved for destructive high-cardinality or unusual actions rather than every loc deletion, because normal undo is available.

Read-only source/neighbor regions must block deletion with a clear reason.

## 11. Eyedropper/sample tool

Sampling a loc can arm placement with its:

- definition ID;
- loc type;
- orientation optionally;
- plane optionally.

Sampling terrain can load brush values such as:

- underlay;
- overlay;
- terrain shape;
- rotation;
- supported flags.

Sampling does not mutate the document.

## 12. Terrain height editing

Terrain height editing operates on exact semantic corner-height values.

Modes may include:

- raise/lower by integer delta;
- set exact height;
- flatten to sampled/entered height;
- smooth, only once its algorithm is explicitly defined as editor policy;
- ramp/gradient, only with deterministic integer output.

The UI may use continuous mouse motion to determine an integer delta, but the committed result is exact semantic data.

Brush overlays must show affected corners/tiles before commit where practical.

## 13. Terrain sculpt stroke

One pointer stroke is one transaction.

Requirements:

- deterministic brush mask for a given path/settings;
- exact before-state for every affected corner;
- no duplicate cumulative application merely because egui produced different frame rates;
- cross-zone and cross-region effects collected into one transaction;
- missing/unwritable neighbor context surfaced before or during the stroke;
- cancellation restores all touched corners exactly.

Frame-rate-independent input sampling is important for reproducible edits.

## 14. Underlay/overlay painting

Paint tools operate on semantic definition IDs/absence.

Brush modes:

- set;
- erase/clear where format permits;
- replace matching value;
- fill contiguous area later if deterministic adjacency rules are defined.

The viewport preview should distinguish the semantic value from the final reference/enhanced rendered appearance.

Because `TERRAIN-004` remains revision-sensitive, terrain-color presentation problems must not be hidden by modifying semantic underlay/overlay data.

## 15. Terrain shape and rotation

Tile topology edits must use verified shape IDs `0..12` and semantic rotation values.

The UI should provide both:

- raw shape/rotation fields for expert accuracy;
- visual shape picker/rotation control.

Preview geometry is generated through the same semantic terrain pipeline used for the scene, not a separate UI approximation.

Invalid combinations for a target profile are rejected with context.

## 16. Tile settings/flags

Only named/verified settings should receive friendly labels.

Unknown or revision-specific bits must remain inspectable as raw data and should not be silently cleared when another field is edited.

The editor should preserve unrecognized bits through round-trip save/export where the target format allows it.

## 17. Brush architecture

A generic brush can define:

- center tile/corner;
- radius/extent;
- shape (`point`, `square`, `circle-like mask`, custom later);
- falloff when meaningful;
- target planes;
- operation payload.

However, brush geometry is editor policy while output values remain semantic.

A brush implementation must provide a deterministic affected-key set before/while applying the transaction.

## 18. Snapping

Snapping categories:

- tile-anchor snapping for locs;
- orientation snapping to semantic steps;
- terrain corner snapping by definition;
- optional camera/gizmo visual snapping.

Snapping must be visible. A candidate that will commit to a different semantic location/orientation than the ghost shows is a correctness bug.

## 19. Gizmos

World-space gizmos are interaction devices, not alternate transforms.

Supported initial gizmos:

- loc move arrows/plane drag affordance;
- discrete rotation ring/handles;
- terrain brush radius;
- selection bounds/footprint.

Gizmo hit testing precedes normal viewport selection when active.

Gizmos render through the editor overlay/render path and use stable semantic selection as input.

## 20. Plane interaction

Tools operate on an explicit active/editing plane unless the operation intentionally spans multiple planes.

Ghosted other planes are non-interactive by default.

Bridge-linked tiles may display both source/storage relation and linked-below content, but clicks must resolve deterministically according to active tool/plane filters.

A user must be able to determine which semantic plane will be edited before committing.

## 21. Region boundaries

Region boundaries should be optionally visible.

When an operation crosses a region border:

- both regions must be loaded and writable for a normal edit;
- transaction summary shows affected regions;
- renderer zones may rebuild independently;
- export dirtiness is tracked per affected region.

An unloaded neighbor is not treated as an empty writable region.

## 22. Camera controls

Recommended default interaction pattern:

- primary click: selection/tool action;
- secondary or middle drag: orbit/pan according to configured scheme;
- wheel/pinch: zoom/dolly;
- modifier + drag: alternate pan/orbit;
- `F`: frame selection;
- top/front/side shortcuts can be added later.

Exact defaults should be configurable and exposed through the shortcut/action registry. The blueprint does not encode a platform-specific hardwired mouse scheme as semantic behavior.

## 23. Command/action registry

User actions should have stable IDs, for example:

```text
file.open_project
file.save_project
file.export
edit.undo
edit.redo
edit.copy
edit.paste
selection.delete
tool.select
tool.place_loc
tool.terrain_height
view.frame_selection
view.toggle_plane_0
render.reference_profile
render.toggle_priority_debug
```

Menus, toolbar buttons, shortcuts, command palette, and context menus invoke the same action IDs.

This avoids duplicate behavior paths.

## 24. Shortcut architecture

Shortcuts are mappings from input chords to action IDs.

Requirements:

- user-configurable;
- platform-aware display labels;
- conflict detection;
- context scopes (`Global`, `Viewport`, `TextEditing`, `Panel`, `Tool`);
- text fields consume normal typing before global editing shortcuts where appropriate;
- destructive actions cannot fire while an unrelated modal has focus.

Shortcut preferences are user settings, not project semantic state.

## 25. Context menus

Context menus are convenience surfaces over the action registry.

Examples for a loc:

- select definition in asset browser;
- duplicate;
- delete;
- rotate;
- frame camera;
- inspect source definition;
- copy ID/coordinates;
- show semantic/render diagnostics.

A context action must execute the same command as its toolbar/shortcut equivalent.

## 26. Inspector editing

Numeric/text inspector edits should use staged commit behavior:

- while typing, validate locally;
- on Enter/focus commit, execute one command;
- Escape cancels staged input;
- repeated spinner/drag edits may coalesce into one history transaction.

Invalid text must not partially corrupt semantic data.

Read-only fields must be visibly read-only rather than accepting input that is ignored.

## 27. Timeline and animation preview

Timeline state is non-destructive by default.

Controls should include:

- play/pause;
- deterministic tick/frame;
- step forward/back;
- loop;
- speed for editor playback;
- selected animation/sequence where relevant;
- reset to project/default preview state.

Playback uses editor time to advance an explicit deterministic semantic/render preview tick. Pausing freezes animated texture/model preview.

The current timeline position does not mark the project dirty.

## 28. Morph/varbit/varp preview

A preview panel must allow explicit values for morph-driving state.

Requirements:

- search/add varbit/varp preview entries;
- show which selected locs depend on them;
- display current resolved morph definition;
- indicate null/fallback branch;
- reset preview state;
- never write these preview values into the map unless a future project feature explicitly models scenario state.

This is necessary to inspect morphing locs without hardcoding one branch.

## 29. Render-profile controls

Viewport controls expose at minimum:

- Reference profile;
- Enhanced profile;
- deterministic tick;
- plane/roof filters;
- MSAA/sampling options appropriate to profile;
- diagnostics.

Switching profiles must not rebuild or mutate semantic map data.

## 30. Diagnostic modes

The editor should provide direct toggles for renderer diagnostics defined in Checkpoint 5, including:

- wireframe/topology;
- base normals;
- merged normals;
- face priority;
- alpha;
- authored face bias;
- material/texture IDs;
- semantic object IDs;
- storage plane;
- render level;
- zone boundaries;
- dirty generations;
- static/dynamic path;
- depth;
- culling/filter reason.

Where possible, clicking a diagnostic primitive should lead back to the owning semantic object/face metadata rather than only showing a color.

## 31. Problems integration

Tools validate candidate edits against current Problems/validation services.

Blocking problems stop commit and explain why.

Warnings may allow commit but must remain visible.

Examples:

- missing model for requested loc type: blocking placement visualization/export depending on workflow;
- unwritable destination region: blocking move;
- revision-gated terrain-color presentation: warning/problem, not a reason to rewrite tile data;
- unsupported editor-only transform: blocking export until resolved.

## 32. Tool cancellation

Every interactive tool gesture must define cancellation.

Cancellation triggers:

- Escape;
- switching tools;
- project close/reload;
- losing required semantic target;
- explicit cancel button where relevant.

Cancel returns the semantic document to the pre-gesture state and removes temporary overlays.

## 33. Modal versus modeless design

Prefer modeless editing for routine operations:

- docked inspectors;
- asset browser;
- tool options;
- timeline;
- diagnostics.

Use modal dialogs for actions requiring explicit interruption/decision:

- unsaved project close;
- source-cache mismatch/rebase;
- destructive project-level operation;
- export blockers requiring user choice;
- recovery selection.

## 34. Accessibility and feedback

Important state changes need more than color:

- selection: outline + optional fill;
- hover: separate outline style;
- invalid placement: icon/text plus distinct ghost pattern;
- warning: status/problem badge plus text;
- active plane: label/badge;
- dirty state: symbol/text;
- recording transaction/brush: cursor/overlay feedback.

Keyboard-only access to major menus/actions and inspector fields should be maintained by egui conventions.

## 35. Performance interaction rule

Editor interaction should remain responsive while background scene/render work is pending.

During a heavy semantic edit:

- commit semantic transaction promptly;
- display latest valid renderer generation until new zones are ready;
- show compiling/dirty indication where useful;
- do not block the UI waiting for every zone GPU upload unless correctness requires it for that action.

Selection/picking requests always carry generation identity so visible-old/new-state transitions cannot select the wrong semantic object.

## 36. Acceptance conditions

The tool/interaction layer is correct when:

1. every persistent gesture maps to one or more explicit semantic commands inside one user-intent transaction;
2. ghosts and gizmos never imply unsupported persistent transforms;
3. selection is semantic and survives GPU rebuilds;
4. terrain strokes are deterministic and frame-rate independent;
5. tool cancellation restores exact state;
6. preview/timeline/render controls never dirty the map;
7. region/plane ownership is visible before cross-boundary edits;
8. menus, shortcuts, context menus, and toolbars route through one action system;
9. unsupported edits are blocked or clearly marked non-exportable, never silently dropped;
10. renderer lag/recompilation cannot cause a command to target stale semantic identity.
