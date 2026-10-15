# RustOSRS Development Handoff

Last updated: 2026-10-15

`ARCHITECTURE.md` is the authoritative architecture contract. This handoff records the certified integrated baseline, completed subsystem state, remaining roadmap, and next allowed boundary.

## Project invariant

`MapDocument` / `EditorSession` is authoritative edited map state. World-worker edits, rendered geometry, GPU resources, picker data, previews, animation instances, dependency/proof snapshots, export inspection state, and UI state are projections only.

If every projection vanished, the complete edited map must still exist correctly in the semantic session.

## Mandatory development protocol

1. Keep exactly one active development branch/system at a time.
2. Do not start a second feature/refactor branch while another subsystem is active.
3. Break each subsystem into reviewable checkpoints.
4. Update this handoff with exact SHAs and validation as checkpoints close.
5. Keep one subsystem on the same branch until complete.
6. Do not mix unrelated architecture, renderer/117HD, cache, or UI work into an active subsystem branch.
7. Before merge, validate the exact final branch head with Tier A/B/C CI.
8. **Merge the current subsystem into `main` before opening any new development branch.**
9. After merge, validate exact integrated `main`, update this handoff, and certify the final handoff-bearing `main` head if docs advance it.
10. Exact SHA matters. A green run certifies only the SHA it tested.
11. One logical user gesture produces at most one logical semantic transaction/history entry.
12. Failed projection preparation must not knowingly leave semantic authority and visible projection divergent.
13. Do not introduce architecture-guard debt exceptions for convenience.

## Current repository state

Active development subsystem: **none**.

Open subsystem PR: **none; verified map export PR #54 is merged**.

No next development branch has been created.

The exact integrated export merge commit is green across Tier A/B/C. This handoff commit advances `main` once more and therefore must receive its own exact-head Tier A/B/C certification before another subsystem starts.

## Latest completed subsystem: verified map cache export and read-only inspection

Merged PR:
#54 - `feat(export): verified map cache export and inspection`

Feature branch:
`Neosback/verified-map-export`

Current-main baseline used for the final feature comparison:
`d23f3e5d59f0e5b6522448897f6956d629be30e6`

Final feature branch head:
`83aacecdae29926d2cbc6f85577a173093d269cd`

Final branch CI:
`38715679736` / #1317 - SUCCESS across Tier A/B/C.

Merge commit on `main`:
`ad548d524976114b8d70f695f81f4ba0ad256468`

Integrated-main CI:
`38716729696` / #1319 - SUCCESS across Tier A/B/C.

The final branch was 9 commits ahead and 0 behind current `main` before PR #54 merged. Generated Python `__pycache__` artifacts found in the incoming work were removed before final certification.

## Verified export subsystem result

The editor now has a fail-closed export path that can write edited map data into a staged cache copy, prove the written result, and reopen only a specifically validated edited export for isolated inspection without weakening ordinary pristine-cache matching.

### Captured semantic authority

Export preparation captures immutable inputs on a worker:

- source cache
- target profile
- `MapDocument`
- semantic edit revision
- authored resident regions
- project diffs that are still pending because their regions have not streamed yet

Resident authored state supersedes duplicate pending projection records for the same region. Pending-only project regions are reconstructed against pristine source data on scratch state, so export does not force them into residency or mutate authoring history.

A source/project replacement changes the export epoch. Superseded jobs and stale edit revisions cannot install success into the current export runtime.

### Preflight validation

Every candidate map square is rebuilt against the pristine source square before any output is published.

Preflight rejects or blocks at least:

- authored regions missing from the source cache
- project diff records that write outside their owning region
- removed objects absent from the pristine baseline
- terrain channels that cannot be represented by the supported map format
- out-of-range floor IDs / invalid terrain flags
- heights that cannot be stored exactly by the target map encoding
- missing terrain/location files in an existing map group
- unsupported target profile/source changes during preparation

Prepared terrain and location streams are decoded again and diffed against the captured expected semantic `MapDocument`. Publication cannot proceed when the encoded streams differ from captured semantics.

### Existing multi-file map groups are preserved

`SourceSquares` reads every logical file of an existing map group, not only terrain file 0 and locations file 1.

Export replaces only files 0 and 1 and retains files 2+ exactly. This is important for build-241 map groups that carry additional files.

### Staged publication

Publication writes into a unique staging directory beside the chosen destination.

Safety properties include:

- destination must be empty
- destination must not overlap the source cache folder
- cache-copy export copies only the JS5 cache files
- loose export writes independent map-group containers
- write or verification failure removes staging and does not publish partial output
- a concurrently populated destination is preserved rather than overwritten
- source cache bytes remain unchanged
- successful publication is a final directory rename after verification

### JS5 cache-copy repack

Cache-copy mode repacks only existing groups and keeps affected reference-table metadata consistent.

For each replaced group it:

- preserves the original compression mode
- preserves whether the container had a two-byte version suffix
- appends a new sector chain instead of overwriting the old chain in place
- updates CRC-32
- updates compressed/uncompressed sizes when the reference table carries them
- increments the group version
- rewrites the affected reference table using its original compression
- preserves unaffected groups and reference tables byte-for-byte

Group-version increment is now checked. `u32::MAX` fails with `VersionOverflow` before the group or reference table is moved, rather than wrapping or panicking.

New groups are still intentionally unsupported because the current writer only replaces rows already present in the source reference table.

### Export manifest and independent verification

A cache-copy export writes `rustosrs-export.json` using format `rustosrs-map-export-v2`.

The manifest records:

- mode
- profile
- pristine source fingerprint
- semantic edit revision
- complete output cache digest
- changed map regions/groups
- prepared payload SHA-256
- terrain/height/object change counts

`VerifiedExport::validate` then checks the exported cache against the original cache and manifest before an inspection capability can be created.

Verification proves at least:

- manifest format/mode is supported
- source fingerprint/profile match the actual original cache
- output digest matches complete stored cache bytes
- content-index inventory is unchanged
- non-map reference tables are unchanged
- unlisted groups are byte-identical to the original
- listed changed groups correspond to valid source map groups
- changed group payload matches its prepared manifest digest
- preserved logical files above IDs 0 and 1 are unchanged
- reference-table CRC/version/size metadata exactly matches the permitted group rewrite
- cache files do not change during verification

Tampering with non-map groups, preserved map files, manifest region identity, output bytes, or reference-table metadata causes verification failure even if a caller recomputes the overall output digest.

Verified export validation currently fails closed for map reference-table rows carrying hash or Whirlpool columns rather than pretending those metadata can be reproduced safely.

### Strict pristine matching remains strict

An edited cache copy intentionally has a different physical fingerprint from its pristine source.

`CacheRepository::open_matching` still rejects an arbitrary edited directory against the pristine profile. The subsystem does **not** weaken normal profile matching.

A changed export can be read through normal cache/world consumers only through the private `VerifiedExport` capability created after full validation. The capability stores the validated output fingerprint and file stamps, and reopening fails if the files change later.

The `VerifiedExport` source capability is native-only; it is not exposed on `wasm32`.

### Read-only export inspection

The editor can inspect a validated cache-copy export in an isolated `EditorApp` viewer.

Inspection:

- uses the verified export source
- starts at authored content when available
- copies presentation preferences/camera context only
- forces select mode
- provides camera navigation, picking, selection, focus, and inspector UI
- does not route authoring tools, semantic edit commands, scripts, context-menu mutation, save, or export commands
- parks the original authoring application and returns to it without changing its document, history, dirty ownership, or semantic revision

The settlement acceptance test covers create/edit, Undo/Redo, project save/reopen, export before pending project regions stream, verified inspection, semantic comparison, preserved extra group files, and return to untouched authoring state.

### Export-focused regression coverage

The merged subsystem includes regression coverage for:

- stale/superseded worker results
- source/project replacement epochs
- persistent worker failures and disconnected workers
- pending unloaded project regions
- live authored authority over duplicate pending records
- cross-border semantic edits
- exact unstorable-height reporting
- invalid pending-region ownership
- floor-ID truncation blockers
- semantic stream round-trip before publication
- cache-copy and loose-mode semantic equivalence
- preservation of files 2+
- staged cleanup after write/verification failure
- occupied/overlapping/concurrently populated destinations
- captured-revision immutability while later edits continue
- strict pristine fingerprint matching
- verified export capability reopening
- later file mutation invalidating a verified token
- unlisted non-map modifications
- preserved-file tampering
- invalid/duplicate/nonexistent manifest groups
- incorrect reference-table metadata
- JS5 sector-chain round trips and independent vendored-reader agreement
- group version overflow failing before mutation

There is also an extended real build-241 cache-copy/read-through acceptance test. It is `#[ignore]` in ordinary CI because it requires `RUSTOSRS_TARGET_CACHE_DIR` pointing at the pinned real target cache.

## Previous completed subsystem: Object dependency proof and zone invalidation

Merged PR:
#52 - `refactor(editor): prove Object zone invalidation by accepted output`

Object branch:
`refactor/object-zone-invalidation`

Final Object branch head:
`86c724cd67add8ec96b011effc258cd0256f489c`

Final branch CI:
`38708254817` / #1302 - SUCCESS across Tier A/B/C.

Merge commit on `main`:
`e8f1102739345b171ee216cf47996fb2f055b114`

Integrated-main CI:
`38708782538` / #1304 - SUCCESS across Tier A/B/C.

Object invalidation is exact-zone capable without relying on a guessed dependency radius. Exact 8x8 static installation is used only with complete/current accepted producer-output proof; otherwise Object edits retain the conservative 3x3 producer-region fallback.

Canonical Object mutation footprints preserve exact removed/added `ObjectPlacement` identity and Undo/Redo polarity. Full contextual producer candidates, request identity, projection epoch, presentation generation, Height dependencies, Object proof freshness, picker/inspection state, and animation state all remain tied to accepted builds.

## Integrated edit/projection architecture

Current `main` includes:

- central semantic `EditRuntime` execute/replay gateway
- Paint, Height, Paste, Path, Objects, semantic Undo, and semantic Redo through the gateway
- receipt-driven projection invalidation
- pure terrain 8x8 zone invalidation
- Height exact 8x8 zone invalidation only with complete/current dependency proof
- conservative Height region fallback for unproven cases
- Object exact 8x8 static installation only with complete/current accepted producer-output proof
- conservative Object 3x3 producer-region fallback for unproven cases
- full contextual scene assembly with zone-scoped final static geometry installation
- stale asynchronous region-build rejection by request identity, projection epoch, and presentation generation
- accepted-build Height and Object proof freshness tied to the same projection identity
- exact canonical Object removed/added mutation footprints through semantic history
- immutable export snapshots over semantic authority and pending project state
- verified export inspection isolated from semantic authoring state

## Runtime ownership status

Completed boundaries:

- `EditRuntime`
- `RenderRuntime`
- `WorldRuntime`
- `InteractionRuntime` field ownership

Future ownership work:

- `InteractionRuntime` behavioral encapsulation
- `ProjectRuntime`
- `UiRuntime`
- `DiagnosticsRuntime`

## Reference renderer remaining work

Known remaining or partially stale audit areas include:

- side-grid shadow exactness
- minPlane/originalPlane behavior
- GPU depth/raster semantics
- alpha modes
- sampler behavior
- external visual oracle/gates

## 117HD current high-level state

Already substantially implemented:

- render-profile/HDR foundation
- HD data/material tables
- HD terrain/model extraction
- material replacement
- ground blending/normals
- environments/fog/directional lighting
- directional shadows/filtering
- water surfaces, flow/normal maps, foam, Fresnel, underwater tint/caustics
- animated-loc HD material routing
- material flow maps
- parallax/displacement and displaced shadow position

Large remaining systems include:

- dynamic/placed point lights
- sky/star/moon/aurora/day-night systems
- sky reflection/water integration
- remaining post-processing/settings parity
- wind/character displacement and remaining model/material edge cases

## Cache/export current state

Implemented:

- locations exact 2936/2936
- terrain 2935/2936 retained semantics
- five-file map-group preservation/repack
- semantic export of existing map groups in loose and cache-copy modes
- immutable preparation over authored plus pending project state
- semantic decode/diff round-trip before publication
- staged cache-copy publication with failure cleanup
- JS5 group/reference-table CRC, size, version, compression, and suffix preservation
- full output digest + per-group payload manifest
- independent original-vs-export verification
- native verified-export read capability without weakening pristine matching
- isolated read-only export inspection

Known limitations:

- only existing map-square groups are repacked
- new groups absent from the reference table are not created
- only map index 5 is rewritten by the editor export path
- hashed/Whirlpool map reference-table rows fail closed in verified export validation
- edited cache copies intentionally do not match the pristine-cache fingerprint/profile through ordinary matching
- verified export capability is native-only
- the pinned real build-241 end-to-end export acceptance test requires external cache data and is ignored in ordinary CI

## Remaining major roadmap

Each item is a separate future subsystem and therefore a separate branch after the final handoff-bearing `main` head is certified and the user explicitly directs continuation:

1. `InteractionRuntime` behavioral encapsulation
2. `ProjectRuntime` ownership extraction
3. `UiRuntime` ownership extraction
4. `DiagnosticsRuntime` ownership extraction
5. Reference renderer parity gates and external visual oracle
6. 117HD dynamic/placed lights
7. 117HD sky/day-night/reflection systems and remaining effects
8. cache new-group support, broader index repack, and any future ordinary profile/fingerprint strategy beyond verified inspection
9. project/browser lifecycle polish and future import/merge product work

## Next allowed boundary

**STOP AFTER FINAL HANDOFF-BEARING `main` CERTIFICATION.**

Do not create a new development branch until:

1. the exact `main` head containing this handoff is green across Tier A/B/C
2. PR #54, the export merge, integrated-main certification, and final handoff certification are reported to the user
3. the user explicitly directs the next subsystem
