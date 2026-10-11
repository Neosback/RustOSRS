from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one match, found {count}\n--- pattern ---\n{old}")
    file.write_text(text.replace(old, new, 1))


# Share the resolved material table cheaply with dynamic models. All existing callers use
# MaterialSet through shared references/method calls, so Arc's Deref keeps their API unchanged.
replace_once(
    "crates/osrs-world/src/hd/mod.rs",
    """    pub materials: MaterialSet,""",
    """    pub materials: Arc<MaterialSet>,""",
)
replace_once(
    "crates/osrs-world/src/hd/mod.rs",
    """        let materials = MaterialSet::build(&data, &settings, vanilla_textures);
        let grounds = GroundMaterials::new(&data, &materials);""",
    """        let materials = Arc::new(MaterialSet::build(&data, &settings, vanilla_textures));
        let grounds = GroundMaterials::new(&data, &materials);""",
)

# Split model emission into override resolution plus a reusable resolved face loop. Animated
# locs resolve the same ModelOverrideManager result while the region still has HdContext, then
# reuse this exact face loop after each animation pose.
replace_once(
    "crates/osrs-world/src/hd/model.rs",
    """pub fn model_faces(hd: &HdContext, model: &ReferenceLitModel, site: ModelSite) -> Vec<HdFace> {
    let override_ = hd.models.get(site.object_id, site.world, &hd.areas);
    let placement = site.placement;
""",
    """pub fn model_faces(hd: &HdContext, model: &ReferenceLitModel, site: ModelSite) -> Vec<HdFace> {
    let override_ = hd.models.get(site.object_id, site.world, &hd.areas);
    model_faces_resolved(
        &hd.materials,
        hd.settings.hide_fake_shadows(),
        override_,
        model,
        site,
    )
}

/// Emit one model after its object/location override has already been resolved. Dynamic objects
/// use this after posing so their material replacement, UV, alpha and shadow semantics are the
/// same as static locs.
pub(crate) fn model_faces_resolved(
    materials: &MaterialSet,
    hide_fake_shadows: bool,
    override_: &ResolvedModelOverride,
    model: &ReferenceLitModel,
    site: ModelSite,
) -> Vec<HdFace> {
    let placement = site.placement;
""",
)
replace_once(
    "crates/osrs-world/src/hd/model.rs",
    """                material = hd.materials.from_vanilla_texture(id);""",
    """                material = materials.from_vanilla_texture(id);""",
)
replace_once(
    "crates/osrs-world/src/hd/model.rs",
    """        if hd.settings.hide_fake_shadows()
            && face_override.hide_vanilla_shadows""",
    """        if hide_fake_shadows && face_override.hide_vanilla_shadows""",
)
replace_once(
    "crates/osrs-world/src/hd/model.rs",
    """        let material_flags = hd.materials.has_transparency(material);
        let double_sided = face_override.double_sided_faces || hd.materials.double_sided(material);""",
    """        let material_flags = materials.has_transparency(material);
        let double_sided = face_override.double_sided_faces || materials.double_sided(material);""",
)

# Dynamic models retain only the resolved state needed by that face loop. The material table is
# shared by Arc and the per-object override is resolved once during region extraction.
replace_once(
    "crates/osrs-world/src/animation.rs",
    """use osrs_core::{""",
    """use crate::hd::model::{ModelSite, ResolvedModelOverride, model_faces_resolved};
use osrs_core::{""",
)
replace_once(
    "crates/osrs-world/src/animation.rs",
    """use osrs_render::{ModelPlacement, SceneGeometry};""",
    """use osrs_hd::materials::MaterialSet;
use osrs_render::{ModelPlacement, SceneGeometry};""",
)
replace_once(
    "crates/osrs-world/src/animation.rs",
    """/// One placed animated model slot.
#[derive(Debug, Clone)]
pub struct AnimatedInstance {
    pub model: Arc<AnimatedModel>,
    /// World zone coordinates of the owning zone.
    pub zone: (i32, i32),
    pub level: u8,
    pub min_plane: u8,
    /// Model origin in zone-local units.
    pub local: (i32, i32, i32),
    /// Extra instance rotation in JAU.
    pub rotation: u16,
}
""",
    """/// Resolved 117HD state for one animated loc. This is captured while region extraction still
/// owns the HdContext, then reused after each animation pose.
#[derive(Debug, Clone)]
pub struct AnimatedHdState {
    pub materials: Arc<MaterialSet>,
    pub override_: Arc<ResolvedModelOverride>,
    pub hide_fake_shadows: bool,
    pub object_id: u32,
    pub world: (i32, i32, i32),
    pub zone_origin_world: (i32, i32),
}

/// One placed animated model slot.
#[derive(Debug, Clone)]
pub struct AnimatedInstance {
    pub model: Arc<AnimatedModel>,
    /// `Some` only when this region was extracted through the 117HD profile.
    pub hd: Option<AnimatedHdState>,
    /// World zone coordinates of the owning zone.
    pub zone: (i32, i32),
    pub level: u8,
    pub min_plane: u8,
    /// Model origin in zone-local units.
    pub local: (i32, i32, i32),
    /// Extra instance rotation in JAU.
    pub rotation: u16,
}
""",
)
replace_once(
    "crates/osrs-world/src/animation.rs",
    """            let zone = geometry.zone_mut(instance.zone.0, instance.zone.1);
            zone.group_mut(instance.level, instance.min_plane)
                .push_model(
                    &posed,
                    ModelPlacement {
                        x: instance.local.0,
                        y: instance.local.1,
                        z: instance.local.2,
                        orientation: instance.rotation,
                    },
                );""",
    """            let placement = ModelPlacement {
                x: instance.local.0,
                y: instance.local.1,
                z: instance.local.2,
                orientation: instance.rotation,
            };
            let builder = geometry
                .zone_mut(instance.zone.0, instance.zone.1)
                .group_mut(instance.level, instance.min_plane);
            if let Some(hd) = &instance.hd {
                let faces = model_faces_resolved(
                    &hd.materials,
                    hd.hide_fake_shadows,
                    &hd.override_,
                    posed.as_ref(),
                    ModelSite {
                        object_id: hd.object_id,
                        world: hd.world,
                        placement,
                        zone_origin_world: hd.zone_origin_world,
                    },
                );
                builder.push_hd_faces(
                    &faces,
                    [instance.local.0, instance.local.1, instance.local.2],
                );
            } else {
                builder.push_model(posed.as_ref(), placement);
            }""",
)

# Resolve animated HD state from the same HdContext used by static extraction. This keeps
# object/area override selection tied to the original loc anchor, exactly like the static path.
replace_once(
    "crates/osrs-world/src/extract.rs",
    """use crate::{AnimatedInstance, LocRenderable, TextureTable, WorldLoc, WorldScene};""",
    """use crate::{
    AnimatedHdState, AnimatedInstance, LocRenderable, TextureTable, WorldLoc, WorldScene,
    hd::HdContext,
};""",
)
replace_once(
    "crates/osrs-world/src/extract.rs",
    """pub fn extract_animated_instances(
    world: &WorldScene,
    owned: Option<OwnedTiles>,
) -> Vec<AnimatedInstance> {""",
    """pub fn extract_animated_instances(
    world: &WorldScene,
    owned: Option<OwnedTiles>,
    hd: Option<&HdContext>,
) -> Vec<AnimatedInstance> {""",
)
replace_once(
    "crates/osrs-world/src/extract.rs",
    """        for slot in loc_slots(loc, origin) {
            let LocRenderable::Animated(index) = slot.renderable else {
                continue;
            };
            let Some(model) = world.animated.get(index) else {
                continue;
            };
            instances.push(AnimatedInstance {
                model: model.clone(),
                zone: (zone_base_x + scene_zone_x, zone_base_z + scene_zone_z),
                level,
                min_plane,
                local: (slot.x, slot.y, slot.z),
                rotation: slot.rotation,
            });
        }""",
    """        let zone = (zone_base_x + scene_zone_x, zone_base_z + scene_zone_z);
        let world_tile = (
            world.window.base_x + loc.tile.x as i32,
            world.window.base_y + loc.tile.y as i32,
            i32::from(loc.plane.index().get()),
        );
        let animated_hd = hd.map(|hd| AnimatedHdState {
            materials: Arc::clone(&hd.materials),
            override_: Arc::new(
                hd.models
                    .get(loc.object_id.get(), world_tile, &hd.areas)
                    .clone(),
            ),
            hide_fake_shadows: hd.settings.hide_fake_shadows(),
            object_id: loc.object_id.get(),
            world: world_tile,
            zone_origin_world: (zone.0 * ZONE_LOCAL_UNITS, zone.1 * ZONE_LOCAL_UNITS),
        });
        for slot in loc_slots(loc, origin) {
            let LocRenderable::Animated(index) = slot.renderable else {
                continue;
            };
            let Some(model) = world.animated.get(index) else {
                continue;
            };
            instances.push(AnimatedInstance {
                model: model.clone(),
                hd: animated_hd.clone(),
                zone,
                level,
                min_plane,
                local: (slot.x, slot.y, slot.z),
                rotation: slot.rotation,
            });
        }""",
)
replace_once(
    "crates/osrs-world/src/extract.rs",
    """use osrs_scene::placement::PlacementKind;""",
    """use osrs_scene::placement::PlacementKind;
use std::sync::Arc;""",
)

# Region streaming owns the profile decision. Passing its existing borrowed HdContext into
# animation extraction means Reference regions keep the old path and HD regions capture the exact
# same material/override resolution as their static geometry.
replace_once(
    "crates/osrs-world/src/streaming.rs",
    """    let animations = extract_animated_instances(&world, owned);""",
    """    let animations = extract_animated_instances(&world, owned, hd);""",
)

# Remove this one-shot script from the final tree after it applies the patch.
Path("scripts/apply_animated_hd_parity.py").unlink()
