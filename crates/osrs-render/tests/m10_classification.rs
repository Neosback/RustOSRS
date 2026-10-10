use osrs_render::{
    RenderDrawPlan, RenderPath, RenderRequirement, RenderRequirements, RenderScenePass,
    classify_renderable,
};

#[test]
fn empty_requirements_are_static() {
    let classification = classify_renderable(RenderRequirements::new());

    assert_eq!(classification.path(), RenderPath::Static);
    assert!(classification.requirements().is_empty());
}

#[test]
fn non_ordered_instability_is_dynamic() {
    let requirements = RenderRequirements::new()
        .with(RenderRequirement::AnimationFrameGeometry)
        .with(RenderRequirement::MorphStateRegeneration)
        .with(RenderRequirement::TemporaryPreviewGeometry)
        .with(RenderRequirement::ProfileDynamicReconstruction);
    let classification = classify_renderable(requirements);

    assert_eq!(classification.path(), RenderPath::Dynamic);
    assert_eq!(classification.requirements(), requirements);
}

#[test]
fn ordered_requirements_dominate_other_dynamic_reasons() {
    let camera_ordered = RenderRequirements::new()
        .with(RenderRequirement::AnimationFrameGeometry)
        .with(RenderRequirement::CameraDependentFaceOrdering);
    assert_eq!(
        classify_renderable(camera_ordered).path(),
        RenderPath::Ordered
    );

    let transparency_ordered = RenderRequirements::new()
        .with(RenderRequirement::MorphStateRegeneration)
        .with(RenderRequirement::TransparencyOrdering);
    assert_eq!(
        classify_renderable(transparency_ordered).path(),
        RenderPath::Ordered
    );
}

#[test]
fn requirement_iteration_is_stable_and_duplicate_free() {
    let requirements = RenderRequirements::new()
        .with(RenderRequirement::TransparencyOrdering)
        .with(RenderRequirement::AnimationFrameGeometry)
        .with(RenderRequirement::TransparencyOrdering)
        .with(RenderRequirement::ProfileDynamicReconstruction);

    assert_eq!(
        requirements.iter().collect::<Vec<_>>(),
        vec![
            RenderRequirement::AnimationFrameGeometry,
            RenderRequirement::TransparencyOrdering,
            RenderRequirement::ProfileDynamicReconstruction,
        ]
    );
}

#[test]
fn draw_plan_preserves_input_order_within_each_path() {
    let classifications = [
        classify_renderable(RenderRequirements::new()),
        classify_renderable(
            RenderRequirements::new().with(RenderRequirement::AnimationFrameGeometry),
        ),
        classify_renderable(
            RenderRequirements::new().with(RenderRequirement::CameraDependentFaceOrdering),
        ),
        classify_renderable(
            RenderRequirements::new().with(RenderRequirement::MorphStateRegeneration),
        ),
        classify_renderable(
            RenderRequirements::new().with(RenderRequirement::TransparencyOrdering),
        ),
        classify_renderable(RenderRequirements::new()),
    ];

    let plan = RenderDrawPlan::from_classifications(&classifications);

    assert_eq!(plan.static_indices(), &[0, 5]);
    assert_eq!(plan.dynamic_indices(), &[1, 3]);
    assert_eq!(plan.ordered_indices(), &[2, 4]);
    assert_eq!(
        plan.passes(),
        &[
            RenderScenePass::PrepareTransientOrdered,
            RenderScenePass::OpaqueStatic,
            RenderScenePass::OpaqueDynamic,
            RenderScenePass::OrderedScene,
        ]
    );
}

#[test]
fn draw_plan_omits_unused_core_passes_without_reordering_remaining_work() {
    let classifications = [
        classify_renderable(RenderRequirements::new()),
        classify_renderable(
            RenderRequirements::new().with(RenderRequirement::AnimationFrameGeometry),
        ),
    ];

    let plan = RenderDrawPlan::from_classifications(&classifications);

    assert_eq!(
        plan.passes(),
        &[
            RenderScenePass::OpaqueStatic,
            RenderScenePass::OpaqueDynamic
        ]
    );
    assert!(plan.ordered_indices().is_empty());
}
