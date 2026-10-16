# Cache Decoder Landscape — FileStore vs rs-cache, Opcode Tables, Verdict

Question answered: for `editor_cache` (Rust), do we use
`/Users/tylercovalt/Documents/rs-cache-master`, port OpenRune-FileStore, or
hybrid? Verdict up front: **use rs-cache as the running foundation and port
the missing decoders into it, with FileStore's opcode tables as the spec**
(§5). Everything below was verified first-hand (sources read, rs-cache test
suite executed: 30/30 pass offline).

## 1. What rs-cache actually is (verified)

- Single crate `rs-cache 0.9.0` (jimvdl), memmap-based read-only cache I/O
  (`Cache::new(path)`, `read(index, archive)`, checksums, Huffman, ISAAC),
  filesystem delegated to `rune-fs 0.2.0`. OSRS badge says rev 180 — treat as
  "old-rev-era opcodes", not as a rev ceiling (format code is largely
  rev-agnostic; see gaps).
- Loaders (all tested working): `ItemLoader`, `NpcLoader`, `ObjectLoader`,
  `MapLoader`, `LocationLoader::load(id, keys: &[u32; 4])` (XTEA keys are a
  load-time parameter — matches our out-of-band-keys plan).
- `MapDefinition { region_x/y, data[z][x][y]: { height: u8, attr_opcode,
  settings, overlay_id: i8, overlay_path, overlay_rotation, underlay_id } }`
  + `region_base_coords` + `blocked_tiles`; `Location { id: u32, loc_type: u8,
  orientation: u8, pos: (x, y, z) }` — exactly the tuple the R1 dispatch needs.
- Tests: `cargo test --offline` → 30/30 pass (basic, checksum, loader incl.
  Lumbridge locations + bank_table/dungeon_door/furnace, read). It runs today.

## 2. Object opcode coverage: rs-cache vs FileStore (verified lists)

FileStore `ObjectCodec.kt` handles:
`1,2,5,6,7,14,15,17,18,19,21,22,23,24,27,28,29,39,40,41,42,61,62,64–75,77,92,78,79,81,89,60,82,91,93,90,95,96,30–34(base actions),100–102(entity/sub/conditional ops),249(params)` —
plus `revisionIsOrAfter(revision, 220)` gates on newer sound fields.

rs-cache `obj_def.rs` handles:
`0,1,2,5,14,15,17,18,19,21,22,24,27,28,29,30–34,39,40,41,62,64–75,77,78,79,81,82,92,249,23(skip)`.

rs-cache gaps that matter for rendering (all confirmed absent):
- `6,7` — 32-bit model IDs (newer caches; rev-240 objects can reference them).
- `42` recolAll, `60/61` mapAreaId/category.
- `77/92` read **varp only — no varbit, no `transforms` default handling**
  (`multiVarBit` doesn't exist in rs-cache): varbit-driven multilocs
  misresolve. This alone disqualifies drop-in use for morph rendering.
- `89–96,100–102` newer sound/entity-op fields (sounds don't render; entity
  ops don't either — listed for completeness, safe to defer).
- Whole decoders absent: **models (`ModelData`), textures/materials,
  overlay/underlay defs, sequences/spotanims** — only item/npc/obj/map/loc
  definitions exist.
- Cosmetic but telling: `decord_displacement` typo (vs `decorDisplacement`
  everywhere else) — do not propagate the typo into Rust names.

FileStore gaps: none found in render-relevant opcodes (the earlier suspected
missing opcode 24 was a grep-truncation artifact — `24 → animationId`
exists with 65535→−1 handling).

## 3. Render-relevant defaults/values (single table, both stacks agree)

`decorDisplacement` 16 · `clipType` −1 (0 = full warp, N×256 = partial) ·
`sizeX/sizeY` 1 · `modelSizeX/Y/Z` 128 (no-op when all 128) ·
`offsetX/Y/Z` 0 · `contrast` raw byte (engine ×25, R20) · `ambient` raw ·
`interactType` 0 · `animationId` −1 (65535 sentinel) · underlay/overlay/texture
`id+1` (0 = none) · `recolAll` −1 · `supportsItems` derived (solid≠0) ·
`TEXTURE_SIZE` 128, `TEXTURE_COUNT` 256, 8 mips, RGBA8 array.

## 4. Map/location decode notes for the Rust port

- Terrain `MapData.height` is `u8` in rs-cache — verify range against rev-240
  maps before trusting it for slope shading (R11 needs full int heights;
  FileStore's `Landscape` path is the cross-check).
- `overlay_id: i8` (−1 = none) vs FileStore `id+1` convention — normalize at
  the `editor_cache` boundary, don't leak both conventions.
- `LocationLoader` already threads XTEA keys per region — keep that shape;
  source keys out-of-band for 237+ (main MD C10).
- Texture average-RGB (R13 textured overlays) has no rs-cache home (no texture
  defs) — port FileStore's `TextureType.averageRgb` + `TextureCodec` first;
  it's on the overlay critical path.

## 5. Verdict: hybrid (use + port)

1. **Depend on rs-cache** for cache I/O + item/npc/obj/map/loc definitions
   (it compiles offline, passes its suite, matches our loader shapes).
2. **Port into `editor_cache`** from FileStore specs, in this order:
   transforms-with-varbit (77/92 + `multiVarBit`), 32-bit model IDs (6/7),
   `recolAll`/category/mapAreaId, `ModelData` decoder, texture defs + average
   RGB, overlay/underlay defs, sequence/spotanim defs.
3. **Keep FileStore as spec + tooling**: opcode tables above, TOML
   dump/edit round-trips, `OpenRS2` fetch, Displee for cache surgery. Never
   re-derive an opcode from the deob when FileStore already pins it.
4. Fix the `decord_displacement` typo at the port boundary; keep
   `decorDisplacement` everywhere in Rust.
