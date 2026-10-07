# Source Group Inventory

Status: **Checkpoint 1 inventory**

This document inventories reference material at the group level. Its purpose is not to assert behavior. It records what each group can legitimately prove and what it cannot.

Baseline repository snapshot: `489b0603bc7c2c9fabd2614d204f809fb017d0cb`.

## 1. Imported primary/reference trees

| Group | Tree SHA | What it is useful for | What it cannot prove alone |
|---|---|---|---|
| `runelite-master/` | `5afef996a992bacc73655860681269242e90d6e8` | Newer RuneLite API/client/GPU behavior and interfaces | Exact OSRS software-client construction semantics where deob internals are absent |
| `OpenRune-FileStore-main/` | `55f571db4b23f2d528786e1cdfbcba0061fd201a` | Independent cache/opcode/decoder evidence and tooling patterns | Canonical OSRS behavior without target-revision verification |
| `rs-cache-master/` | `fae41f98352fc804e5d13d9bd2e836ab1e2635cd` | Existing Rust cache I/O/loader candidate and implementation evidence | Suitability for newer revisions or missing render-critical decoders |
| `reference-fixtures/` | `7aa46bc0c0f2611d4f999e5a310ad3243b6c0491` | Executed expected values suitable for differential tests | Complete scene/rendering parity |
| `reference-shaders/` | `bcf531292db5b5a95c9d9298e9f3dc28e11309d7` | Staged RuneLite shader/reference math | A canonical WGSL design or a single-revision semantic target |
| `tools/` | `a27903aba0e68bfb0728164d686f69547ddb517a` | Differential/reference tooling | Semantic truth without pinned source input |
| `docs/api/` | `9000331d0d84ee56462bffc07b70cef500096de9` | Generated API navigation | Construction/runtime semantics by itself |

## 2. Shader/reference families

The staged RuneLite GPU tree is `d33651c6b87ec5d61c43ab2e1e69d15ed6083877`.

### 2.1 Live scene shader references

Files include:

- `vert.glsl`
- `frag.glsl`
- `hsl_to_rgb.glsl`
- supporting generated/include behavior described in `reference-shaders/README.md`

Use for:

- packed HSL interpretation
- texture animation input shape
- relevant scene vertex/fragment behavior
- understanding RuneLite GPU presentation decisions

Classification: **primary renderer implementation reference**.

Not automatically OSRS semantic truth. Any math shared with software-client semantics must be proven separately.

### 2.2 Historical compute/priority references

Files include:

- `comp.glsl` / `comp.cl`
- `comp_unordered.glsl` / `comp_unordered.cl`
- `priority_render.glsl` / `priority_render.cl`
- `common.glsl` / `common.cl`
- `cl_types.cl`

The staged README states these are from the January 2026 tree and are not shipped/loaded by name in the October 2026 tree.

Use for:

- algorithm archaeology
- priority-math corroboration
- understanding historical GPU approaches

Classification: **historical renderer evidence**.

They must not be copied wholesale into the Rust renderer or treated as the active October pipeline.

### 2.3 Presentation/UI shader references

Files include:

- `vertui.glsl`
- `fragui.glsl`
- `colorblind.glsl`
- `scale/bicubic.glsl`
- `scale/hybrid.glsl`
- `scale/xbr_lv2_*`

Use for optional feature research only.

Classification: **renderer/editor presentation policy reference**.

No OSRS map-scene semantic requirement may depend on these files.

### 2.4 Region metadata

`regions/regions.txt` is RuneLite plugin data used by unrelated-map filtering behavior.

Classification: **RuneLite feature data**, not baseline OSRS scene semantics.

## 3. Verification tooling

### 3.1 `tools/deob-harness/`

Tree SHA: `1424d79679c95b490990173d15f26c6a9efa8bb1`.

Key files:

- `README.md`
- `run.sh`
- `src/Dumper.java`
- compile-only JS stubs

Current executed coverage includes:

- terrain shape/rotation tables
- all 52 shape x rotation triangulations
- wall orientation/offset tables
- color-function sweeps
- `Model.contourGround`
- synthetic `ModelData.toModel` lighting
- wall-decoration nudge behavior
- wall/decor/floor scene storage
- roof/diagonal/multi-tile placement and scene-capacity cases

Strategic role: **reference oracle harness**.

Required evolution:

- attach provenance metadata to fixture groups
- split/index fixtures by future spec
- add missing highest-risk semantics such as cross-model normals, transform combinations, mirrored winding, morph/animation paths, and priority/transparency fixtures

### 3.2 `tools/runelite-mcp/`

Tree SHA: `b343c13895d2f23e6e71b1e6b5c7e1eb2f1a2dfc`.

Key files:

- `server.py`
- `gen_api_docs.py`
- `README.md`

Role:

- search/navigation over the local reference snapshot
- source-range reads
- staged shader reads
- generation of `docs/api/`

Classification: **research/navigation infrastructure**.

It is not an oracle. Its answers must terminate in underlying source.

## 4. Generated API group

`docs/api/` is generated from the same parser used by `tools/runelite-mcp/` and currently covers render-relevant API surfaces such as:

- scene/tile structures
- model/model-data/mesh interfaces
- object composition and scene objects
- texture interfaces
- coordinate/projection helpers
- draw callbacks
- collision/flags
- animation-related interfaces

Use for:

- locating fields/methods
- understanding public contracts
- cross-linking later specs to human-readable API surfaces

Do not use for:

- exact scene-construction algorithms
- integer arithmetic order
- opcode decoding semantics
- hidden deob paths
- proving absence of behavior

Classification: **generated reference**.

## 5. Fixture group

Current primary fixture:

`reference-fixtures/deob_golden.txt`

Strengths:

- exact values from executed reference algorithms
- integer equality suitable for Rust differential testing
- broad terrain and placement coverage relative to the current research stage

Weaknesses:

- monolithic format
- weak per-case provenance
- no explicit future spec IDs
- insufficient normal/priority/animation/morph coverage
- no complete golden visual scenes

Final verification design should preserve raw oracle output while also exposing machine-readable cases indexed by semantic spec.

## 6. Research document relationship

The root `RUNELITE_*.md` set is not discarded. It becomes a navigation and derivation layer.

During later checkpoints each useful claim will be one of:

- promoted into an atomic spec
- converted into an ADR/project decision
- retained as historical rationale
- marked obsolete
- left as unresolved research

Once the canonical specs exist, root research rule numbers such as `R1` through `R27` must no longer be treated as normative identifiers.

## 7. Provenance gaps to close

Before final merge readiness, source provenance must include enough information to reproduce every normative claim without relying on a developer's original workstation.

Current gaps:

1. historical `/Users/...` paths appear throughout the research corpus
2. October RuneLite and January melxin/deob evidence are mixed
3. staged historical shaders need explicit "not active pipeline" metadata
4. golden fixture generation does not yet encode source revision beside each semantic group
5. imported trees need upstream commit/tag/repository metadata where available, in addition to this repository's tree SHA
6. cache target revision and supported revision range have not yet been chosen

These are documentation correctness issues, not cosmetic cleanup.

## 8. Checkpoint 1 result

The repository is now classified into:

- primary/reference implementations
- executable oracle evidence
- generated navigation references
- historical renderer evidence
- project architecture proposals
- research synthesis
- disputed/revision-sensitive claims

This is sufficient to begin Checkpoint 2 without treating any existing architecture or rendering claim as automatically canonical.