# osrs-reference guidance

## Owns

- Reference fixtures, independent parity evidence, comparison helpers, and integration validation against pinned sources/oracles.

## Does not own

- Production semantics or runtime behavior.
- Helpers that production crates must import to make the application work.

## Invariants

- `osrs-reference` is validation-only. Production crates must never depend on it.
- Keep reference inputs pinned, reproducible, and clearly distinguished from implementation output.
- Prefer semantic hashes/structures plus visual evidence where appropriate; screenshots alone are not sufficient proof of exact behavior.
- A discovered OSRS compatibility bug should become a fixture/regression instead of an undocumented special case.
- Do not weaken a reference merely to make the current implementation pass.

## Before adding code

1. Identify the independent source/oracle and pin it precisely.
2. Check for an existing fixture family before creating another harness.
3. Keep expected/reference generation separate from code under test.
4. Document prerequisites for tests that require external cache/source data.

## Verification

Run only the affected fixture/parity test while iterating. Full parity/workspace verification belongs at integration checkpoints, not every reference-data edit.
