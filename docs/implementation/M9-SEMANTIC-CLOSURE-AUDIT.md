# M9 Semantic Parity Closure Audit

Status: **PASS, merge gated on final exact-head CI and PR verification**

Branch: `impl/m9-semantic-parity-closure`

M8/main baseline: `23a2602ed62000a4843d7118b34427569ac176b9`

Parity-disposition head before this audit update: `e3fcaf255357d9a8a21cbcf79d00ad6cfdb953b7`

## Purpose

M9 is the hard semantic gate before `osrs-render` begins. This audit records which apparent gaps are true upstream semantic gaps, which rows belong to later renderer milestones, which revision-sensitive behavior remains blocked, and how deterministic semantic-scene identity is proven.

M9 does not implement renderer behavior early merely to make the parity matrix look complete.

## Checkpoint 1 closure

The first M9 checkpoint closed the two remaining upstream semantic coverage gaps identified by the row-by-row audit:

- `COORD-001`: exact world/region/local conversion, negative local coordinates, rotated footprint centers, and exact terrain vertex positions now execute together in `crates/osrs-reference/tests/m9_semantic_closure.rs`.
- `LOC-PLACEMENT-005`: the ownership-level contract now has a direct exact regression proving deterministic retention of collision operation identity, projectile blocking, clipping/model-clipping/ground-obstruction inputs, rotated definition footprint, and wall displacement metadata.

The canonical `LOC-PLACEMENT-005` specification explicitly does not require RustOSRS to invent the reference client's private collision/shadow/occlusion bit-array formulas before narrower contracts own them. M9 therefore closes the ownership/regeneration contract without claiming unimplemented private grid formulas.

## Target capability diagnostics

`crates/osrs-cache/src/capability.rs` exposes revision-gate diagnostics from the target profile as typed capabilities.

For the pinned build-241 target:

- extended object model IDs: `required`;
- object sound layout 220+: `required`;
- sequence layout 226+: `required`;
- texture layout 233+: `required`;
- terrain color builder: `blocked`, spec `TERRAIN-004`.

The M9 integration suite proves the terrain-color capability is surfaced as blocked and that no guessed implementation is silently substituted.

## Deterministic semantic scene identity

M9 adds `rustosrs-semantic-scene-v1` in `osrs-scene`.

The canonical hash:

- is SHA-256;
- is renderer-independent;
- hashes explicit fixed-width semantic values rather than debug strings;
- walks planes and tiles in deterministic plane/y/x order;
- includes source/storage plane identity;
- includes flat and shaped terrain semantic fields;
- includes fixed-layer loc identity and complete placement plans;
- includes game-object occupancy, footprint identity, edge masks, placement state, and scene instance identity;
- includes linked-below semantic tiles recursively;
- distinguishes absent values with explicit option tags.

This is a verification identity, not a persistence format. Future authoritative semantic fields must be deliberately incorporated into the hash contract before they are claimed to be protected by semantic-document identity checks.

### Pinned M9 golden hashes

| Golden semantic scene | `rustosrs-semantic-scene-v1` SHA-256 |
|---|---|
| composed terrain + floor decoration + dual boundary + wall decoration + game object | `50974f0232192dbd97c623d31bdbe208432ef64a13121c2852230afcc262f4a6` |
| four-plane linked-below column | `bf7a7876864142f75a29c47836f331da6cfb97b512d500274fd3c8035c3a296c` |

`m9_semantic_closure.rs` rebuilds each scene independently, requires exact structural equality, and requires both rebuilds to reproduce the pinned digest.

## Final parity row disposition

`docs/verification/PARITY-MATRIX.md` now reflects the M9 ownership decision:

| Row | M9 disposition | Reason |
|---|---|---|
| `COORD-001` | `EXISTING` | exact cross-layer semantic regression covers required tile/local, footprint-center, and terrain-position cases |
| `LOC-PLACEMENT-005` | `EXISTING` at ownership/regeneration contract | all currently normative definition/placement inputs required to regenerate side effects are retained deterministically; narrower private bit-grid formulas are not part of this spec |
| `TERRAIN-004` | `BLOCKED` | exact full terrain-color builder/oracle remains unpinned; typed target capability exposes the limitation |
| `FACE-001` | `DEFERRED-M10` | M10 owns semantic-to-render metadata preservation proof |
| `FACE-002` | `DEFERRED-M10/M12` | M10 owns exact ordered face preparation; M12 owns renderer realization |
| `FACE-003` | `DEFERRED-M10/M12` | M10 owns structural alpha interpretation inputs; M12 owns renderer realization |
| `FACE-004` | `DEFERRED-M12` | authored bias realization is renderer policy |
| `TEXTURE-001` | `DEFERRED-M10/M12` | CPU material/UV handoff belongs to M10 and GPU realization belongs to M12 |
| `PLANES-004` | `DEFERRED-M10` | renderer grouping must prove it cannot mutate semantic planes |
| renderer side of `COORD-003` | `DEFERRED-M10` | semantic coordinate separation already exists; render conversion is intentionally renderer-owned |

These renderer-owned rows are not knowingly deferred P0/P1 semantic bugs. Their required semantic inputs already exist upstream; the missing proof is at the owning render-extraction or GPU layer.

## C-003 / C-005 provenance review

M9 reviewed the mixed-snapshot and historical-local-path limitations rather than falsely marking them resolved.

`C-003` and `C-005` remain `REVISION_SENSITIVE` because the original developer-machine melxin checkout used for the historical harness is not known to be byte-identical to the pinned public revision.

That limitation does not contaminate M9 production promotion:

- current canonical semantic specs terminate in pinned public repository commits/blob identities, checked-in exact artifacts, or accepted project decisions;
- `SOURCE-PINS.md` byte-gates the checked-in historical `deob_golden.txt` and `Dumper.java` artifacts;
- the historical local checkout is classified only as corroborating evidence;
- ordinary CI does not depend on the old absolute path or local checkout;
- the M9 coordinate, placement ownership, capability, and semantic-hash tests do not read or invoke the unpinned historical tree.

Therefore the historical whole-snapshot equivalence gate remains explicit, while no M9 `EXISTING` promotion relies on that unresolved checkout as terminal authority.

## Permanent verification gate

Tier C includes an explicit M9 step:

```text
cargo test --locked -p osrs-scene --lib semantic_hash::tests
cargo test --locked -p osrs-reference --test m9_semantic_closure
```

This is in addition to the full M3 through M8 semantic parity stack.

## Pre-PR validation and branch scope

Checkpoint validation completed successfully on head `6ff8bbbbd4b56e5fa74f87a8fd898c379b9d80dd` in workflow `38054905546` before the final documentation-only parity/audit commits:

- Tier A: PASS, including architecture guardrails, regeneration safety, historical fixture index, rustfmt, locked workspace check, and strict clippy;
- Tier B: PASS, including the pinned semantic-scene digest regressions;
- Tier C: PASS, including M3 through M8 and the explicit M9 semantic closure/golden hash step.

After the parity-matrix update, the branch remained directly based on the M8 main baseline with no divergence:

- merge base: `23a2602ed62000a4843d7118b34427569ac176b9`;
- ahead: 14 commits before this audit update;
- behind: 0;
- changed files: 10;
- scope: M9 capability diagnostics, semantic hash implementation/tests, CI gate, lockfile dependency declaration, parity tracking, and M9 audit only.

No renderer, GPU, or editor implementation is present in the M9 branch.

Because this audit commit changes the exact branch head, Tier A/B/C must pass again on the final head before the PR is opened.

## Milestone PR gate

M9 may merge only if all of the following remain true:

1. final exact-head Tier A, Tier B, and Tier C pass;
2. branch remains based directly on `23a2602ed62000a4843d7118b34427569ac176b9` with zero commits behind;
3. exact PR file list matches the audited M9 scope;
4. PR head remains unchanged through verification;
5. PR-triggered CI passes on that exact head;
6. PR remains mergeable and base-stable;
7. squash merge uses an expected-head SHA lock;
8. post-merge `main` is verified and work stops before M10.

## Explicit non-claims

M9 does not claim:

- complete `TERRAIN-004` terrain-color parity;
- renderer face ordering or transparency realization;
- GPU texture/material realization;
- picking or editor behavior;
- broader skeletal animation execution outside the M8 verified scope;
- private collision/shadow/occlusion bit-grid formulas that do not yet have a narrower normative contract.
