use crate::{RenderClassification, RenderPath};

/// Core CPU scene-pass description produced before materials, visibility, zones,
/// picking, or GPU command encoding exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderScenePass {
    PrepareTransientOrdered,
    OpaqueStatic,
    OpaqueDynamic,
    OrderedScene,
}

/// Deterministic renderer-owned grouping of extracted snapshot indices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderDrawPlan {
    static_indices: Box<[usize]>,
    dynamic_indices: Box<[usize]>,
    ordered_indices: Box<[usize]>,
    passes: Box<[RenderScenePass]>,
}

impl RenderDrawPlan {
    /// Build a core draw plan without consulting semantic state.
    ///
    /// Input order is preserved within each path so later material/zone work can
    /// add batching without changing this classification contract.
    pub fn from_classifications(classifications: &[RenderClassification]) -> Self {
        let mut static_indices = Vec::new();
        let mut dynamic_indices = Vec::new();
        let mut ordered_indices = Vec::new();

        for (index, classification) in classifications.iter().copied().enumerate() {
            match classification.path() {
                RenderPath::Static => static_indices.push(index),
                RenderPath::Dynamic => dynamic_indices.push(index),
                RenderPath::Ordered => ordered_indices.push(index),
            }
        }

        let mut passes = Vec::with_capacity(4);
        if !ordered_indices.is_empty() {
            passes.push(RenderScenePass::PrepareTransientOrdered);
        }
        if !static_indices.is_empty() {
            passes.push(RenderScenePass::OpaqueStatic);
        }
        if !dynamic_indices.is_empty() {
            passes.push(RenderScenePass::OpaqueDynamic);
        }
        if !ordered_indices.is_empty() {
            passes.push(RenderScenePass::OrderedScene);
        }

        Self {
            static_indices: static_indices.into_boxed_slice(),
            dynamic_indices: dynamic_indices.into_boxed_slice(),
            ordered_indices: ordered_indices.into_boxed_slice(),
            passes: passes.into_boxed_slice(),
        }
    }

    pub fn static_indices(&self) -> &[usize] {
        &self.static_indices
    }

    pub fn dynamic_indices(&self) -> &[usize] {
        &self.dynamic_indices
    }

    pub fn ordered_indices(&self) -> &[usize] {
        &self.ordered_indices
    }

    pub fn passes(&self) -> &[RenderScenePass] {
        &self.passes
    }
}
