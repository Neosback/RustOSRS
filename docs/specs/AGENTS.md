# OSRS specification guidance

## Owns

- Durable behavioral contracts for OSRS/cache/scene/render/editor semantics that implementation and tests are expected to preserve.

## Does not own

- Current branch status, temporary implementation plans, chat-derived assumptions, or unverifiable claims about upstream behavior.

## Invariants

- Separate observed/source-backed facts from editor policy and implementation notes.
- Cite or identify pinned evidence for non-obvious compatibility rules.
- Record coordinate spaces, plane rules, ordering, signedness, units, sentinel values, and lossy boundaries explicitly when relevant.
- A compatibility bug fix should update the spec when it reveals a missing rule, and should gain a regression fixture/test.
- Do not rewrite a spec merely to match current output without independent evidence.

## Before editing

1. Check `index.md` and relevant ADRs/research/fixtures.
2. Verify the implementation location that owns the rule.
3. Prefer one canonical spec section over repeating the same rule across documents.
4. Link specialized research rather than copying large evidence dumps into the normative contract.

## Verification

Documentation-only changes do not require a Rust build. Validate links/claims against the referenced source, fixture, or current implementation and run only an existing doc/static guard if one covers the change.
