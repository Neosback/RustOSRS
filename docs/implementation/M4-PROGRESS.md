# M4 Progress: Model Decode and Exact Construction

Status: **Checkpoint 1 complete**  
Branch: `impl/m4-model-decode-construction`  
Baseline: M3 squash merge `6e350afeb73c96f1b1ccd050ef427028e3015987`

M4 owns raw ModelData decoding and the exact pre-GPU construction semantics required by `MODEL-BUILD-001..003`, `COORD-002`, and the M4 portions of `FACE-001`. It also establishes the ownership/cache foundation later consumed by `MODEL-BUILD-004/005`, M7 normals/lighting, and M8 contour/animation work.

## Non-negotiable boundaries

- No wgpu mesh is an M4 semantic authority or acceptance artifact.
- No renderer-side negative scaling may substitute for semantic mirroring/winding.
- No floating-point transform matrix may replace audited integer ModelData transform order.
- Raw decoded source models remain immutable shared inputs after cache admission.
- Scene normal reconciliation, final lighting, contouring, and animation execution remain with their owning later milestones.
- Missing typed/untyped model selection is preserved as semantic absence, never replaced with fallback geometry.

## Pinned decode evidence

Primary semantic pin: `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`.

The pinned `ModelData(byte[])` constructor dispatches by the final two source bytes:

| Trailer | Pinned decoder | Canonical `ModelEncoding` |
|---|---|---|
| `FF FD` | `method5265` | `TrailerFfFd` |
| `FF FE` | `method5266` | `TrailerFfFe` |
| `FF FF` | `method5287` | `TrailerFfFf` |
| anything else | `method5268` | `Legacy` |

The same pin defines `Buffer.readShortSmart()` as:

- first byte `< 128`: unsigned byte minus `64`;
- otherwise: unsigned short minus `49152`.

That primitive drives ModelData vertex-coordinate deltas and face-index delta streams, so M4 uses one shared bounds-checked implementation rather than format-local copies.

## Checkpoint sequence

### Checkpoint 1: decoder substrate

- [x] branch from exact M3 merge baseline;
- [x] audit roadmap/spec/acceptance/parity contracts;
- [x] add model ID to the standard decode provenance subject;
- [x] add checked absolute/forked cursors for parallel ModelData streams;
- [x] add exact signed short-smart primitive and boundary tests;
- [x] final clean-head Tier A/B/C CI readback on `cb7dfcff664c0f996802d6a6cf67100e9d0b8c6f`.

### Checkpoint 2: raw ModelData format decode

- implement trailer-family dispatch;
- probe/pin the format families actually encountered by target cache 2727;
- implement all encountered build-241 format branches without destructive metadata filtering;
- preserve vertices, topology, face metadata, texture triangles/mapping, skins, skeletal inputs, format identity, and provenance;
- add malformed/truncated format tests.

### Checkpoint 3: model source repository and reusable raw variants

- read model index `7` through `CacheRepository`;
- add profile/revision-aware raw model cache identity;
- establish distinct mirrored raw variant keys;
- prove immutable decoded source ownership.

### Checkpoint 4: selection, combination, and mirror semantics

- close `MODEL-BUILD-001`;
- close `MODEL-BUILD-002`;
- exact typed/untyped selection;
- exact multi-model combine;
- mirror vertices plus winding;
- untyped/type-10 special-case behavior from pinned source.

### Checkpoint 5: exact instance transform pipeline

- type-4 `256` JAU recenter and `(45, 0, -45)` translation;
- ordinary orientation quarter turns;
- recolor;
- retexture;
- resize;
- final definition translation;
- combined order-sensitive exact integer fixture;
- two-instance source immutability control.

### Checkpoint 6: M4 verification closure

- model decode fuzz smoke;
- exact P0/P1 fixtures for `MODEL-BUILD-001..003` and `COORD-002`;
- advance parity matrix rows to `EXISTING` only with linked artifacts;
- M4 exit audit;
- Tier A-C clean head before PR/merge.

## Checkpoint 1 result

Checkpoint 1 deliberately does not claim raw model decoding yet. It closes the shared cursor/provenance primitives that every format decoder needs, records the exact pinned dispatch contract before format-specific parsing begins, and preserves all M3 regression gates on the clean branch head.
