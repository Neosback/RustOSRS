# M3 Exit Audit

Status: **COMPLETE pending PR merge**  
Milestone: **M3 - Cache transport and definition decoding**  
Branch: `impl/m3-cache-definition-decoding`

This audit closes M3 against `docs/blueprint/17-IMPLEMENTATION-ROADMAP.md`, `docs/implementation/M1-DECODER-ACCEPTANCE.md`, and ADR-0010. It evaluates cache transport, target-aware source/archive access, byte decoding, canonical definition inputs, raw terrain/location acquisition, definition-level floor color post-decode state, and decoder error/provenance behavior.

M4 model binary decoding/construction, M5 broader differential-fixture infrastructure, M6 scene placement and plane semantics, M7 normals/lighting, M8 runtime morph/animation ownership, renderer behavior, and editor behavior remain outside M3.

`TERRAIN-004` remains explicitly blocked. M3 does not guess the full terrain-color builder.

## Scope reviewed

The M3 branch changes are confined to the cache/decode boundary, the minimum canonical floor-definition additions needed by `TERRAIN-003`, M3 verification fixtures/tests, and CI/documentation wiring:

- `crates/osrs-cache/src/decode/`
- `crates/osrs-cache/src/transport.rs`
- `crates/osrs-cache/src/lib.rs`
- `crates/osrs-cache/tests/m3_decode_fixtures.rs`
- `crates/osrs-cache/tests/m3_fuzz_smoke.rs`
- `crates/osrs-core/src/floor_color.rs`
- floor-definition fields in `crates/osrs-core/src/definitions.rs`
- `crates/osrs-core/src/lib.rs`
- `reference-fixtures/decode/m3-p0.txt`
- Tier C wiring in `.github/workflows/ci.yml`
- M3 verification/exit documentation

A branch comparison against the M2 merge commit `2901ba6d04792eca580c8f9c574cb3843eefa83a` was reviewed before closure. No M4 model decoder/construction, M6 scene construction, M7 lighting/finalization, M8 runtime dynamic-model, GPU renderer, or editor implementation is included.

## Roadmap deliverables

### Production cache source/archive repository - PASS

ADR-0010's private `rune-fs` spike has been promoted into a RustOSRS-owned production repository boundary.

`CacheRepository` provides:

- logical content-index enumeration;
- group enumeration;
- exact encoded reference-table reads;
- exact encoded group reads;
- decoded group reads;
- logical file reads from multi-file groups;
- target-aware map-square file access;
- RustOSRS-owned group/reference metadata;
- caller-owned XTEA input;
- typed RustOSRS errors;
- configurable encoded-size, decoded-size, and file-count limits.

No `rune-fs` type crosses the public `osrs-cache` boundary.

The logical multi-file splitter is implemented in RustOSRS with checked arithmetic and exact data/table coverage checks. M3 deliberately does not call the dependency's panic-prone archive-file-group splitting helper on untrusted malformed input.

### Compression, bounds, and transport failure containment - PASS

Before dependency decompression, the transport validates encoded compression-container lengths and declared decoded output size against configured limits. Logical-file splitting validates file counts, chunk-table dimensions, cumulative sizes, table coverage, and data coverage before constructing outputs.

Dependency calls are contained behind RustOSRS error mapping. Dependency panics reached through the wrapped operations are converted to typed `DependencyPanic` outcomes rather than becoming an intended production decode path.

This does not assert that dependency-internal unsafe code is intrinsically safe. ADR-0010's replacement/patch escape hatch remains valid if future targets expose a transport limitation.

### Target/profile identity and cache fingerprint - PASS

Repository construction starts from a validated `TargetProfile` and computes the complete `rustosrs-cache-v1` fingerprint from exact encoded reference-table/group bytes. Opening the repository fails if the computed fingerprint differs from the profile-pinned cache identity.

For the selected target, the verified fingerprint remains:

`ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38`

Reference metadata retains CRC, reference hash, Whirlpool digest, version, name hash, file IDs, and encoded length. For this exact-target workflow, the full pinned cache fingerprint is the artifact-identity/integrity gate; M3 does not invent a second semantic truth from dependency-specific checksum behavior.

### XTEA ownership and selected-target behavior - PASS

The repository accepts caller-owned XTEA words with provider/key-fingerprint provenance while keeping raw key material out of `Debug` and artifact provenance.

The selected OpenRS2 cache 2727 profile explicitly reports `xtea_keys_present: 0`, `xtea_keys_total: 0`, and `key_set_id: none-required-by-source`. Therefore build-241 `read_map_square()` correctly reads the numeric combined map group without injecting keys.

The generic XTEA transport boundary remains covered by the committed legacy-cache regression test for profiles/data that require it.

### Build-241 map-square resolution - PASS

Build 241 does not expose usable map-index archive name hashes. M3 therefore implements target-aware numeric map-square resolution rather than treating historical `mX_Y` / `lX_Y` name lookup as universal truth.

For revision 237+ layout:

- cache index is `5`;
- group ID is packed from region X/Y;
- terrain is logical file `0`;
- locations are logical file `1`.

Older builds fail the modern resolver explicitly instead of silently falling back to an unproven layout.

### Object definition decoding - PASS

The RustOSRS-owned object decoder preserves the canonical M3 inputs needed later for placement, construction, morphs, animation, normals, and lighting, including:

- typed and untyped model tables;
- target-gated 32-bit model IDs;
- size and placement/collision-related definition flags;
- decoration displacement and support-items behavior;
- rotation/non-flat-shading/contour inputs;
- sequence ID;
- signed ambient and contrast;
- scale and signed translation;
- recolor/retexture pairs;
- map-scene/map-icon/category/editor metadata;
- actions and target string behavior;
- varbit/varp morph selectors;
- nullable transform entries;
- opcode-92 explicit fallback;
- target-era noncanonical fields consumed at exact verified widths.

Unknown target opcodes become contextual typed errors. No first-model fallback or scene behavior is introduced at decode time.

### Floor definitions and `TERRAIN-003` - PASS

Underlay/overlay byte decoding preserves target defaults and opcodes, including primary RGB, overlay texture, `hideUnderlay`, and optional secondary RGB.

M3 also closes the definition-level post-decode state required by `TERRAIN-003`:

- underlay weighted hue;
- saturation;
- lightness;
- hue multiplier with target minimum clamp;
- overlay primary HSL;
- optional secondary HSL;
- target post-decode ordering.

Exact vectors cover black, white, saturated colors, negative hue behavior, primary/secondary colors, and hue-multiplier boundaries.

This stops at floor-definition semantics. Neighborhood blending, slope lighting, jitter, texture fallback, and complete terrain-color construction remain `TERRAIN-004` and are not implemented here.

### Varbit/varp decoding - PASS

M3 preserves:

- varbit backing varp ID;
- inclusive start/end bit endpoints;
- varp client type required by the current target/editor input model;
- stable typed definition identity and target provenance;
- explicit unknown-opcode failure.

Runtime var-state selection and morph resolution remain M8.

### Texture definition inputs - PASS for M3 scope

The build-241 revision-233+ texture layout is revision-gated and preserves:

- texture ID/provenance;
- source sprite/file ID;
- average RGB;
- target transparency flag as canonical opacity;
- animation direction;
- animation speed.

Historical multi-sprite composition arrays are not fabricated for a target layout that does not encode them. Full model texture mapping, texture pixel/material construction, UV semantics, animation application, and renderer behavior remain later milestones under `TEXTURE-001`.

### Sequence metadata - PASS for M3 scope

The build-241 revision-226+ sequence decoder preserves the metadata needed for later M8 ownership, including:

- frame IDs and delays;
- frame step;
- interleave data;
- loop count;
- precedence/priority defaults and explicit overrides;
- equipment overrides;
- reply/restart mode;
- skeletal animation ID;
- skeletal range;
- skeletal mask.

Target-only noncanonical fields are consumed at verified widths. Broader secondary-source opcodes are not silently imported when the build-241 contract does not support them.

Frame archive, skeleton/base, and runtime animation execution remain explicitly deferred to M8 by the M1 acceptance contract.

### Raw terrain decode - PASS

Terrain decoding preserves exactly the raw semantic inputs owned by M3:

- all four encoded/source planes;
- plane -> local X -> local Y traversal;
- default versus explicit source height opcode state;
- overlay encoded ID;
- overlay shape and rotation;
- tile settings;
- underlay ID;
- region identity separate from region-local tile coordinates.

No bridge adjustment, storage-plane relinking, collision-plane transformation, terrain topology, height interpretation, or terrain-color builder is applied in the decoder.

### Location stream decode - PASS

Location decoding preserves:

- extended-smart object-ID accumulation;
- smart packed-position accumulation;
- full `u32` canonical object identity path;
- region-local X/Y;
- unchanged encoded/source plane;
- loc type;
- orientation;
- region identity outside each local entry.

Malformed packed positions outside four 64x64 source planes fail explicitly. Placement, footprint, height sampling, orientation dispatch, and scene side effects remain M6.

### Contextual decoder error/provenance envelope - PASS

Public decode failures can identify:

- target profile ID;
- target profile digest;
- cache fingerprint;
- decoder schema version;
- build/revision context;
- index/group/file identity;
- semantic decode subject when applicable;
- opcode when known;
- exact byte offset/range when known;
- XTEA provider/key fingerprint when present, never raw keys.

`DecodeSubject` covers object/floor/var/texture/sequence definition IDs and terrain/location region coordinates. This closes the M1 requirement that a decode failure identify the definition or region being decoded instead of forcing diagnostics to infer it from archive/file numbers.

Unknown opcodes, invalid values, limit violations, unterminated fields, trailing bytes, and truncation use typed RustOSRS error categories rather than logging and continuing.

## M1 decoder-acceptance closure

| M1 acceptance section | M3 result |
|---|---|
| 1. Transport and archive contract | PASS |
| 2. Terrain/map tile decode | PASS for raw M3 ownership |
| 3. Location stream decode | PASS |
| 4. Object definition decode | PASS for M3-required canonical fields and revision gates |
| 5. Floor underlay definitions | PASS |
| 6. Floor overlay definitions | PASS |
| 7. Varbit/varp definitions | PASS |
| 8. Model data decode | DEFERRED to M4 as specified |
| 9. Texture definitions/material inputs | PASS for build-241 M3 definition inputs; downstream material/model mapping remains later |
| 10. Sequence definitions | PASS for M3 metadata ownership |
| 11. Frames/skeletons | DEFERRED to M8 as specified |
| 12. Decoder error/provenance envelope | PASS for M3 decoder families; model-format identity becomes applicable with M4 model decode |
| 13. Acceptance method | PASS for M3 through source-pinned checked-in vectors, unit/error coverage, and offline Tier C gates |

Deferred M4/M8 rows are not M3 failures because the acceptance contract explicitly assigns them to later milestones.

## Verification exit gates

### Checked-in P0 fixture set

`reference-fixtures/decode/m3-p0.txt` contains ten source-pinned M3 vectors covering:

1. extended object model IDs;
2. object morph fallback/null semantics;
3. underlay weighted HSL state;
4. overlay primary/secondary/default-changing opcodes;
5. varbit range semantics;
6. varp client type;
7. build-241 texture layout;
8. skeletal sequence metadata;
9. raw terrain tile fields;
10. extended-smart location decoding.

The fixture header pins the selected target profile and source/tooling evidence. `crates/osrs-cache/tests/m3_decode_fixtures.rs` verifies those bytes against canonical RustOSRS outputs offline.

### Decoder malformed-input smoke

`crates/osrs-cache/tests/m3_fuzz_smoke.rs` runs a deterministic 512-case malformed-input smoke suite across every M3 decoder family. Decoder and reader unit tests additionally cover unknown opcodes, truncation, unterminated fields, trailing bytes, malformed packed positions, revision gates, and exact error context.

This is intentionally a CI-stable fuzz smoke gate, not a claim that later dedicated fuzzing infrastructure is unnecessary.

### Allocation/bounds coverage

Reader and transport tests cover bounded strings/reads, oversized declared decompression output, file-count limits, safe multi-file splitting, truncated/inconsistent file tables, arithmetic overflow paths, and zero transport-limit rejection.

### CI status at audit closure

Before the final documentation commits, the M3 code path had already passed the repository's full Tier A, Tier B, and newly introduced Tier C sequence. The later semantic-subject provenance closure patch was itself gated by full-workspace `cargo check` plus `cargo test -p osrs-cache` before it was allowed to commit.

The documentation-finalized branch head must pass normal read-only Tier A, Tier B, and Tier C before an M3 pull request is opened or merged. A documentation commit is not allowed to weaken that requirement.

## Boundary review

M3 does not implement or claim:

- raw model binary decoding;
- object model selection/combination;
- mirror/winding construction;
- orientation transforms or the type-4 256-JAU path;
- recolor/retexture/resize/translation execution;
- terrain topology or shaped surfaces;
- object/loc scene dispatch;
- footprint transposition or height sampling;
- bridge/storage/collision-plane scene semantics;
- base or merged normals;
- final reference lighting;
- runtime morph selection;
- contouring;
- animation execution;
- frame/skeleton resource decoding;
- semantic-to-render extraction;
- wgpu resources/passes;
- editor document/tool behavior.

Those remain explicit later-milestone gates.

## Carryovers

### M4

M4 owns target ModelData binary decoding and exact model construction, including typed/untyped selection, combination, mirror/winding, orientation transforms, the type-4 256-JAU path, recolor/retexture/resize/translation order, and construction ownership.

### M5

M5 owns the broader source-manifest differential-fixture infrastructure and regeneration workflow. M3's checked-in decode vectors provide the P0 fixture base without prematurely claiming the complete M5 harness.

### M6

M6 owns semantic terrain topology, loc placement, footprint/center height sampling, source/storage/collision plane behavior, bridge relinking, and placement side effects.

### M8

M8 owns frame/skeleton resources, runtime morph selection, animation execution/ownership, contouring, and dynamic semantic model resolution.

### `TERRAIN-004`

The full terrain-color builder remains `BLOCKED`. M3's exact floor-definition HSL implementation closes `TERRAIN-003` only.

## Exit decision

M3 is implementation-complete subject to the normal CI gate on the documentation-finalized branch head. Once Tier A, Tier B, and Tier C are green on that head, the milestone is ready for a pull request.

No M3 pull request is opened or merged by this audit itself.
