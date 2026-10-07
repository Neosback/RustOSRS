# Golden Scene Catalog

Status: **Checkpoint 7 verification plan**

Golden scenes are deterministic composed fixtures that can drive semantic, render-structural, and visual verification from one source definition.

A golden scene is **not just a screenshot**.

Each scene has:

- a stable scene ID;
- target profile/source identity;
- exact semantic inputs;
- explicit preview/runtime state;
- expected semantic scene summary;
- expected render-extraction summary where applicable;
- one or more fixed camera views for P3 image comparison;
- diagnostic views useful for failure triage.

## Golden-scene manifest

Recommended fields:

```yaml
scene_id: GS-001-wall-corner-normal-merge
schema_version: 1
target_profile: melxin-2026-01-28
owned_specs:
  - LOC-PLACEMENT-001
  - NORMALS-002
  - NORMALS-003
  - NORMALS-004
semantic_input: scenes/GS-001.scene.json
state:
  varbits: {}
  varps: {}
  animation_tick: 0
  render_tick: 0
views:
  - id: overview
    camera: {...}
    viewport: [1280, 720]
    profile: reference-v1
expected:
  semantic_summary: scenes/GS-001.semantic.json
  extraction_summary: scenes/GS-001.render.json
  image: images/GS-001-overview.png
```

## Determinism requirements

All scenes explicitly pin:

- camera;
- render tick;
- animation frame/tick;
- varbit/varp state;
- plane visibility;
- roof state;
- terrain jitter/random inputs;
- render profile version;
- viewport dimensions;
- source/cache identity when real cache data is involved.

No reference scene depends on wall clock, random startup state, editor panel layout, or pointer position.

## Initial canonical scene set

### GS-001: Wall corner normal merge

Purpose:

- prove type-2 dual-arm boundary construction;
- prove separate meshes can share accumulated lighting normals;
- prove final baked lighting consumes merged normals.

Owned specs:

- `LOC-PLACEMENT-001`
- `MODEL-BUILD-004`
- `NORMALS-002`
- `NORMALS-003`
- `NORMALS-004`
- `LIGHTING-001`

Scene ingredients:

- qualifying `nonFlatShading` wall/corner models;
- coincident vertices at one modular join;
- a nearby nonmatching control object.

Required semantic assertions:

- two arms remain separate object/model identities;
- expected matched vertex set;
- expected merged normal values;
- no unintended mesh welding;
- exact final lighting differs from unmerged control where expected.

Recommended diagnostic images:

- reference color;
- base normals;
- merged normals;
- face render type.

### GS-002: Wall decoration placement matrix

Purpose:

- exercise loc types `4..8` across orientations;
- verify default and custom wall displacement.

Owned specs:

- `LOC-PLACEMENT-001`
- `LOC-PLACEMENT-002`
- `MODEL-BUILD-003`

Scene ingredients:

- four orientation quadrants;
- wall with custom displacement;
- empty-wall control tiles.

Required semantic assertions:

- exact offsets/orientation values;
- dual-renderable type-8 behavior;
- type-7 opposite-side behavior.

### GS-003: Mirror, winding, and culling

Purpose:

- prove semantic mirroring/winding and renderer front-face conventions work together.

Owned specs/ADRs:

- `MODEL-BUILD-002`
- `NORMALS-001`
- `COORD-002`
- ADR-0004

Scene ingredients:

- asymmetric model whose front/back is obvious;
- mirrored and unmirrored instances;
- orientations below/above the audited mirror threshold.

Required assertions:

- exact semantic indices/winding;
- expected base normals;
- no inside-out culling in reference render.

### GS-004: Transform order stress

Purpose:

- make model-construction ordering differences visually and structurally obvious.

Owned spec:

- `MODEL-BUILD-003`

Scene ingredients:

- type-4 diagonal decoration with `orientation > 3`;
- recolor;
- retexture;
- nonuniform resize;
- translation offsets.

Required assertions:

- exact final integer vertices;
- exact material substitutions;
- fixed reference image.

### GS-005: Contoured multi-tile objects

Purpose:

- verify footprint rotation, height sampling, and contouring on slopes.

Owned specs:

- `LOC-PLACEMENT-003`
- `CONTOUR-001`
- `COORD-001`

Scene ingredients:

- asymmetric sloped terrain;
- non-square object in orientations `0..3`;
- clip `0` and nonzero clip controls.

Required assertions:

- exact footprint/center;
- exact contoured vertex output;
- no shared-source mutation.

### GS-006: Bridge four-plane stack

Purpose:

- prove source/collision/storage/render-level separation and structural link-below behavior.

Owned specs:

- `PLANES-001`
- `PLANES-002`
- `PLANES-003`
- `PLANES-004`

Scene ingredients:

- synthetic tile column across all four planes;
- tagged game objects;
- bridge bit state;
- distinguishable surfaces on every level.

Required assertions:

- exact collision-plane result;
- exact storage relinking;
- linked-below identity;
- tagged object plane change;
- renderer roof/visibility toggles do not mutate semantic state.

Recommended diagnostics:

- storage plane;
- render level;
- linked-below relation;
- pick/object IDs.

### GS-007: Priority, alpha, and authored bias

Purpose:

- expose face ordering failures that simple depth rendering would hide.

Owned specs/ADRs:

- `FACE-002`
- `FACE-003`
- `FACE-004`
- ADR-0004
- ADR-0005

Scene ingredients:

- crafted model/face set with priorities `0..11`;
- priority-10/11 threshold crossings;
- opaque and alpha faces;
- coplanar faces with distinct authored bias.

Required assertions:

- exact CPU/reference face order before draw;
- correct alpha classification;
- exact bias handoff;
- stable reference image from several camera angles.

Recommended diagnostics:

- face priority;
- alpha;
- bias;
- depth.

### GS-008: Texture triangle and animation

Purpose:

- verify object texture mapping and deterministic texture animation.

Owned spec:

- `TEXTURE-001`

Scene ingredients:

- explicit texture-face model;
- no-texture-face control;
- animated material with nonzero direction/speed.

Required assertions:

- expected UVs;
- correct semantic texture ID -> material mapping;
- render tick `0`, `1`, and a later fixed tick produce expected deterministic UV offsets;
- pause freezes animation.

### GS-009: Morphing dynamic loc

Purpose:

- verify morph selection, null state, footprint change, and dynamic model ownership.

Owned specs:

- `MORPH-001`
- `ANIMATION-001`
- `LOC-PLACEMENT-003`

Scene states:

- varbit selects first transform;
- selector falls to fallback transform;
- null transform;
- transform changes footprint dimensions.

Required assertions:

- exact selected definition;
- exact footprint/center;
- null state has no model;
- source cached model remains unchanged after animation pose.

### GS-010: Terrain shape gallery

Purpose:

- visual/composition companion to exact `13 x 4` terrain topology fixtures.

Owned specs:

- `TERRAIN-001`
- `TERRAIN-002`

Scene ingredients:

- every shape and rotation laid out in a labeled grid;
- distinct corner heights/colors;
- textured and untextured examples;
- sentinel hidden tile.

Exact topology remains the primary proof. The image is for composed renderer regression.

### GS-011: Region-border semantic continuity

Purpose:

- exercise multi-region coordinate handling and neighbor context.

Owned specs/editor areas:

- `COORD-003`
- `TERRAIN-001`
- multi-region editor architecture.

Scene ingredients:

- content crossing a 64x64 region boundary;
- multi-tile loc near the boundary;
- terrain shapes/heights spanning both sides.

Required assertions:

- stable world/local identity;
- no double scene-origin application;
- zone/region rebuild leaves semantic positions unchanged.

The complete terrain-color blending portion of this scene remains blocked by `TERRAIN-004` until its oracle is resolved.

### GS-012: Terrain color border

Status: **BLOCKED**

Purpose:

- eventually prove full slope lighting and 11x11 neighborhood color continuity across region/scene borders.

Owned spec:

- `TERRAIN-004`

Do not establish expected canonical values until the builder provenance gate closes.

### GS-013: Initial vs pending replacement

Purpose:

- expose the distinct initial static and live replacement construction pipelines.

Owned specs:

- `LOC-PLACEMENT-004`
- `MODEL-BUILD-004`
- `NORMALS-003`

Required assertions:

- initial qualifying object reaches pre-lighting ModelData reconciliation;
- pending static replacement uses its already-lit replacement path;
- category removal/replacement state remains correct.

### GS-014: Renderer generation and dirty-zone rebuild

Purpose:

- verify renderer compilation changes no semantic truth.

Owned ADRs:

- ADR-0005
- renderer generation architecture.

Scenario:

1. compile scene generation N;
2. edit one zone at generation N+1;
3. intentionally finish stale N compile after N+1;
4. require stale artifact rejection;
5. rebuild device/resources;
6. compare semantic and render-extraction summaries.

Expected:

- only affected zones change;
- semantic hash unchanged by renderer lifecycle;
- stale artifacts never become current.

### GS-015: Editor cross-region transaction

Purpose:

- composed editor verification rather than OSRS visual parity.

Scenario:

- move/duplicate/delete locs across a region boundary;
- terrain stroke crosses boundary;
- commit as one transaction;
- undo to exact baseline;
- redo to exact final state;
- serialize/reload between operations.

Owned architecture:

- ADR-0008
- ADR-0009
- editor transaction blueprint.

## Image comparison policy

Reference images should use a canonical GPU runner where possible.

Baseline comparison outputs should report at least:

- total differing pixels;
- maximum per-channel difference;
- mean/percentile error;
- optional SSIM-like structural metric for diagnostics.

The actual merge threshold must be calibrated against known same-code backend/driver variance. Do not invent a permissive threshold in the blueprint.

Exact semantic/extraction outputs remain the authority for contracts beneath P3.

## Failure artifact retention

When a golden scene fails in CI, retain/upload diagnostic artifacts where practical:

- actual image;
- expected image;
- amplified diff image;
- semantic summary;
- render-extraction summary;
- frame/profile/camera manifest;
- adapter/backend information;
- relevant diagnostic render layers.

This turns a visual regression from a screenshot mystery into a layer-owned failure.
