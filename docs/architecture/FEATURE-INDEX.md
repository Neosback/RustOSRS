# Generated feature index

<!-- GENERATED FILE: edit scripts/editor_features.json, not this file. -->

Canonical source: `scripts/editor_features.json`. Regenerate with 
`python3 scripts/feature_index.py --write`.

Use `python3 scripts/features.py find "<concept>"` before adding a feature, planner, 
gateway, or history path.

| ID | Feature | Status | Owner | Kind | Mutation |
|---|---|---|---|---|---|
| `appearance` | Appearance | implemented | `osrs-hd::theme` | workspace | mutating |
| `building-copy-demonstration` | Building copy demonstration | implemented | `osrs-lab::reconstruction` | workspace | mutating |
| `buildings` | Buildings | implemented | `osrs-edit::building_kit` | workspace | mutating |
| `height` | Height | implemented | `osrs-edit::height` | tool | mutating |
| `objects` | Objects | implemented | `osrs-edit::object_edit` | tool | mutating |
| `paint` | Paint | implemented | `osrs-edit::paint` | tool | mutating |
| `prefab-studio` | Prefab Studio | implemented | `osrs-edit::prefab/building/studio` | workspace | mutating |
| `prefabs` | Prefabs | implemented | `osrs-edit::prefab` | workspace | mutating |
| `procedural-lab` | Procedural Lab | implemented | `osrs-lab::action/env` | workspace | mutating |
| `select` | Select | implemented | `osrs-editor::interaction_runtime/selection` | tool | read-only |
| `spline` | Spline | implemented | `osrs-edit::path` | tool | mutating |
| `wfc-prefab-floors` | WFC prefab floors | implemented | `osrs-edit::building_learning` | workspace | mutating |

## Navigation

### Appearance

- **ID:** `appearance`
- **Aliases:** `theme`, `theme studio`, `appearance theme`, `hd theme`
- **Owner:** `osrs-hd::theme`
- **Status:** `implemented`
- **Controller:** `crates/osrs-editor/src/app/theme_studio/mod.rs::draw`
- **Planner:** `crates/osrs-hd/src/theme/mod.rs::plan`
- **Gateway:** `crates/osrs-editor/src/app/edit_runtime.rs::execute_appearance`
- **Behavior tests:** `crates/osrs-hd/tests/themes.rs::ordered_rules_override_only_authored_properties_and_explain_winners`, `crates/osrs-editor/src/app/theme_studio/mod.rs::cancelling_preview_keeps_authority_and_restores_the_visible_base`, `crates/osrs-editor/src/session/appearance_tests.rs::failed_projection_and_empty_theme_gesture_preserve_redo_and_dirty_regions`, `crates/osrs-editor/src/session/appearance_tests.rs::mixed_history_preserves_map_revision_and_restores_appearance_exactly`

### Building copy demonstration

- **ID:** `building-copy-demonstration`
- **Aliases:** `building reconstruction`, `copy demonstration`, `building copy`, `reconstruction task`
- **Owner:** `osrs-lab::reconstruction`
- **Status:** `implemented`
- **Controller:** `crates/osrs-editor/src/app/prefab_studio/lab.rs::generate_demonstration`
- **Planner:** `crates/osrs-lab/src/reconstruction.rs::reconstruction_task`
- **Gateway:** `crates/osrs-lab/src/env.rs::step`
- **Behavior tests:** `crates/osrs-lab/src/reconstruction_tests.rs::reconstruction_preserves_sparse_tiles_occurrences_and_geographic_family`, `crates/osrs-lab/src/tests.rs::invalid_actions_preserve_document_and_raw_target_writes_are_unrepresentable`, `crates/osrs-lab/src/reconstruction_tests.rs::copy_task_legal_actions_reach_exact_and_undo_redo`, `crates/osrs-lab/src/reconstruction_tests.rs::copy_task_legal_actions_reach_exact_and_undo_redo`

### Buildings

- **ID:** `buildings`
- **Aliases:** `building generator`, `procedural building`, `procgen building`, `building kit`
- **Owner:** `osrs-edit::building_kit`
- **Status:** `implemented`
- **Controller:** `crates/osrs-editor/src/app/prefab_panel/build.rs::show`
- **Planner:** `crates/osrs-edit/src/building_kit/generate.rs::generate`
- **Gateway:** `crates/osrs-editor/src/app/editing/prefab_commit.rs::commit_map_patch`
- **Behavior tests:** `crates/osrs-editor/src/app/prefab_runtime/tests.rs::the_built_in_kit_plans_a_building_that_applies_undoes_and_saves`, `crates/osrs-editor/src/app/prefab_runtime/tests.rs::choosing_a_site_ends_the_gesture_and_centres_the_site_on_the_click`, `crates/osrs-editor/src/app/editing/history/routing/tests.rs::a_missing_tile_fails_the_composite_projection_and_restores_the_session`, `crates/osrs-editor/src/app/editing/history/routing/tests.rs::composite_undo_and_redo_replay_terrain_height_and_object_together`

### Height

- **ID:** `height`
- **Aliases:** `terrain height`, `sculpt`, `height sculpt`, `flatten terrain`
- **Owner:** `osrs-edit::height`
- **Status:** `implemented`
- **Controller:** `crates/osrs-editor/src/app/editing.rs::sculpt_at`
- **Planner:** `crates/osrs-edit/src/height/commit.rs::build_terrain_height_patch`
- **Gateway:** `crates/osrs-editor/src/app/editing/commit.rs::commit_height_stroke`
- **Behavior tests:** `crates/osrs-edit/src/height/commit.rs::repeated_samples_fold_into_one_final_height_operation`, `crates/osrs-editor/src/app/editing/controller_tests.rs::cancelling_height_preview_restores_absolute_height_and_existing_overrides`, `crates/osrs-editor/src/app/editing/tests.rs::one_height_stroke_is_one_semantic_session_history_entry`, `crates/osrs-editor/src/app/editing/tests.rs::semantic_height_projection_tracks_undo_redo_and_preserves_other_channels`

### Objects

- **ID:** `objects`
- **Aliases:** `object edit`, `objects`, `locs`, `placements`, `object placement`
- **Owner:** `osrs-edit::object_edit`
- **Status:** `implemented`
- **Controller:** `crates/osrs-editor/src/app/object_tool.rs::object_actions`
- **Planner:** `crates/osrs-edit/src/object_edit.rs::build_object_patch`
- **Gateway:** `crates/osrs-editor/src/app/object_tool.rs::edit_objects`
- **Behavior tests:** `crates/osrs-edit/src/object_edit.rs::move_is_remove_then_add_in_one_patch`, `crates/osrs-editor/src/app/interaction_runtime/tests.rs::cancel_object_drag_discards_in_progress_gesture`, `crates/osrs-editor/src/app/object_tool/gateway_tests.rs::successful_object_gateway_projection_uses_post_commit_state_and_finalizes_once`, `crates/osrs-editor/src/app/object_tool/commit.rs::semantic_object_undo_projection_is_validated_from_session_state`

### Paint

- **ID:** `paint`
- **Aliases:** `terrain paint`, `overlay paint`, `underlay paint`, `tile paint`
- **Owner:** `osrs-edit::paint`
- **Status:** `implemented`
- **Controller:** `crates/osrs-editor/src/app/editing.rs::paint_at`
- **Planner:** `crates/osrs-edit/src/paint.rs::build_terrain_paint_patch`
- **Gateway:** `crates/osrs-editor/src/app/editing/commit.rs::commit_paint_stroke`
- **Behavior tests:** `crates/osrs-edit/src/paint.rs::repeated_samples_fold_to_one_final_operation_per_channel`, `crates/osrs-editor/src/app/editing/controller_tests.rs::cancelling_paint_preview_restores_channels_without_consuming_redo`, `crates/osrs-editor/src/app/editing/tests.rs::one_paint_stroke_is_one_semantic_session_history_entry`, `crates/osrs-editor/src/app/editing/tests.rs::semantic_paint_projection_tracks_undo_redo_and_preserves_unarmed_height`

### Prefab Studio

- **ID:** `prefab-studio`
- **Aliases:** `prefab authoring`, `prefab capture`, `capture prefab`, `prefab editor`
- **Owner:** `osrs-edit::prefab/building/studio`
- **Status:** `implemented`
- **Controller:** `crates/osrs-editor/src/app/prefab_studio/mod.rs::frame`
- **Planner:** `crates/osrs-edit/src/prefab/building/studio.rs::capture_prefab_over`
- **Gateway:** `crates/osrs-editor/src/app/editing/prefab_commit.rs::commit_map_patch`
- **Behavior tests:** `crates/osrs-edit/src/prefab/building/tests.rs::sparse_courtyard_capture_and_studio_roundtrip_exclude_pad_and_keep_default_floor`, `crates/osrs-editor/src/app/prefab_studio/session.rs::failed_save_and_cancel_leave_the_scene_available`, `crates/osrs-editor/src/app/prefab_studio/tests.rs::studio_baseline_is_untracked_and_user_commit_is_one_entry`, `crates/osrs-editor/src/app/prefab_studio/tests.rs::studio_undo_returns_to_prefab_and_parked_world_never_changes`

### Prefabs

- **ID:** `prefabs`
- **Aliases:** `prefab`, `prefab placement`, `paste prefab`, `place prefab`
- **Owner:** `osrs-edit::prefab`
- **Status:** `implemented`
- **Controller:** `crates/osrs-editor/src/app/prefab_panel/viewport.rs::armed_input`
- **Planner:** `crates/osrs-edit/src/prefab/place.rs::plan_place_prefab`
- **Gateway:** `crates/osrs-editor/src/app/editing/prefab_commit.rs::commit_map_patch`
- **Behavior tests:** `crates/osrs-edit/src/prefab/tests.rs::one_turn_moves_things_where_a_clockwise_quarter_turn_puts_them`, `crates/osrs-editor/src/app/prefab_runtime/tests.rs::removing_the_armed_prefab_disarms_it`, `crates/osrs-editor/src/app/editing/history/routing/tests.rs::a_missing_tile_fails_the_composite_projection_and_restores_the_session`, `crates/osrs-editor/src/app/editing/history/routing/tests.rs::composite_undo_and_redo_replay_terrain_height_and_object_together`

### Procedural Lab

- **ID:** `procedural-lab`
- **Aliases:** `procgen lab`, `lab`, `procedural environment`, `training environment`
- **Owner:** `osrs-lab::action/env`
- **Status:** `implemented`
- **Controller:** `crates/osrs-editor/src/app/prefab_studio/lab.rs::show`
- **Planner:** `crates/osrs-lab/src/action.rs::plan`
- **Gateway:** `crates/osrs-lab/src/env.rs::step`
- **Behavior tests:** `crates/osrs-lab/src/tests.rs::tool_actions_undo_redo_and_replay_restore_exact_state`, `crates/osrs-lab/src/tests.rs::invalid_actions_preserve_document_and_raw_target_writes_are_unrepresentable`, `crates/osrs-lab/src/tests.rs::tool_actions_undo_redo_and_replay_restore_exact_state`, `crates/osrs-lab/src/tests.rs::failures_do_not_destroy_redo_and_budget_is_bounded`

### Select

- **ID:** `select`
- **Aliases:** `selection`, `marquee`, `marquee select`
- **Owner:** `osrs-editor::interaction_runtime/selection`
- **Status:** `implemented`
- **Controller:** `crates/osrs-editor/src/app/interaction_runtime/selection.rs::begin_marquee`
- **Planner:** -
- **Gateway:** -

### Spline

- **ID:** `spline`
- **Aliases:** `path`, `terrain path`, `spline path`, `road path`
- **Owner:** `osrs-edit::path`
- **Status:** `implemented`
- **Controller:** `crates/osrs-editor/src/app/spline_tool/input.rs::spline_input`
- **Planner:** `crates/osrs-edit/src/path.rs::build_terrain_path_patch`
- **Gateway:** `crates/osrs-editor/src/app/editing/path.rs::apply_terrain_path`
- **Behavior tests:** `crates/osrs-edit/src/path/tests.rs::path_patch_sets_the_overlay_and_keeps_everything_else`, `crates/osrs-editor/src/app/interaction_runtime/spline/tests.rs::remove_last_and_clear_reset_the_preview_state`, `crates/osrs-editor/src/app/editing/path/tests.rs::one_path_patch_is_one_persistent_session_revision`, `crates/osrs-editor/src/app/editing/path/tests.rs::semantic_path_projection_tracks_undo_redo_and_the_separate_height_step`

### WFC prefab floors

- **ID:** `wfc-prefab-floors`
- **Aliases:** `wfc floors`, `wave function collapse floors`, `floor synthesis`, `wfc prefab`
- **Owner:** `osrs-edit::building_learning`
- **Status:** `implemented`
- **Controller:** `crates/osrs-editor/src/app/prefab_studio/catalog.rs::show`
- **Planner:** `crates/osrs-edit/src/building_learning/placement.rs::plan_wfc_floors`
- **Gateway:** `crates/osrs-editor/src/app/editing/prefab_commit.rs::commit_map_patch`
- **Behavior tests:** `crates/osrs-editor/src/app/prefab_studio/catalog/tests.rs::wfc_preview_cancel_leaves_author_document_and_history_unchanged`, `crates/osrs-editor/src/app/prefab_studio/catalog/tests.rs::wfc_preview_cancel_leaves_author_document_and_history_unchanged`, `crates/osrs-editor/src/app/prefab_studio/catalog/tests.rs::wfc_author_commit_survives_save_and_exact_undo_redo`, `crates/osrs-editor/src/app/prefab_studio/catalog/tests.rs::wfc_author_commit_survives_save_and_exact_undo_redo`
