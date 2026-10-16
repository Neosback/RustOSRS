# Canonical OSRS Semantic Specifications

Status: **Checkpoint 4 canonical specification set**

This directory contains the normative OSRS semantic contracts for RustOSRS. These files are implementation requirements for reusable OSRS crates only to the extent allowed by each specification's evidence status.

`docs/RUNELITE_*.md` files and `docs/research/` are research material. `docs/blueprint/09-SEMANTIC-AUDIT.md` and `10-TERRAIN-MATERIAL-PLANE-AUDIT.md` are source-audit records. This directory is where audited behavior becomes an implementation contract.

## Specification rule

Every atomic contract uses the following fields:

```text
SPEC: DOMAIN-NAME-NNN
Domain: OSRS_SEMANTIC
Status: VERIFIED | DERIVED | REVISION_SENSITIVE | ...
Target revision: exact snapshot or explicit gate
Primary evidence: repository + commit/blob + source symbol
Executable evidence: fixture/harness case, or REQUIRED when missing
Applies to: exact scope
Does not apply to: exclusions
Required behavior: normative behavior
Integer semantics: order/rounding/overflow requirements
Invariants: conditions that must remain true
Failure signature: expected symptom when wrong
Required tests: Rust/differential/golden ownership
Related specs: cross-links
```

`VERIFIED` means implementation may depend on the rule for the pinned target. It does not mean the behavior is guaranteed unchanged in future OSRS revisions.

`DERIVED` means the ownership/behavior contract is directly supported by the pinned source walkthrough but still lacks the executable evidence required for a `VERIFIED` claim.

`REVISION_SENSITIVE` means the contract is intentionally blocked from unconditional implementation. Code may implement it only behind a target-profile/revision guard with the evidence gap recorded.

## Canonical ownership

All specs in this directory are `OSRS_SEMANTIC` unless a file explicitly states otherwise.

Renderer choices such as reverse-Z, wgpu pipeline layouts, buffer organization, clip-space bias formulas, MSAA, anisotropy, zone batching, roof-removal UI, and editor camera feel belong to Checkpoints 5 and 6. They must not be smuggled into an OSRS semantic spec.

Likewise, runtime ownership is distinct from rendering: actor clocks, movement, var-driven morph resolution, effect lifetimes, projectile simulation and CS2 interpreter state are semantic/runtime concerns. A renderer consumes their resolved projection and does not become their authority.

## Target source anchor

The primary public deob anchor for the current semantic set is:

`melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc` (2026-01-28)

The checked-in `RuneLite-melxin/` tree mirrors the source used for the current actor/runtime walkthrough. The source declares game revision 235 while the selected RustOSRS cache is revision 241, so absence from the older source never silently removes newer decoder fields.

The existing local deob harness is corroborating evidence but is not yet reproducibly pinned. See `docs/verification/SOURCE-PINS.md`.

Imported October 2026 RuneLite sources may independently corroborate a behavior, but they remain a separate snapshot and may not silently upgrade a January rule to cross-revision truth.

## Specification registry

| File | Atomic specs | Status |
|---|---|---|
| `loc-placement.md` | `LOC-PLACEMENT-001` through `LOC-PLACEMENT-006` | Canonical |
| `model-build.md` | `MODEL-BUILD-001` through `MODEL-BUILD-005` | Canonical |
| `normals-lighting.md` | `NORMALS-001` through `NORMALS-004`, `LIGHTING-001` | Canonical |
| `morph-animation-contouring.md` | `MORPH-001`, `ANIMATION-001`, `CONTOUR-001` | Canonical |
| `actor-runtime-rendering.md` | `ACTOR-RUNTIME-001`, `ACTOR-MODEL-001`, `ENTITY-EFFECT-001`, `UI-CS2-001` | Source-audited / executable evidence required |
| `terrain.md` | `TERRAIN-001` through `TERRAIN-004` | Mixed verified/revision-gated |
| `planes-bridges.md` | `PLANES-001` through `PLANES-004` | Canonical |
| `face-materials.md` | `FACE-001` through `FACE-004`, `TEXTURE-001` | Canonical semantic metadata + reference oracle |
| `coordinates.md` | `COORD-001` through `COORD-003` | Canonical |

## Promotion gates

Before a new spec becomes `VERIFIED` it must have:

1. an exact ownership domain;
2. a reproducible source pin;
3. a stated target revision/profile;
4. exact integer semantics where relevant;
5. no unresolved contradiction in `docs/blueprint/02-CONTRADICTION-REGISTER.md`;
6. an executable fixture when practical, or a recorded required fixture when it does not yet exist;
7. a Rust test owner;
8. a failure signature.

A source citation without a test plan is incomplete. A screenshot without semantic evidence is insufficient. A renderer implementation detail is not an OSRS semantic rule.

## Implementation mapping

The intended ownership is:

- `osrs-core`: canonical definitions, coordinates, model/face metadata, morph inputs, pure integer helpers and reusable pose/model primitives
- `osrs-scene`: placement, footprints, bridge/storage-plane state, scene normal reconciliation/finalization
- `osrs-cache`: decode bytes into the fields consumed by these specs, preserving target-revision distinctions
- future `osrs-playtest`: deterministic actor/effect runtime, movement, sequence clocks, var context, projectile simulation and replay/source identity
- future UI/CS2 runtime: script/interpreter state, widget/client UI state and event execution
- `osrs-render`: consume the resolved semantic/runtime result without redefining it
- `osrs-reference`: differential/reference executors and golden fixtures
- `osrs-editor`: editing workflow only

No renderer or UI presentation crate may repair incorrect semantic state with visual offsets, rotations, hidden fallback model selection, shader-only behavior, renderer-owned animation clocks, or renderer-owned var/script state.
