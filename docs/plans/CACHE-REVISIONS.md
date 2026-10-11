# Cache revisions: how OpenRune does it, what we do, what to add

Status: findings + first slice built (`osrs-cache::build_format`, 2026-10-11). Scope: **builds 240 and up**.
Sources: `OpenRune-Server` (server, `or-cache`), `OpenRune-FileStore-main` (the `dev.or2` cache/codec library the
server depends on), OpenRS2's `caches.json`.

## 1. How the OpenRune server handles a revision change

* **One number, one place.** `game.yml` has `revision: 240.2` (major.minor). `readRevision()` parses it into
  `(major, minor, environment)`; the same major feeds the cache builder, every codec constructor
  (`ObjectCodec(revision)`, `NPCCodec(revision)`, ...), the gameval handler, the network layer (it refuses to
  start if `RSProtConstants.REVISION != config.revision`) and the MCP tools.
* **Changing revision = edit that line, run `FRESH_INSTALL`.** `FreshCache` finds the matching cache in
  OpenRS2's `caches.json` (`findRevision(revision, subRev, OLDSCHOOL, environment)`), downloads `disk.zip`
  (plus `keys.json` only below 237), builds, then clears the incremental state so the next pack starts clean.
  The downloaded zip is cached under a per-revision directory.
* **Format differences are `if (revision >= N)` inside the codecs.** The complete list (everything else is
  identical across versions):

  | build | where | what |
  |---|---|---|
  | 210 | NPC | old render-priority/ops layout (<=210) |
  | 220 | object, NPC, sound | sound layout, ops layout |
  | 223 | NPC | render priority encoding |
  | 226 | sequence, sound | sound effect list layout |
  | 230 | gamevals, CS2 | gameval archive exists, CS2 packing allowed |
  | 232 | texture, NPC op 111, gameval components | old texture layout, component group v2, varcs group |
  | 237 | maps, params, identity kit, spot anim, components, entity ops, xteas | numeric map group `(x<<8)\|y` with files 0 (tiles) / 1 (locs), no XTEA, param type flag/id, extended ops |
  | 238 | world map | numeric groups for details/composite/geography |

  **For 240 and up every one of these is already on its "new" side.** The server's revision logic therefore
  does nothing between 240 and 241: it only matters when a *new* layout change appears (so far none above 238).
* **Verification and incremental packing.** `cacheTool { incremental = true; incrementalDatabase(...);
  verification = FINGERPRINT | OUTPUT_CRC }` keeps a state DB per cache kind (`incremental_live`,
  `incremental_server`), repacks only what changed, and `PackedMapSquares.record(x, y)` doubles as a "changed
  squares" log.
* **Gamevals.** From 230 the cache itself carries names for objs, npcs, locs, seqs, spotanims, varbits,
  components, dbtables and so on (archive 24). The server dumps them to `.rscm` files and a `max-ids.toml`
  (custom ids must be above the cache max), and generates typed Kotlin tables from them.

## 2. What we already do (and where we are stricter)

* One validated **target profile per cache** (`profiles/*.yaml`) with a verified fingerprint, provenance on every
  decoded definition, and **revision gates that must be `required`**; decoders fail loudly rather than fall back
  to an older layout (`resolve_map_square` refuses builds < 237, texture/sequence decoders refuse a profile that
  does not require their gate). This is stricter and safer than a bare integer.
* The cost: a new cache needs a new profile, and nothing checked that a profile's gates agreed with its build.

## 3. Built now: `osrs-cache::build_format`

* `MIN_SUPPORTED_BUILD = 240`, a `FORMAT_CHANGES` table (the list above, oldest first, each entry naming the
  profile gate it maps to), and `validate_profile_build`, called from `TargetProfile::validate`.
* A profile with build < 240, or whose declared gates disagree with its build (dropped, or not `required`), is
  rejected at load. Tests: the committed 241 profile is consistent; 240/241/242/250 accept the same gate set;
  239 is rejected; a dropped or `blocked` gate is rejected.
* The 237 map layout constant stays in `decode/map.rs`; 240+ never takes the old path.

## 4. Adding revision N (240, 242, ...)

OpenRS2 has weekly live caches for 238-241 (ids 2533..2735; 240 = 2649..2720, last 2720 on 2026-09-23; 241 = 2727,
2735). None has XTEA keys (not needed from 237). Steps, to become `cargo xtask cache add --build N`:

1. Pick the cache id from `caches.json` (latest live id for that build, or an explicit id).
2. Download `disk.zip`, extract, compute the `rustosrs-cache-v1` fingerprint.
3. Write `profiles/osrs-live-N-DATE-openrs2-ID.yaml` from the metadata (timestamp, groups, sizes, empty indices)
   and the gate set `build_format` derives for N.
4. Run the decode smoke tests against it (`m3_decode_fixtures`, `m4_target_probe` made profile-parametric).
5. If a decoder fails, that is a **new layout change**: add a `FormatChange` entry, fix the decoder, keep older
   supported builds green.

**Not done yet (needs your yes):** downloading the 240 cache (~190 MB). Without it the 240 profile cannot carry a
verified fingerprint, and we cannot find out whether 240 -> 241 changed any layout (the server says no).

## 5. Ideas worth taking from the server

1. **Profile auto-detection by fingerprint**: open a cache dir, fingerprint it, pick the matching profile from
   `profiles/`; unknown fingerprint -> "no profile for this cache, run `cache add`". Removes
   `RUSTOSRS_TARGET_CACHE_DIR` + a hard-coded profile.
2. **Gamevals (archive 24)**: decode names for locs/objs/npcs/seqs/spotanims/varbits/components. Gives the editor
   the internal name next to every id (`TREE`, `CRATE_...`), name search in the object library (WORKSPACE-V2
   step 7), better inspector text, and `.rscm` export compatible with server scripts.
3. **Incremental derived data**: our model repository, world-map/minimap tiles and HD texture atlas rebuild from
   scratch. Key them by group CRC/fingerprint (the server's `FINGERPRINT` verification) and rebuild only changed
   groups; the editor's save step reports "changed map squares" the way `PackedMapSquares` does.
4. **Map pack/export**: `PackMaps` shows exactly what the writer must emit for 237+: group `(x<<8)|y`, file 0 =
   terrain, file 1 = locs, no XTEA. That is WORKSPACE-V2 step 8; port its encoding with a round-trip test
   against the decoder (decode -> encode -> byte-identical for untouched squares).
5. **Collision flags overlay**: the server's `CollisionFlagMap`, `BlockWalk`, `LocInfo` (shape/layer/angle) and
   `ObjectServerType` (`blockWalk`, `blockRange`, `forceApproachFlags`) are the reference for the flags channel
   and a walkability overlay in the paint tool.
6. **Object/NPC/item sprite factories** (`ObjectSpriteFactory`, ...): the model for rendering library
   thumbnails from our own renderer into an atlas, cached by id + revision.
7. **Wiki/examine text** (`.data/raw-cache/examines`, osrs-mcp wiki tool): examine strings for the inspector.
8. **Type dumper / MCP**: a read-only `cache_search` over defs (name, id, category) is what a Cmd+K "go to
   object/npc/item" needs; same index as the gamevals.
9. **Dev-ergonomics**: `max-ids.toml` (reserve custom id space above cache max) matters if the editor ever
   writes custom definitions.

Order: (1) `cache add` tool + 240 profile, (2) gamevals decode, (3) derived-data keys, then the map writer with
WORKSPACE-V2 step 8.

## 6. Status (2026-10-11): 240 and 241 both supported, opened by fingerprint

* `osrs_cache::registry` bundles one profile per supported build; `CacheRepository::open_matching` fingerprints a
  folder and opens it under the matching profile (an unknown cache is reported, not guessed). The editor, the
  floor palette and the asset workers go through it; nothing hard-codes 241 any more.
* `make_profile` (example) writes a profile from a downloaded cache; `cache_diff` compares two caches per
  index. Build 240 (OpenRS2 cache 2720, 2026-09-23) was downloaded, profiled
  (`profiles/osrs-live-240-2026-09-23-openrs2-2720.yaml`, 117 560 groups) and tested: Lumbridge renders
  byte-identically to 241, the Reference and HD pixel goldens, all world and render tests pass on it, and the
  floor census (2 936 map squares, 476 overlays, 221 underlays) matches.
* 240 -> 241 differs by 148 groups of 117 560 (configs 9, sprites 17, clientscripts 97, sound effects 14,
  models 3, gamevals 5, interfaces 1, binary 3): no map, texture, model-layout or definition-layout change, as the
  OpenRune server's revision gates predicted.
* The editor has File > Open cache folder... (system dialog), a recent list, remembers the last cache, and shows
  a first-run card (with the supported builds and a link to OpenRS2) when no cache can be opened.
