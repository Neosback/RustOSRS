//! Serialization of the existing form; canonical constructors/transport own semantic values.
use super::super::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
pub(in super::super) fn required_option<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}
pub(in super::super) mod locks {
    use super::*;
    pub(in super::super::super) fn serialize<S: Serializer>(
        locks: &[osrs_edit::ObjectPlacement],
        s: S,
    ) -> Result<S::Ok, S::Error> {
        let text = MapPatch::new(locks.iter().map(|p| MapOp::AddObject { placement: *p }))
            .to_patch_json_v1()
            .map_err(serde::ser::Error::custom)?;
        text.serialize(s)
    }
    pub(in super::super::super) fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Vec<osrs_edit::ObjectPlacement>, D::Error> {
        let text = String::deserialize(d)?;
        let patch = MapPatch::from_patch_json_v1(&text).map_err(serde::de::Error::custom)?;
        patch
            .ops()
            .iter()
            .map(|op| match op {
                MapOp::AddObject { placement } => Ok(*placement),
                _ => Err(serde::de::Error::custom(
                    "Locks must contain exact object occurrences only",
                )),
            })
            .collect()
    }
}
pub(in super::super) mod region {
    use super::*;
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Record {
        origin: [i32; 2],
        width: u16,
        height: u16,
    }
    pub(in super::super::super) fn serialize<S: Serializer>(
        site: &Option<RectSite>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        site.map(|r| Record {
            origin: [r.origin().x, r.origin().y],
            width: r.width(),
            height: r.height(),
        })
        .serialize(s)
    }
    pub(in super::super::super) fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<RectSite>, D::Error> {
        Option::<Record>::deserialize(d)?
            .map(|r| {
                RectSite::new(MapTile::new(r.origin[0], r.origin[1]), r.width, r.height)
                    .ok_or_else(|| serde::de::Error::custom("Invalid repair region"))
            })
            .transpose()
    }
}
pub(super) fn validate(form: &BuildForm, origin: MapTile) -> Result<(), String> {
    if !(1..=osrs_edit::MAX_PREFAB_SIDE).contains(&form.width)
        || !(1..=osrs_edit::MAX_PREFAB_SIDE).contains(&form.height)
    {
        return Err("Building form exceeds supported site/floor bounds".into());
    }
    form.floors
        .checked_add(1)
        .ok_or("Unsupported floor count")?;
    let site = RectSite::new(origin, form.width, form.height).ok_or("Invalid building site")?;
    // Canonical planners use a small neighbor/grade border around the site.
    if origin.x.checked_sub(128).is_none()
        || origin.y.checked_sub(128).is_none()
        || origin
            .x
            .checked_add(i32::from(site.width()) + 128)
            .is_none()
        || origin
            .y
            .checked_add(i32::from(site.height()) + 128)
            .is_none()
    {
        return Err("Site is too close to signed coordinate limits".into());
    }
    if form.locks.len() > 65536 {
        return Err("Too many locked occurrences".into());
    }
    if let Some(sketch) = &form.sketch {
        if sketch.floors.len() > usize::from(osrs_edit::MAX_PREFAB_SIDE).pow(2) * 4 {
            return Err("Sketch exceeds site cell bounds".into());
        }
        if sketch
            .floors
            .iter()
            .any(|c| c.x >= form.width || c.y >= form.height || c.plane > 3)
        {
            return Err("Sketch cells exceed the captured form".into());
        }
        RectSite::new(sketch.origin, form.width, form.height).ok_or("Invalid sketch origin")?;
    }
    Ok(())
}
