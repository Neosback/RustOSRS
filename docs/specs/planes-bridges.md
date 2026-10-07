# Plane and Bridge Specifications

Primary deob target: `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

Primary source pins:

- `DynamicObject.java` blob `3ff5ba2c1c4fb4cc914326cc70223720d5f2e77d`
- `Scene.java` blob `f15260a63103952fe8f5ffbdb62f5c7c39d94565`
- `Tiles.java` blob `e4655da97d297f3d4fb53e9e7ae9816f1bdd5790`

Independent October RuneLite API corroboration:

- `Tile.java` and `Scene.java` in the imported `runelite-master/` tree
- `SceneUploader.java` blob `83ac701f1b2ee7a039879941bcc710527dbc54c1`

## SPEC: PLANES-001

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

### Required concept separation

RustOSRS must not represent all plane behavior with a single undifferentiated integer.

The semantic scene needs distinguishable concepts for at least:

- encoded/source plane from map-location data;
- collision plane used for collision-map updates;
- scene/storage plane after bridge relinking;
- render/height level used to obtain terrain heights where applicable;
- linked-below tile relation.

A renderer-specific roof/VIS_BELOW grouping level is a separate derived concern and is not part of this semantic plane identity.

### Invariants

No helper named generically `bridge_adjust_plane()` may be used for all of these responsibilities. Every call site must identify which plane domain it requests.

### Failure signature

Objects appear on the correct visual level but collide on the wrong level, bridge tiles disappear, height sampling uses an unrelated plane, or editing/export changes bridge structure.

### Required tests

Strongly typed/newtype plane-domain tests or equivalent API-level compile-time/runtime guardrails plus a bridge fixture exercising all listed plane concepts.

### Related specs

`PLANES-002`, `PLANES-003`, `LOC-PLACEMENT-003`.

---

## SPEC: PLANES-002

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`

### Initial decoded placement and collision plane

The decoded location retains its encoded plane for the initial scene-placement call.

Before placement, the audited loader checks bridge bit `2` in tile settings plane `1` at the target tile. When that bit is set, collision processing uses:

```text
collisionPlane = encodedPlane - 1
```

when the result is valid.

This collision adjustment does not rewrite the encoded/source placement plane passed into the scene builder.

### Invariants

- bridge collision adjustment is not evidence for a universal render or height-plane adjustment;
- collision map may be absent when the adjusted result is invalid;
- source plane remains available for deterministic rebuild/export.

### Failure signature

Bridge scenery blocks movement on the wrong floor, or editor round-trips rewrite loc planes merely because collision is projected downward.

### Required tests

Encoded planes `0..3` with bridge bit off/on, verifying collision-plane result and unchanged source placement plane.

### Related specs

`PLANES-001`, `LOC-PLACEMENT-005`.

---

## SPEC: PLANES-003

**Domain:** `OSRS_SEMANTIC`  
**Status:** `VERIFIED`  
**Primary evidence:** `Scene.setLinkBelow(x, y)`

### Required bridge relinking

For a tile coordinate selected for link-below processing, the audited scene operation structurally shifts scene tile references:

- previous plane `1` tile becomes storage plane `0`;
- previous plane `2` tile becomes storage plane `1`;
- previous plane `3` tile becomes storage plane `2`;
- moved tile plane values decrement accordingly;
- qualifying game objects rooted at that coordinate have their stored plane decremented;
- the previous plane-0 tile becomes the new plane-0 tile's linked-below tile;
- the previous plane-3 slot is cleared after the shift.

If the new plane-0 slot would otherwise be null, a plane-0 tile is created before linking the old plane-0 tile beneath it.

### Invariants

Link-below is structural semantic state. It cannot be represented only by a visibility flag in the renderer.

### Failure signature

Duplicated bridge surfaces, missing lower tile, objects left one plane above their tile, bridge selection/edit operations targeting the wrong storage tile.

### Required tests

Four-plane synthetic tile column with tagged game objects proving exact reference movement, linked-below identity, and top-slot clear.

### Related specs

`PLANES-001`, `PLANES-004`.

---

## SPEC: PLANES-004

**Domain:** `OSRS_SEMANTIC` for the boundary; RuneLite map-level calculation is `RENDERER_POLICY` evidence  
**Status:** `VERIFIED` separation

### Required boundary

The semantic scene may expose a render/height level distinct from storage plane and a linked-below relation.

RuneLite's later GPU uploader additionally derives a local map level for `VIS_BELOW` and roof-range grouping by checking bridge state. That calculation is an upload/visibility strategy and must not be reused as the canonical collision/source/storage plane rule.

Likewise, RuneLite roof IDs and roof-removal ranges are derived runtime/editor-friendly infrastructure, not cache loc plane data.

### Invariants

`osrs-core`/`osrs-scene` must not depend on RuneLite roof-id generation. `osrs-render` may derive comparable grouping from semantic scene data in Checkpoint 5.

### Failure signature

A change in renderer roof-removal strategy unexpectedly changes loc placement, collision, or saved map plane values.

### Required tests

Semantic scene tests should remain unchanged when renderer roof grouping is disabled/replaced.

### Related specs

`PLANES-001`, future renderer ADR for roof grouping/removal.