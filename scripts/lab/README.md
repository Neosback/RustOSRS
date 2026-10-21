# Local procedural learning

The native editor's Prefab Studio has an isolated Edit session and a Lab view. Source and target
artifacts are local JSON; no service credentials or external training runtime are needed.

Build the tool server with `cargo build --locked -p osrs-lab`. Generate a first task with:

```sh
target/debug/osrs-lab task-tavern path/to/kit.json 7 > target/tavern-task.json
/opt/homebrew/bin/python3 scripts/lab/train.py reconstruct \
  --task target/tavern-task.json --method imitate --out target/tavern-actions.json
/opt/homebrew/bin/python3 scripts/lab/train.py reconstruct \
  --task target/tavern-task.json --method search --seed 7 --out target/search-actions.json
```

`imitate` is the visible-target curriculum baseline. It emits ordinary paint, height and object
gestures and is not a benchmark policy. `search` receives the current state, legal palette and scalar
score only. Its seeded bounded trials are an initial baseline, not a trained neural agent. Import
an action trace in Studio Lab to step or play it with the real renderer.

`osrs-lab serve` consumes one JSON request per line and writes one response per line. Requests
accept `env` (default 0); independent environment IDs and request arrays permit batching.
Commands: `reset` with task filename/seed/mode (`visible` or `score_only`), `observe`, `legal`,
`step` with tagged `action`, `undo`, `replay` with actions, and `learn` with reviewed symbolic grids.
Errors are explicit JSON responses. Invalid attempts consume budget and remain in the replay.
The target is omitted entirely in `score_only` observations. The trusted task loader and generated
curriculum are host infrastructure, never agent tools. Separate host processes/filesystem access
are needed when running an untrusted policy; JSON observation isolation is not an OS sandbox.

Actions are `paint_rect`, `height_stroke`, `place`, `remove`, `move`, `turn`, `wall_line`,
`room_rect`, `undo`, `redo`. Coordinates are task-local; bounds, palette, scene slots and remaining
budget constrain each action. There is no raw patch, prefab paste or target-building action.

Export source examples with the `osrs-editor` `building_dataset` example, then review memberships
and object roles in the capture UI. Train an integer-weight role classifier from confirmed labels:

```sh
/opt/homebrew/bin/python3 scripts/lab/train.py fit --dataset path/to/dataset \
  --prefabs path/to/prefab-library --out target/roles.json
```

Optional `--kit-in KIT --kit-out NEW_KIT` embeds the model in a kit's `finish.learned_roles`.
Runtime suggestions require the exact cache fingerprint. Metrics split by geographic group or
authored family; rotated/copy variants stay together. Empty held-out sets report null accuracy.
Unreviewed census labels and 117HD area names stay weak labels, separate from human evaluation.
Placement frequency tables are exported separately with that qualification.

Learned WFC grids use row-major cells with y increasing north and sockets in N/E/S/W order.
The `learn` command returns a versioned catalog of sorted patterns, integer frequencies and only
observed adjacency pairs. Import it in Studio Edit / WFC to pin patterns, solve and repair a bounded
neighborhood. Kits can bind catalog sockets to their own ordered roof asset patterns. A catalog
with more than 512 distinct patterns is rejected explicitly; it is never silently truncated.

Learn a symbolic catalog from reviewed sparse captures (holes never become adjacency samples):

```sh
/opt/homebrew/bin/python3 scripts/lab/catalog.py --prefabs path/to/reviewed-style-library \
  --style lumbridge --role tavern --out target/tavern-catalog.json
target/debug/osrs-lab task-relocate path/to/reviewed.prefab.json 7 > target/relocation-task.json
```

Relocation tasks preserve the source geographic/family group across rotations and seed variants.
Their targets come from the same foundation planner as map placement; the policy still reconstructs
using ordinary tools. A model's classification evidence margin is not a calibrated probability.

## Reconstruct a saved building

Open a prefab (for example `test`) in the separate Builder / AI Lab / WFC window, choose **AI Lab**,
then **Generate copy demonstration**. This starts an empty reconstruction, generates a visible-target
tool trace, and verifies the replay. Use Step, Play, replay speed, next-action inspection and Reset
to watch it. Export task/action trace and accept a reconstruction as a new prefab when reviewed.
The world remains editable in its own window and has separate history. Normal editor scripts pause
in Lab mode so they cannot bypass the tool environment by mutating its render projection.

Task, actions, trainer log and metrics are saved in `lab-runs/` beside the editor settings. Jobs use
Python 3.11+ (override with `RUSTOSRS_PYTHON`) and an `osrs-lab` executable beside `osrs-editor`.
Source-development builds locate `scripts/lab/train.py` in this checkout. Job cancellation drops
results and stops the worker process; it cannot commit an authored edit.

The same curriculum can be exported headlessly:

```sh
target/debug/osrs-lab task-copy "/path/to/test.prefab.json" > target/test-task.json
/opt/homebrew/bin/python3 scripts/lab/train.py reconstruct --binary target/debug/osrs-lab \
  --task target/test-task.json --method imitate --seed 7 --out target/test-actions.json
```

Task construction preserves exact occurrences, rotations, sparse terrain and source heights.
Dimensions must come from reviewed prefab metadata or a resolved definition lookup; unresolved
assets are rejected rather than guessed as 1×1. Nearby captured buildings share a geographic group.

An exact teacher reconstruction verifies the task and tools; it is not evidence of a trained model.
The next policy-training stage needs observation/action examples, legal tool masks, rewards and a
learner that predicts actions. Confirm wall/door/roof relationships for structural learning, add
varied buildings, and evaluate on separate building families/geographic groups. Keep augmented
rotations in their source group. One `test` building is useful for an overfit smoke test, not an
unseen-building generalization claim. Metrics explicitly record `target_visible: true` and
`learned_policy: false` for demonstrations.
