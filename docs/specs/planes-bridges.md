# Plane and Bridge Specifications

Primary deob target: `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Primary source pins:

- `DynamicObject.java` blob `3ff5ba2c1c4fb4cc914326cc70223720d5f2e77d`
- `Scene.java` blob `f15260a63103952fe8f5ffbdb62f5c7c39d94565`
- `Tile.java` blob `74c221f904f8b4748a7733eeade057934f1e0879`
- `Tiles.java` blob `e4655da97d297f3d4fb53e9e7ae9816f1bdd5790`
- `class470.java` blob `1cd9cad5cb4be865dcae94dc633bba821644dc84`, symbol `method9712(WorldView)`

Independent imported RuneLite corroboration:

- `SceneUploader.java` blob `83ac701f1b2ee7a039879941bcc710527dbc54c1`

## SPEC: PLANES-001

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

RustOSRS must not model plane behavior with one undifferentiated integer. The semantic scene distinguishes at least:

- encoded/source plane from map-location data;
- collision plane used for collision-map updates;
- storage plane after bridge relinking;
- height/render sampling plane where the owning path requires it;
- `Tile.plane`, the mutable scene plane after relinking;
- `Tile.originalPlane`, the tile's construction-plane provenance;
- `Tile.minPlane`, the client visibility-floor constraint;
- linked-below tile relation.

Renderer roof grouping remains a derived renderer concern, but `originalPlane` and `minPlane` are not merely RuneLite GPU conveniences. They exist in the pinned game-client scene model and participate in client traversal/visibility.

No generic `bridge_adjust_plane()` may stand in for these distinct contracts.

---

## SPEC: PLANES-002

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

### Initial decoded placement and collision plane

The decoded location retains its encoded plane for the scene-placement call.

Before placement, the audited loader checks bridge bit `2` in tile-settings plane `1`. When set, collision processing uses:

```text
collisionPlane = encodedPlane - 1
```

when valid.

This collision adjustment does not rewrite encoded/source placement identity and is not a universal height/render/storage adjustment.

---

## SPEC: PLANES-003

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`  
**Primary evidence:** `Scene.setLinkBelow(x, y)`

For a linked-below coordinate, the target operation structurally shifts scene storage:

- old plane `1` tile -> storage plane `0`;
- old plane `2` tile -> storage plane `1`;
- old plane `3` tile -> storage plane `2`;
- each moved tile's mutable `plane` decrements;
- qualifying rooted game objects decrement their stored plane;
- the old plane-0 tile becomes the new plane-0 tile's `linkedBelowTile`;
- old plane-3 storage clears.

If needed, a new plane-0 tile is created before linking the old plane-0 tile below it.

`originalPlane` is intentionally distinct from this mutable storage/plane shift and remains provenance for later visibility/roof logic.

Link-below is structural semantic state, not a renderer-only visibility flag.

---

## SPEC: PLANES-004

**Domain:** `OSRS_SEMANTIC` for tile visibility fields; imported RuneLite upload grouping is `RENDERER_POLICY` corroboration  
**Status:** `SOURCE_VERIFIED / IMPLEMENTATION_PARTIAL`

### `Tile.minPlane`

The pinned terrain builder assigns `minPlane` after terrain tile construction:

```text
if tileSettings[plane][x][y] has bit 8:
    minPlane = 0
else if plane > 0 and tileSettings[1][x][y] has bridge bit 2:
    minPlane = plane - 1
else:
    minPlane = plane
```

The pinned `Scene` later checks tile `minPlane` against the current scene plane while deciding whether a tile can participate in draw traversal. Therefore `minPlane` is target client scene state and belongs on the semantic side of the renderer boundary.

### `Tile.originalPlane`

`Tile` stores both mutable `plane` and immutable construction-time `originalPlane`. Bridge relinking can move/decrement the mutable plane while original-plane identity remains available for later scene/render logic.

RustOSRS must not collapse these values if a later visibility path requires the distinction.

### Imported RuneLite `maplevel`

RuneLite's `SceneUploader` derives a local `maplevel` for tile-settings/roof-range lookup. A bridge can change which original-plane settings and roof arrays are consulted.

This does **not** mean RuneLite moves the uploaded tile geometry into a lower storage-plane pass. The uploader still fetches geometry from `tiles[level][x][y]`; `maplevel` changes associated visibility/settings lookup.

Therefore:

- semantic source/collision/storage/min/original-plane identities remain authoritative;
- renderer roof grouping may derive additional local grouping values;
- renderer grouping must not mutate semantic plane state.

### Implementation gate

Before M10/M11 plane handling is considered closed, exact tests must prove:

1. `minPlane` construction for ordinary, bit-8, and bridge-bit cases;
2. `originalPlane` survives link-below storage movement;
3. renderer grouping may inspect these values without changing semantic scene hashes;
4. RuneLite-style `maplevel` affects settings/roof grouping only and does not rewrite tile storage identity.

### Related specs

`PLANES-001`, `PLANES-003`, `TERRAIN-004`, renderer visibility/roof policy.
