# ADR-0003: Semantic, Renderer, and Editor Boundaries

Status: **Accepted**  
Date: **2026-10-07**

## Context

The existing research corpus mixes OSRS behavior, RuneLite GPU implementation details, and editor-product recommendations. Without a formal ownership boundary, implementation work could accidentally treat reverse-Z, colorblind processing, a particular texture-array size, or UI camera defaults as OSRS semantic truth.

The inverse risk also exists: renderer code could silently reinterpret semantic data because a GPU-oriented representation is more convenient.

## Decision

Every normative requirement belongs to exactly one ownership domain:

- `OSRS_SEMANTIC`
- `RENDERER_POLICY`
- `EDITOR_POLICY`

The definitions and evidence rules in `docs/blueprint/01-EVIDENCE-STATUS.md` are authoritative.

Additional architectural rules:

1. `OSRS_SEMANTIC` data is represented independently of wgpu and editor state.
2. Renderer extraction may derive optimized representations, but those are disposable and cannot become the only copy of semantic information.
3. Renderer improvements must be explicitly identified as project policy.
4. Parity/reference rendering and enhanced/editor presentation must remain conceptually separable.
5. Editor tools mutate semantic document state first; render resources update afterward.
6. When a feature spans domains, its contract is split rather than assigned wholesale to one layer.

Example:

```text
face priority value/meaning          -> OSRS_SEMANTIC
algorithm used to submit/sort faces -> RENDERER_POLICY
priority debug visualization         -> EDITOR_POLICY / diagnostic UX
```

## Consequences

Positive:

- semantic correctness can be tested without a GPU
- renderer modernization is possible without redefining OSRS behavior
- UI/product decisions cannot accidentally become engine semantics
- future client reuse is straightforward

Costs:

- some features require multiple coordinated specs/ADRs
- more explicit conversion boundaries and diagnostic structures are needed

## Alternatives considered

### Treat RuneLite GPU behavior as the canonical engine architecture

Rejected because RuneLite includes plugin-specific renderer policy and implementation choices that are not synonymous with OSRS scene semantics.

### Treat rendered screenshots as the only truth

Rejected because multiple semantic errors can coincidentally produce similar images, while correct semantics can render differently under intentional presentation improvements.

## Constraints/invariants affected

- A1 through A4
- D1 through D4
- E1 through E4
- F1 through F3

in `docs/blueprint/07-ARCHITECTURE-INVARIANTS.md`.

## Follow-up

- Checkpoint 3 classifies disputed rendering-related claims by domain.
- Checkpoint 4 creates atomic semantic specs.
- Checkpoint 5 chooses concrete wgpu renderer policy.
- Checkpoint 6 defines editor UX on top of the same semantic state.
