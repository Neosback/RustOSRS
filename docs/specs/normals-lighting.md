# Normals and Lighting Specifications

Primary target: `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Primary source pins:

- `ModelData.java` blob `2cc9406b2504fbd4fae0c0c952aa2d133809e928`
- `Scene.java` blob `f15260a63103952fe8f5ffbdb62f5c7c39d94565`
- `ObjectComposition.java` blob `079451cd9a6dcfd2666efd15b0524250eaafe4c4`

Correction provenance: `docs/implementation/REFERENCE-PROVENANCE-CORRECTION-2026-10-10.md`.

## SPEC: NORMALS-001

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

Base normals are derived from semantic integer model geometry before final model lighting. Flat and smooth behavior remains distinguishable through face-render metadata.

Invariants:

- mirrored winding is already correct before normal generation;
- normal calculation does not mutate shared decoded geometry across semantic instances;
- face render-type metadata survives until its owning operation.

Required tests include exact smooth/flat triangle and quad normals, mirrored winding, and source immutability.

---

## SPEC: NORMALS-002

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`  
**Primary evidence:** `ModelData.method5262(...)`

Cross-model normal reconciliation operates between distinct eligible `ModelData` instances.

Given models `A` and `B`, translation `(dx, dy, dz)`, and `hideMatchedFaces`:

1. ensure bounds/base normals;
2. compare eligible vertices after relative translation;
3. initialize scene-local merged normals from each model's base normal when required;
4. accumulate the matching model's normal components and magnitude;
5. track matched vertices for the merge generation;
6. if at least three vertices matched and `hideMatchedFaces == true`, fully matched faces receive render type `2` on both instances.

This is **not mesh welding**. Vertex arrays, faces, scene objects, and semantic identities remain separate.

---

## SPEC: NORMALS-003

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`  
**Primary evidence:** `Scene.method5585`, `Scene.method5586`, `Scene.method5587`, and `ModelData.method5262`

### Scene-finalization behavior

The pinned `Scene` implementation performs real cross-model normal reconciliation during scene ModelData finalization. Older research stating that `Scene` has no such wall/object normal logic is superseded.

For boundary objects:

- reconcile each eligible ModelData arm against the audited neighboring eligible ModelData search;
- reconcile the second arm when present;
- when both arms are ModelData, reconcile those two arms directly at zero translation without matched-face hiding;
- convert to final lit Models only after required reconciliation.

For game objects:

- reconcile eligible ModelData against neighboring boundary/game objects using footprint-aware relative translation;
- then convert to final Model.

For floor decorations:

- use the dedicated neighboring-floor-decoration reconciliation path;
- qualifying same-plane neighbor calls can request matched-face hiding;
- then convert to final Model.

The traversal can include the plane above with height-derived Y translation. The exact `hideMatchedFaces` argument is caller/path dependent. Do not generalize one boolean value to all neighbor relationships.

### Critical distinction

The source merges **normal state**, and may mark fully coincident faces render type `2` when requested. It does not perform boolean mesh union or weld the source objects into one topology.

Required tests retain dual-arm boundaries, wall/game neighbors, floor-decoration suppression, plane-above neighbor behavior, and final-lighting deltas.

---

## SPEC: NORMALS-004

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

During final ModelData-to-Model lighting, a merged normal produced by reconciliation takes precedence over the original base vertex normal. If no merged normal exists, use the base vertex normal.

The merge is therefore presentation-affecting semantic construction, not diagnostic-only metadata.

---

## SPEC: LIGHTING-001

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

### Object light parameters

For static object conversion:

```text
ambient  = objectDefinition.ambient + 64
contrast = objectDefinition.contrast + 768
lightX   = -50
lightY   = -10
lightZ   = -50
```

### Contrast decode scale

`objectDefinition.contrast` in the expression above is already decoder-scaled.

Pinned `ObjectComposition` opcode `39` performs:

```text
contrast = readByte() * 25
```

The later model-lighting path then adds `768` to that stored/scaled value. Implementations must not multiply by `25` a second time during lighting, and specifications/tests must not read the stored field as the raw cache byte.

`ambient` has its own decoded byte field and receives the later `+64` lighting offset as documented.

### Exact arithmetic

`ModelData.toModel` derives light-vector magnitude and per-normal contrast using reference integer arithmetic and shift/clamp behavior. Floating-point approximation is not accepted for semantic/golden parity tests.

### Required tests

- known lighting vector;
- opcode-39 decode value proving the `*25` scale exactly once;
- flat and smooth faces;
- merged-normal case;
- textured/untextured cases;
- valid ambient/contrast extremes.

### Related specs

`MODEL-BUILD-004`, `NORMALS-004`, `FACE-001`.
