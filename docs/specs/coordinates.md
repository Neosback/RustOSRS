# Coordinate Specifications

Primary semantic/reference anchors:

- public deob scene/model sources at `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`
- imported RuneLite `Constants.java` blob `407831d491b39afd1230552f7475f86ce9e79be2`
- imported RuneLite `Perspective.java` blob `648a593690b777c1522c7afb16203f547e044b88`

## SPEC: COORD-001

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

### Required local-unit contract

One map tile is exactly:

```text
128 local units
```

Canonical semantic calculations for loc centers, terrain vertices, contouring, footprint placement, and tile-relative transformations use this unit system.

Useful exact values include:

```text
half tile       = 64
quarter tile    = 32
three-quarter   = 96
```

### Invariants

- world tile coordinates and local sub-tile coordinates are distinct types/concepts;
- integer semantic operations remain in local units until a renderer-specific conversion boundary;
- a GPU world scale may differ internally only if conversion is explicit and lossless for required semantic values.

### Failure signature

Systematic half-tile placement errors, contour/terrain mismatch, selection bounds offset from visible geometry.

### Required tests

Round-trip tile/local conversion, negative/positive local offsets where supported, footprint center cases, terrain shape exact local positions.

### Related specs

`LOC-PLACEMENT-003`, `TERRAIN-001`, `CONTOUR-001`.

---

## SPEC: COORD-002

**Domain:** `OSRS_SEMANTIC` for model/orientation angular conventions  
**Status:** `VERIFIED` for the audited rotation paths

### Required angular behavior

Loc orientation and ModelData quarter-turn operations must use the reference angular conventions rather than editor-camera Euler assumptions.

The audited model path uses the source client's integer sine/cosine tables/general JAU rotation where applicable, plus dedicated quarter-turn operations for ordinary loc orientations.

A type-4 diagonal decoration recenter uses a `256` JAU rotation before its ordinary orientation handling as specified by `MODEL-BUILD-003`.

### Scope boundary

Editor orbit-camera pitch/yaw defaults are not part of this spec. Camera UX belongs to `EDITOR_POLICY`; projection/depth conventions belong to `RENDERER_POLICY`.

### Invariants

Object orientation cannot depend on whichever math library convention the UI camera happens to use.

### Failure signature

Quarter-turn signs reversed, diagonal decorations rotated the wrong way, editor preview differs from exported/reference orientation.

### Required tests

Exact vertex output for all four loc orientations and the special 256-JAU diagonal path.

### Related specs

`MODEL-BUILD-003`, future camera renderer/editor ADRs.

---

## SPEC: COORD-003

**Domain:** `OSRS_SEMANTIC` boundary definition  
**Status:** `VERIFIED` separation

### Required coordinate-space separation

The reusable foundation must keep these responsibilities distinct:

1. **world/map coordinates**: region/global tile identity;
2. **scene/storage coordinates**: tile position inside a constructed scene;
3. **local coordinates**: 128-unit tile/sub-tile semantic positions;
4. **model-local coordinates**: asset-relative vertex space before scene placement;
5. **camera/view/clip coordinates**: renderer-owned presentation space.

Bridge/storage-plane changes do not redefine world tile identity. Renderer camera transforms do not mutate semantic local/model coordinates.

### RuneLite extended-scene note

The imported RuneLite API exposes a 104x104 normal scene and 184x184 extended scene representation. That extended size is useful reference/runtime infrastructure but is **not** promoted here as a universal OSRS cache invariant. RustOSRS scene capacity/border policy will be decided by scene/renderer architecture while preserving enough neighbor context for the semantic algorithms that require it.

### Failure signature

Object/model coordinates double-apply scene offsets, region-border content shifts, renderer refactors change saved map positions.

### Required tests

Typed conversion/round-trip tests and a region-border scene fixture proving renderer offsets do not alter semantic world/local positions.

### Related specs

`PLANES-001`, `TERRAIN-004`, future renderer architecture.