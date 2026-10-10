# Effective Parity Corrections, 2026-10-10

Status: **Normative overlay on `PARITY-MATRIX.md` until the next milestone matrix rewrite**

Source: `docs/implementation/REFERENCE-PROVENANCE-CORRECTION-2026-10-10.md`.

This file exists because M9 correctly recorded the implementation state known at the time, but one provenance conclusion was later disproven. Historical M5/M9 text is not rewritten as though the correction was known earlier.

## Effective row changes

| Contract | Previous effective wording | Correct current state | Owning next proof |
|---|---|---|---|
| `TERRAIN-004` source provenance | BLOCKED / revision-sensitive because builder source was thought missing | **VERIFIED source oracle**: `class470.method9712(WorldView)` at exact public pin | none for provenance |
| `TERRAIN-004` Rust production | BLOCKED together with source uncertainty | **REQUIRED**: production builder and executable differential fixture do not exist yet | dedicated semantic closure before reference visual parity |
| terrain target capability | blocked because implementation could not be promoted without source | **`source_verified`**: exact oracle is pinned; this does not claim production support | keep production coverage separate and promote it only after Rust builder/fixture exists |
| `PLANES-004` tile minimum plane | treated mainly as renderer/RuneLite grouping separation | **VERIFIED client scene behavior** for `Scene.setTileMinPlane`; RuneLite `maplevel` remains separate renderer policy | semantic exact fixture + renderer grouping fixture |
| `FACE-002` software priority oracle | verified, renderer deferred | unchanged oracle; **captured RuneLite GPU dispatch is conditional**, not universal | M10/M12 classifier/render-mode proof |
| `FACE-003` alpha sentinel scope | draw-time `-1` emphasized | **expanded** to include ModelData `-1 -> render type 2`, `-2 -> render type 3/gray 128`, plus separate draw-time `-1 -> 253` | production/extraction regressions |
| `TEXTURE-001` sampling | structural material/UV boundary | **expanded renderer reference** with 128x128 RGBA upload, zero-pixel transparency, alpha discard, brightness, filter/wrap state | M12 GPU material/sampling proof |
| reverse-Z compare | project `GreaterEqual` | **Reference profile uses strict `Greater`** to match captured RuneLite `GL_GREATER` | M11 depth fixture |
| P3 visual evidence | project Reference screenshot treated as reference golden | project-generated image is **regression evidence only**; independent parity claim requires external pinned raster oracle | M11/M12 external-oracle work |

## TERRAIN-004 closure rule

The target profile's `terrain_color_builder` state is `source_verified` because its exact source oracle is known. That state is a provenance classification, not an implementation-readiness flag.

Production coverage becomes `EXISTING` only when all of the following are true:

1. production Rust builder exists;
2. exact source-derived fixture covers the core builder path;
3. clipping/shadow-grid dependency is represented;
4. tile minimum-plane output is represented;
5. ordinary Tier C executes the production path;
6. active specs and parity matrix can then promote production coverage to `EXISTING`.

Until then, keep the capability state `source_verified` and the production status `REQUIRED` rather than conflating those two dimensions.

## Milestone consequence

M10 CPU renderer extraction can continue because this correction does not invalidate CP1-CP5 contracts. However, M11/M12 must not claim terrain visual parity while `TERRAIN-004` production remains REQUIRED.

The preferred sequence is:

1. finish bounded M10 structural work;
2. close `TERRAIN-004` production semantics with an exact fixture before or at the M10-to-M11 boundary;
3. use those deterministic terrain colors in the first reference viewport;
4. add an independent external visual oracle before using "1:1 visual parity" as a verified claim.
