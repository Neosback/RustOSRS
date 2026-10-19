# Architecture and AI foundation merge stopping point

Date: 2026-10-19

Branch: `refactor/architecture-ai-foundation`

Baseline main: `ff7a1be7020f6202987beaef2c792b569c8164da`

## Decision

This is the deliberate integration boundary for the architecture and AI foundation cleanup. It supersedes the **current-status markers only** in `ARCHITECTURE-AI-FOUNDATION.md` for this merge decision. The original document remains the long-term roadmap and design record.

The branch is stopping after the CP2 semantic-authority/history foundation reached a coherent, machine-guarded boundary. CP3 through CP11 remain useful future roadmap work, but they are explicitly deferred to future branches and are not blockers for this stopping-point merge.

## Completed scope at this boundary

- CP0 cleanup contract and incremental-refactor rules
- CP1 feature discovery, canonical feature registry, generated feature index, and scoped `AGENTS.md` guidance
- CP2A authority audit and ownership gates
- CP2B transient stroke preview separated from semantic history
- CP2C retired legacy history replay/payload authority
- CP2D immutable `MapReplayFootprint`
- CP2E authoritative editor replay descriptors
- CP2F generic replay projector foundation
- CP2G active replay driven from semantic descriptors with the legacy replay-footprint argument removed
- CP2H parallel application chronology/marker stacks removed
- CP2I obsolete tool-specific replay projection paths removed
- CP2J authority ratchets and reproducible architecture dashboard baseline

The branch also includes the final mechanical formatting pass and the remaining stale replay-test caller update needed for repository-wide validation.

## Deferred scope

CP3 through CP11 in `ARCHITECTURE-AI-FOUNDATION.md` are intentionally deferred. Future work should start from updated `main` on a new branch and re-audit live state before continuing the roadmap.

This merge does **not** claim that projection ownership, renderer parity hardening, performance instrumentation, deterministic repro bundles, headless automation, or MCP work is complete.

## Merge-readiness gate for this stop

Before merging this boundary:

1. the temporary CP2 validation workflow must not land on `main`;
2. architecture, authority, and feature-index guards must pass;
3. repository formatting must pass;
4. native/wasm Clippy and the repository test gate must pass through the normal CI workflow;
5. the PR must remain mergeable against current `main`;
6. the resulting `main` SHA must be verified after merge.

After merge, retire the cleanup branch before starting the next long-lived architecture branch.
