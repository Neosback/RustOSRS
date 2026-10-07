# Blueprint Evidence Status Model

Status: **Checkpoint 1 foundation**

This document defines the vocabulary used by RustOSRS blueprint documents and future specifications. It exists to prevent research notes, generated documentation, renderer policy, and executed OSRS behavior from being treated as equivalent kinds of truth.

## 1. Evidence status

Every normative-looking claim must carry one of these statuses until it is promoted into a final specification.

| Status | Meaning | May implementation depend on it? |
|---|---|---:|
| `VERIFIED` | Proven against a pinned primary source and, where practical, an executable fixture or differential test. | Yes. |
| `DERIVED` | Logically derived from verified inputs, with derivation recorded and no contradictory primary evidence known. | Yes, but the derivation must remain reviewable. |
| `PROJECT_DECISION` | A RustOSRS architecture, renderer, UX, or product decision rather than inherited OSRS behavior. | Yes. |
| `RESEARCH` | Useful conclusion from existing notes that has not yet been independently revalidated for the canonical blueprint. | No. |
| `HYPOTHESIS` | Plausible but unproven behavior. | No. |
| `DISPUTED` | Existing sources or documents conflict, or a claim improperly conflates different mechanisms. | No. |
| `REVISION_SENSITIVE` | Behavior may be correct only for a particular source/cache/client revision. | Only with an explicit target-revision guard. |
| `DEFERRED` | Intentionally outside the current editor milestone, but not forbidden by the reusable foundation. | No current implementation requirement. |
| `OBSOLETE` | Superseded research or architecture that must not be used as design authority. | No. |

`VERIFIED` is not permanent. If the pinned source changes, the claim returns to `REVISION_SENSITIVE` or `RESEARCH` until its evidence is refreshed.

## 2. Semantic ownership

Every future spec also declares exactly one ownership domain.

### `OSRS_SEMANTIC`
Behavior inherited from the target OSRS data/client semantics. Accidental divergence is a correctness bug.

Examples:

- cache opcode meaning
- loc type dispatch and placement
- model selection and transformation order
- model/terrain integer math
- normals and any cross-model normal accumulation rules
- contouring
- terrain shape tables
- HSL packing and terrain color construction
- face alpha/priority/texture metadata
- morph and animation definition semantics
- bridge/plane behavior

### `RENDERER_POLICY`
RustOSRS rendering decisions that may intentionally differ while preserving OSRS scene meaning.

Examples:

- reverse-Z
- MSAA
- anisotropic filtering
- GPU buffer organization
- wgpu pipeline layout
- optional lighting presentation improvements
- frame scheduling and thread count
- diagnostic render modes

A renderer-policy decision must state whether it changes reference appearance and how semantic parity remains testable underneath it.

### `EDITOR_POLICY`
Editor workflow and UX owned entirely by RustOSRS.

Examples:

- eframe/egui shell
- Catppuccin theme
- docking
- gizmos
- undo/redo
- project persistence
- shortcuts
- autosave/recovery
- selection behavior

## 3. Evidence classes

A status answers "how sure are we?". An evidence class answers "what kind of thing supports the claim?"

1. **Executed oracle**: reproducible output from a pinned deob/reference implementation.
2. **Primary implementation source**: exact source path/blob/tree containing the behavior.
3. **Independent corroborating implementation**: a second implementation whose semantics are known to correspond.
4. **Generated reference**: generated API docs or indexes used for navigation.
5. **Research synthesis**: existing `RUNELITE_*.md` analysis.
6. **Project rationale**: architecture/UX reasoning owned by RustOSRS.
7. **Visual intuition**: useful during investigation, never sufficient on its own.

## 4. Minimum provenance record

Every promoted `OSRS_SEMANTIC` spec must record:

- target cache/client revision, where known
- repository tree/blob/commit used as evidence
- source path
- class/method/field/table or opcode
- whether the source is October 2026 RuneLite, January 2026 melxin/deob, OpenRune FileStore, rs-cache, or another source
- executable fixture/harness case, if one exists
- assumptions required to compare sources from different revisions
- expected failure symptom
- required Rust test ownership

Absolute developer-machine paths such as `/Users/...` are historical provenance only. They are not sufficient reproducibility metadata for the final blueprint.

## 5. Mixed-revision rule

The current corpus intentionally contains material from more than one snapshot. Therefore:

> Evidence from different revisions may corroborate a stable semantic rule, but it must never be silently merged into a single `VERIFIED` claim.

A cross-revision claim becomes `VERIFIED` only after one of the following:

1. the exact behavior is confirmed unchanged in both snapshots, or
2. the blueprint explicitly selects one snapshot as authoritative for that contract, or
3. a revision-gated spec describes the differences.

This applies especially to:

- deob scene construction versus newer RuneLite API/GPU code
- staged compute/priority shaders retained from an older tree
- cache opcode support across older `rs-cache` assumptions and newer FileStore/revision behavior
- texture/model id widths and definition fields introduced in newer cache revisions

## 6. Promotion gate

A research claim may become normative only when all applicable questions are answered:

1. What ownership domain is this claim in?
2. What exact revision/snapshot does it describe?
3. What primary source proves it?
4. Is there contradictory evidence elsewhere in the corpus?
5. Is the behavior semantic, or merely one renderer's implementation strategy?
6. Does integer order/overflow/rounding matter?
7. What fixture or differential test will fail if the Rust implementation drifts?
8. What visible or structural artifact results from getting it wrong?

If any answer is materially unknown, the status cannot be `VERIFIED`.

## 7. Specification status line

Future atomic specs use a header shaped like:

```text
SPEC: MODEL-NORMALS-001
Domain: OSRS_SEMANTIC
Status: VERIFIED
Target revision: ...
Primary evidence: ...
Executable evidence: ...
```

The body then records algorithm, inputs, outputs, invariants, exceptions, failure symptoms, and required tests.

## 8. Architecture rule

Reusable OSRS crates must encode `OSRS_SEMANTIC` behavior without depending on editor UI or renderer policy. The dependency direction will be finalized in Checkpoint 2, but this evidence model already forbids using eframe, egui, Catppuccin, wgpu, RuneLite plugin settings, or viewport preferences as semantic authority.