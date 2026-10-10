use osrs_scene::{SceneGrid, semantic_scene_hash_v1};

/// Immutable identity for one semantic scene generation consumed by the renderer.
///
/// `serial` provides monotonic stale-work rejection while `scene_hash` binds the
/// generation to exact authoritative semantic content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SemanticGeneration {
    serial: u64,
    scene_hash: [u8; 32],
}

impl SemanticGeneration {
    pub const fn new(serial: u64, scene_hash: [u8; 32]) -> Self {
        Self { serial, scene_hash }
    }

    pub fn from_scene(serial: u64, scene: &SceneGrid) -> Self {
        Self::new(serial, semantic_scene_hash_v1(scene))
    }

    pub const fn serial(self) -> u64 {
        self.serial
    }

    pub const fn scene_hash(self) -> [u8; 32] {
        self.scene_hash
    }

    pub const fn supersedes(self, other: Self) -> bool {
        self.serial > other.serial
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_binds_serial_to_exact_semantic_scene_hash()
    -> Result<(), Box<dyn std::error::Error>> {
        let scene = SceneGrid::new(2, 2, 1)?;
        let first = SemanticGeneration::from_scene(7, &scene);
        let same_content = SemanticGeneration::from_scene(8, &scene);

        assert_eq!(first.scene_hash(), same_content.scene_hash());
        assert_ne!(first, same_content);
        assert!(same_content.supersedes(first));
        assert!(!first.supersedes(same_content));
        Ok(())
    }
}
