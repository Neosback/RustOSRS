# Terrain oracle fixtures

`lumbridge-3x3.oracle-in` / `lumbridge-3x3.oracle-out` are the input and output of the pinned-client
terrain oracle (`tools/deob-harness/src/TerrainOracle.java`) for the 3x3 regions around Lumbridge
(regions 49..51 x 49..51) in a 104x104 scene whose tile (0,0) is world tile (3176,3176).

* Source revision: `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`.
* Cache: OpenRS2 cache 2727, build 241, fingerprint `ae76dad7...fdb38`.
* The oracle executes the real `class264.loadTerrain`, `ScriptFrame.method749`, and
  `class470.method9712`. The `.oracle-out` header records the client's random hue/lightness walk
  (`rnd hue= lightness=`); the Rust test feeds those values back as explicit jitter.
* Texture averages are the oracle's deterministic stub `(id * 257) & 0xFFFF`.
* No loc placement was run, so `Tiles_underlays2` (shadow grid) is empty; shadow writes remain a
  separately owned input.

Regenerate (candidate-only; never overwrites):

```sh
cargo run --release -p osrs-reference --example export_terrain_oracle_input -- \
  "$RUSTOSRS_TARGET_CACHE_DIR" /tmp/lumbridge.oracle-in 3176 3176 \
  49,49 50,49 51,49 49,50 50,50 51,50 49,51 50,51 51,51
sh tools/deob-harness/run-terrain-oracle.sh --checkout PATH --expected-commit \
  1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc --bcprov bcprov-jdk15on-1.52.jar \
  --input /tmp/lumbridge.oracle-in --output /tmp/lumbridge.oracle-out
```

Consumed by `crates/osrs-reference/tests/terrain_oracle_diff.rs`.
