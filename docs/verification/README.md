# RustOSRS Verification Registry

Status: **Checkpoint 7 normative verification registry**

This directory is the canonical index for source provenance, differential fixtures, semantic parity coverage, golden scenes, CI tiers, fuzzing, and benchmark policy.

## Files

| File | Purpose |
|---|---|
| `SOURCE-PINS.md` | exact source/tree/blob provenance and unresolved revision gates |
| `PARITY-MATRIX.md` | every canonical semantic spec mapped to required verification |
| `REFERENCE-FIXTURES.md` | fixture format, manifests, regeneration, differential-oracle policy |
| `GOLDEN-SCENES.md` | deterministic composed semantic/reference scene catalog |
| `CI-FUZZ-BENCHMARKS.md` | local/CI tiers, property tests, fuzz targets, performance regression policy |

The overall architecture is defined by `docs/blueprint/16-VERIFICATION-ARCHITECTURE.md`.

## Verification status vocabulary

Coverage status in this directory means:

- **EXISTING**: a checked-in executable fixture/test already covers the stated contract;
- **PARTIAL**: existing evidence covers only a subset of the required cases;
- **REQUIRED**: the blueprint defines the required test/fixture, but it does not yet exist;
- **BLOCKED**: a source/revision gate prevents creating a canonical oracle without first resolving provenance;
- **PLANNED-GPU**: requires the future wgpu implementation/reference runner;
- **PLANNED-EDITOR**: requires the future editor/document implementation.

These statuses describe implementation coverage, not evidence certainty. A semantic spec may be `VERIFIED` from source while its Rust differential fixture is still `REQUIRED`.

## Test classes

### Exact fixture

A source-derived input/output artifact compared byte-for-byte or value-for-value.

### Differential test

Runs equivalent normalized input through the reference oracle and Rust implementation, then compares the owned output.

### Semantic golden scene

A composed scene with exact expected semantic state and render-extraction state.

### Visual golden

A deterministic Reference-profile render compared with documented raster tolerance.

### Property/state-machine test

Generates structured inputs/operation sequences and asserts invariants rather than one known output.

### Fuzz test

Feeds malformed/untrusted byte inputs to parser/serializer boundaries and asserts safety/robustness.

### Failure-injection test

Forces I/O, stale-generation, GPU lifecycle, or concurrency failures and verifies state remains correct.

## Required execution model

Ordinary Rust tests must not require Java, external source checkouts, or network access.

Checked-in reference fixtures are consumed directly by Rust tests.

Oracle regeneration is an explicit maintainer/developer workflow that records source pins and harness revision before replacing expected data.

## Golden-data change rule

A golden/fixture update must never be accepted as a blind snapshot refresh.

Any change to expected canonical data requires:

1. identify the owning spec/ADR;
2. explain why the expected result changed;
3. record source/harness/profile changes;
4. review semantic diffs before image diffs;
5. update fixture manifest/hash;
6. preserve old fixtures when they represent a still-supported target profile.

A command equivalent to `UPDATE_GOLDENS=1` may exist for developer convenience, but CI must never update goldens automatically.

## Canonical target caveat

The public January 28, 2026 melxin/deob commit is the main pinned semantic source for the current spec set. The historical local deob tree used by `tools/deob-harness` is not yet proven byte-identical. See `SOURCE-PINS.md`.

The complete terrain-color builder remains blocked under `TERRAIN-004` until its exact builder/oracle provenance is closed.

## Implementation ownership

- `osrs-cache`: decode fixtures, parser properties/fuzzing;
- `osrs-core`: model/definition/math exact tests;
- `osrs-scene`: placement, bridges, morph state, normal reconciliation, scene goldens;
- `osrs-render`: extraction, sort, GPU ABI, reference visual goldens, device lifecycle;
- `osrs-reference`: source-oracle adapters, fixture regeneration/comparison tooling;
- `osrs-editor`: command/history/project/autosave/export/state-machine verification.

## Rule

A screenshot is never the terminal proof for an exact semantic contract.
