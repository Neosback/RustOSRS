# Legacy Research Corpus

Status: **Historical research only — non-normative**

The root-level `RUNELITE_*.md` files predate the canonical RustOSRS blueprint. They remain in the repository because they contain valuable source-discovery notes, formulas, file maps, and historical reasoning, but they are **not implementation specifications**.

## Authority rule

When a legacy research statement conflicts with any of the following, the legacy statement loses:

1. `docs/specs/` for OSRS semantic behavior;
2. accepted `docs/adr/` records for RustOSRS-owned architecture/product decisions;
3. `docs/blueprint/` for system/editor/renderer architecture and roadmap;
4. `docs/verification/` for source pins, fixture ownership, parity coverage, and CI policy.

A legacy statement must never be used as the terminal authority for production code or a regression test.

## Legacy document map

| Legacy document | Still useful for | Canonical replacement / authority |
|---|---|---|
| `RUNELITE_RENDER_SOURCES.md` | source discovery and file inventory | `docs/blueprint/03-SOURCE-GROUP-INVENTORY.md`, `docs/verification/SOURCE-PINS.md` |
| `RUNELITE_GPU_PIPELINE.md` | RuneLite GPU research and historical algorithm details | `docs/blueprint/11-RENDERER-ARCHITECTURE.md`, `12-GPU-DATA-PASSES.md`, ADR-0004..0006 |
| `RUNELITE_SCENE_AND_MATERIALS.md` | API/scene research and field discovery | `docs/specs/`, `docs/blueprint/05-SYSTEM-ARCHITECTURE.md`, `09-SEMANTIC-AUDIT.md`, `10-TERRAIN-MATERIAL-PLANE-AUDIT.md` |
| `RUNELITE_RUNTIME_RULES.md` | original R1-R27 research trail | `docs/specs/` plus semantic audit documents |
| `RUNELITE_DEOB_READING_GUIDE.md` | deob navigation and historical pseudocode | `docs/verification/SOURCE-PINS.md`, canonical specs with pinned source |
| `RUNELITE_RUST_PORT_NOTES.md` | early Rust/wgpu/egui design research | `docs/blueprint/06-CRATE-ARCHITECTURE.md`, `11-15`, ADR-0001..0009 |
| `RUNELITE_HARDEST_PARTS.md` | historical risk brainstorming | `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md`, `docs/verification/PARITY-MATRIX.md` |
| `RUNELITE_CACHE_STACK.md` | rs-cache/FileStore investigation | M1 in `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md`; cache dependency choice still requires an implementation-era ADR |

## Known superseded or quarantined claims

The following legacy conclusions are specifically unsafe to copy into implementation.

### Normal merging

Legacy language asserting that normals are strictly per-model or that there is no cross-model normal merging is superseded.

Canonical authority:

- `docs/specs/normals-lighting.md`
- `NORMALS-001..004`
- `docs/blueprint/09-SEMANTIC-AUDIT.md`

Separate meshes can remain topologically separate while eligible `ModelData` instances reconcile coincident vertex normals before final lighting. Mesh welding and normal accumulation are different operations.

### `editor_*` crate architecture

Legacy crate names such as `editor_core`, `editor_cache`, `editor_render`, and `editor_app` are not the accepted architecture.

Canonical authority:

- ADR-0001
- `docs/blueprint/06-CRATE-ARCHITECTURE.md`

Accepted primary crates are `osrs-core`, `osrs-cache`, `osrs-scene`, `osrs-render`, `osrs-reference`, and `osrs-editor`.

### Mandatory wasm support

Legacy instructions that the core must target wasm or that browser support is a hard requirement are superseded.

Canonical authority: ADR-0002.

RustOSRS is native-first. wasm is deferred and must not distort the native editor architecture unless a future ADR changes the decision.

### Mirroring through renderer transforms

Any implication that mirroring can be represented merely by a negative GPU scale is unsafe.

Canonical authority:

- `MODEL-BUILD-002`
- ADR-0004 renderer winding/culling conventions

OSRS model mirroring changes semantic geometry/winding before rendering.

### Universal bridge-adjusted plane

Legacy prose that collapses bridge behavior into one adjusted plane is superseded.

Canonical authority:

- `PLANES-001..004`
- `docs/blueprint/10-TERRAIN-MATERIAL-PLANE-AUDIT.md`

Encoded/source plane, collision plane, storage plane, render/height level, linked-below relation, and renderer roof grouping are distinct concepts.

### Generic floor-decoration height lift

Any generic `+1`/`+2` ground-decoration lift claim is refuted for the audited path.

Canonical authority: `LOC-PLACEMENT-006`.

### `TEXTURE_COUNT = 256` as semantic truth

Historical RuneLite GPU texture capacity is not a cache or OSRS semantic invariant.

Canonical authority:

- `TEXTURE-001`
- `docs/blueprint/12-GPU-DATA-PASSES.md`

RustOSRS renderer capacity is adapter-aware and maps semantic texture IDs through renderer-owned material/page allocation.

### `class470` terrain-builder attribution

The legacy attribution of the complete terrain-color builder to obfuscated `class470` is stale for the pinned public January source.

Canonical authority:

- `TERRAIN-004`
- `docs/verification/SOURCE-PINS.md`

The full terrain-color/11x11 builder remains `REVISION_SENSITIVE` and blocked from unconditional parity claims until a reproducible source/oracle is pinned.

### “FileStore is the spec”

OpenRune FileStore is valuable independent implementation evidence, not the OSRS oracle.

Canonical authority:

- evidence hierarchy in `docs/blueprint/01-EVIDENCE-STATUS.md`
- M1 in `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md`

### `rs-cache` hybrid recommendation

The old recommendation to adopt `rs-cache` and port missing pieces is research, not an accepted dependency decision.

Canonical authority: M1 of the implementation roadmap. Implementation must perform the compatibility spike and record the chosen strategy in a new ADR.

### Priority ordering simplification

Face priority must not be reduced to a generic `(priority, depth)` sort.

Canonical authority:

- `FACE-002`
- ADR-0005
- `docs/verification/PARITY-MATRIX.md`

### Screenshot evidence

A visual match never overrides an exact P0/P1/P2 failure.

Canonical authority:

- `docs/blueprint/08-PARITY-MODEL.md`
- `docs/blueprint/16-VERIFICATION-ARCHITECTURE.md`

Screenshot tolerance begins at P3 reference visual parity.

## Research-use workflow

When a legacy document contains a potentially useful detail:

1. use it to locate the relevant source/method/table;
2. check whether a canonical spec or ADR already owns the behavior;
3. follow the source pin from the canonical document;
4. if no canonical contract exists, record the issue as research/revision-sensitive rather than implementing from the legacy prose;
5. add or update the appropriate spec/ADR and verification requirement before production code relies on it.

## Historical-content policy

The legacy files are intentionally preserved substantially as written so the evolution of the research remains inspectable. Their presence does not imply current endorsement.

Do not “fix” a legacy document in place merely to make it agree with current architecture when doing so would erase useful provenance. Promote the corrected result into the canonical blueprint/spec/ADR system instead.
