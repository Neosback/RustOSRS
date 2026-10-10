# Contradiction and Open-Question Register

Status: **Living register, reconciled through the M10 foundation audit**

This register records claims that must not become implementation requirements until their evidence and ownership are clear.

Severity:

- **P0**: can produce structurally wrong OSRS geometry/scene semantics.
- **P1**: can produce visibly wrong rendering or revision breakage.
- **P2**: architecture/product ambiguity that can cause coupling or rework.

## C-001: Cross-model normal behavior

**Severity:** P0  
**Status:** `RESOLVED`

Pinned `Scene.method5585/method5587` performs cross-`ModelData` normal reconciliation through `ModelData.method5262`. Separate objects remain separate meshes, but eligible coincident normals are accumulated and fully matched faces may be marked render type `2` when hiding is enabled.

Canonical owner: `docs/specs/normals-lighting.md`.

## C-002: Mesh welding versus visual continuity

**Severity:** P0  
**Status:** `RESOLVED`

Topology/identity and normal reconciliation are separate. No production rule may infer "no mesh welding" to mean "no cross-object normal reconciliation."

## C-003: Mixed January/October evidence snapshots

**Severity:** P1  
**Status:** `REVISION_SENSITIVE`

The corpus includes pinned public deob/client source and a separately imported RuneLite GPU tree. Claims must terminate in exact file/blob/tree identity and may not pretend these sources are one snapshot.

The historical developer-machine deob checkout is still not proven byte-identical as a whole to the public pin. This remains relevant only where a claim still depends on that old tree rather than a directly pinned public file/method.

## C-004: Mixed shader provenance

**Severity:** P1  
**Status:** `RESOLVED AT OWNERSHIP LEVEL`

The staged shader directory contains live, historical, and presentation material. It is evidence only. RustOSRS ports verified contracts by responsibility rather than transliterating the directory wholesale.

## C-005: Historical local filesystem paths

**Severity:** P1  
**Status:** `REVISION_SENSITIVE`

Absolute paths and human-readable dates are provenance only, never identity. Checked-in fixture bytes and promoted public source pins are hash-gated. Whole historical checkout equivalence remains unclaimed.

## C-006: Differential fixture coverage

**Severity:** P0  
**Status:** `OPEN COVERAGE`

M3-M10 now have substantial exact fixture/test coverage, including model transforms, placement, normals, lighting, morph/contour/legacy animation scope, priority preparation, UV preparation, and zones.

Remaining high-risk gaps include:

- complete `TERRAIN-004` differential fixture;
- target `minPlane/originalPlane` end-to-end visibility fixture;
- Reference GPU depth/bias/alpha fixtures;
- texture pixel/sampler/cutout GPU fixtures;
- an independent external visual oracle.

The terrain source itself is no longer blocked.

## C-007: Reusable crate architecture

**Severity:** P2  
**Status:** `RESOLVED`

Accepted crates are `osrs-core`, `osrs-cache`, `osrs-scene`, `osrs-render`, `osrs-reference`, and `osrs-editor`. Reusable semantics are not editor-owned.

## C-008: Mandatory wasm support

**Severity:** P2  
**Status:** `RESOLVED`

RustOSRS is native-first. wasm remains deferred unless a future ADR changes that decision.

## C-009: Editor versus reusable client foundation

**Severity:** P2  
**Status:** `RESOLVED`

The initial product is an editor, while lower `osrs-*` crates remain reusable enough for future reference/client applications where correctness is preserved.

## C-010: Cache dependency contract

**Severity:** P1  
**Status:** `RESOLVED`

ADR-0010 selects `rune-fs 0.2.0` as private read-only transport. RustOSRS owns revision-aware decoders and canonical semantic structures. No `rune-fs` type crosses the public cache/core boundary.

`TERRAIN-004` is no longer a cache/source blocker under this item. Its remaining work is semantic implementation plus fixture coverage.

## C-011: "FileStore is the spec"

**Severity:** P1  
**Status:** `RESEARCH ONLY`

OpenRune FileStore remains valuable corroborating decoder/tooling evidence, not the terminal OSRS oracle.

## C-012: Fixed texture count/capacity

**Severity:** P1  
**Status:** `RESOLVED`

Semantic texture IDs are full-width and independent of imported RuneLite's fixed renderer capacity. Renderer allocation uses material handles/pages and must surface capability failure explicitly.

## C-013: Reverse-Z Reference state

**Severity:** P1  
**Status:** `RESOLVED AS POLICY, GPU IMPLEMENTATION PENDING`

Imported RuneLite evidence establishes clear depth `0` and strict `GL_GREATER`. ADR-0004 now defines RustOSRS Reference mode as:

```text
clear depth   = 0
depth compare = Greater
near          = larger depth
far           = smaller depth
```

The imported projection has no finite far plane. `GreaterEqual` is no longer the Reference baseline. Authored bias requires distance-sensitive fixtures before GPU parity is claimed.

## C-014: Camera controls versus semantic angles

**Severity:** P2  
**Status:** `RESOLVED`

JAU/object orientation remains semantic/reference math. Camera controls and editor framing are editor policy.

## C-015: RuneLite region filtering

**Severity:** P2  
**Status:** `RESOLVED`

RuneLite plugin region lists and visibility preferences are not baseline scene semantics. Workspace membership and visibility filters are editor/render state.

## C-016: Generated API docs as proof

**Severity:** P1  
**Status:** `RESOLVED`

Generated/API pages are navigation evidence only. Normative claims require pinned source and/or executable evidence through their owning spec.

## C-017: Verification-strength vocabulary

**Severity:** P1  
**Status:** `RESOLVED`

The project distinguishes verified, derived, project decision, research, hypothesis, disputed, revision-sensitive, deferred, obsolete, source-verified/implementation-required, and structural/GPU-pending states rather than flattening them into "verified."

## C-018: Generic floor-decoration lift

**Severity:** P1  
**Status:** `RESOLVED / REFUTED`

The audited path stores floor-decoration Z at the supplied semantic height. No generic `+1/+2` lift is part of the canonical contract.

## C-019: Initial versus runtime model ownership

**Severity:** P0  
**Status:** `RESOLVED`

Initial scene construction, pending/static replacement, morph resolution, contouring, animation working copies, and scene normal-finalization are distinct semantic pipelines. M7/M8 exact tests own these boundaries.

## C-020: Software priority versus RuneLite GPU sorting

**Severity:** P0  
**Status:** `RESOLVED`

Pinned software/client `Model.method5946` defines the priority `0..11` threshold/special-queue oracle implemented by M10 CPU preparation.

Imported RuneLite GPU is path-specific:

- full priority queues run only when `prioritySort=true`, notably `SORTED_NO_DEPTH`;
- ordinary dynamic upload passes `false`;
- static opaque work is not universally priority-sorted;
- `Zone` alpha work uses model-distance/face-depth ordering.

RustOSRS Reference mode intentionally selects the software/client ordering contract for priority-sensitive content. A RuneLite-GPU comparison profile would be separate.

## C-021: Complete terrain-builder source attribution

**Severity:** P0  
**Status:** `RESOLVED AT SOURCE / IMPLEMENTATION OPEN`

The earlier register statement that public pinned `class470` was unrelated text-layout code was wrong.

At:

`melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

`class470.java` blob:

`1cd9cad5cb4be865dcae94dc633bba821644dc84`

contains final static `method9712(WorldView)`, the complete terrain-construction routine.

It includes slope lighting, placement-shadow consumption, separable radius-5/11x11 underlay blending, overlays, tile emission, `minPlane`, scene normal finalization, and bridge relinking.

`FriendSystem.java` blob:

`b8cf51b6ee673181d8a115e28153a77f5978c390`

pins the placement-derived shadow writes consumed by that builder.

Current `TERRAIN-004` status is `SOURCE_VERIFIED / IMPLEMENTATION_REQUIRED`. C-021 is no longer a provenance blocker. Production Rust implementation and an executable differential fixture remain mandatory before implementation parity is claimed.

## C-022: Bridge/plane concepts collapsed into one rule

**Severity:** P0  
**Status:** `RESOLVED AT CONTRACT LEVEL`

The canonical model distinguishes:

- source/encoded plane;
- collision plane;
- storage plane;
- mutable `Tile.plane`;
- immutable `Tile.originalPlane`;
- `Tile.minPlane`;
- linked-below state;
- renderer-derived settings/roof grouping such as RuneLite `maplevel`.

Imported RuneLite `maplevel` can change settings/roof lookup while geometry remains `tiles[level]`; it does not rewrite semantic storage identity.

End-to-end Rust representation/visibility tests for `minPlane/originalPlane` remain an implementation gate, not a conceptual contradiction.

## C-023: Alpha blending and depth-write assumptions

**Severity:** P1  
**Status:** `SOURCE_VERIFIED / GPU IMPLEMENTATION OPEN`

The imported RuneLite snapshot does not establish a universal modern rule that blended alpha disables depth writes. Explicit no-depth render modes/ranges are separate.

Reference GPU work must test alpha ordering, strict `Greater`, depth writes, and explicit no-depth behavior instead of inheriting conventional assumptions.

## C-024: Texture filtering shorthand

**Severity:** P1  
**Status:** `RESOLVED AT SOURCE / GPU IMPLEMENTATION OPEN`

The imported renderer is not simply "nearest filtered":

- MAG = `NEAREST`;
- MIN at filtering level 0 = `NEAREST`;
- MIN at level >=1 = `NEAREST_MIPMAP_LINEAR`;
- imported default level = 1;
- S = clamp-to-edge;
- T = default repeat in the audited setup.

Physical GPU realization remains a later M12 gate.

## C-025: Self-generated visual goldens as external proof

**Severity:** P1  
**Status:** `RESOLVED AT VERIFICATION-POLICY LEVEL`

RustOSRS-generated Reference screenshots are internal regression evidence (`V3R`). External measured parity (`V3`) requires an independently generated, provenance-complete client/RuneLite oracle artifact.

The project currently has no promoted external V3 visual oracle.

## Documentation quarantine

Root `RUNELITE_*.md` files remain non-normative research. Git history preserves the original research text; current root files may be reduced to corrected research indexes when stale prose poses a material implementation risk.

## Resolution workflow

For every newly resolved item:

1. pin exact evidence;
2. state the chosen/verified behavior;
3. update the owning spec or ADR;
4. attach implementation/test ownership;
5. update `PARITY-MATRIX.md`;
6. do not mark implementation complete merely because source provenance is complete.