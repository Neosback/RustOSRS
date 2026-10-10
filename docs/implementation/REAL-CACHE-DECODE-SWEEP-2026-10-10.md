# Real build-241 cache decode sweep (2026-10-10)

The M3 decoders were developed against synthetic fixtures. Sweeping the real pinned cache
(OpenRS2 2727, build 241) with `crates/osrs-reference/examples/cache_decode_sweep.rs` and
`object_decode_sweep.rs` found three disagreements, all now fixed.

| Family | Result before | Result after |
|---|---|---|
| object definitions (index 2 group 6) | 61,081 / 62,522 decode | **62,522 / 62,522** |
| terrain streams (index 5) | 0 / 2,936 decode (trailing flags byte) | **2,936 / 2,937** |
| underlays, overlays, textures, sequences, varbits, varps | all decode | all decode |
| location streams | 2,936 / 2,937 | 2,936 / 2,937 (4,980,884 locs) |
| models (index 7) | 62,043 / 62,043 | unchanged |

The remaining map square, region (98,199), contains 3-byte stub files in both terrain and
locations; it is not a real map and the client would not load it.

## Corrections

1. **Terrain trailing flags byte.** Real region files end with a flags byte; bit 0 marks a second
   64x64 block of tile-opcode streams that the client reads and discards
   (`class337.method7281`, `class148.method3945`). `decode_terrain` now consumes both.
2. **Object opcodes 42, 96, 100, 101, 102.** Added from the October 2026 `runelite-master`
   `ObjectLoader`/`EntityOpsLoader`: 42 `fullRecolor` (u16), 96 `raise` (u8), 100 sub-op
   (`index`, `subId`, text), 101 conditional op, 102 conditional sub-op. `full_recolor` and
   `ground_raise` are retained on `ObjectDefinition`; **their render semantics are not yet
   verified** (the pinned January deob predates them).
3. **Opcode 100 format.** `OpenRune-FileStore` decodes opcode 100 as a sub-id list terminated by
   zero; the real cache only parses completely with RuneLite's single-entry form. Three objects
   (29228, 29495, 29560, each with ~740 entity ops) distinguish the two.

## Source authority for build-241 definition formats

Use `runelite-master/cache` (October 2026) as the primary reference for definition byte layouts;
use `OpenRune-FileStore` as secondary and verify disagreements against real data. The pinned
January deob (`melxin/runelite@1ad572d7`) remains the semantic oracle for scene/model/terrain
algorithms but is **not valid for build-241 object or model definitions**
(`tools/deob-harness/src/ObjectDecodeProbe.java`: 57,661 of 62,522 definitions leave trailing
bytes).
