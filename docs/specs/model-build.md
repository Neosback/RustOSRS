# Model Construction Specifications

Primary target: `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Primary source pins:

- `ObjectComposition.java` blob `079451cd9a6dcfd2666efd15b0524250eaafe4c4`
- `ModelData.java` blob `2cc9406b2504fbd4fae0c0c952aa2d133809e928`
- `Model.java` blob `c2aa55c0e8fea89fae0da33d782119f8c109cacf`

## SPEC: MODEL-BUILD-001

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`  
**Primary evidence:** `ObjectComposition.getModelData(type, orientation)`  
**Executable evidence:** REQUIRED expansion of deob harness

### Required model selection

When the definition has no typed `models[]` table:

- only requested loc type `10` is accepted by the audited generic model path;
- all listed `modelIds` are loaded/combined for that type;
- a missing `modelIds` array yields no model.

When a typed `models[]` table exists:

- scan for the requested type;
- use the corresponding `modelIds[i]`;
- no exact type match returns no model.

There is no semantic fallback to the first model, nearest type, or type `10` geometry.

### Invariants

`None`/null model is a valid result. Consumers must preserve absence rather than substitute geometry.

### Failure signature

Objects rendered with the wrong variant, invisible reference objects becoming visible, fences/gates using generic meshes, incorrect wall geometry.

### Required tests

- typed hit;
- typed miss;
- untyped type `10` multi-model combine;
- untyped non-10 rejection;
- empty/no model id case.

### Related specs

`LOC-PLACEMENT-001`, `MODEL-BUILD-002`.

---

## SPEC: MODEL-BUILD-002

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

### Applies to

Raw ModelData mirror selection and mirrored geometry.

### Required behavior

For the typed model path:

```text
mirror = isRotated XOR (orientation > 3)
```

Mirrored raw-model caching is distinct from the unmirrored cache entry. The audited implementation separates the cache key before loading/mirroring.

Mirroring changes geometry and winding. A mirrored model must not retain the original face winding if the reference mirror operation swaps the applicable triangle indices.

The untyped/type-10 combination path carries its audited special-case mirror behavior and must be differential-tested rather than normalized into the typed formula without proof.

### Invariants

- mirror is a model-construction semantic, not a negative scale applied in the renderer;
- culling/front-face policy must consume already-correct semantic winding;
- cached raw mirrored data must not be repeatedly mirrored per instance.

### Failure signature

Inside-out walls, reversed culling, dark/inverted lighting, mirrored decorations facing the wrong direction.

### Required tests

Mirror geometry + winding fixture, typed `isRotated` true/false x `orientation <=3/>3`, and the audited untyped/type-10 special case.

### Related specs

`MODEL-BUILD-003`, `NORMALS-001`.

---

## SPEC: MODEL-BUILD-003

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

### Required transform order

After raw ModelData resolution and creation of an instance-working model, transformations occur in this order:

1. when requested type is `4` and `orientation > 3`, apply the special diagonal decoration recenter operation: rotate by `256` JAU and translate `(45, 0, -45)`;
2. reduce ordinary orientation with `orientation & 3`;
3. apply orientation rotation for `1`, `2`, or `3` quarter-turns using the audited ModelData integer operations;
4. apply every recolor pair;
5. apply every retexture pair;
6. resize when any model scale axis differs from `128`;
7. translate by definition offsets when nonzero.

### Integer semantics

All model-space transform operations are semantic integer operations. Preserve the audited operation order and truncation. Do not compose them into a floating-point matrix and assume equivalent rounding.

### Invariants

Recolor/retexture do not change geometry. Resize precedes final definition translation. Diagonal decoration recenter occurs before the ordinary orientation transform.

### Failure signature

Offset/resized objects displaced from their tile, diagonal wall decorations shifted half a tile, recolor/retexture mismatches, orientation-specific geometry drift.

### Required tests

- all four ordinary orientations;
- type-4 orientation `>3` recenter;
- combined recolor + retexture + resize + translate fixture where changing order changes output;
- exact integer vertex equality.

### Related specs

`LOC-PLACEMENT-002`, `COORD-002`.

---

## SPEC: MODEL-BUILD-004

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

### Required static entity behavior

The initial scene entity-building path must preserve the `nonFlatShading` distinction:

- `nonFlatShading == false`: transformed ModelData is converted to a lit `Model` using the loc lighting parameters and may be cached as a lit entity;
- `nonFlatShading == true`: ambient/contrast are retained on ModelData, vertex normals are calculated, ModelData is cached in that semantic form, and a scene-local copy is returned so it may participate in normal reconciliation before final lighting.

Contouring is applied to an appropriate working copy after the branch according to the audited path.

### Invariants

A cache keyed only to one final `GpuMesh` representation is insufficient. RustOSRS must be able to preserve pre-lighting ModelData state for qualifying initial static entities.

### Failure signature

Hard seams where normal reconciliation should occur, contour edits corrupting shared assets, inconsistent initial-vs-runtime appearance.

### Required tests

Same object definition with `nonFlatShading` false vs true, verifying returned semantic representation and shared-cache immutability.

### Related specs

`LOC-PLACEMENT-004`, `NORMALS-003`, `LIGHTING-001`.

---

## SPEC: MODEL-BUILD-005

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED` ownership rule derived directly from audited copy/cache paths

### Required ownership behavior

Decoded/raw model assets are immutable shared inputs once admitted to the reusable cache layer.

Any operation that can alter instance output, including:

- mirroring during distinct raw-variant construction;
- orientation transformation;
- recolor/retexture;
- resize/translation;
- contouring;
- animation pose;
- scene normal accumulation;
- matched-face suppression;

must operate on a cache-safe variant or an instance/scene working copy according to the owning semantic path.

### Does not require

Deep-copying every array for every model. Structural sharing is allowed when the source path proves the shared field will not be mutated.

### Invariants

A scene edit or preview must never mutate canonical decoded source geometry used by another object instance.

### Failure signature

Editing or rendering one object changes another object using the same model id, orientation-dependent corruption after cache hits, animation/contour state leaking across instances.

### Required tests

Two instances sharing one decoded model where one is recolored/transformed/contoured/posed. The untouched instance and shared source must remain byte-for-byte semantically unchanged.

### Related specs

`NORMALS-002`, `CONTOUR-001`, `ANIMATION-001`.