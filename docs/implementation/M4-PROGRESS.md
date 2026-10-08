# M4 Progress: Model Decode and Exact Construction

Status: **Checkpoint 2 implementation complete; clean-head CI pending**  
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

- [x] implement trailer-family dispatch;
- [x] probe/pin the format families actually encountered by target cache 2727;
- [x] implement every encountered build-241 format branch without destructive metadata filtering;
- [x] preserve vertices, topology, face metadata, texture triangles/mapping, skins, skeletal inputs, format identity, and provenance;
- [x] add malformed/truncated format tests;
- [ ] final clean-head Tier A/B/C CI readback.

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

Checkpoint 1 closed the shared cursor/provenance primitives that every format decoder needs, recorded the exact pinned dispatch contract before format-specific parsing began, and preserved all M3 regression gates.

## Checkpoint 2 target evidence

A temporary target-only GitHub Actions probe downloaded the exact OpenRS2 cache 2727 disk export, opened it through the production `CacheRepository`, enumerated model index `7`, and passed every non-empty model file through `decode_model_data` on commit `1fbcb6f9281932c2bbf8d75fb6a188ce1441cd4a`.

The exhaustive build-241 result was:

- model groups: `62,043`;
- `FF FD`: `35,103`;
- `FF FE`: `26,940`;
- `FF FF`: `0`;
- legacy: `0`;
- decoded model count exactly matched encoded family counts;
- multi-file model groups: `0`;
- empty model files: `0`.

The same sweep exercised real target metadata beyond basic topology:

- texture render type `0`: `6,393` occurrences;
- texture render type `1`: `484` occurrences;
- texture render type `2`: `4,238` occurrences;
- texture render type `3`: `2` occurrences;
- models carrying authored face-bias data: `13,647`;
- models carrying skeletal vertex data: `374`.

Therefore the selected build-241 target requires only the `FF FD` and `FF FE` decoders. `FF FF` and legacy dispatch remain explicit typed decode failures rather than guessed historical compatibility paths. Their absence is target evidence, not a claim that those encodings never existed in OSRS.

The retained ignored test `crates/osrs-cache/tests/m4_target_probe.rs` reproduces the target family-count and all-model decode assertions when `RUSTOSRS_TARGET_CACHE_DIR` points at the separately obtained pinned cache. Ordinary repository CI does not download the 182 MiB target cache.

### Canonical preservation decisions

- `FF FD` retains render types `0..=3`, simple and complex texture mapping inputs, face colors, render types, default/per-face priorities, signed alpha, textures, selectors, skins, skeletal influences, and authored face bias.
- `FF FE` decodes the packed face-information layout but preserves the source-derived optional arrays instead of applying the client's later destructive cleanup that nulls redundant texture selectors/render-type arrays.
- Complex texture mapping words for render types `1..=3` are not model-vertex indices. Canonical validation therefore applies model-vertex bounds to texture triangles only for render type `0`.
- Model format identity records `TrailerFfFd` or `TrailerFfFe`; no separate version is invented when the target encoding provides no verified version field.
- Every malformed/truncated failure remains inside the standard target/cache/archive/file plus `Model(id)` provenance envelope.

Temporary target/download and write-capable checkpoint workflows are removed after capturing this evidence. Checkpoint 3 repository/caching/variant work has not started.
