# RustOSRS Development Handoff

Last updated: 2026-10-15

`ARCHITECTURE.md` is the authoritative architecture contract. This file records the current integrated baseline, branch discipline, validation state, known hazards, and next allowed boundary.

## Project invariant

`MapDocument` / `EditorSession` is authoritative edited map state. World-worker edits, rendered geometry, GPU resources, picker data, previews, and UI state are projections only.

If every projection vanished, the complete edited map must still exist correctly in the semantic session.

## Mandatory development protocol

These rules apply to all continuing development unless the user explicitly changes them.

1. Keep exactly one active development branch/system at a time.
2. Do not start a second feature/refactor branch while the current branch is active.
3. Break each subsystem into small, reviewable checkpoints.
4. Stop after checkpoints unless the user explicitly directs continuation or completion/merge.
5. Update `DEV_HANDOFF.md` as checkpoints close so repository state, exact SHAs, validation, hazards, and the next boundary stay current.
6. Keep a subsystem on the same branch across checkpoints unless there is a concrete reason to close it.
7. Do not mix unrelated architecture, renderer/117HD, cache, or UI work into the active subsystem branch.
8. Before merging a completed subsystem, validate the exact final branch head with Tier A/B/C CI.
9. **Merge the current subsystem into `main` before opening any new development branch.**
10. After merge, validate exact integrated `main`, record the result here, and certify the final handoff-bearing `main` head if the documentation update itself advances `main`.
11. Do not create the next branch until the prior subsystem is merged, final `main` is green, and the user explicitly directs the next subsystem.
12. One logical user gesture produces at most one logical semantic transaction/history entry.
13. Failed projection preparation must not leave semantic authority and visible projection knowingly divergent.
14. Exact SHA matters. A green run certifies only the SHA it tested.
15. New or fully refactored production Rust files remain under architecture guard limits. Do not introduce debt exceptions for convenience.

The user explicitly reinforced rule 9 on 2026-10-15 after noticing multiple refs while `main` had not advanced. Treat merge-before-new-branch as a hard operating rule.

## Current integrated baseline

Latest integrated subsystem:
PR #51 - `refactor(editor): zone-scope certified Height invalidation`

Certified final Height branch head:
`55de16566a1a46337b678239ab8d9b352d5348f2`

Branch CI:
`38696356751` / #1256 - SUCCESS.

Integrated Height merge commit on `main`:
`fb2fd2874b18d11bef689fa1954751bcdc64273b`

Post-merge integrated `main` CI:
`38696859752` / #1258 - SUCCESS.

That post-merge run passed:

- Tier A architecture boundaries and architecture guard
- reference regeneration safety
- historical deob golden index
- rustfmt
- workspace compile
- strict Clippy
- WebAssembly portability
- Tier B full workspace tests
- Tier C M3 checked-in decode fixtures and deterministic fuzz
- M4 fixture inventory, exact model construction, and fuzz
- M5 normalized fixture runner
- M6 semantic scene suite
- M7 normals/lighting/finalization
- M8 morph/contour/animation/replacement
- M9 semantic closure and golden hashes

After the merge certification, `docs/plans/HEIGHT-ZONE-INVALIDATION.md` was updated on `main` in docs-only commit:
`5e266b104fd471725bbfa027c90d92d6f0f20a8a`

This handoff refresh itself creates the final docs-only `main` head. That exact resulting SHA must receive a final CI pass before another development branch is allowed. Do not make another documentation commit merely to self-record that SHA; report it externally after CI.

## Repository branch/PR state

- PR #51 is merged.
- There are no open PRs.
- No new development branch has been created after the Height merge.
- `tmp-noop-should-not-create` points to old H2 snapshot `0fbd4d907004e9b6b0353842f9bec9f9ff963c2b`.
- Comparison `tmp-noop-should-not-create...main` showed the temporary ref is **0 commits ahead and 27 commits behind** integrated `main` before the docs refresh.
- Therefore the temporary ref contains no unmerged work and is not an active subsystem.

If branch-deletion capability becomes available, `tmp-noop-should-not-create` may be deleted as cleanup. Its existence must never be treated as permission to develop on it.

## Completed Height zone invalidation subsystem

Goal achieved:
Height edits can now use exact 8x8 projection-zone invalidation when dependency metadata proves every affected output, while preserving conservative region fallback whenever correctness cannot be proven.

### H1 - dependency audit

COMPLETE AND CERTIFIED.

Head:
`73694a9d97fd2845c8ed818400430709d6c704c5`

CI:
`38688385096` / #1221 - SUCCESS.

Conclusion: a universal fixed Height radius was not proven safe across placement, contouring, dynamic models, and downstream scene relationships.

### H2 - dependency metadata contract

COMPLETE AND CERTIFIED.

Head:
`c59eea2cadffbc5861f6ecf8c138a9a0b6b11f99`

CI:
`38689828881` / #1227 - SUCCESS.

Delivered canonical Height source identity, deterministic source-to-zone mapping, freshness stamps, explicit complete/incomplete state, producer-region validation, and fail-closed lookup/merge semantics.

### H3 - runtime integration and planner

COMPLETE.

Delivered:

- canonical `HeightVertexCoord` in finalized footprints, including `SourcePlane`
- canonical Height identity through semantic Undo/Redo replay
- worker-produced `HeightDependencySnapshot` from full contextual scenes
- terrain corner and slope-light dependency capture
- rotated loc placement midpoint dependency capture
- static lit-model contour dependency capture using actual final model X/Z radius
- dependency installation only from accepted/current worker results
- freshness tied to projection epoch and presentation generation
- exact-zone planning only when every required resident non-empty producer is complete/current
- mandatory region fallback for absent, stale, incomplete, unsupported, Object-mixed, or otherwise unproven cases
- full contextual scene assembly unchanged
- semantic authority retained in `EditorSession`

### H4 - adversarial coverage

COMPLETE.

Coverage includes:

- 8x8 zone edges
- 64x64 region edges
- negative Euclidean zone ownership
- rotated/multi-tile placement midpoint sampling
- contour sample expansion/clamping
- deterministic dependency deduplication
- missing/stale/incomplete metadata fallback
- semantic Undo/Redo canonical Height footprints
- stale in-flight build rejection
- cancelled older result not consuming a newer request

`ModelData` normal-reconciliation and posed animated/dynamic contour cases intentionally remain fail-closed. They do not receive false exact-zone certification.

### H5 - exact-head certification and merge

COMPLETE.

Final branch head:
`55de16566a1a46337b678239ab8d9b352d5348f2`

Branch CI:
`38696356751` / #1256 - SUCCESS.

PR #51 merged as:
`fb2fd2874b18d11bef689fa1954751bcdc64273b`

Post-merge CI:
`38696859752` / #1258 - SUCCESS.

## Integrated projection/edit architecture

`main` now includes:

- central semantic `EditRuntime` execute/replay gateway
- Paint, Height, Paste, Path, Objects, semantic Undo, and semantic Redo through the gateway
- receipt-driven projection invalidation
- pure terrain 8x8 zone invalidation
- Height exact 8x8 zone invalidation when complete/current dependency proof exists
- conservative Height region fallback for unproven cases
- full contextual scene assembly with zone-scoped final static geometry emission
- stale asynchronous region build rejection by request identity, projection epoch, and presentation generation
- accepted-build Height dependency freshness tied to the same projection identity

## Remaining major roadmap

Each item below is a separate future subsystem and therefore a separate future branch. The current subsystem must always be merged before the next one starts.

1. Object dependency footprint and zone invalidation.
2. `InteractionRuntime` behavioral encapsulation.
3. `ProjectRuntime` ownership extraction.
4. `UiRuntime` ownership extraction.
5. `DiagnosticsRuntime` ownership extraction.
6. Reference renderer parity gates and external visual oracle work.
7. 117HD dynamic/placed lights.
8. 117HD sky/day-night/reflection systems and remaining effects.
9. Edited-cache fingerprint/profile strategy, new-group support, and broader index repack when justified.
10. Project/browser lifecycle polish and future import/merge product work.

## Cache/export current state

Map stream encoding and existing map-group cache-copy repacking are implemented. Known limitations remain:

- only existing map-square groups are repacked
- new groups absent from the reference table are not created
- other indices are not generally rewritten by the editor export path
- an edited cache copy intentionally no longer matches the bundled pristine-cache fingerprint/profile

## Runtime ownership current state

Completed ownership boundaries:

- `EditRuntime`
- `RenderRuntime`
- `WorldRuntime`
- `InteractionRuntime` field ownership

Future behavioral/ownership extraction remains separate work.

## 117HD current high-level state

Already substantially implemented:

- render-profile/HDR foundation
- HD data layer/material tables
- HD terrain/model extraction
- material replacement
- ground blending and normals
- environments/fog/directional lighting
- directional shadows and filtering
- water surfaces, flow/normal maps, foam, Fresnel, underwater tint/caustics
- animated-loc HD material routing
- material flow maps
- parallax/displacement and displaced shadow position

Large remaining systems include dynamic/placed point lights, full sky/day-night/reflection systems, remaining post-processing/settings parity, and remaining model/material edge cases.

## Next allowed boundary

**STOPPED AFTER HEIGHT SUBSYSTEM MERGE.**

Before any new development branch may be created:

1. certify the exact handoff-bearing `main` head created by this documentation refresh
2. confirm no open PRs and no unmerged work on temporary refs
3. report the integrated state to the user
4. wait for explicit user direction naming or authorizing the next subsystem

The likely next architecture item is Object dependency footprint and zone invalidation, but do not create that branch until the user explicitly directs it after this final `main` certification.
