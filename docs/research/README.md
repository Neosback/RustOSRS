# Legacy Research Corpus

Status: **Historical research only, reconciled through the M10 foundation audit**

The root-level `RUNELITE_*.md` files predate the canonical RustOSRS blueprint. They remain useful for source discovery and historical reasoning, but they are **not implementation specifications**.

## Authority rule

When research text conflicts with current canonical documentation, the canonical owner wins:

1. `docs/specs/` for OSRS semantic behavior;
2. accepted `docs/adr/` records for RustOSRS-owned decisions;
3. `docs/blueprint/` for architecture and roadmap;
4. `docs/verification/` for source pins, parity status, fixture ownership, and CI policy.

A root research statement must never be the terminal authority for production code or a regression test.

## M10 foundation-audit correction

The earlier research quarantine itself contained one now-proven false statement: it said the pinned public `class470` terrain-builder attribution was stale and that the complete terrain builder remained source-blocked.

That is superseded.

At exact public source:

`melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

`class470.java` blob:

`1cd9cad5cb4be865dcae94dc633bba821644dc84`

contains final static `method9712(WorldView)`, the complete target terrain-construction routine.

`FriendSystem.java` blob:

`b8cf51b6ee673181d8a115e28153a77f5978c390`

pins the placement-derived shadow-grid writes consumed by that builder.

Therefore:

- `TERRAIN-004` is **source verified**, not source blocked;
- production Rust implementation plus an exact differential fixture are still required;
- old research prose saying public `class470` is unrelated text-layout code must not be used.

Canonical owners are `docs/specs/terrain.md`, `docs/verification/SOURCE-PINS.md`, `docs/verification/PARITY-MATRIX.md`, and `docs/implementation/M10-FOUNDATION-AUDIT.md`.

## Other known superseded research claims

### Cross-model normals

Separate scene objects are not topologically welded, but qualifying `ModelData` instances **do** reconcile coincident normals before final lighting. Matched faces can be suppressed when the owning merge enables hiding.

Canonical owner: `docs/specs/normals-lighting.md`.

### Universal bridge-adjusted plane

Source/encoded plane, collision plane, storage plane, mutable `Tile.plane`, `originalPlane`, `minPlane`, linked-below state, and renderer grouping are distinct contracts.

Canonical owner: `docs/specs/planes-bridges.md`.

### Generic floor-decoration lift

The audited placement path does not apply a generic `+1/+2` floor-decoration height lift.

Canonical owner: `docs/specs/loc-placement.md`.

### Fixed texture count as semantic truth

Imported RuneLite renderer capacity is not an OSRS texture-ID invariant. RustOSRS preserves full-width texture identity and maps it through renderer-owned handles.

Canonical owner: `docs/specs/face-materials.md`.

### Universal RuneLite priority sorting

The software/client priority algorithm and imported RuneLite GPU behavior are separate targets. RuneLite GPU enables the full priority queue only on selected render-mode paths, notably `SORTED_NO_DEPTH`.

Canonical owners: `docs/specs/face-materials.md` and ADR-0005.

### `GreaterEqual` Reference depth

Imported RuneLite uses reverse-Z clear `0` with strict `GL_GREATER`. RustOSRS Reference mode now uses `Greater`.

Canonical owner: ADR-0004.

### Conventional transparency depth writes

The imported RuneLite snapshot does not establish a universal rule that blended alpha disables depth writes. Explicit no-depth render modes are separate.

Canonical owner: `docs/blueprint/12-GPU-DATA-PASSES.md`.

### "Nearest" as the full texture sampler rule

Imported magnification is nearest. Minification is nearest only at filtering level `0`; at level `>=1` it is `NEAREST_MIPMAP_LINEAR`, and the imported configuration defaults to level `1`. S wraps clamp-to-edge; T remains default repeat in the audited setup.

Canonical owner: `docs/specs/face-materials.md`.

### Self-generated screenshot as external parity proof

A RustOSRS-generated image is internal regression evidence. External visual parity requires an independently generated, provenance-complete client/RuneLite oracle.

Canonical owner: `docs/blueprint/16-VERIFICATION-ARCHITECTURE.md`.

## Legacy document map

| Legacy document | Current use | Canonical replacement |
|---|---|---|
| `RUNELITE_RENDER_SOURCES.md` | source-discovery index | `docs/verification/SOURCE-PINS.md` |
| `RUNELITE_GPU_PIPELINE.md` | renderer research index | `docs/blueprint/11-RENDERER-ARCHITECTURE.md`, `12-GPU-DATA-PASSES.md`, ADR-0004..0006 |
| `RUNELITE_SCENE_AND_MATERIALS.md` | historical scene/material discovery | `docs/specs/`, semantic/terrain audits |
| `RUNELITE_RUNTIME_RULES.md` | corrected research index | `docs/specs/` |
| `RUNELITE_DEOB_READING_GUIDE.md` | deob navigation | source pins plus owning specs |
| `RUNELITE_RUST_PORT_NOTES.md` | historical Rust feasibility notes | crate/renderer/editor blueprints and ADRs |
| `RUNELITE_HARDEST_PARTS.md` | historical risk discovery | roadmap, parity matrix, foundation audit |
| `RUNELITE_CACHE_STACK.md` | historical cache-dependency research | ADR-0010 and M1 implementation records |

## Historical-content policy

Git history is the archive for original research conclusions. When stale root prose creates a material implementation risk, the current file may be replaced by a corrected non-normative research index while the original remains recoverable from Git history.

That is what the M10 foundation audit does for the highest-risk root notes. This does not promote those root files to specification authority.

## Research-use workflow

When a research document contains a potentially useful detail:

1. use it to locate the owning source/method/table;
2. identify the canonical spec or ADR that owns the behavior;
3. follow the exact source pin from canonical documentation;
4. if no canonical contract exists, classify the finding as research/revision-sensitive rather than implementing from prose;
5. update the appropriate spec/ADR and verification requirement before production code depends on it.

Do not repair an earlier semantic failure with renderer or editor compensation.