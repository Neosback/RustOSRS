# Initial-placement collision snapshots

Status: **pinned revision-235 primitive differential coverage; build-241 editor integration**.
This is clipping inspection, not a complete client routefinder, interaction-reach test or playtest runtime.

## SPEC: COLLISION-001

- **Domain:** `OSRS_SEMANTIC` for clipping writes; `EDITOR_POLICY` for inspection freshness/presentation.
- **Status:** `VERIFIED` for the pinned revision-235 primitive cases; revision-241 integration is tested
  separately and does not upgrade all movement semantics to cross-revision verification.
- **Target / primary evidence:** exact snapshot and symbols below; complete file SHA-256 pins in
  `scripts/collision_oracle/sources.json`.
- **Executable evidence:** `collision-deob.txt` (94 full-grid cases), scene/editor unit tests and the
  prerequisite-gated `collision_projection` real-cache test.
- **Integer semantics:** exact u32 flag ORs; bounded stencil writes; checked world/scene conversion.
- **Invariant / failure signature:** render admission cannot remove clipping; missing land must never
  become a known free cell; storage relinking must not rewrite source/collision planes.
- **Required behavior, exclusions and tests:** specified below; related contracts `LOC-PLACEMENT-005`
  and `PLANES-002`.

## Source and scope

The existing `LOC-PLACEMENT-005` side-effect planner and `PLANES-002` remain authoritative for selecting the
operation and collision plane. This implementation materializes those plans, not a parallel placement
decision. Initial placements use the original definition's clipping flags and rotated footprint,
independently of the effective visual morph, model availability, slot replacement or capacity admission.
Runtime replacement/removal must follow its own audited lifecycle; it cannot be inferred from this
initial-placement builder or implemented by clearing a bit shared by another contributor.

Sources at `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`:

- `CollisionMap.java` SHA-256 `c8dd1468081d0bb17d42e29caf941e932552b1a985647bb6dd1f1b5f1795d25a`:
  constructor/`clear`, `method6338`/`method6332`, `method6355`, `method6340`, `addGameObject`,
  `setBlockedByFloor` and `setBlockedByFloorDec`.
- `class337.method7281` clears bit `1073741824` on actual loaded terrain in usable interior cells.
- `class470.method9712` projects bit-1 floor blockers downward when plane-1 bridge bit 2 is set.
- `DynamicObject.method2054` chooses initial loc collision planes; `FriendSystem.addObjects` applies
  clipping outside renderable/scene-insertion-success conditions.
- `WorldView` uses ordinary `CollisionMap(width,height,false)` for the top-level world (`id == -1`).

The mirror declares revision 235, while RustOSRS's selected cache is 241. The primitive differential
evidence below does not certify all revision-241 movement behavior. Expanded sub-world collision maps,
instances, low-detail filtering, interaction reach, actor-size stepping and line-of-sight traversal
are outside this checkpoint. Reference geometry and semantic scene hashes are unchanged by design.

## Representation and ownership

`osrs-scene::collision` owns immutable `CollisionSnapshot` and worker-local `CollisionSnapshotBuilder`.
It imports only canonical core values. It has no GPU, cache transport, UI, clock or document authority.
The pure `CollisionStencil` translates an existing `CollisionSideEffect` into at most three rectangular
writes. Large footprints are clipped before iteration; no footprint-sized allocation occurs.

The ordinary 104×104 reference grid initializes its usable interior (`1 <= x,y < 99`) to unloaded
bit `0x40000000`; edge padding is `0x00ffffff`. Actual loaded regions clear only the unloaded bit in
that interior. Missing-region height fill never marks land loaded. A queried cell distinguishes
outside-window, missing land and usable loaded land; zero visible geometry is not evidence of free space.

Walking walls use bits `1,2,4,8,16,32,64,128` in NW,N,NE,E,SE,S,SW,W order; projectile walls use
the corresponding bit shifted left 9. Footprint walking/projectile masks are `256` / `131072`;
floor-decoration and terrain blockers are `262144` / `2097152`.

Terrain provenance records all contributing source-plane bits. Loc provenance retains original ID,
anchor, type, orientation and source plane, plus the projected collision plane and exact write stencil.
Both wall arms, neighbouring tiles and overlapping contributors remain independently explainable.
Missing definitions are counted explicitly and prevent a snapshot from certifying traversability.

`osrs-world` builds terrain coverage before consuming loc streams, adds clipping immediately after
placement planning and before constructing models, then shares the frozen snapshot through `WorldScene`
and `RegionInfo`. Snapshots are disposable and leave residency with their owning region; no global
collision cache is introduced. Storage is four 104×104 u32 flag grids, one u8 terrain-source grid per
plane (216,320 bytes of cell payload), plus bounded-by-input loc provenance.

`WorldRuntime` installs inspection data only behind existing FIFO/generation gates. Installed producer
epochs, dirty regions/zones and pending builds prevent the collision inspector showing old evidence
as current. Unload/source replacement clears retained inspection and epoch state. This view is for
inspection only; future runtime activation needs an explicit authoritative revision/snapshot boundary.
Terrain flag edits carrying `ChangeImpact::COLLISION` use the conservative 3×3 producer ring: bridge
projection can move an entire anchored footprint between collision planes beyond the radius-5 floor
blend. Ordinary material/height paths keep their existing proofs. Object output comparison already
builds full contextual producers and installs complete inspection independently of narrowed GPU zones.

## Verification

`scripts/collision_oracle/regenerate.py` compiles the **unmodified external** `CollisionMap.java` with
five pinned mapping annotations and our small driver. No deob implementation is checked in. The
checked-in fixture hashes every cell of 94 maps, including all boundary types/orientations, projectile
flags, loaded/unloaded initialization, floor/decor blockers, zero/large footprints and edge clipping.
The Rust test independently builds and hashes its complete corresponding maps.

```sh
python3 scripts/collision_oracle/regenerate.py --runelite-root /path/to/RuneLite-melxin --check
cargo test --locked -p osrs-scene collision
cargo test --locked -p osrs-editor collision
RUSTOSRS_TARGET_CACHE_DIR=/path/to/b241/cache cargo test --locked -p osrs-world --test collision_projection -- --ignored
```

Additional tests cover bridge/source-plane separation, neighbour provenance, overlapping contributions,
unknown coverage, invalid recipes and rebuild removal. A real-cache test places a blocking static
definition under an unsupported model type, proves clipping survives model omission, then inversely
projects the edit and compares all cells with the original rebuild. This last test exercises worker
projection reversal; existing edit-gateway tests separately own semantic transaction/undo correctness.

The inspector layout test covers expanded explanations/provenance and status messages at 240/320/480
logical points and scales 1/2. It does not certify native mouse interaction. Routefinding, controllable
actors, variable-driven state transitions, audio playback and entity rendering remain subsequent work.
