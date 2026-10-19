# scripts guidance

## Owns

- Repository verification, architecture guards, feature navigation, performance/report tooling, fixture/oracle utilities, and local developer automation.

## Does not own

- Product/runtime behavior, duplicate semantic rules, or a second source of truth for architecture concepts already encoded in code/registry/specs.

## Invariants

- Keep tools deterministic, dependency-light, and runnable with the repository's documented Python version.
- Prefer one canonical parser/loader for machine-readable data. Do not let each script reinterpret the same file differently.
- Guards should explain actionable violations and ratchet debt downward rather than silently accepting growth.
- Generated docs must come from a clearly named machine source and have a drift check.
- Verification should escalate from cheap/static to focused build/test to full gate. Do not make the expensive gate the default inner loop.
- Never claim CI or test status that was not actually observed.

## Before adding a script

1. Search this directory for an existing command/parser that can be extended.
2. Reuse `feature_registry.py`, `editor_architecture.py`, `verify.py`, and other canonical helpers rather than copying logic.
3. Keep output useful to both humans and coding agents.
4. Add focused Python regression coverage for new parsing/guard behavior.

## Verification

Run the directly affected Python unit test or command first. `python3 scripts/verify.py --full --stage quick` is appropriate for a completed architecture/tooling checkpoint; the build stage is not required for documentation/parser-only changes unless they affect generated Rust/build inputs.
