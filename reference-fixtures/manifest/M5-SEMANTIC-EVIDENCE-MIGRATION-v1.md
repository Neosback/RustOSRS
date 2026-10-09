# M5 Semantic Evidence Migration v1

Status: source-pinned manual normalization  
Milestone: M5 reference fixture infrastructure

## Purpose

This migration records the source derivation for the M5 evidence-only fixture families that must exist before their production owners are implemented:

- base normal generation;
- cross-model normal-merge controls;
- structural `Scene.setLinkBelow` plane relinking.

These artifacts are deliberately `evidence_only`. M5 validates their source pins, normalized schemas, expected hashes, and deterministic inventory inclusion. M5 does not implement M6 plane/scene behavior or M7 normal generation/merging merely to make these fixtures executable early.

## Source revision

Repository: `melxin/runelite`  
Commit: `1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Pinned files:

- `runescape-client/src/main/java/ModelData.java`
  - blob: `2cc9406b2504fbd4fae0c0c952aa2d133809e928`
  - symbols: `calculateVertexNormals`, `method5262`
- `runescape-client/src/main/java/Scene.java`
  - blob: `f15260a63103952fe8f5ffbdb62f5c7c39d94565`
  - symbol: `setLinkBelow`

## Base normal fixtures

The synthetic triangle is:

- A `(0, 0, 0)`
- B `(128, 0, 0)`
- C `(0, 0, 128)`
- face `(0, 1, 2)`

`calculateVertexNormals` computes the integer cross product, repeatedly shifts it until every component is within the reference bound, then normalizes to length 256 with integer division.

For this winding the exact normalized vector is:

`(0, -256, 0)`

### `normals.base.smooth_triangle`

With the default smooth render type, the vector is accumulated into all three vertex normals. Each vertex therefore has:

`x=0, y=-256, z=0, magnitude=1`

No face-normal array is materialized.

### `normals.base.flat_triangle`

With face render type `1`, the three vertex normal accumulators remain zero and the face-normal slot contains:

`(0, -256, 0)`

This makes the smooth/flat branch observable without requiring lighting output.

## Normal merge fixtures

The merge controls use two identical smooth triangles and source-pin `ModelData.method5262`.

Each triangle begins with the same three base vertex normals:

`(0, -256, 0, magnitude=1)`

### `normals.merge.coincident_triangle.hide_false`

At translation `(0, 0, 0)`, all three vertices coincide. The reference merge copies each base normal into the merged-normal slot and adds the opposite model's base normal. Every matched merged normal on both models is therefore:

`(0, -512, 0, magnitude=2)`

With matched-face hiding disabled, the absent face-render-type arrays remain absent.

### `normals.merge.coincident_triangle.hide_true`

The same three coincident vertices satisfy the reference minimum match count. With hiding enabled, the fully matched face in each model receives render type `2` in addition to the merged normals above.

### `normals.merge.translated_negative`

Translation `(1, 0, 0)` prevents every exact integer vertex match in the crafted triangles. No merged-normal arrays are created and no face-render-type arrays are created.

This is the negative control that prevents a merge fixture from proving only the positive path.

## Plane/link-below fixture

### `planes.link_below.four_plane_column`

The input models a four-plane tile column at scene coordinate `(10, 20)` with stable labels `p0` through `p3`.

`Scene.setLinkBelow` moves original planes `1`, `2`, and `3` into storage slots `0`, `1`, and `2`, respectively. Each moved tile's stored `plane` field is decremented once. Storage slot `3` is cleared and the original plane-0 tile becomes `linkedBelowTile` from the new slot-0 tile.

The fixture also makes the game-object predicate observable:

- an object whose decoded tag type is `2` and whose `startX/startY` equal the relinked coordinate has its stored plane decremented;
- a type-2 object anchored on another tile is not decremented;
- an object of another tag type anchored on the target tile is not decremented.

The fixture records exact post-relink storage slots, tile planes, object planes, the original plane-0 linked-below label, and the cleared top slot.

## Acceptance boundary

These fixtures are accepted as exact source-pinned evidence, not as proof that RustOSRS production executors already exist.

Promotion rules remain:

- `NORMALS-001` and `NORMALS-002` advance only when M7 production normal code executes the same normalized contracts exactly;
- `PLANES-003` advances only when M6 scene construction executes the four-plane relinking contract exactly;
- `evidence_only` remains forbidden for fixture kinds that already have an M5 production semantic executor.
