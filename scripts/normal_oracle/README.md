# Pinned normal-method oracle

Python 3.11+, JDK 17+ and the source files in `sources.json` are required only for regeneration.
Ordinary Rust tests consume the accepted JSON fixtures offline.

```sh
python3 scripts/normal_oracle/regenerate.py --runelite-root /path/to/RuneLite-melxin --check
python3 scripts/normal_oracle/regenerate.py --runelite-root /path/to/RuneLite-melxin --output /tmp/new-normal-candidates
cargo test --locked -p osrs-reference --test integration m5_fixture_runner::
```

The adapter verifies full source SHA-256 pins before extracting the exact
`ModelData.calculateVertexNormals`, `calculateBounds`, `method5262` and `VertexNormal` copy
constructor bodies. They compile unchanged in a storage-only shell; a trivial superclass supplies
the height field, and holder classes supply the original normal fields. The Java driver translates
input and output only. No Rust implementation output contributes to expected values.

This isolates integer normal generation and pair reconciliation from scene traversal, cache decoding
and rendering. The source declares game revision 235; the selected RustOSRS cache is build 241.
Matching these fixtures does not prove scene-neighbor traversal or external visual parity.

`--check` compares exact parsed JSON values, preserving order, integers, null slots and absent arrays.
`--output` requires a new directory outside `reference-fixtures`, resolves symlinks and refuses
overwrite. Inspect candidates before accepting outputs and updating manifest hashes. Java never
receives an accepted output path. The five historical fixtures retain their original manual
normalization provenance; this oracle independently replays them.

The five new manifests record the SHA-256 of concatenated bytes of `regenerate.py`,
`NormalOracle.java`, `model_fields.java.txt`, `sources.json`, in that order. This pins the complete
adapter as well as its external sources; `test_normal_oracle.py` guards that revision.

The added cases cover signed integer normalization with mixed face types, two-vertex matches below
the hiding threshold, a flat neighbor with no eligible normals, signed XYZ translation, and duplicate
vertices contributing multiple pair matches without welding.
