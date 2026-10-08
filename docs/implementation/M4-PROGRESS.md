# M4 Progress: Model Decode and Exact Construction

Status: **Checkpoint 4 implementation complete; final clean-head CI pending**  
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
- [x] clean implementation head `43607c47de339db2e08255bf148e7ee1b7ac89d7` passed Tier A/B/C.

### Checkpoint 3: model source repository and reusable raw variants

- [x] read model index `7` through `CacheRepository`;
- [x] add profile/revision-aware raw model cache identity;
- [x] establish distinct mirrored raw variant keys;
- [x] prove immutable decoded source ownership and working-copy isolation;
- [x] sweep all `62,043` pinned build-241 models through the repository layer;
- [x] cleanup/evidence head `74f4991ce41d9625a9fe0f6ec91b4e87cf956e2d` passed Tier A/B/C after temporary-workflow removal.

### Checkpoint 4: selection, combination, and mirror semantics

- [x] close `MODEL-BUILD-001` construction semantics;
- [x] close `MODEL-BUILD-002` mirror semantics;
- [x] implement exact typed/untyped selection;
- [x] implement exact multi-model combination;
- [x] mirror vertices plus face winding;
- [x] preserve the untyped/type-10 special-case behavior from pinned source;
- [x] implementation head `7191ee7a6607049ec6782260af90c291f6a3a55e` passed Tier A/B/C;
- [ ] final documentation-complete exact-head CI readback.

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

Temporary target/download and write-capable Checkpoint 2 workflows were removed after capturing this evidence.

## Checkpoint 3 repository and ownership result

`ModelSourceRepository` now binds the verified `CacheRepository` transport to model index `7` and the target-aware ModelData decoder. The reusable raw-model cache stores immutable `Arc<SourceModel>` entries rather than mutable instance geometry.

The exact raw cache identity is:

```text
RawModelCacheKey = build + TargetProvenance + ModelId + RawModelVariant
```

`TargetProvenance` already contains the profile ID, profile digest, cache fingerprint, and decoder schema version. Build is retained explicitly in the repository key so revision identity is not inferred from naming conventions.

`RawModelVariant` currently distinguishes:

- `Unmirrored`: authoritative decode loaded directly from target model index `7`;
- `Mirrored`: a separate derived-cache slot reserved for the audited mirror geometry/winding operation owned by Checkpoint 4.

Checkpoint 3 did not implement mirroring. It established the cache-key and ownership boundary required to prevent mirrored/unmirrored aliasing before Checkpoint 4 authored mirrored geometry.

### Ownership and admission rules

- repeated requests for one unmirrored model return the same shared `Arc<SourceModel>`;
- an externally supplied derived model cannot replace the authoritative unmirrored cache entry;
- mirrored and unmirrored variants have distinct keys even for the same model ID;
- admitted model ID must exactly match the cache key;
- admitted model target provenance must exactly match the repository key;
- re-admitting semantically identical data under the same key reuses the existing shared entry;
- conflicting semantic data under the same key is rejected;
- `SourceModel::to_working_copy()` produces owned mutable state, and mutation of that working copy leaves the cached source unchanged.

These rules establish the M4 foundation for `MODEL-BUILD-005` without forcing deep copies of immutable source assets.

### Pinned target repository sweep

A temporary read-only verification workflow ran on commit `f3f46d874ce33c634fc47f4372700698c3835f8b` using OpenRS2 cache 2727. Workflow run `37790526558`, job `113356389991`, completed successfully.

The retained ignored test loaded every build-241 model through `ModelSourceRepository::load_unmirrored` and asserted the repository behavior in addition to the existing decode checks:

- all `62,043` model groups loaded and were retained as unmirrored repository entries;
- encoded family counts remained `35,103` `FF FD` and `26,940` `FF FE`;
- decoded family counts exactly matched encoded family counts;
- repeated loading of the same model returned the same `Arc` allocation;
- the corresponding mirrored slot remained absent before Checkpoint 4 construction;
- repository cached-variant count exactly matched the `62,043` target model groups;
- multi-file model groups remained `0`;
- empty model files remained `0`;
- texture render-type and face-bias/skeletal coverage remained unchanged from Checkpoint 2.

The temporary target-cache workflow was removed immediately after capturing this proof.

## Checkpoint 4 exact construction result

Checkpoint 4 separates pure semantic construction from cache acquisition. `osrs-core::model_construction` owns selection, raw mirroring, and multi-model combination. `osrs-cache::object_model` only resolves the selected model IDs through `ModelSourceRepository`, obtains the required raw variant, and returns an owned pre-transform `AssembledModel`.

### Exact selection semantics

The pinned `ObjectComposition.getModelData(type, orientation)` behavior is preserved directly:

- an untyped opcode-5-style model table is accepted only when the requested loc type is exactly `10`;
- an absent or empty untyped model-ID list produces semantic absence;
- every untyped model ID is retained in source order for later combination;
- a typed opcode-1-style table requires an exact loc-type match;
- a typed miss produces semantic absence;
- there is no fallback to the first model, nearest type, or type `10`.

`resolve_object_model` returns `Ok(None)` for those legitimate semantic misses instead of creating fallback geometry.

### Exact raw mirror semantics

Typed models use the pinned rule:

```text
mirror = isRotated XOR (orientation > 3)
```

The accepted untyped/type-10 path deliberately does not reuse that formula. Pinned source initializes its mirror flag from `isRotated` and only toggles it for requested type `2` with orientation greater than `3`. Because the same branch has already rejected every requested type other than `10`, that toggle is unreachable for a valid untyped selection. RustOSRS therefore uses:

```text
untyped type-10 mirror = isRotated
```

The mirror operation itself follows the pinned `ModelData` operation rather than renderer policy:

- negate every model-local Z coordinate with wrapping signed-integer behavior;
- swap face index A with face index C to reverse winding;
- leave the immutable unmirrored source untouched;
- do not use renderer-side negative scale as a substitute;
- do not rewind texture-triangle metadata, because the pinned mirror method does not do so.

The cache-backed resolver now populates `RawModelVariant::Mirrored` on demand. Mirrored and unmirrored entries remain distinct reusable raw variants under the Checkpoint 3 revision/profile/model/variant key.

### Construction order

The pre-transform path is now explicit:

1. select exact model ID or IDs;
2. load each authoritative unmirrored raw source;
3. obtain/cache the mirrored raw variant for each source when the selection requires it;
4. combine the selected raw variants when more than one model ID is present;
5. hand the owned `AssembledModel` to the later instance-transform pipeline.

Checkpoint 5 still owns type-4 special rotation/translation, ordinary orientation turns, recolor, retexture, resize, and final definition translation.

### Exact multi-model combination

The pinned `ModelData(ModelData[], int)` plus its vertex lookup helper establish the combination behavior implemented in `combine_source_models`:

- vertices are admitted lazily from faces and render-type-0 texture triangles;
- vertex deduplication uses exact X/Y/Z equality;
- the first matching coordinate wins, preserving deterministic first-seen order;
- later duplicate coordinates do not overwrite the first admitted vertex skin or skeletal metadata;
- face topology is remapped through the deduplicated vertex table;
- texture-face selectors are offset by the accumulated texture-triangle count from earlier source models;
- per-face priority output is materialized when any source already has per-face priorities or uniform source priorities differ;
- when that output exists, a source without a per-face priority array contributes its uniform default priority;
- optional render type, alpha, face skin, texture, selector, and authored face-bias outputs are materialized only when required by the reference-constructor contract;
- missing values use the corresponding reference zero/default/sentinel semantics;
- multi-model construction materializes vertex-skin output, with missing source skin values represented by the reference integer default `0`;
- skeletal vertex output is materialized only when at least one source carries skeletal metadata;
- every contributing source identity and format is retained, rather than inventing a synthetic singular `ModelId` for combined geometry;
- source target provenance must match across all constituents.

Complex texture mapping for render types `1..=3` keeps the canonical preservation policy established in Checkpoint 2. Those three source words are not treated as model-vertex indices and are not destructively remapped. Only render-type-0 texture triangle vertex triplets participate in the model vertex dedup/remap pass.

### Checkpoint 4 fixtures and CI

The exact construction unit fixtures cover:

- typed exact-match selection and typed miss behavior;
- all four typed `isRotated` / `orientation > 3` XOR combinations;
- untyped non-type-10 rejection;
- untyped type-10 all-model selection;
- untyped mirror behavior independent of high orientation;
- absent and empty model-list semantic absence;
- Z mirroring plus A/C face-winding reversal;
- wrapping signed negation at `i32::MIN`;
- immutable-source control during mirror construction;
- exact XYZ deduplication and first-seen vertex order;
- face-index remapping;
- texture-selector offsetting;
- differing uniform-priority expansion to per-face priorities;
- first-source vertex skin/skeletal ownership on duplicate coordinates;
- preservation of all contributing source descriptors;
- complex texture-mapping preservation without model-vertex remapping.

Implementation head `7191ee7a6607049ec6782260af90c291f6a3a55e` passed the complete ordinary CI chain in workflow run `37805786555`:

- Tier A static quality: architecture boundaries, architecture-guard tests, rustfmt, workspace check, and strict clippy all passed;
- Tier B workspace tests passed, including the new exact construction fixtures;
- Tier C checked-in M3 parity fixtures and deterministic decoder fuzz smoke passed.

Checkpoint 5 instance-transform work has not started.
