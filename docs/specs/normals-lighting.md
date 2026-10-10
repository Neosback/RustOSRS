# Normals and Lighting Specifications

Primary target: `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Primary source pins:

- `ModelData.java` blob `2cc9406b2504fbd4fae0c0c952aa2d133809e928`
- `Scene.java` blob `f15260a63103952fe8f5ffbdb62f5c7c39d94565`
- `ObjectComposition.java` blob `079451cd9a6dcfd2666efd15b0524250eaafe4c4`

## SPEC: NORMALS-001

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

Base normals are derived from semantic integer model geometry before final object lighting. Flat and smooth faces remain distinct according to ModelData render metadata.

Mirrored winding must already be correct before normal generation. Normal calculation and later reconciliation operate on cache-safe/scene-local state, never by mutating canonical shared model input.

---

## SPEC: NORMALS-002

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`  
**Primary evidence:** `ModelData.method5262(...)`

Cross-model reconciliation is real target behavior.

Given two ModelData instances, relative translation, and `hideMatchedFaces`:

1. ensure bounds/base normals;
2. compare eligible translated integer vertex positions;
3. accumulate the other model's normal into scene-local merged-normal state at coincident vertices;
4. track matched vertices for the current merge generation;
5. when at least three vertices match and `hideMatchedFaces == true`, mark any fully matched face render type `2` on both models.

This is **normal reconciliation, not mesh welding**. Vertices/faces/object identity remain separate. Render type `2` is a matched-face suppression marker consumed by later model conversion/rendering.

No positional match means no merge. `hideMatchedFaces=false` still permits normal accumulation.

---

## SPEC: NORMALS-003

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`  
**Primary evidence:** pinned `Scene.method5585`, `method5586`, `method5587`

### Required scene finalization

The target `Scene` walks qualifying ModelData before final `toModel(...)` conversion and performs cross-object reconciliation.

For boundary objects:

- each ModelData arm is reconciled against qualifying neighboring boundary/game-object ModelData;
- when both arms are ModelData, the two arms are additionally reconciled directly at zero translation with `hideMatchedFaces=false`;
- final lighting occurs only after reconciliation.

For game objects:

- footprint-aware neighboring boundary/game-object reconciliation uses exact relative translations and average-height deltas;
- final conversion follows reconciliation.

For floor decorations:

- the dedicated adjacent-floor-decoration path uses `hideMatchedFaces=true` for its qualifying same-plane matches.

For the general neighboring boundary/game traversal, the method begins the same-plane pass with matched-face hiding enabled and then disables hiding when advancing to the plane-above pass. This distinction is part of the source behavior and must not be flattened into one global merge flag.

### Correction to historical root notes

Any root research note saying the engine "never merges walls" or that `Scene` contains no normal-reconciliation logic is superseded.

The precise rule is:

- the engine does **not** weld wall geometry into a single mesh;
- it **does** reconcile normals between separate wall/object ModelData instances;
- some fully coincident faces are suppressed by render type `2` when the owning merge call enables matched-face hiding.

---

## SPEC: NORMALS-004

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

During final ModelData-to-Model lighting, a merged vertex normal takes precedence over the base normal. If no merged normal exists, use the base normal.

Cross-model reconciliation therefore changes final baked lighting and is not diagnostic-only metadata.

---

## SPEC: LIGHTING-001

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

### Decoded object-light inputs

Object-definition opcode `29` stores signed ambient directly.

Object-definition opcode `39` stores:

```text
contrast = signed_byte * 25
```

The Rust decoder already performs this multiplication. Consequently, the `objectDefinition.contrast` consumed below is **already multiplied by 25** and must not be multiplied again during lighting.

### Required static-object light parameters

```text
ambient  = decodedObjectAmbient + 64
contrast = decodedObjectContrast + 768
lightX   = -50
lightY   = -10
lightZ   = -50
```

`ModelData.toModel` then derives light-vector magnitude and per-normal contrast with the pinned integer arithmetic and shifts.

### Alpha/render-type sentinel interaction

Before face lighting, the pinned conversion interprets raw face alpha sentinels:

```text
alpha == -2 -> effective render type 3
alpha == -1 -> effective render type 2
```

For untextured faces:

- effective type `3` produces constant gray color `128` with flat-color marker `c = -1`;
- effective type `2` produces suppressed-face marker `c = -2`.

For textured faces, unsupported effective render types likewise produce `c = -2` suppression.

RustOSRS already implements this in `osrs-core::lighting`; downstream renderer extraction must preserve and later honor the `c == -2` suppression marker rather than drawing the triangle merely because its topology remains present.

### Exactness

Reference semantic lighting preserves exact integer operation order, clamps, signed sentinel interpretation, and merged-normal precedence. Enhanced editor lighting, if added, remains a separately selected renderer profile and must not replace this semantic result.

### Required verification

- flat/smooth base cases;
- merged-normal lighting delta;
- textured/untextured branches;
- alpha `-1/-2` sentinel cases;
- decoded contrast multiplication-by-25 boundary cases;
- ambient/contrast extremes in valid target range;
- proof that suppressed `c == -2` faces cannot be re-admitted by the Reference renderer path.

### Related specs

`MODEL-BUILD-004`, `NORMALS-004`, `FACE-001`, `FACE-003`.
