# Height Zone Invalidation Plan

Status: COMPLETE, MERGED, AND POST-MERGE CERTIFIED.

Integrated PR: #51 - `refactor(editor): zone-scope certified Height invalidation`

Certified final branch head:
`55de16566a1a46337b678239ab8d9b352d5348f2`

Branch CI:
`38696356751` / #1256 - SUCCESS.

Integrated `main` merge commit:
`fb2fd2874b18d11bef689fa1954751bcdc64273b`

Post-merge `main` CI:
`38696859752` / #1258 - SUCCESS.

## Goal

Narrow finalized Height edit projection work from the conservative region-ring policy to 8x8 world zones only when runtime dependency metadata proves every affected projection zone.

Semantic ownership does not change. `EditorSession` remains authoritative and 8x8 zones remain disposable projection/cache granularity only.

## Safety invariant

For every Height semantic transaction, zone-scoped invalidation is permitted only when the selected zone set is a proven superset of every projection output that can change because of the touched Height vertices.

If that proof is absent, stale, incomplete, unsupported, or otherwise untrusted, the conservative region fallback is mandatory.

## Delivered architecture

- Finalized Height footprints preserve canonical `HeightVertexCoord`, including `SourcePlane`.
- Semantic Undo/Redo reconstruct the same canonical Height identity.
- The world worker extracts `HeightDependencySnapshot` from the full contextual `WorldScene` before optional zone-filtered static geometry emission.
- Terrain corner and slope-light reads are represented.
- Static loc placement midpoint samples are represented from rotated definition footprints.
- Static lit-model contour dependencies use actual model X/Z radius sampling.
- `ModelData` normal-reconciliation cases fail closed because cross-model dependency closure is not yet proven.
- Posed animated/dynamic contour cases fail closed when active pose bounds can change the Height read-set.
- Accepted dependency metadata is tied to request identity, projection epoch, and presentation generation.
- Source replacement, presentation rebuild, unload, and stale-result rejection prevent old metadata from remaining trusted.
- Exact zones are used only when every required resident non-empty producer has complete/current metadata.
- Missing, stale, incomplete, unsupported, Object-mixed, or otherwise unproven cases retain conservative region invalidation.
- Full contextual scene assembly remains unchanged.
- Semantic authority remains in `EditorSession`.

## Checkpoints

### H1 - dependency audit and protocol

COMPLETE AND CERTIFIED.

Certified content head:
`73694a9d97fd2845c8ed818400430709d6c704c5`

CI:
`38688385096` / #1221 - SUCCESS.

Conclusion: a universal fixed Height radius was not proven safe. Fine-grained invalidation therefore required explicit dependency metadata with conservative fallback.

### H2 - dependency metadata contract

COMPLETE AND CERTIFIED.

Certified code/test head:
`c59eea2cadffbc5861f6ecf8c138a9a0b6b11f99`

CI:
`38689828881` / #1227 - SUCCESS.

Delivered canonical source keys, deterministic source-to-zone mapping, producer-region validation, freshness stamps, explicit complete/incomplete state, fail-closed lookup/merge behavior, and boundary/freshness tests.

### H3 - runtime integration and zone planner

COMPLETE.

Delivered canonical Height receipts, worker-produced dependency snapshots, accepted-build installation, freshness enforcement, exact-zone planning when proven, and region fallback otherwise.

### H4 - adversarial boundary and replay coverage

COMPLETE.

Coverage includes:

- 8x8 zone edges
- 64x64 region edges
- negative Euclidean ownership
- rotated/multi-tile placement midpoint sampling
- contour sample expansion/clamping
- dependency deduplication
- missing/stale/incomplete metadata fallback
- semantic Undo/Redo Height footprints
- stale in-flight build rejection
- cancelled older build results not consuming newer requests

Dynamic/morphed contoured models remain deliberately fail-closed rather than falsely certified for exact-zone invalidation.

### H5 - certification and merge

COMPLETE.

Exact final branch head:
`55de16566a1a46337b678239ab8d9b352d5348f2`

CI `38696356751` / #1256 passed:

- Tier A architecture boundaries and guard
- reference regeneration safety
- historical deob golden index
- rustfmt
- workspace compile
- strict Clippy
- WebAssembly portability
- Tier B full workspace tests
- Tier C M3-M9 semantic/reference parity

PR #51 merged to `main` as:
`fb2fd2874b18d11bef689fa1954751bcdc64273b`

Post-merge CI `38696859752` / #1258 passed the same Tier A/B/C gate set on exact integrated `main`.

## Branch discipline after completion

The Height subsystem is integrated. No new development branch should be created until `DEV_HANDOFF.md` records this merged state and that handoff-bearing `main` head is certified.

After that certification, the next subsystem may start only on explicit user direction, and it must again follow the merge-current-before-new-branch rule.
