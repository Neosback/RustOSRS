# M2 Exit Audit

Status: **COMPLETE pending PR merge**  
Milestone: **M2 - `osrs-core` semantic foundation**  
Branch: `impl/m2-core-semantic-foundation`

This audit closes the M2 implementation milestone against `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md`. It evaluates only M2-owned representation and pure-helper behavior. Cache decoding, model construction, scene construction, normal/lighting algorithms, dynamic morph/animation behavior, rendering, and editor policy remain owned by later milestones.

## Scope reviewed

M2 changes are confined to `osrs-core` plus M2 implementation documentation:

- `crates/osrs-core/src/ids.rs`
- `crates/osrs-core/src/coords.rs`
- `crates/osrs-core/src/coordinate_math.rs`
- `crates/osrs-core/src/orientation.rs`
- `crates/osrs-core/src/provenance.rs`
- `crates/osrs-core/src/definitions.rs`
- `crates/osrs-core/src/model.rs`
- `crates/osrs-core/src/lib.rs`
- `docs/implementation/M2-PROGRESS.md`
- `docs/implementation/M2-EXIT-AUDIT.md`

No M3 decoder, M4 model-construction, M6 scene, M7 lighting/finalization, M8 dynamic-model, renderer, or editor implementation is included.

## Roadmap deliverables

### Identity and coordinates - PASS

Implemented stable typed concepts for:

- object/model/texture/sequence/varbit/varp and supporting semantic IDs;
- global map tiles and regions;
- in-region and scene-storage tile coordinates;
- exact 128-unit semantic local coordinates;
- model-local coordinates;
- source/storage/collision/render-level distinctions;
- cache-independent target/profile provenance.

Model IDs retain the full `u32` domain required by the selected target profile.

### Exact coordinate helpers - PASS

The semantic unit contract is explicit:

- tile = `128`;
- half tile = `64`;
- quarter tile = `32`;
- three-quarter tile = `96`.

Round-trip helpers use checked integer arithmetic and Euclidean division/remainder where negative coordinates require it.

The exit audit identified one missing roadmap gate: exact tile/footprint centers. M2 now includes pure `tile_center` and `footprint_center` helpers using the exact integer term:

```text
origin_local + footprint_tiles * 64
```

The helper deliberately does not transpose dimensions for orientation, sample heights, clamp scene edges, or place objects. Those remain M6 responsibilities.

Tests cover 1x1 centers, 2x3/3x2 non-square centers, negative world tiles, zero-size rejection, and overflow failure.

### Canonical definitions - PASS

Canonical cache-independent values exist for:

- object definitions;
- typed vs untyped object model tables;
- object morph selectors, nullable entries, and explicit fallback;
- object model scale/translation, recolor, and retexture inputs;
- underlays and overlays;
- textures/material inputs;
- varbits and varps;
- frame and skeletal sequence inputs needed by later animation ownership.

Revision-specific opcode interpretation remains M3. Definition values do not expose `rune-fs` or cache codec types.

### Model representation - PASS

Validated `SourceModel` state preserves:

- vertices and triangle topology;
- face colors;
- face render type;
- per-face priority or model default priority;
- signed face alpha;
- texture IDs;
- optional texture-face selectors;
- texture triangles and source mapping parameters;
- authored face bias;
- vertex and face skin metadata;
- skeletal bone/weight source data;
- model-format identity/provenance.

Recolor/retexture transformation inputs are retained canonically on `ObjectDefinition`, where the audited model-construction pipeline obtains them. They are not duplicated into raw `SourceModel` state.

Optional source arrays preserve absence. `None` is not replaced with zero-filled data merely for convenience.

### Working-model ownership - PASS

`SourceModel` is immutable through its public API and derives comparison/hash behavior suitable for regression/immutability tests.

`to_working_copy()` produces a deep mutable semantic copy. Working state keeps:

- base vertex normals;
- optional flat-face normals;
- merged vertex-normal slots separate from base normals;
- animation groups separate from source skin arrays.

Geometry/topology/render-type mutation invalidates derived normal state through the working-model API. No topology welding is implied by merged normal state.

Actual normal calculation/merge, lighting, transform execution, contouring, and animation algorithms are not implemented in M2.

## Owned specification closure

### COORD-001 - M2 representation/helper requirements satisfied

- exact 128-unit tile scale encoded;
- half/quarter/three-quarter constants encoded;
- world tiles and local coordinates are distinct types;
- tile/local and region round trips covered;
- exact tile/footprint centers covered;
- no renderer world scale exists in `osrs-core`.

Terrain-shape exact positions and full loc-placement fixtures remain with their owning M6 implementation/tests.

### COORD-002 - M2 helper conventions satisfied

- loc orientations are distinct from editor/camera angles;
- model orientations preserve the audited `orientation > 3` branch;
- exact integer quarter-turn mappings exist;
- JAU values have an explicit 0..2047 semantic domain;
- the special `256`-JAU decoration angle is named explicitly.

The general 256-JAU sine/cosine transform plus type-4 recenter/translation sequence remains M4, as required by `MODEL-BUILD-003`.

### COORD-003 - PASS

The API separates:

1. world/map tile identity;
2. scene/storage tile coordinates;
3. semantic local coordinates;
4. model-local coordinates;
5. renderer-owned camera/view/clip coordinates, which are absent from `osrs-core`.

Source/storage/collision/render-level plane concepts are also distinct without implementing M6 bridge relinking early.

### FACE-001 - M2 representation requirements satisfied

The canonical source-model boundary retains every FACE-001 metadata class required at this stage, including optional-array absence/default distinction and authored bias. Renderer packing is absent from M2.

The later decode/model-construction fixture that proves retention from cache bytes through semantic-to-render handoff remains owned by M4/M5/M10+ and is not falsely claimed here.

## Verification exit gates

| Gate | Result |
|---|---|
| exact coordinate round trips | PASS |
| tile constants exact | PASS |
| tile and footprint centers exact | PASS |
| semantic IDs are typed rather than renderer integers | PASS |
| optional face arrays preserve absence/default semantics | PASS |
| source model hash/comparison supports immutability tests | PASS |
| Tier B exact tests | PASS on the M2 code head before this documentation-only closure commit |

The final documentation-complete head must also pass Tier A and Tier B before the M2 PR is merged.

## Boundary review

No `wgpu`, `egui`, `eframe`, `rune-fs`, archive codec, scene ownership, or editor document type is present in `osrs-core`.

No current public API executes:

- cache opcode decoding;
- object model selection;
- mirror semantics;
- model transformation order;
- scene placement/bridge relinking;
- normal calculation or cross-model merge;
- final lighting;
- morph resolution;
- contouring;
- animation execution;
- GPU/render packing.

Those remain explicit later-milestone gates.

## Carryovers

M3 starts only after this milestone is merged. Its first responsibility is cache transport/definition decoding into these canonical values. M3 must not mutate the M2 semantic contracts to accommodate convenience decoder structures without a documented compatibility reason.

M4 remains responsible for ModelData decoding and exact construction semantics, including model selection, mirroring, the type-4 `256`-JAU path, recolor/retexture/resize/translation order, and source/working ownership during construction.

M6 remains responsible for orientation-aware footprint transposition, height sampling, scene placement, planes/bridges, and loc side effects.

M7 remains responsible for base normal calculation, cross-model normal accumulation/suppression, scene finalization, and exact reference lighting.

M8 remains responsible for morph selection, animation-group construction/execution, dynamic-object model resolution, and contouring.

## Exit decision

M2 is implementation-complete. The milestone may be merged once the exact documentation-finalized branch head passes Tier A and Tier B and the PR diff remains limited to M2 scope.
