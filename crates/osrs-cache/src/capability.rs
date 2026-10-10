//! Explicit target capability diagnostics for revision-gated behavior.
//!
//! M9 must not let a blocked target feature disappear behind an absent code path.
//! These diagnostics expose the target profile's revision gates in a typed form
//! that later renderer/editor layers can surface without reinterpreting profile
//! strings or duplicating gate names.

use crate::profile::TargetProfile;

/// Revision-sensitive target capabilities that downstream layers may need to
/// surface as supported, required, blocked, or otherwise gated behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TargetCapability {
    ExtendedObjectModelIds,
    ObjectSoundLayout220Plus,
    SequenceLayout226Plus,
    TextureLayout233Plus,
    TerrainColorBuilder,
}

impl TargetCapability {
    pub const ALL: [Self; 5] = [
        Self::ExtendedObjectModelIds,
        Self::ObjectSoundLayout220Plus,
        Self::SequenceLayout226Plus,
        Self::TextureLayout233Plus,
        Self::TerrainColorBuilder,
    ];

    /// Stable target-profile gate name.
    pub const fn revision_gate_name(self) -> &'static str {
        match self {
            Self::ExtendedObjectModelIds => "extended_object_model_ids",
            Self::ObjectSoundLayout220Plus => "object_sound_layout_220_plus",
            Self::SequenceLayout226Plus => "sequence_layout_226_plus",
            Self::TextureLayout233Plus => "texture_layout_233_plus",
            Self::TerrainColorBuilder => "terrain_color_builder",
        }
    }
}

/// Borrowed diagnostic view of one target capability gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetCapabilityDiagnostic<'a> {
    pub capability: TargetCapability,
    pub gate_name: &'static str,
    pub state: &'a str,
    pub spec: Option<&'a str>,
    pub note: Option<&'a str>,
}

impl TargetCapabilityDiagnostic<'_> {
    /// A blocked capability must be surfaced to users instead of silently
    /// falling through to an approximate implementation.
    pub fn is_blocked(self) -> bool {
        self.state == "blocked"
    }
}

impl TargetProfile {
    /// Return the exact revision-gate diagnostic for a typed capability.
    ///
    /// Validated target profiles contain every gate in [`TargetCapability::ALL`].
    /// `None` remains explicit for callers that construct an unvalidated profile
    /// value directly.
    pub fn capability_diagnostic(
        &self,
        capability: TargetCapability,
    ) -> Option<TargetCapabilityDiagnostic<'_>> {
        let gate_name = capability.revision_gate_name();
        let gate = self.revision_gates.get(gate_name)?;
        Some(TargetCapabilityDiagnostic {
            capability,
            gate_name,
            state: gate.state.as_str(),
            spec: gate.spec.as_deref(),
            note: gate.note.as_deref(),
        })
    }

    /// Iterate every currently blocked typed capability in stable declaration
    /// order so diagnostics remain deterministic.
    pub fn blocked_capabilities(
        &self,
    ) -> impl Iterator<Item = TargetCapabilityDiagnostic<'_>> + '_ {
        TargetCapability::ALL
            .into_iter()
            .filter_map(|capability| self.capability_diagnostic(capability))
            .filter(|diagnostic| diagnostic.is_blocked())
    }
}
