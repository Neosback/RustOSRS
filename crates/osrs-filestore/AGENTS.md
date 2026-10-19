# osrs-filestore guidance

## Owns

- Low-level JS5 sector/index storage, container compression, and XTEA/storage primitives.
- Byte-accurate transport/storage operations beneath `osrs-cache`.

## Does not own

- OSRS map/model semantics, editable state, UI, rendering, or world orchestration.
- Feature-specific cache interpretations that belong in `osrs-cache`.

## Invariants

- Treat bytes, offsets, sector chains, index metadata, compression, and encryption as format contracts.
- Keep APIs small and transport-focused.
- Do not leak editor or renderer concepts downward.
- Fail explicitly on malformed storage rather than silently manufacturing semantic defaults.

## Before adding code

1. Search existing sector/container/index helpers.
2. Confirm the concern is raw storage rather than archive semantics.
3. Preserve exact binary behavior and error boundaries.
4. Prefer focused fixtures over generalized abstractions with unclear format ownership.

## Verification

Use the smallest storage/container test first. Run `python3 scripts/verify.py -p osrs-filestore` only for crate-wide changes.
