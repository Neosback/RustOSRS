use super::*;
use crate::session::{
    ExportError, HeightExportPolicy, baseline_document, diff_region, export_region_with_policy,
};
use osrs_cache::decode::{ArchiveFileProvenance, DecoderContext, decode_locations, decode_terrain};
use osrs_world::{SourceSquare, SourceSquares};

pub(crate) fn prepare(snapshot: ExportSnapshot) -> PreparedExport {
    let squares = SourceSquares::open(snapshot.source.clone());
    match squares {
        Ok(squares) if squares.profile_id() == snapshot.profile => {
            let fingerprint = squares.fingerprint();
            prepare_with(snapshot, fingerprint, |region| {
                squares.read(region).map_err(|e| e.to_string())
            })
        }
        result => {
            let message = match result {
                Err(error) => error.to_string(),
                Ok(_) => "The source profile changed during export preparation.".to_owned(),
            };
            let mut prepared = empty(&snapshot, String::new());
            prepared.issues.push(ExportIssue::blocker(
                "source-unavailable",
                message,
                "Reopen the supported original cache and retry.",
            ));
            prepared
        }
    }
}

fn empty(snapshot: &ExportSnapshot, fingerprint: String) -> PreparedExport {
    PreparedExport {
        stamp: snapshot.stamp,
        source: snapshot.source.clone(),
        profile: snapshot.profile.clone(),
        fingerprint,
        regions: Vec::new(),
        unchanged: 0,
        issues: Vec::new(),
    }
}

pub(crate) fn prepare_with(
    snapshot: ExportSnapshot,
    fingerprint: String,
    mut read: impl FnMut(RegionCoord) -> Result<Option<SourceSquare>, String>,
) -> PreparedExport {
    let mut regions = snapshot.authored.clone();
    regions.extend(snapshot.pending.iter().map(|d| d.region));
    let mut prepared = empty(&snapshot, fingerprint);
    for region in regions {
        let result =
            read(region)
                .map_err(|e| {
                    ExportIssue::blocker("source-read", e, "Restore the source cache and retry.")
                })
                .and_then(|square| {
                    square.ok_or_else(|| ExportIssue::blocker(
                "missing-region", "Authored region is missing from the original cache.",
                "Restore the original map data; creating new regions is not supported yet.",
            ))
                })
                .and_then(|square| prepare_region(&snapshot, square));
        match result {
            Ok(Some(region)) => {
                for change in &region.conversions {
                    prepared.issues.push(ExportIssue {
                        severity: IssueSeverity::Warning, code: "height-conversion", region: Some(region.region), location: Some(change.position),
                        message: format!("Height {} exports/reloads as {} using EikenB00m truncation (including byte 1). The project keeps its exact height.", change.authored, change.reloaded),
                        remedy: "Save the project to retain full editing precision.".to_owned(),
                    });
                }
                prepared.regions.push(region);
            }
            Ok(None) => prepared.unchanged += 1,
            Err(mut issue) => {
                issue.region = Some(region);
                prepared.issues.push(issue);
            }
        }
    }
    if prepared.regions.is_empty() && prepared.issues.is_empty() {
        prepared.issues.push(ExportIssue {
            severity: IssueSeverity::Warning,
            code: "no-changes",
            region: None,
            location: None,
            message: "The captured map matches the source cache.".to_owned(),
            remedy: "Make an edit before exporting.".to_owned(),
        });
    }
    prepared
}

fn semantic_issue(error: impl std::fmt::Debug) -> ExportIssue {
    ExportIssue::blocker(
        "invalid-region",
        format!("{error:?}"),
        "Correct the affected region and validate again.",
    )
}

fn prepare_region(
    snapshot: &ExportSnapshot,
    square: SourceSquare,
) -> Result<Option<PreparedRegion>, ExportIssue> {
    let region = square.region;
    let mut expected =
        baseline_document(&square.terrain, &square.locations).map_err(semantic_issue)?;
    // A dirty region's authoritative document supersedes its pending load record. Pending-only
    // regions are reconstructed on scratch state, without changing residency or user history.
    let diff = if snapshot.authored.contains(&region) {
        diff_region(&square.terrain, &square.locations, &snapshot.document)
            .map_err(semantic_issue)?
    } else {
        snapshot
            .pending
            .iter()
            .find(|d| d.region == region)
            .cloned()
            .ok_or_else(|| semantic_issue("missing pending region"))?
    };
    validate_diff(&diff, &expected)?;
    expected
        .apply_patch(&diff.to_patch(&expected))
        .map_err(semantic_issue)?;
    let converted = export_region_with_policy(&square.terrain, &square.locations, &expected, HeightExportPolicy::EikenTruncate).map_err(|e| {
        let mut issue = semantic_issue(&e);
        if let ExportError::HeightNotStorable { position, .. } = e {
            issue.code = "height-not-storable";
            issue.location = Some(position);
            issue.remedy = "Use a height representable in the map format (steps of 8 relative to the lower plane).".to_owned();
        }
        issue
    })?;
    let expected = converted.expected;
    let Some(exported) = converted.region else {
        return Ok(None);
    };
    let mut files = square.files;
    for (id, bytes) in &mut files {
        match *id {
            0 => *bytes = exported.terrain.clone(),
            1 => *bytes = exported.locations.clone(),
            _ => {}
        }
    }
    if !files.iter().any(|(id, _)| *id == 0) || !files.iter().any(|(id, _)| *id == 1) {
        return Err(semantic_issue("map group lacks terrain or location file"));
    }
    // Prove semantic round-trip during preflight, before any output can be written.
    let profiles = osrs_cache::registry::bundled_profiles().map_err(semantic_issue)?;
    let profile = profiles
        .iter()
        .find(|p| p.id == snapshot.profile)
        .ok_or_else(|| semantic_issue("unsupported profile"))?;
    let context = DecoderContext::from_profile(profile).map_err(semantic_issue)?;
    verify_files(region, square.group_id, &files, &expected, &context)?;
    Ok(Some(PreparedRegion {
        region,
        group: square.group_id,
        files,
        changes: exported.changes,
        conversions: converted.conversions,
        expected,
    }))
}

fn validate_diff(diff: &RegionDiff, baseline: &MapDocument) -> Result<(), ExportIssue> {
    for &(position, tile) in &diff.tiles {
        let invalid_id = tile
            .underlay
            .is_some_and(|id| id.get() == 0 || id.get() > 0x7fff)
            || tile
                .overlay
                .is_some_and(|o| o.id.get() == 0 || o.id.get() > 0x7fff);
        if invalid_id || tile.flags > 32 {
            let mut issue = ExportIssue::blocker(
                "terrain-not-storable",
                "Terrain channel is outside the target map format.",
                "Choose valid floor IDs and flags for the supported cache.",
            );
            issue.location = Some(position);
            return Err(issue);
        }
    }
    let positions = diff
        .tiles
        .iter()
        .map(|(p, _)| *p)
        .chain(
            diff.heights
                .iter()
                .map(|(h, _)| MapPosition::new(h.corner, h.plane)),
        )
        .chain(
            diff.objects_added
                .iter()
                .chain(&diff.objects_removed)
                .map(|o| o.position),
        );
    if positions
        .into_iter()
        .any(|p| p.tile.split_region().0 != diff.region)
        || diff
            .heights
            .iter()
            .any(|(h, _)| baseline.height_at(*h).is_none())
        || diff
            .objects_removed
            .iter()
            .any(|o| !baseline.contains_object(*o))
    {
        return Err(semantic_issue(
            "project diff does not belong to its source region",
        ));
    }
    Ok(())
}

pub(super) fn verify_files(
    region: RegionCoord,
    group: u32,
    files: &[(u32, Vec<u8>)],
    expected: &MapDocument,
    context: &DecoderContext,
) -> Result<(), ExportIssue> {
    let file = |wanted| {
        files
            .iter()
            .find(|(id, _)| *id == wanted)
            .map(|(_, b)| b.as_slice())
            .ok_or_else(|| semantic_issue("missing map stream"))
    };
    let terrain = decode_terrain(
        file(0)?,
        context,
        &ArchiveFileProvenance::new(5, group, Some(0)),
        region,
    )
    .map_err(semantic_issue)?;
    let locations = decode_locations(
        file(1)?,
        context,
        &ArchiveFileProvenance::new(5, group, Some(1)),
        region,
    )
    .map_err(semantic_issue)?;
    if !diff_region(&terrain, &locations, expected)
        .map_err(semantic_issue)?
        .is_empty()
    {
        return Err(semantic_issue(
            "exported streams differ from the captured semantic map",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
