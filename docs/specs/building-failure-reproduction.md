# Rejected building attempt reproduction v1

Status: native editor policy adopted by [ADR-0019](../adr/ADR-0019-rejected-building-reproduction.md).
Owner: existing [building reproduction](../../crates/osrs-editor/src/app/prefab_runtime/build/reproduction.rs),
[failure envelope/verifier](../../crates/osrs-editor/src/app/prefab_runtime/build/reproduction/failure.rs),
[planner failure](../../crates/osrs-editor/src/app/prefab_runtime/build/plan/error.rs).
Tests: [failure unit tests](../../crates/osrs-editor/src/app/prefab_runtime/build/reproduction/failure/tests.rs).

## Capture and invocation

**Save reproduction…** captures the currently selected successful preview, or the first current
rejected attempt when no preview is usable. The candidate list also offers **Save report…** beside
each current rejected seed. This works in mixed successful/failed batches without selecting or
applying another candidate. Existing report files are preserved. Equivalent scripts are:

```text
prefab reproduce PATH
prefab reproduce PATH failure=SEED
```

The explicit selector must identify a retained current failed seed. It never changes the request
seed or regenerates work at capture time. Native batches try eight wrapping offsets; synchronous
script planning tries its actual request seed only. A bundle records one rejected attempt, not a
whole batch result or a proof that every possible seed is unsatisfiable.

Capture requires the same source/project profile and preview input admission used by successful
reproduction. Form/site/plane, source generation, semantic revision and frozen definition facts must
still match. Retained failure records share the batch input; they contain no document/kit snapshots.
Only explicit capture clones a document/kit. Clear/replacement releases old records, while an already
frozen bounded export may finish independently. Worker cancellation/panic/disconnection and errors
before planner invocation do not manufacture a rejected-attempt record.

The same native command verifies both declared formats before initializing a window/GPU/cache:

```sh
cargo run --locked -p osrs-editor -- --reproduce-building /absolute/path/failure.json
```

## Closed envelope and failure identity

| Required field | Value / meaning |
|---|---|
| `format` | `rustosrs-building-failure-v1` |
| `feature` | `buildings` |
| `planner_contract` | `rustosrs-editor-building-failure-v1` |
| `inputs_digest` | Existing v1 building input SHA-256 grammar |
| `inputs` | Exact shared successful-reproduction Inputs record |
| `expected_failure` | Required `stage`, `message`, `cause` fields |
| `expected_initial_digest` | Canonical v1 MapDocument digest |

The shared [input contract](building-reproduction.md) retains signed world/map transport, actual base
request and offset candidate forms, complete kit, canonical footprint facts, source provenance and
recorded build context. All its bounds, required nullable channels and request/candidate checks apply.
No default reconstruction or caller-provided raw patch is executed. Successful v1 bundles and their
digest grammar remain unchanged. Dispatch inspects only the format and passes original bytes to the
appropriate closed decoder, so nested/top-level duplicate fields remain rejectable.

`stage` is a closed enum: `request`, `generation`, `clearing`, `map_validation`,
`candidate_validation`, `thumbnail`. The planner assigns it at the actual failing boundary.
`message` preserves the product diagnostic without a seed prefix. `cause` preserves exact lower-owner
diagnostic evidence, including canonical error debug output where available; request-only failures
have an empty cause. Neither field is parsed to infer an error code. All three fields must match
exactly. This captures conservative failure evidence rather than inventing another model for every
lower crate's errors. Wording/debug-format changes across builds can therefore cause an intentional
mismatch even when a coarse rejection category remains the same. Build context helps diagnose it.

## Verification and result

After protocol/input/schema checks and canonical restoration, the verifier compares the initial map
digest and calls the existing `build/plan.rs::plan` with the frozen intent. Unexpected success is a
`planning outcome` mismatch. A different stage, message or opaque cause is a `planning failure`
mismatch carrying expected/actual evidence. The map must still have its exact initial digest.
Malformed/schema/input failures are verifier errors, not reproduced planning outcomes.

An exact rejection emits JSON with `status: MATCH`, `outcome: rejected`, actual failed seed/failure,
initial/result map digests, `operation_count: 0`, source and captured/verifier build contexts. There
is no patch digest, synthetic patch or history transaction. Matching reproduction returns exit 0;
mismatches and verifier errors emit categorized JSON FAILED and return nonzero. A reproduced
rejection does not mean generation succeeded. Existing successful bundles retain their original
MATCH report and normal isolated EditRuntime commit/undo/redo verification.

Tests cover synchronous request rejection, real all-failed wrapping batches and explicit seed
selection, generation/constraint and candidate validation stages, stale/frozen lifetime, mismatched
completion identity, mixed batch capture, every failure identity channel, unexpected success,
initial-map integrity, closed/duplicate schemas, native publication and successful-format dispatch.
Fresh-process and live-cache evidence is recorded separately in the handoff.

Workspace appearance/scenario, render/picker settling/images, recovery journals, other operations,
MCP, independent OSRS visual parity and global time/byte/worker budgets remain separate work.
