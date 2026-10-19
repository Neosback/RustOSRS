//! Product generation modes select canonical stages, then validate the resulting full candidate.
use super::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::app) enum GenerationMode {
    #[default]
    Generate,
    Complete,
    Furnish,
    ReplaceStyle,
    Repair,
}

pub(super) fn includes(op: &MapOp, form: &BuildForm, kit: &BuildingKit) -> bool {
    use osrs_edit::BuildingStage::*;
    if let Some(sketch) = &form.sketch
        && let MapOp::AddObject {placement} | MapOp::RemoveObject {placement} = op
    {
        let t = placement.position.tile;
        let on_floor = |x:i32,y:i32| sketch.floors.iter().any(|c| c.plane==0 && sketch.origin.x+i32::from(c.x)==x && sketch.origin.y+i32::from(c.y)==y);
        let nearby = match placement.kind.get() {
            0..=8 | 12..=21 => (-1..=1).any(|dx| (-1..=1).any(|dy| on_floor(t.x+dx,t.y+dy))),
            _ => on_floor(t.x,t.y),
        };
        if !nearby { return false; }
    }
    if form.mode == GenerationMode::Repair {
        let tile = match op {
            MapOp::SetHeight {coord,..} => coord.corner,
            MapOp::SetTerrain {coord,..} => coord.tile,
            MapOp::AddObject {placement} | MapOp::RemoveObject {placement} => placement.position.tile,
        };
        if !form.region.is_some_and(|r| r.contains(tile)) { return false; }
    }
    let stage = kit.stage_of(op);
    if form.stage.is_some_and(|s| s != stage) {
        return false;
    }
    match form.mode {
        GenerationMode::Furnish => stage == Furnishings,
        GenerationMode::ReplaceStyle => {
            matches!(stage, Floors | Circulation | Shell | Roof | Decoration)
        }
        _ => true,
    }
}
pub(super) fn filter_generated(
    patch: &MapPatch,
    form: &BuildForm,
    kit: &BuildingKit,
    document: &MapDocument,
) -> MapPatch {
    MapPatch::new(
        patch
            .ops()
            .iter()
            .filter(|op| includes(op, form, kit))
            .filter(|op| {
                if form.mode != GenerationMode::Complete {
                    return true;
                }
                match op {
                    MapOp::SetHeight { .. } => false,
                    MapOp::SetTerrain { coord, .. } => {
                        document.terrain_at(*coord) == osrs_edit::TerrainTile::default()
                    }
                    MapOp::AddObject { placement } => {
                        !document.object_entries().iter().any(|old| {
                            old.position == placement.position
                                && category(old.kind.get()) == category(placement.kind.get())
                        })
                    }
                    MapOp::RemoveObject { .. } => false,
                }
            })
            .copied(),
    )
}
fn category(kind: u8) -> u8 {
    match kind {
        0..=3 => 0,
        4..=8 => 1,
        22 => 2,
        _ => 3,
    }
}

pub(super) fn validate(
    document: &MapDocument,
    patch: &MapPatch,
    form: &BuildForm,
    kit: &Kit,
    origin: MapTile,
    plane: SourcePlane,
    size_of: &dyn Fn(ObjectId, LocKind) -> (u8, u8),
) -> Result<osrs_lab::Verdict, String> {
    let mut candidate = document.clone();
    candidate
        .apply_patch(patch)
        .map_err(|e| format!("Candidate failed: {e:?}"))?;
    if form.locks.iter().any(|p| !candidate.contains_object(*p)) {
        return Err("Candidate would change a locked occurrence".into());
    }
    let planes = (form.floors + 1).min(4 - plane.index().get());
    let prefab = osrs_edit::capture_prefab(
        &candidate,
        osrs_edit::CaptureSpec {
            origin,
            width: form.width,
            height: form.height,
            base_plane: plane,
            planes,
            terrain: true,
            heights: true,
            objects: true,
        },
        "candidate",
        PrefabSource::Authored,
    )
    .map_err(|e| e.to_string())?;
    let mut table = osrs_lab::kit_objects(&kit.file);
    let palette: Vec<_> = prefab
        .objects
        .iter()
        .map(|p| {
            let (w, h) = size_of(
                ObjectId::new(p.id),
                LocKind::new(p.kind).unwrap_or_else(|| unreachable!()),
            );
            table.entry(ObjectId::new(p.id)).or_insert_with(|| {
                osrs_scene::building_census::ObjectInfo {
                    size: (w, h),
                    ..Default::default()
                }
            });
            let size = table[&ObjectId::new(p.id)].size;
            osrs_lab::PaletteEntry {
                id: p.id,
                kind: p.kind,
                size: [size.0, size.1],
            }
        })
        .collect();
    let names = table
        .iter()
        .map(|(id, info)| (id.get(), info.name.clone()))
        .collect();
    let snapshot = osrs_lab::Snapshot {
        tiles: prefab.tiles,
        heights: prefab.heights,
        objects: prefab.objects,
    };
    let verdict =
        osrs_lab::validate_candidate(snapshot, form.width, form.height, planes, palette, names)?;
    if !verdict.usable {
        return Err(format!(
            "Candidate rejected: {}",
            verdict.problems.join("; ")
        ));
    }
    Ok(verdict)
}
