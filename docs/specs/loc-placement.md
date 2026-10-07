# Loc Placement Specifications

Primary target: `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Primary source pins:

- `FriendSystem.java` blob `b8cf51b6ee673181d8a115e28153a77f5978c390`
- `ObjectComposition.java` blob `079451cd9a6dcfd2666efd15b0524250eaafe4c4`
- `Scene.java` blob `f15260a63103952fe8f5ffbdb62f5c7c39d94565`
- `Tiles.java` blob `e4655da97d297f3d4fb53e9e7ae9816f1bdd5790`
- `class150.java` blob `e126091563f245bdb18dc5d3ff91a38434eff961`

## SPEC: LOC-PLACEMENT-001

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`  
**Target revision:** public deob `1ad572d7...`  
**Primary evidence:** `FriendSystem.addObjects(...)`, `Tiles` orientation tables  
**Executable evidence:** existing wall/decor/footprint cases in `reference-fixtures/deob_golden.txt`; full dispatch matrix expansion REQUIRED

### Applies to

Initial decoded map-location placement for loc types `0..22` and `>=12` behavior represented by the audited client.

### Required behavior

The type-to-scene-layer dispatch is:

| Loc type | Required scene construction |
|---|---|
| `22` | floor decoration |
| `10`, `11` | game object using the definition footprint; type `11` carries flag/orientation variant `256` |
| `0` | single boundary object arm using straight wall orientation flags |
| `1` | single boundary arm using diagonal wall orientation flags |
| `2` | dual-arm boundary object; first arm built with orientation `orientation + 4`, second with `(orientation + 1) & 3` |
| `3` | single boundary arm using diagonal wall flags |
| `4` | flush wall decoration |
| `5` | wall decoration displaced by full existing-wall displacement |
| `6` | diagonal wall decoration displaced by half existing-wall displacement |
| `7` | opposite-side wall decoration with no displacement |
| `8` | dual wall decoration, using half displacement |
| `9` | 1x1 game-object path |
| `>=12` | 1x1 game-object path |

A requested geometry type that returns no model remains absent; placement must not invent fallback geometry.

### Invariants

- type `2` has two renderable arms and both are semantically meaningful;
- type `9` is not a boundary object;
- type `>=12` is not generically treated as a wall;
- scene storage choice is determined before rendering and must not be repaired in `osrs-render`.

### Failure signature

Missing wall corners, incorrect diagonal walls, roof-like locs stored in the wrong layer, missing floor decorations, duplicated or displaced game objects.

### Required tests

`osrs-scene` table-driven unit test covering every type `0..22`, plus representative `>=12`; differential fixture for every wall/decor type and all four orientations.

### Related specs

`LOC-PLACEMENT-002`, `MODEL-BUILD-001`, `PLANES-001`.

---

## SPEC: LOC-PLACEMENT-002

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

### Required orientation/displacement tables

```text
straight wall flags = {1, 2, 4, 8}
diagonal wall flags = {16, 32, 64, 128}
cardinal offset X   = {1, 0, -1, 0}
cardinal offset Z   = {0, -1, 0, 1}
diagonal offset X   = {1, -1, -1, 1}
diagonal offset Z   = {-1, -1, 1, 1}
```

Type `5` uses the existing boundary object's definition displacement, defaulting to `16` when no wall is present.

Types `6` and `8` use half that displacement, defaulting to `8` when no wall is present.

Type `7` uses the opposite orientation and no displacement.

### Integer semantics

All displacement arithmetic is integer arithmetic. Half displacement uses the audited integer behavior; no floating-point interpolation belongs in placement.

### Failure signature

Torches, signs, banners, and other wall decorations embedded inside walls, floating away from walls, or attached to the wrong side.

### Required tests

All types `4..8` x orientations `0..3`, with and without an existing wall, and at least one non-default `decorDisplacement` definition.

### Related specs

`LOC-PLACEMENT-001`, `MODEL-BUILD-003`.

---

## SPEC: LOC-PLACEMENT-003

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

### Applies to

Footprint and placement-center computation for initial object placement.

### Required behavior

- definition footprint is `sizeX x sizeY`;
- orientations `1` and `3` transpose the placement footprint;
- world/local object center is computed from the rotated footprint, not from a tile corner;
- game-object storage uses the footprint center;
- floor decoration uses the tile center;
- height is derived from the applicable scene height samples for the audited placement path;
- footprint sampling and plane selection must follow the path-specific plane contract in `planes-bridges.md` rather than a universal bridge adjustment.

### Integer semantics

Tile-local scale is `128` units. Half-tile/half-footprint terms are integer operations. Preserve operation order and truncation.

### Invariants

Rotating a non-square footprint changes width/depth ownership but must not move its intended center to a corner.

### Failure signature

2x3/3x2 objects shifting by half or whole tiles, objects floating or sinking on slopes, incorrect selection bounds.

### Required tests

Square and non-square footprints at every orientation, including edge-of-scene clamp behavior and sloped-height samples.

### Related specs

`COORD-001`, `PLANES-001`, `MORPH-001`.

---

## SPEC: LOC-PLACEMENT-004

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

### Required behavior

Initial region construction and live/pending replacement are distinct semantic pipelines.

Initial static placement uses the audited entity-building path capable of returning `ModelData` for `nonFlatShading` objects. Those objects may survive until scene-level normal reconciliation and final lighting.

Pending/live replacement removes the existing category object and, for its static replacement path, uses an already-lit model path.

### Invariants

- do not implement one universal `build_loc_model()` that always returns a final lit model;
- do not route initial non-flat static objects through the pending-spawn path;
- old-slot removal/collision restoration and new placement remain ordered replacement semantics.

### Failure signature

Normal seams on initial static structures, incorrect runtime replacement behavior, shared-cache mutation, collision state left behind after replacement.

### Required tests

One qualifying initial `nonFlatShading` object proving it reaches normal reconciliation, and one pending replacement of the same definition proving the path remains already-lit and category-replacing.

### Related specs

`MODEL-BUILD-004`, `NORMALS-003`.

---

## SPEC: LOC-PLACEMENT-005

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED` at the ownership level; individual side-grid bit formulas may require later narrower specs

### Required behavior

Initial placement is not only visible-object insertion. Definition flags also drive derived semantic state such as collision, clipping/shadow state, model-clipping/occlusion state, and wall displacement metadata.

RustOSRS must preserve enough canonical definition + placement information to regenerate those side effects deterministically. It must not treat imported visible geometry as the entire map meaning.

### Does not require

Copying obfuscated Java arrays or field names into Rust one-for-one.

### Failure signature

A scene that looks approximately correct but exports incorrect collision/occlusion metadata or cannot rebuild after edits.

### Required tests

Definition-driven side-effect tests should be added as the corresponding cache fields are promoted into narrower specs.

### Related specs

`MODEL-BUILD-005`, `PLANES-001`.

---

## SPEC: LOC-PLACEMENT-006

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

### Required behavior

There is no generic global `+1` or `+2` height lift for floor/ground decorations in the audited placement/storage/upload path.

`Scene.newFloorDecoration(...)` stores the supplied height unchanged. The initial placement path supplies the computed terrain height. Any visual lift must originate from model geometry, definition transforms, or another specifically proven mechanism.

### Prohibited behavior

Do not add a renderer or scene offset merely to suppress Z-fighting for all floor decorations.

### Failure signature

Every floor decoration hovering above its intended surface, inconsistent picking/collision height, mismatch against reference placement.

### Required tests

A floor decoration on flat ground and on a slope must retain the exact supplied semantic Z without an implicit lift.

### Related specs

`COORD-001`, `PLANES-002`.