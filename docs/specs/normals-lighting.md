# Normals and Lighting Specifications

Primary target: `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Primary source pins:

- `ModelData.java` blob `2cc9406b2504fbd4fae0c0c952aa2d133809e928`
- `Scene.java` blob `f15260a63103952fe8f5ffbdb62f5c7c39d94565`
- `ObjectComposition.java` blob `079451cd9a6dcfd2666efd15b0524250eaafe4c4`

## SPEC: NORMALS-001

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`  
**Primary evidence:** `ModelData.calculateVertexNormals()` and final model conversion paths  
**Executable evidence:** existing lighting fixture partially covers output; dedicated normal fixture REQUIRED

### Required behavior

Base normals are derived from semantic integer model geometry before final model lighting. Flat/smooth face behavior must remain distinguishable according to ModelData face-render metadata.

Normal generation belongs to model/scene semantics. The renderer may consume baked colors or semantic normals depending on the selected rendering profile, but it must not silently recompute a different topology-normal interpretation and call it parity.

### Invariants

- mirrored winding from `MODEL-BUILD-002` must already be correct before normal generation;
- normal calculation cannot mutate shared decoded geometry in a way that leaks between semantic instances;
- face render-type metadata must survive until the operation that owns it.

### Failure signature

Inverted diffuse response, faceted surfaces where smooth shading is expected, smooth surfaces where flat shading is expected.

### Required tests

Crafted triangle/quad ModelData cases with exact normal components/magnitudes and mirrored winding comparisons.

### Related specs

`MODEL-BUILD-002`, `NORMALS-002`, `LIGHTING-001`.

---

## SPEC: NORMALS-002

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`  
**Primary evidence:** `ModelData.method5262(...)`

### Applies to

Cross-model normal reconciliation between separate eligible `ModelData` instances.

### Required behavior

Given two ModelData values `A` and `B`, a translation `(dx, dy, dz)`, and `hideMatchedFaces`:

1. calculate/ensure bounds and base vertex normals for both;
2. compare eligible vertices after applying the supplied relative translation;
3. when translated positions coincide and the relevant source normal magnitude is non-zero, create scene-local merged-normal state as needed;
4. initialize each merged normal from that model's base normal;
5. add the other model's normal components and magnitude into the merged normal;
6. record the matched vertices for the current merge generation;
7. if at least three vertices matched and `hideMatchedFaces == true`, any face whose three vertices all matched receives face render type `2` on both ModelData instances.

### Critical distinction

This is **not mesh welding**. Vertex arrays, faces, object identities, and scene objects remain separate. Only lighting-normal state and optional matched-face render metadata are reconciled.

### Integer semantics

Vertex position comparison and normal accumulation use the audited integer coordinate/normal values. Do not epsilon-match floating-point GPU vertices.

### Invariants

- no positional match means no cross-model normal change;
- `hideMatchedFaces=false` may still merge normals;
- face hiding requires the audited matched-vertex threshold and all three face vertices matched;
- cached canonical ModelData must not be globally mutated by a scene-specific merge.

### Failure signature

Visible lighting seams between modular static objects, or conversely missing faces where matched-face suppression was incorrectly applied.

### Required tests

- positive coincident triangle merge;
- translated negative case with zero matches;
- positive merge with `hideMatchedFaces=false`;
- positive merge with `hideMatchedFaces=true`, proving render type `2` on fully matched faces;
- source cache immutability.

### Related specs

`NORMALS-003`, `MODEL-BUILD-004`.

---

## SPEC: NORMALS-003

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`  
**Primary evidence:** audited `Scene` ModelData finalization traversal

### Required scene-finalization behavior

Scene finalization must process qualifying renderables that are still ModelData before their final conversion to Model.

For boundary objects:

- reconcile the first eligible ModelData arm against neighboring eligible scene ModelData;
- reconcile the second arm when present;
- when both boundary arms are ModelData, reconcile the two arms directly with zero translation and `hideMatchedFaces=false`;
- only then convert the arms to final lit Models.

For game objects:

- reconcile eligible ModelData against the audited neighboring boundary/game-object search using footprint-aware relative translations;
- then convert to final Model.

For floor decorations:

- use the floor-decoration neighbor reconciliation path;
- that path may request matched-face hiding for qualifying adjacent floor-decoration merges;
- then convert to final Model.

The neighbor traversal may consider the source plane and the plane above according to the audited scene logic, with translation Y derived from relative average tile heights.

### Invariants

Final lighting must occur after required normal reconciliation, not before it.

### Failure signature

Correct isolated models but incorrect lighting at object seams, especially wall corners and repeated modular scenery.

### Required tests

Scene fixture containing:

- a type-2 dual-arm boundary object with qualifying ModelData;
- neighboring wall/game ModelData at matching vertices;
- adjacent floor decorations with a positive matched-face case;
- a plane-above neighbor case.

### Related specs

`LOC-PLACEMENT-004`, `NORMALS-002`, `LIGHTING-001`.

---

## SPEC: NORMALS-004

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

### Required final-normal selection

During final ModelData-to-Model lighting, a per-vertex merged normal produced by normal reconciliation takes precedence over the original base vertex normal. If no merged normal exists for a vertex, use the base vertex normal.

### Invariants

Cross-model merge state must affect final baked lighting. It is not diagnostic-only metadata.

### Failure signature

The merge routine appears to run successfully in tests, but final rendered colors remain identical to unmerged output.

### Required tests

A fixture where merged-normal components intentionally change the final per-vertex lit color and exact output differs from the no-merge control.

### Related specs

`NORMALS-002`, `LIGHTING-001`.

---

## SPEC: LIGHTING-001

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED` for the pinned object-lighting path  
**Executable evidence:** existing deob lighting/HSL helper fixture plus required expanded flat/smooth cases

### Required object-light parameters

For static object conversion:

```text
ambient  = objectDefinition.ambient + 64
contrast = objectDefinition.contrast + 768
lightX   = -50
lightY   = -10
lightZ   = -50
```

`ModelData.toModel` derives light-vector magnitude and per-normal contrast using the reference integer arithmetic/shift semantics.

Model-lighting HSL/lightness helper behavior must preserve the pinned integer clamps and sentinel rules exercised by the deob harness.

### Scope

This spec defines the reference OSRS semantic lighting conversion. A future enhanced editor presentation mode may intentionally use different real-time lighting, but it must remain separately selectable and must not replace the parity output or mutate canonical model semantics.

### Integer semantics

Exact integer arithmetic, clamp boundaries, and shift order are required for parity output. Floating-point approximations are not acceptable for exact semantic/golden tests.

### Failure signature

Systematic object brightness mismatch, wrong contrast on identical cache assets, subtle palette differences that vary by surface normal.

### Required tests

- existing known lighting vector fixture retained;
- flat and smooth faces;
- merged-normal case;
- textured/non-textured lighting cases where applicable;
- ambient/contrast extremes within valid decoded ranges.

### Related specs

`MODEL-BUILD-004`, `NORMALS-004`, `FACE-001`.