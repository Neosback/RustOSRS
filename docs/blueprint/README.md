# RustOSRS Blueprint

This directory is the canonical home for the new RustOSRS architecture and specification program.

Existing root-level `RUNELITE_*.md` documents remain research inputs until individual claims are revalidated and promoted.

## Current blueprint documents

| Document | Purpose | Current status |
|---|---|---|
| `00-DOCUMENT-INVENTORY.md` | Inventory and classification of the pre-blueprint corpus | Checkpoint 1 complete |
| `01-EVIDENCE-STATUS.md` | Evidence/status vocabulary and promotion rules | Foundation |
| `02-CONTRADICTION-REGISTER.md` | Conflicts, revision hazards, and open questions that block normative specs | Active register |
| `03-SOURCE-GROUP-INVENTORY.md` | Group-level source, shader, fixture, tooling, and provenance inventory | Checkpoint 1 complete |
| `04-PROJECT-CHARTER.md` | Mission, product scope, quality gates, parity ownership | Checkpoint 2 complete |
| `05-SYSTEM-ARCHITECTURE.md` | Layering, data flow, ownership, determinism, mutation boundaries | Checkpoint 2 complete |
| `06-CRATE-ARCHITECTURE.md` | Reusable `osrs-*` crates and dependency rules | Checkpoint 2 complete |
| `07-ARCHITECTURE-INVARIANTS.md` | Enforceable architecture guardrails | Checkpoint 2 complete |
| `08-PARITY-MODEL.md` | Target profiles, truth categories, parity levels, exact-vs-tolerance rules | Checkpoint 2 complete |

Architecture decisions live in `docs/adr/`.

Accepted so far:

- `ADR-0001-reusable-osrs-crate-boundaries.md`
- `ADR-0002-native-first-editor.md`
- `ADR-0003-semantic-renderer-editor-boundaries.md`

## Planned blueprint set

The final system is expected to cover:

1. project charter and parity boundaries
2. truth/evidence model
3. system architecture
4. crate architecture and dependency rules
5. cache architecture
6. canonical scene architecture
7. rendering architecture
8. editor architecture
9. diagnostics and observability
10. verification strategy
11. implementation roadmap
12. atomic OSRS semantic specifications
13. architecture decision records
14. source pins and golden-scene/fixture indexes

## Canonical rule

A root research document may help locate evidence, but it cannot be the terminal authority for a final implementation requirement.

Final semantic specs must end their evidence chain in pinned source and/or executable reference behavior. Project-owned renderer/editor behavior must end in an explicit architecture decision.

## Work sequence

| Checkpoint | Scope | Status |
|---|---|---|
| 0 | Repository audit | Complete |
| 1 | Documentation/evidence inventory | Complete |
| 2 | Truth model + architecture | Complete |
| 3 | Rendering semantic audit | Not started |
| 4 | Canonical OSRS specifications | Not started |
| 5 | Rust/wgpu renderer blueprint | Not started |
| 6 | Editor blueprint | Not started |
| 7 | Verification blueprint | Not started |
| 8 | Implementation roadmap | Not started |
| 9 | Documentation reconciliation | Not started |
| 10 | Merge readiness | Not started |

## Current architecture decisions

Checkpoint 2 established:

- reusable lower-level crates are OSRS-owned, not editor-owned
- `osrs-scene` does not depend on a concrete cache implementation
- `osrs-render` consumes semantic truth rather than defining it
- `osrs-editor` is the first composition root and owns product workflow only
- `osrs-reference` is development/test infrastructure, never a production runtime dependency
- shared decoded assets are immutable by default
- semantic edits precede render invalidation/GPU updates
- revision-sensitive behavior must remain explicit through a target/parity model
- exact semantic tests cannot be replaced by screenshot tolerance
- native desktop is first-class; wasm support is deferred
- editor non-client scope does not prohibit future client reuse of shared crates

Checkpoint 3 is specifically responsible for source-level semantic verification. It must not silently resolve disputed behavior by adopting the old research prose.

## Current branch policy

All blueprint work is being accumulated on one branch:

`blueprint/osrs-editor-foundation`

No pull request should be opened until the complete blueprint branch is ready for merge.
