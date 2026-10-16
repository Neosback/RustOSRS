# Path census of the pinned build-241 map

Reproduce: `cargo run --release -p osrs-world --example path_census -- $RUSTOSRS_TARGET_CACHE_DIR target/census/paths.json` (about 4 s). It reads every map square (2,936 squares), plane 0 only, and analyses *path-like* overlay components. The JSON holds the full tables (overlay catalogue, neighbourhood prior, per-overlay underlays and widths); this page records what was measured and what it does and does not show.

**Definition.** A path-like component is a 4-connected set of tiles with one overlay id, at least 10 tiles, at least 6 medial-axis ("ridge") tiles, mean width at most 10 tiles and area / width² at least 3. Components touching a map-square edge are skipped (they are cut by the square). Widths come from a chamfer distance transform (`2·d − 1`), so they are approximate and favour odd values. Heights are the final vertex heights from the terrain loader. 3,321 components qualify. Overlay 42 (the hidden magenta overlay, 819k tiles) is not a path and must be ignored in any use of the catalogue.

## Findings

| Question | Result | Confidence |
|---|---|---|
| How wide are paths? | Median ridge width per component: 1 tile 58.5%, 2 tiles 7.9%, **3 tiles 21.7%**, 4 tiles 3.1%, 5 tiles 5.7%, 7+ tiles 3%. Narrow trails dominate; roads are mostly 3. By overlay it varies (overlay 228: 88% width 1; overlay 81: 42% width 3). | Medium: width estimator is approximate |
| Does width change along a path? | Yes. Coefficient of variation per component: median 0.24, p75 0.44, p90 0.59 (many 1-wide trails are constant). A global width cannot reproduce this. | Medium |
| How long are they? | Median 16 ridge tiles (p75 28, p90 56, p99 193); median area 47 tiles. | High |
| How are edges tiled? | 44.7% of boundary tiles are partial shapes against 4.7% of interior tiles. Given the 8-neighbour pattern (232 distinct patterns), the single most common (shape, rotation) explains **74.9%** of tiles. Straight edges (e.g. masks 199, 124, 31, 241) are **full tiles ~77% of the time**; partial shape 6 appears ~13%. Fully enclosed tiles are full 99%. | High (large counts) |
| How do paths end? | Medial-axis endpoints are a full tile (0,0) in 16,774 of 19,616 cases (85%): ends are square, not tapered. | Medium (ridge endpoints include 1-wide strips) |
| What lies beneath? | Underlay under path tiles: none 16.6%, same as the modal surrounding underlay 47.4%, a different one 36.0%. Overlays have typical partners but with spread (overlay 14: underlay 63 in 51%; overlay 11: underlay 96 in 33%; overlay 26: underlay 61 in 68%). | High |
| How do paths meet other overlays? | Pair-specific. Many pairs meet with full tiles on both sides ("hard", e.g. 445↔448 1,988 hard vs 6 soft); a few interlock with partial shapes ("soft", e.g. 110↔42). Majority hard. | Medium |
| Are paths graded (cut/filled)? | **No evidence of it.** Bed height minus the ground 4–6 tiles out: median 0, p25 −31, p75 +16 (units; 128 = a tile). Slope along path per tile: p50 0, p90 40, p99 226, against p90 24, p99 128 for terrain anywhere (paths run on steeper than average ground). Second difference along the axis: p75 0 against 8 anywhere (smoother in the bulk) but p99 240 against 136 (occasional abrupt steps). | High for "follows the ground"; the smoothing observation is weak |
| Are wide paths level across their width? | **No.** Height difference one tile either side of the axis (paths ≥ 3 wide): p50 0, p75 8, p90 208, p99 512, against p90 48, p99 232 for two vertices apart anywhere. Wide paths sit on steeper cross-slopes than ordinary ground, not flatter. | High |

## What this means for the Spline tool

- Real paths **follow the terrain**. Defaulting to a level, graded bed is not how this map is built; grading should stay off by default and be an explicit "fix this ground" action, not a style.
- The strongest rule in the data is the **edge-pattern prior**: choose the most common (shape, rotation) for each neighbourhood. It prefers full tiles on straight edges and square ends, and reproduces the real map's look better than maximising area match.
- Keep the **existing underlay** beneath a path unless the user chooses otherwise: that matches 47% of the real map and the other 36% were already different.
- Width should be **per point** (or a preset: trail 1, path 3, road 5), not one global value.
- Where a path meets a different overlay the real map mostly abuts with full tiles; a path should not blend into a non-path overlay.

## How the tool uses it

`scripts/gen_path_data.py` turns the census into `crates/osrs-editor/src/terrain_path/census.txt` (committed; the editor never needs the cache for it). The Spline tool then:

- **Occupancy first, then bitmask shapes.** The path is a route; the tool first decides which tiles carry the overlay by rasterising the band onto the tile grid (a tile counts when the band covers 45% of it, 30% for paths under two tiles wide so a thin diagonal stays connected; the centre line's tiles always count), then cleans wide paths (lonely spurs removed, one-tile pits filled). Each occupied tile's shape is then chosen from its 8-neighbour bitmask: the real map's most common (shape, rotation) for that neighbourhood, with neighbouring edges made to agree and the band's area a minor term (`PRIOR_WEIGHT` 30). On a battery of 15 drawn paths (`terrain_path/fitter/tests.rs`): wide paths have **no spurs and no pits** (2 spurs and 7 pits before), full tiles on straight edges 52% → ~73% against the real ~77%, tiles that are their neighbourhood's most common choice ~90%, fidelity to the drawn band ~0.91. Partial shapes on boundary tiles are ~30% against the real 45%: the tool is cleaner than the real map. A five-wide road at an arbitrary angle comes out as a regular staircase, five (six where a corner piece steps out) tiles across.
- **Presets.** Width presets Trail 1 / Path 3 / Road 5, per control point, with optional corners (round points curve, square points bend). Minimum width is one tile.
- **Crossing rules.** An overlay is a *path* when it has at least 10 path-like components and at least 10% of its tiles in path-like strips (60 overlays; the hidden magenta overlays never are). A path replaces path overlays and open ground, keeps the underlay beneath, and leaves every other overlay (water, floors, hidden overlays) alone: those tiles are shown in red and counted ("N left alone"). The 10% line is a judgement: lowering it to 5% would also treat large mixed overlays as paths; the data does not say where "road" ends and "plaza" begins.
- **Ground.** Paths follow the ground by default. "Smooth ground along the path" eases bumps along the path (profile smoothing 8 tiles, slope limit 40 units/tile = the real map's steepest-10% slope) and keeps the cross-slope, because real paths are not level across their width.

## Limits

Components are cut at map-square edges (long roads are counted as several), plane 0 only, bridges and stairs are not distinguished, and "path-like" is a geometric guess, not a label: some large rivers' banks or paving qualify. Widths are approximate. Nothing here says which overlays *should* be paths; it measures how the existing map uses them.
