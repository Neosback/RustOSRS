# M5 Model Fixture Migration Record v2

Status: `SOURCE_PINNED_MANUAL_NORMALIZATION_CORRECTION`

This record corrects the normalized input/output pair for `model.transform.type4_order` after the Checkpoint 3 repository-wide semantic runner proved that the v1 artifact did not reproduce the executable M4 case it cited.

The production transform implementation was not changed by this correction. M4's executable transform fixture remained green. The defect was isolated to the manually normalized M5 artifact.

## Oracle source

- repository: `melxin/runelite`
- commit: `1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`
- `runescape-client/src/main/java/ObjectComposition.java`
  - blob: `079451cd9a6dcfd2666efd15b0524250eaafe4c4`
- `runescape-client/src/main/java/ModelData.java`
  - blob: `2cc9406b2504fbd4fae0c0c952aa2d133809e928`
- `runescape-client/src/main/java/Rasterizer3D.java`
  - blob: `f32216b5e564c6a03a173438e3b19004c27c1c9e`

## Executable Rust source case

The corrected normalized fixture mirrors the exact M4 test introduced in commit `384b1274d40bbc8b4acd383ee31364b82a2e08cc`:

`crates/osrs-core/tests/m4_instance_transform.rs`

Test: `combined_pipeline_matches_reference_order_exactly`

Exact pre-transform semantic input:

- vertices: `(128, 64, 0)`, `(0, 0, 0)`, `(0, 128, 0)`
- face color: `100`
- face texture: `7`
- requested loc type: `4`
- orientation: `5`
- recolors: `100 -> 200`, then `200 -> 300`
- retextures: `7 -> 8`, then `8 -> 9`
- scale: `(256, 64, 128)`
- definition translation: `(10, -5, 20)`

Exact expected output:

- vertices: `(-262, 27, -115)`, `(-80, -5, -25)`, `(-80, 59, -25)`
- face color: `300`
- face texture: `9`

## Why v1 was corrected

The v1 transform artifact contained a different manually constructed input and expected geometry while claiming to normalize the M4 order-sensitive case. Checkpoint 2 validated schema shape and expected-file hashes, so that internal inconsistency could not be detected until Checkpoint 3 executed every normalized fixture through production semantics.

The Checkpoint 3 runner failed closed on the mismatch. The correction changes the fixture artifact to the actual cited M4 case rather than weakening equality, altering production semantics, or accepting the runner's output as a new oracle.

## Normalization rules

- Inputs and expected outputs remain UTF-8 JSON objects using normalized schema version `1`.
- Integer vertex order is preserved exactly.
- Authored recolor/retexture order is preserved exactly.
- No coordinate sorting, epsilon rounding, renderer-space conversion, or hidden pre-transform stage is permitted.
- Expected-output SHA-256 is calculated over the exact checked-in expected bytes, including the final newline.

## Regeneration boundary

This remains manual source-pinned migration evidence, not a claim of Java/deob executable generation. Checkpoint 4 owns executable oracle/regeneration adapters.
