use crate::{RenderMesh, RenderOrigin, SemanticGeneration};

/// Disposable immutable CPU render snapshot for one semantic generation.
///
/// Fields are private and no mutable accessors are exposed. Rebuilding or
/// dropping a snapshot therefore cannot mutate the authoritative scene.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderSnapshot {
    generation: SemanticGeneration,
    origin: RenderOrigin,
    meshes: Box<[RenderMesh]>,
}

impl RenderSnapshot {
    pub fn new(
        generation: SemanticGeneration,
        origin: RenderOrigin,
        meshes: impl Into<Box<[RenderMesh]>>,
    ) -> Self {
        Self {
            generation,
            origin,
            meshes: meshes.into(),
        }
    }

    pub const fn generation(&self) -> SemanticGeneration {
        self.generation
    }

    pub const fn origin(&self) -> RenderOrigin {
        self.origin
    }

    pub fn meshes(&self) -> &[RenderMesh] {
        &self.meshes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_snapshot_retains_generation_and_origin() {
        let generation = SemanticGeneration::new(9, [0x5a; 32]);
        let origin = RenderOrigin::new(1024, 32, -2048);
        let snapshot = RenderSnapshot::new(generation, origin, Vec::<RenderMesh>::new());

        assert_eq!(snapshot.generation(), generation);
        assert_eq!(snapshot.origin(), origin);
        assert!(snapshot.meshes().is_empty());
    }
}
