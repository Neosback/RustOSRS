//! Cache format changes by build, and the supported build range.
//!
//! The OpenRune server decides every layout branch from one integer (`revisionIsOrAfter(rev, N)`), spread over
//! its codecs. We keep the same facts in one table instead: each entry names the build where a layout changed,
//! the target-profile gate that records it, and what it affects. A profile is accepted only when its declared
//! gates agree with its build, so a mislabelled profile (a 240 cache described as 236, a gate dropped) fails at
//! load time rather than as a misread byte deep in a decoder.
//!
//! Only builds from [`MIN_SUPPORTED_BUILD`] up are supported. At 240 and later every entry below is already in
//! effect, so the decoders implement exactly one (modern) layout and a new build only needs a new profile, plus
//! a new entry here if the build changes a layout.

use crate::profile::TargetProfile;

/// Oldest cache build the decoders support.
pub const MIN_SUPPORTED_BUILD: u32 = 240;

/// One layout change, effective from `since_build` (inclusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatChange {
    pub since_build: u32,
    /// Target-profile revision gate that records it, when the decoders gate on it.
    pub gate: Option<&'static str>,
    pub affects: &'static str,
}

/// Every known layout change, oldest first. Source: OpenRune FileStore codecs (`revisionIsOrAfter` sites),
/// cross-checked with the pinned deobfuscated client for the gates the profile already names.
pub const FORMAT_CHANGES: &[FormatChange] = &[
    FormatChange {
        since_build: 220,
        gate: Some("object_sound_layout_220_plus"),
        affects: "object definition ops and sound layout, sequence sound effects, NPC ops",
    },
    FormatChange {
        since_build: 226,
        gate: Some("sequence_layout_226_plus"),
        affects: "sequence definitions: sound effect list layout",
    },
    FormatChange {
        since_build: 230,
        gate: None,
        affects: "game value (gameval) archive 24 exists; CS2 packing supported",
    },
    FormatChange {
        since_build: 232,
        gate: None,
        affects: "component gameval group moves to the 16-bit child layout; NPC opcode 111 retired",
    },
    FormatChange {
        since_build: 233,
        gate: Some("texture_layout_233_plus"),
        affects: "texture definitions",
    },
    FormatChange {
        since_build: 237,
        gate: None,
        affects: "map squares are one numeric group (x << 8 | y) with files 0/1 and no XTEA; \
                  param, identity-kit, spot-anim and component-model layouts; extended entity ops",
    },
    FormatChange {
        since_build: 238,
        gate: None,
        affects: "world map archives use numeric groups (details, composite map/texture, geography)",
    },
];

/// Gates that must read `required` for a build, derived from [`FORMAT_CHANGES`].
pub fn gates_required_for_build(build: u32) -> impl Iterator<Item = &'static str> {
    FORMAT_CHANGES
        .iter()
        .filter(move |change| change.since_build <= build)
        .filter_map(|change| change.gate)
}

/// Whether a layout change is in effect for `build`.
pub fn has_change(build: u32, since_build: u32) -> bool {
    build >= since_build
}

/// Check a profile's declared gates against its build.
///
/// Returns the first disagreement as a message. Gates the table does not mention (such as
/// `extended_object_model_ids` or `terrain_color_builder`) are left to the profile validator.
pub fn validate_profile_build(profile: &TargetProfile) -> Result<(), String> {
    let build = profile.cache_source.build;
    if build < MIN_SUPPORTED_BUILD {
        return Err(format!(
            "build {build} is older than the minimum supported build {MIN_SUPPORTED_BUILD}"
        ));
    }
    for gate in gates_required_for_build(build) {
        match profile.revision_gates.get(gate).map(|g| g.state.as_str()) {
            Some("required") => {}
            Some(state) => {
                return Err(format!(
                    "build {build} requires revision gate {gate:?} but the profile declares it {state:?}"
                ));
            }
            None => {
                return Err(format!(
                    "build {build} requires revision gate {gate:?} but the profile does not declare it"
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    const PROFILE_241: &str =
        include_str!("../../../profiles/osrs-live-241-2026-09-30-openrs2-2727.yaml");

    fn profile() -> Result<TargetProfile, Box<dyn std::error::Error>> {
        Ok(TargetProfile::from_yaml_str(PROFILE_241)?)
    }

    fn error_of(profile: &TargetProfile) -> String {
        validate_profile_build(profile).err().unwrap_or_default()
    }

    #[test]
    fn table_is_sorted_and_starts_below_the_supported_range() {
        assert!(FORMAT_CHANGES.windows(2).all(|w| w[0].since_build < w[1].since_build));
        assert!(FORMAT_CHANGES.iter().all(|c| c.since_build <= MIN_SUPPORTED_BUILD + 1));
    }

    #[test]
    fn every_gated_change_is_in_effect_for_the_supported_range() {
        let gates: Vec<_> = gates_required_for_build(MIN_SUPPORTED_BUILD).collect();
        assert_eq!(
            gates,
            [
                "object_sound_layout_220_plus",
                "sequence_layout_226_plus",
                "texture_layout_233_plus"
            ]
        );
    }

    #[test]
    fn committed_profile_agrees_with_its_build() -> TestResult {
        assert_eq!(validate_profile_build(&profile()?), Ok(()));
        Ok(())
    }

    #[test]
    fn build_240_and_later_accept_the_same_gate_set() -> TestResult {
        let mut p = profile()?;
        for build in [240, 241, 242, 250] {
            p.cache_source.build = build;
            assert_eq!(validate_profile_build(&p), Ok(()), "build {build}");
        }
        Ok(())
    }

    #[test]
    fn builds_below_the_supported_range_are_rejected() -> TestResult {
        let mut p = profile()?;
        p.cache_source.build = 239;
        assert!(error_of(&p).contains("minimum supported build 240"));
        Ok(())
    }

    #[test]
    fn a_dropped_or_downgraded_gate_is_rejected() -> TestResult {
        let mut p = profile()?;
        p.revision_gates.remove("texture_layout_233_plus");
        assert!(error_of(&p).contains("does not declare"));

        let mut p = profile()?;
        if let Some(gate) = p.revision_gates.get_mut("sequence_layout_226_plus") {
            gate.state = "blocked".to_owned();
        }
        assert!(error_of(&p).contains("\"blocked\""));
        Ok(())
    }
}
