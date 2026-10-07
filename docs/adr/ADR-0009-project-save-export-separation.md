# ADR-0009: Project Persistence Is Separate from Target Map Export

Status: **Accepted**  
Date: **2026-10-07**

## Context

RustOSRS needs durable editor projects, crash recovery, semantic history, region workspaces, source-cache provenance, and eventual output to target OSRS/cache/map formats.

Treating the project file as if it were the target cache format would either lose editor metadata/history/recovery capabilities or contaminate OSRS data with editor-specific concerns. Treating export as equivalent to project save would also make dirty-state and failure handling ambiguous.

## Decision

RustOSRS distinguishes:

1. **Project save**: persists the editor's versioned project artifact and current semantic working state/reference payload.
2. **Autosave/recovery**: independently captures recoverable generations of the editor project.
3. **Target export/write**: validates and encodes an immutable semantic document generation into a selected target OSRS/cache/map representation.

The project retains source-cache fingerprint/revision provenance and must detect changed source identity on reopen.

Project save and target export maintain separate dirty/generation status.

Export may fail because a semantic state is not representable by the target profile even though the editor project itself can still be saved safely.

## Consequences

### Positive

- editor-only annotations/preferences do not leak into game data;
- unsupported target features can block export without risking loss of editor work;
- source cache changes are detectable;
- crash recovery can be robust without modifying source cache files;
- export can run from immutable snapshots while editing continues.

### Costs

- users need clear UI distinction between Save and Export;
- project schema migration is required over time;
- provenance/fingerprint management becomes explicit;
- implementation needs separate project and export validation paths.

## Alternatives considered

### Edit target cache/map files directly as the only project state

Rejected because it weakens recovery, undo/history, provenance, multi-region workspaces, and safe experimentation.

### One monolithic editor format that replaces OSRS cache semantics

Rejected because reusable semantic crates and exports must remain compatible with actual target formats.

### Automatically export on every project save

Rejected because export can be expensive, revision-gated, or invalid while the project itself remains perfectly saveable.

## Constraints / invariants affected

- F2: project files do not replace cache semantics.
- C2: revision encoding is normalized at ingestion.
- H3: silent correction is prohibited.

## Follow-up work

- select project serialization format/schema in implementation roadmap;
- define source-cache fingerprint algorithm;
- define target export adapters;
- implement explicit saved-generation and exported-generation UI;
- design recovery/journal retention policy.

## Supersedes / superseded by

None.
