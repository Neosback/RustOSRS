# Plane and Bridge Specifications

Primary deob target: `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Primary source pins:

- `DynamicObject.java` blob `3ff5ba2c1c4fb4cc914326cc70223720d5f2e77d`
- `Scene.java` blob `f15260a63103952fe8f5ffbdb62f5c7c39d94565`
- `Tiles.java` blob `e4655da97d297f3d4fb53e9e7ae9816f1bdd5790`
- `class470.java` blob `1cd9cad5cb4be865dcae94dc633bba821644dc84`, especially `method9712(WorldView)`

Independent RuneLite renderer evidence:

- imported `Tile.java` and `Scene.java`
- imported `SceneUploader.java` blob `83ac701f1b2ee7a039879941bcc710527dbc54c1`

Correction provenance: `docs/implementation/REFERENCE-PROVENANCE-CORRECTION-2026-10-10.md`.

## SPEC: PLANES-001

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

RustOSRS must preserve distinct plane domains for:

- encoded/source plane;
- collision plane;
- scene/storage plane after bridge relinking;
- render/height level;
- tile minimum plane used by scene visibility;
- linked-below relation.

Renderer-specific roof/VIS_BELOW grouping remains a derived renderer concern and is not interchangeable with any semantic plane above.

No generic `bridge_adjust_plane()` may own all of these operations.

---

## SPEC: PLANES-002

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

### Initial placement and collision plane

Decoded locations retain their encoded/source plane for initial scene placement.

When tile-settings plane `1` has bridge bit `2` at the target tile, collision uses:

```text
collisionPlane = encodedPlane - 1
```

when the result remains valid.

This collision projection does not rewrite the encoded/source plane.

### Pending/live height sampling

The pending-spawn/live replacement path has a separate bridge-aware height-sampling rule. For a source plane below `3`, bridge bit `2` can select `sourcePlane + 1` as the heightmap/model-construction plane while scene storage/collision ownership remains separately defined.

Initial decoded placement and pending/live replacement must remain distinct pipelines.

---

## SPEC: PLANES-003

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`  
**Primary evidence:** `Scene.setLinkBelow(x, y)`

For a selected tile coordinate, link-below processing structurally shifts scene tile references:

- prior plane `1` tile becomes storage plane `0`;
- prior plane `2` tile becomes storage plane `1`;
- prior plane `3` tile becomes storage plane `2`;
- moved tile plane values decrement;
- qualifying rooted game objects have their stored plane decremented;
- the old plane-0 tile becomes the new plane-0 tile's linked-below tile;
- the previous plane-3 slot is cleared.

If necessary, a new plane-0 tile is created before linking the old plane-0 tile beneath it.

This is structural semantic state, not merely a renderer visibility bit.

---

## SPEC: PLANES-004

**Domain:** mixed `OSRS_SEMANTIC` scene behavior and `RENDERER_POLICY`  
**Status:** `VERIFIED` separation

### Client tile minimum-plane rule

The pinned client terrain builder `class470.method9712(WorldView)` explicitly assigns each constructed tile's minimum plane using `Scene.setTileMinPlane(...)`.

For a tile on plane `p`:

1. if the current plane's tile settings contain flag `8`, minimum plane is `0`;
2. else if `p > 0` and tile-settings plane `1` contains bridge bit `2`, minimum plane is `p - 1`;
3. otherwise minimum plane is `p`.

This value is source-client scene/runtime behavior. It is not solely a RuneLite GPU upload convention and must remain available in the semantic/derived scene state used for faithful visibility decisions.

### RuneLite `maplevel` is related but different

The imported RuneLite `SceneUploader` derives a local renderer `maplevel` as follows:

```text
maplevel = level
if bridge bit 2 is set:
    maplevel += 1
```

That derived value is used for `VIS_BELOW` and roof-id lookup. The uploader still fetches geometry from:

```text
tiles[level][x][z]
```

not `tiles[maplevel]`.

Therefore documentation must not describe the RuneLite local variable itself as physically moving bridge geometry to another stored tile or lower-plane pass. It is a visibility/roof lookup level layered on top of the scene's existing bridge/storage structure.

### RuneLite roof infrastructure

RuneLite roof IDs, roof ranges, and roof-removal modes are derived renderer/product infrastructure. They must not become cache or canonical loc-plane semantics.

### Invariants

- collision projection from `PLANES-002` is separate;
- storage relinking from `PLANES-003` is separate;
- tile minimum-plane state is separate;
- RuneLite roof/VIS_BELOW `maplevel` is separate;
- changing renderer roof-removal strategy must not mutate source/collision/storage plane data.

### Required tests

- exact tile minimum-plane cases for flags `8`, bridge bit `2`, and ordinary tiles across valid planes;
- renderer test proving bridge `maplevel` can change visibility/roof lookup without changing the uploaded tile storage index;
- semantic scene tests remain invariant under alternate roof grouping policy.

### Related specs

`PLANES-001`, `PLANES-002`, `PLANES-003`, `TERRAIN-004`.
