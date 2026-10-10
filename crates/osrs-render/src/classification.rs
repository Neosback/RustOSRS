/// Renderer-owned stability path for one extracted renderable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderPath {
    /// Geometry/material state can be compiled into a reusable static artifact.
    Static,
    /// Geometry must remain in the per-frame path, but does not require ordered emission.
    Dynamic,
    /// Camera/transparency-sensitive content requires ordered per-frame preparation.
    Ordered,
}

/// Explicit renderer requirements that prevent static compilation.
///
/// These are render-policy reasons only. They do not reinterpret semantic object
/// type, model identity, or placement state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RenderRequirement {
    AnimationFrameGeometry = 0,
    MorphStateRegeneration = 1,
    CameraDependentFaceOrdering = 2,
    TransparencyOrdering = 3,
    TemporaryPreviewGeometry = 4,
    ProfileDynamicReconstruction = 5,
}

impl RenderRequirement {
    pub const ALL: [Self; 6] = [
        Self::AnimationFrameGeometry,
        Self::MorphStateRegeneration,
        Self::CameraDependentFaceOrdering,
        Self::TransparencyOrdering,
        Self::TemporaryPreviewGeometry,
        Self::ProfileDynamicReconstruction,
    ];

    const fn bit(self) -> u8 {
        1 << self as u8
    }

    const fn requires_ordered_path(self) -> bool {
        matches!(
            self,
            Self::CameraDependentFaceOrdering | Self::TransparencyOrdering
        )
    }
}

/// Compact deterministic set of renderer requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct RenderRequirements {
    bits: u8,
}

impl RenderRequirements {
    pub const fn new() -> Self {
        Self { bits: 0 }
    }

    pub const fn with(mut self, requirement: RenderRequirement) -> Self {
        self.bits |= requirement.bit();
        self
    }

    pub const fn contains(self, requirement: RenderRequirement) -> bool {
        self.bits & requirement.bit() != 0
    }

    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }

    pub fn iter(self) -> impl Iterator<Item = RenderRequirement> {
        RenderRequirement::ALL
            .into_iter()
            .filter(move |requirement| self.contains(*requirement))
    }
}

/// Stable renderer classification plus its complete reason set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderClassification {
    path: RenderPath,
    requirements: RenderRequirements,
}

impl RenderClassification {
    pub const fn path(self) -> RenderPath {
        self.path
    }

    pub const fn requirements(self) -> RenderRequirements {
        self.requirements
    }
}

/// Classify one extracted renderable strictly from renderer stability requirements.
///
/// Ordered requirements dominate ordinary dynamic requirements because ordered
/// content must remain in the per-frame preparation path even when it also has
/// animation, morph, preview, or profile-specific regeneration requirements.
pub fn classify_renderable(requirements: RenderRequirements) -> RenderClassification {
    let path = if requirements
        .iter()
        .any(RenderRequirement::requires_ordered_path)
    {
        RenderPath::Ordered
    } else if requirements.is_empty() {
        RenderPath::Static
    } else {
        RenderPath::Dynamic
    };

    RenderClassification { path, requirements }
}
