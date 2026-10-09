# Reference fixture regeneration

This directory owns **development-only** regeneration entry points. Ordinary Rust tests and CI consume checked-in fixtures offline and do not execute public-source oracles.

## Candidate-only rule

Regeneration never writes directly into `reference-fixtures/`.

The explicit command stages a candidate artifact outside the checked-in fixture tree:

```sh
python3 tools/reference-fixtures/regenerate.py \
  --adapter melxin-deob-golden \
  --checkout /path/to/melxin-runelite \
  --bcprov /path/to/bcprov-jdk15on-1.52.jar \
  --candidate /tmp/rustosrs-deob-golden.candidate.txt
```

The `melxin-deob-golden` adapter is pinned to:

`melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`

The lower-level deob harness validates the checkout HEAD again before compiling or running anything.

## No hidden network dependency

The adapter does not clone repositories and does not download Maven artifacts. The caller provides:

- an existing source checkout at the exact pinned commit;
- the compile-only `bcprov-jdk15on-1.52.jar` path;
- a candidate output path outside `reference-fixtures/`.

This keeps source acquisition and third-party dependency acquisition explicit and reviewable.

## Accepting regenerated evidence

A candidate is **not** automatically authoritative and there is no `--accept` mode.

To promote regenerated evidence into the repository:

1. inspect the candidate against the currently checked-in source-derived evidence;
2. confirm the oracle/source pin and harness revision that produced it;
3. normalize only according to the owning fixture contract;
4. update the expected artifact explicitly;
5. update its `expected_sha256` explicitly;
6. update the fixture manifest/source or harness provenance when the generator/source changed;
7. run the repository-wide M5 fixture runner and normal Tier A/B/C CI.

This separation is intentional. A regeneration command cannot silently rewrite expected outputs, update hashes to bless its own output, or mutate provenance in the same execution.

## Current adapter boundary

Checkpoint 4 provides the pinned raw `melxin-deob-golden` adapter needed for later historical-golden migration. The three Checkpoint 2/3 model fixtures remain source-pinned manual normalizations and are **not** falsely advertised as executable Java-generated fixtures.

Checkpoint 5 owns indexing/migration of useful cases from `deob_golden.txt` and additional priority semantic families.
