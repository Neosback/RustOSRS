# Building semantic reproduction v1

Status: native editor policy, adopted by [ADR-0018](../adr/ADR-0018-building-reproduction.md).
Owner: [BuildState reproduction](../../crates/osrs-editor/src/app/prefab_runtime/build/reproduction.rs),
[private envelope](../../crates/osrs-editor/src/app/prefab_runtime/build/reproduction/model.rs),
[native boundary](../../crates/osrs-editor/src/app/prefab_runtime/build/reproduction/native.rs).
Tests: [reproduction/tests.rs](../../crates/osrs-editor/src/app/prefab_runtime/build/reproduction/tests.rs).

## Product and command

With a current building preview and current accepted source metadata, the native Build panel offers
**Save reproduction…**. It writes a new JSON report; an existing file is preserved. Script equivalent:

```text
prefab reproduce PATH
```

The script captures the current selected candidate, so it follows an actual `prefab build` or product
preview. Space-containing destinations use the product dialog. Saving is one bounded background job;
map edits/rerolls/source changes afterward cannot rewrite the frozen input. Completion/failure is
reported through the existing status/notification owner. Capture does not apply the candidate.

Verify an exported file without a window, GPU, installed cache, file mutation or EditorApp:

```sh
cargo run --locked -p osrs-editor -- --reproduce-building /absolute/path/bundle.json
```

The command emits a JSON MATCH report on success. Failure emits JSON FAILED with a categorized error
and returns a nonzero process status. It checks schema/input integrity, replans the operation,
compares expected digests, and executes exact commit/undo/redo through EditRuntime in an isolated
session. It never executes caller-provided raw patches. This is semantic reproduction; it does not
settle or certify render/picker output, independent client fidelity or project recovery.

## Frozen envelope

The required closed top-level fields are:

| Field | Contract |
|---|---|
| `format` | `rustosrs-building-reproduction-v1` |
| `feature` | registered `buildings` feature |
| `planner_contract` | `rustosrs-editor-building-plan-v1` |
| `inputs_digest` | SHA-256 defined below |
| `inputs` | complete frozen Inputs record |
| `expected_building_digest` | v1 canonical generated building patch digest |
| `expected_patch_digest` | v1 canonical apply patch digest (clearing + generated changes) |
| `expected_result_digest` | v1 canonical map digest after normal gateway execution |

Inputs retain source metadata, captured editor build context, local source generation/semantic revision,
world origin `[x,y]`, source plane, full `request` and selected `candidate` forms, `kit_json`, complete
`footprints`, and `initial_map_json`. Generation/revision values are captured observations, not portable
freshness policies. World coordinates remain signed i32 through [canonical map transport](semantic-transport.md).
Transport strings remain intact until the strict canonical decoder; generic JSON Value must not erase
duplicate keys before those decoders see the data.

Both forms include archetype/style, dimensions/floors, base/candidate seed, variation, kit index,
clear-site flag, name, mode, optional stage, drawn sketch, exact locks and optional repair region.
No form fields default during decoding. Archetype/style/mode/stage use their canonical snake_case
unit-enum values; unknown variants reject. All five modes and seven stages retain their existing
planner behavior. The candidate must equal its request except for its seed, whose wrapping difference
is in 0..8. Synchronous candidates have difference zero. The actual captured kit is used directly;
the retained UI kit index is not an instruction to select another machine's kit.

The kit JSON is the complete captured KitFile, parsed/compiled through the existing canonical kit
owner (including learned-role data). Sketch metadata likewise retains its canonical parser/policy.
The closed bundle/form/source/fact records do not introduce an alternative kit or room planner.
Locks use canonical patch transport containing AddObject occurrence data only; this data is never
executed as a lock operation. Repair regions reconstruct RectSite through its checked constructor.

Footprint rows retain canonical object ID, loc kind, optional raw source dimensions and optional
portable declaration. Nullable facts are required fields. The existing FootprintInputs owner restores
them by rerunning its canonical requirement capture against the frozen document/kit and retained
source facts, then comparing complete maps. Missing/duplicate/extra facts and false declarations
reject. Source sizes retain precedence; unresolved required scenery still rejects canonical planning.

Source contains profile ID/digest, cache fingerprint, decoder schema version and source build. Capture
checks accepted region identity against the world worker owner's generation and checks the opened
project profile before freezing. The project and worker source counters are independent; preview
admission separately uses the project generation. Headless verification
validates the declared canonical provenance values, not the authenticity of external cache files.

## Identity and limits

`inputs_digest` is SHA-256 of UTF-8 `rustosrs-building-inputs-v1`, one zero byte, then compact serde JSON
serialization of the decoded Inputs record in its declared field order. Capture emits that record
through the same fixed serializer. JSON whitespace/key order outside retained strings is irrelevant;
bytes inside `kit_json`, lock transport and `initial_map_json` strings are retained input data. This
identifies the captured record, including observations/build context, separately from semantic map/
patch hashes. It is an integrity check, not a signature or authorization to edit live state.

Editor build context records package version, workspace source digest, target, profile, opt level,
rustc version, Cargo feature flags and escaped encoded Rust flags. Workspace source preimage is UTF-8
`rustosrs-workspace-source-v1` + zero, then sorted normalized relative paths. For each file: path length
u64 little-endian, UTF-8 path, byte length u64 little-endian, raw file bytes. Roots are Cargo.toml,
Cargo.lock, rust-toolchain.toml, .cargo, profiles and each workspace crate's Cargo.toml/build.rs/src/data.
Missing optional roots contribute nothing. This conservative source/context identity includes local
uncommitted files; it is not Git HEAD, a binary signature or complete external toolchain provenance.
Cargo watches all three directory roots (crates, profiles and .cargo), so newly added files also
refresh the stamp; existing enumerated files retain individual watches.
`same_recorded_build_context` compares only the recorded fields. Different contexts may produce MATCH;
expected patch/result checks still detect semantic drift.

Bundles are limited to 128 MiB. Native reading checks file kind/size before allocation, then bounds
reading again against growth. Sites are nonempty and bounded by the canonical prefab maximum (128
per side); checked coordinate borders reject unsupported extreme planner positions without narrowing
stored map coordinates. Locks and sketch cells have explicit count ceilings, and canonical kit/room
validators retain their existing limits. These are per-operation transport/input bounds, not a
worst-case solver time, whole-editor memory budget or global worker pool.

## Deterministic clearing order

`objects_in_rect` now returns the existing canonical document order `(position, kind, id, orientation)`;
fallible/infallible footprint-overlap queries inherit it. The ordering key is shared with
MapDocument::object_entries, without changing its established order or v1 map digests. Region storage
remains unordered. Query membership, eight-tile anchor halo, footprint math, clearing scope and
resulting object sets are unchanged. This closes an observed real-cache reproduction defect: source
HashSet iteration previously produced different ordered clearing patches in fresh processes.
Old captured patch expectations remain evidence and may correctly mismatch the fixed planner.

## Result and evidence boundary

MATCH includes feature ID, actual apply/result digests, operation count, source and captured/verifier
build contexts. Mismatch errors identify inputs/building-patch/apply-patch/result/undo/redo stage and
expected versus actual identity. Schema, input, planning and transaction errors remain distinct.
Undo/redo use the same gateway policy and canonical map comparisons; no live projection is requested.

Tests exercise five modes and seven stage views with genuine missing-work fixtures, wrapping native
batch selection, signed coordinates, source-over-declaration dimensions, stale capture rejection,
frozen lifetime, unsupported/schema/provenance/fact/hash errors, cross-build reporting, bounded
background export, complete create-only publication and size/file-kind checks. Existing-file failures
retire staging. Fresh-process command checks remain separate evidence in the handoff.

This v1 format captures successful product candidates. [Rejected attempt reproduction](building-failure-reproduction.md)
uses a separate format on the same owner and command; it never manufactures an accepted candidate.
Additional feature operations, workspace appearance/scenario, rendering/settling, recovery and MCP
remain open. Do not infer those capabilities or independent OSRS visual parity from semantic MATCH.
