# M5 Priority Fixture Migration Record v1

Status: `SOURCE_PINNED_MANUAL_NORMALIZATION`

This record defines the first canonical `FACE-002` priority-order fixture from the pinned public deob source. It is **not** generated from `reference-fixtures/deob_golden.txt`; the historical monolithic dump has no face-priority emission-order section.

## Oracle source

- repository: `melxin/runelite`
- commit: `1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`
- file: `runescape-client/src/main/java/Model.java`
- blob: `c2aa55c0e8fea89fae0da33d782119f8c109cacf`
- symbol: `method5946`

`method5946` groups already depth-bucketed faces by priorities `0..11`, computes three integer thresholds, interleaves priorities `10/11` around priority bands `0`, `3`, and `5`, and then drains any remaining special-priority faces.

Thresholds are:

- `avg12 = (sum(priority 1 depths) + sum(priority 2 depths)) / (count1 + count2)`
- `avg34 = (sum(priority 3 depths) + sum(priority 4 depths)) / (count3 + count4)`
- `avg68 = (sum(priority 6 depths) + sum(priority 8 depths)) / (count6 + count8)`

The implementation uses Java integer division. Special-priority processing starts with priority `10`; priority `11` is not consulted until priority `10` has been exhausted, even if a priority-11 face has a larger depth.

## Crafted normalized input

The fixture uses one face in each ordinary priority `0..9`, four priority-10 faces, and four priority-11 faces. Depth buckets are deliberately chosen so the thresholds differ:

- priority 1 depth `90`, priority 2 depth `70` -> `avg12 = 80`
- priority 3 depth `60`, priority 4 depth `40` -> `avg34 = 50`
- priority 6 depth `30`, priority 8 depth `10` -> `avg68 = 20`

Priority-10 depths are `100, 55, 25, 5`.
Priority-11 depths are `95, 52, 22, 2`.

The face IDs are stable input-order IDs `0..17`. The fixture also carries representative alpha metadata, including ordinary alpha `64` and raw signed sentinel `-1`, but `FACE-002` ordering does not reinterpret alpha.

## Exact source-derived output

Following `Model.method5946` exactly produces:

`10, 0, 1, 2, 11, 3, 4, 12, 5, 6, 7, 8, 9, 13, 14, 15, 16, 17`

Diagnostic thresholds:

- `avg12 = 80`
- `avg34 = 50`
- `avg68 = 20`

The late priority-11 faces are intentional. The routine drains priority `10` before switching to priority `11` rather than globally merging the two special queues by depth.

## M5 execution boundary

The normalized fixture is admitted as `evidence_only` during M5 because the renderer-owned ordered face path is not implemented yet. Ordinary M5 CI still validates its schema, provenance, expected SHA-256, and deterministic inventory inclusion.

A later renderer milestone must execute the production priority-order implementation against this exact fixture before `FACE-002` can advance to `EXISTING`.

Do not treat this record as evidence that the historical developer-machine deob checkout was byte-identical to the public commit. Authority for this fixture is the pinned public `Model.java` blob above.