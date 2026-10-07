# ADR-0002: Native-First Editor

Status: **Accepted**  
Date: **2026-10-07**

## Context

Existing port notes treated `wasm32-unknown-unknown` compatibility as a foundation requirement. The current product goal is a leading-class desktop map editor with strong filesystem workflows, diagnostics, GPU integration, and development velocity.

Requiring browser compatibility now would constrain threading, file I/O, diagnostics, windowing, and potentially GPU architecture before there is a committed web product requirement.

## Decision

RustOSRS is native-first.

- `osrs-editor` targets desktop first.
- Native filesystem, threading, diagnostics, and GPU features may be used when they materially improve the product.
- WebAssembly support is `DEFERRED`, not prohibited.
- Reusable semantic crates should remain portable where doing so is natural, but wasm compilation is not a hard acceptance gate unless a future ADR makes it one.
- Do not add browser fallbacks, wasm-specific scheduling, or compatibility abstractions merely for hypothetical future support.

## Consequences

Positive:

- simpler initial architecture
- fewer conditional code paths
- better ability to use native diagnostics and background asset loading
- no pressure to weaken desktop UX for browser constraints

Costs:

- a future web port may require adapters or targeted refactors
- portability is not automatically guaranteed for every crate

## Alternatives considered

### Native + wasm from day one

Rejected because there is no current product requirement that justifies the architectural cost.

### Explicitly forbid web support forever

Rejected because reusable lower-level crates may still be naturally portable and a future product decision could justify web support.

## Constraints/invariants affected

- F4 in `07-ARCHITECTURE-INVARIANTS.md`
- G1 in `07-ARCHITECTURE-INVARIANTS.md`

## Follow-up

If web delivery becomes a real requirement, create a new ADR that specifies:

- supported crates/targets
- file-source strategy
- threading/executor model
- browser GPU limitations
- editor UX differences
- CI target matrix

## Supersedes

The mandatory wasm requirement in `RUNELITE_RUST_PORT_NOTES.md`.
