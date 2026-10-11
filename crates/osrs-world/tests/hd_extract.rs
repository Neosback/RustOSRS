//! HD extraction against the real Lumbridge scene (needs the pinned build-241 cache).

use osrs_hd::settings::HdSettings;
use osrs_world::{
    SceneWindow, TerrainPresentation, WorldDefinitions, build_world_scene,
    hd::{HdContext, default_assets_dir, extract::extract_owned_geometry_hd},
};
use std::collections::BTreeMap;

#[test]
#[ignore = "requires the pinned build-241 cache (RUSTOSRS_TARGET_CACHE_DIR)"]
fn lumbridge_materials() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::var("RUSTOSRS_TARGET_CACHE_DIR")?;
    let mut definitions = WorldDefinitions::open(dir)?;
    let window = SceneWindow::new(3176, 3176).ok_or("window alignment")?;
    let world = build_world_scene(&mut definitions, window, TerrainPresentation::default())?;
    let started = std::time::Instant::now();
    let textures: Vec<i32> = (0..definitions.textures().images.len() as i32).collect();
    let hd = HdContext::load(&default_assets_dir(), HdSettings::default(), textures)?;
    println!("hd context {:?}", started.elapsed());
    let started = std::time::Instant::now();
    let geometry = extract_owned_geometry_hd(&world, None, &hd);
    println!("hd extraction {:?}", started.elapsed());
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut total = 0;
    for zone in &geometry.zones {
        for group in &zone.groups {
            assert_eq!(group.geometry.opaque.len(), group.geometry.hd_opaque.len());
            assert_eq!(group.geometry.alpha.len(), group.geometry.hd_alpha.len());
            for vertex in group.geometry.hd_opaque.iter().chain(&group.geometry.hd_alpha).step_by(3) {
                let index = usize::from(vertex.material_water[0] & 0x0FFF);
                let name = hd.materials.get(index).map_or("NONE", |m| m.name.as_str());
                *counts.entry(name.to_owned()).or_default() += 1;
                total += 1;
            }
        }
    }
    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    println!("{total} faces");
    for (name, count) in sorted.iter().take(25) {
        println!("{count:7} {name}");
    }
    Ok(())
}

#[test]
#[ignore = "requires the pinned build-241 cache (RUSTOSRS_TARGET_CACHE_DIR)"]
fn winding_statistics() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::var("RUSTOSRS_TARGET_CACHE_DIR")?;
    let mut definitions = WorldDefinitions::open(dir)?;
    let window = SceneWindow::new(3176, 3176).ok_or("window alignment")?;
    let world = build_world_scene(&mut definitions, window, TerrainPresentation::default())?;
    let geometry = osrs_world::extract_scene_geometry(&world);
    let (mut ccw, mut cw) = (0, 0);
    for zone in &geometry.zones {
        for group in &zone.groups {
            for tri in group.geometry.opaque.chunks(3) {
                let p = |i: usize| (tri[i].position[0] as i64, tri[i].position[1] as i64, tri[i].position[2] as i64);
                let (a, b, c) = (p(0), p(1), p(2));
                if a.1 == b.1 && b.1 == c.1 {
                    let o = (b.0 - a.0) * (c.2 - a.2) - (b.2 - a.2) * (c.0 - a.0);
                    if o > 0 { ccw += 1 } else if o < 0 { cw += 1 }
                }
            }
        }
    }
    println!("flat faces: x-z orientation positive {ccw}, negative {cw}");
    Ok(())
}

#[test]
#[ignore = "requires the pinned build-241 cache (RUSTOSRS_TARGET_CACHE_DIR)"]
fn normal_consistency() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::var("RUSTOSRS_TARGET_CACHE_DIR")?;
    let mut definitions = WorldDefinitions::open(dir)?;
    let window = SceneWindow::new(3176, 3176).ok_or("window alignment")?;
    let world = build_world_scene(&mut definitions, window, TerrainPresentation::default())?;
    let textures: Vec<i32> = (0..definitions.textures().images.len() as i32).collect();
    let hd = HdContext::load(&default_assets_dir(), HdSettings::default(), textures)?;
    let geometry = extract_owned_geometry_hd(&world, None, &hd);
    let (mut agree, mut disagree) = (0, 0);
    let mut sample = None;
    for zone in &geometry.zones {
        for group in &zone.groups {
            for (tri, hd_tri) in group.geometry.opaque.chunks(3).zip(group.geometry.hd_opaque.chunks(3)) {
                let p = |i: usize| (tri[i].position[0] as i64, tri[i].position[1] as i64, tri[i].position[2] as i64);
                let (a, b, c) = (p(0), p(1), p(2));
                if a.1 == b.1 && b.1 == c.1 {
                    let o = (b.0 - a.0) * (c.2 - a.2) - (b.2 - a.2) * (c.0 - a.0);
                    let normal = osrs_render::oct_decode(hd_tri[0].normal);
                    if o > 0 && normal[1] > 0.5 {
                        disagree += 1;
                        sample.get_or_insert((a, b, c, normal, hd_tri[0].hsl_flags[3], hd_tri[0].material_water[3]));
                    } else if o > 0 && normal[1] < -0.5 {
                        agree += 1;
                    }
                }
            }
        }
    }
    println!("o>0 faces: normal up {agree}, normal down {disagree}; sample {sample:?}");
    let (mut t_up, mut t_down, mut m_up, mut m_down) = (0, 0, 0, 0);
    let mut model_sample = None;
    for zone in &geometry.zones {
        for group in &zone.groups {
            for (tri, hd_tri) in group.geometry.opaque.chunks(3).zip(group.geometry.hd_opaque.chunks(3)) {
                let terrain = hd_tri[0].hsl_flags[3] & 8 != 0;
                let normal = osrs_render::oct_decode(hd_tri[0].normal);
                match (terrain, normal[1] < 0.0) {
                    (true, true) => t_up += 1,
                    (true, false) => {
                        t_down += 1;
                        if t_down % 150 == 1 {
                            println!("terrain down: {:?} {:?} {:?} plane {} normal {normal:?}", tri[0].position, tri[1].position, tri[2].position, hd_tri[0].material_water[3] >> 8);
                        }
                    }
                    (false, true) => m_up += 1,
                    (false, false) => {
                        m_down += 1;
                        model_sample.get_or_insert((tri[0].position, tri[1].position, tri[2].position, normal));
                    }
                }
            }
        }
    }
    println!("terrain up {t_up} down {t_down}; models up {m_up} down {m_down}; {model_sample:?}");
    Ok(())
}
