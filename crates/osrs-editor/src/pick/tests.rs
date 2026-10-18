use super::*;

#[test]
fn ray_points_project_back_to_their_pixel() {
    let mut camera = FlyCamera::new(3000.0, -2500.0, 1800.0);
    camera.yaw = 0.7;
    camera.pitch = 0.9;
    camera.fov_degrees = 70.0;
    let origin = (128 * 200, 128 * 300);
    let (width, height) = (1280.0_f32, 720.0_f32);
    for (px, py) in [
        (640.0, 360.0),
        (10.0, 10.0),
        (1200.0, 700.0),
        (300.0, 500.0),
    ] {
        let ray = ray_through_pixel(&camera, origin, width, height, px, py);
        for distance in [500.0, 4000.0] {
            let point = [
                ray.origin[0] + ray.dir[0] * distance,
                ray.origin[1] + ray.dir[1] * distance,
                ray.origin[2] + ray.dir[2] * distance,
            ];
            let Some(projected) = project(&camera, origin, width, height, point) else {
                panic!("point is in front of the camera");
            };
            assert!(
                (projected[0] - px).abs() < 0.05,
                "{projected:?} vs {px},{py}"
            );
            assert!(
                (projected[1] - py).abs() < 0.05,
                "{projected:?} vs {px},{py}"
            );
        }
    }
}

#[test]
fn centre_pixel_ray_is_the_camera_forward_vector() {
    let mut camera = FlyCamera::new(0.0, -2000.0, 0.0);
    camera.yaw = -0.4;
    camera.pitch = 0.6;
    let ray = ray_through_pixel(&camera, (0, 0), 1000.0, 800.0, 500.0, 400.0);
    let forward = camera.forward();
    for (component, expected) in ray.dir.iter().zip(forward) {
        assert!((component - f64::from(expected)).abs() < 1e-5);
    }
}

/// Needs the pinned cache (`RUSTOSRS_TARGET_CACHE_DIR`).
#[test]
#[ignore = "requires the pinned build-241 cache"]
fn picking_straight_down_finds_the_object_under_the_ray() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = std::env::var("RUSTOSRS_TARGET_CACHE_DIR")?;
    let mut definitions = osrs_world::WorldDefinitions::open(dir)?;
    let region = osrs_core::coords::RegionCoord::new(50, 50);
    let built = osrs_world::build_region_geometry(
        &mut definitions,
        region,
        osrs_world::TerrainPresentation::default(),
    )?
    .ok_or("region 50,50 missing")?;
    let mut infos = Infos::new();
    infos.insert((50, 50), built.info.clone());
    let mut checked = 0;
    for loc in built
        .info
        .locs
        .iter()
        .filter(|loc| loc.min_plane == 0)
        .step_by(37)
    {
        let Some(slot) = loc.slots.first() else {
            continue;
        };
        let center = [
            f64::from(slot.min[0] + slot.max[0]) / 2.0,
            f64::from(slot.min[2] + slot.max[2]) / 2.0,
        ];
        let ray = Ray {
            origin: [center[0], -20_000.0, center[1]],
            dir: [0.0, 1.0, 0.0],
        };
        let pick = pick(&infos, &ray, 0, &osrs_render::gpu::RoofView::hidden_above());
        let Some(object) = pick.object else { continue };
        let hit = &infos[&object.region].locs[object.index];
        // The hit object's footprint must contain the ray column.
        let contained = hit.slots.iter().any(|s| {
            f64::from(s.min[0]) - 4.0 <= center[0]
                && center[0] <= f64::from(s.max[0]) + 4.0
                && f64::from(s.min[2]) - 4.0 <= center[1]
                && center[1] <= f64::from(s.max[2]) + 4.0
        });
        assert!(contained, "picked object does not contain the ray column");
        checked += 1;
    }
    assert!(checked > 20, "only {checked} locs were checked");
    Ok(())
}

/// Needs the pinned cache. Every sampled object, viewed from a camera in front of it, must
/// be pickable at the pixel its centre projects to (or something nearer must be).
#[test]
#[ignore = "requires the pinned build-241 cache"]
fn objects_are_pickable_at_their_projected_centre() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::var("RUSTOSRS_TARGET_CACHE_DIR")?;
    let mut definitions = osrs_world::WorldDefinitions::open(dir)?;
    let built = osrs_world::build_region_geometry(
        &mut definitions,
        osrs_core::coords::RegionCoord::new(40, 48),
        osrs_world::TerrainPresentation::default(),
    )?
    .ok_or("region 40,48 missing")?;
    let mut infos = Infos::new();
    infos.insert((40, 48), built.info.clone());
    let (mut total, mut missed) = (0, 0);
    for (index, loc) in built.info.locs.iter().enumerate() {
        if loc.min_plane != 0 || loc.plane != 0 {
            continue;
        }
        let Some(slot) = loc.slots.first() else {
            continue;
        };
        // Aim at the centroid of the slot's first drawn triangle (the box centre of a hollow
        // or thin model may lie in empty space).
        let center = match slot.triangles.first() {
            Some(triangle) => {
                let mut c = [0.0_f64; 3];
                for axis in 0..3 {
                    c[axis] = f64::from(slot.origin[axis])
                        + triangle.iter().map(|p| f64::from(p[axis])).sum::<f64>() / 3.0;
                }
                c
            }
            None => [
                f64::from(slot.min[0] + slot.max[0]) / 2.0,
                f64::from(slot.min[1] + slot.max[1]) / 2.0,
                f64::from(slot.min[2] + slot.max[2]) / 2.0,
            ],
        };
        let mut camera = FlyCamera::new(0.0, 0.0, 0.0);
        camera.pitch = 0.9;
        // 700 units south and 900 up, looking north at the centre.
        camera.x = center[0] as f32;
        camera.y = (center[1] - 900.0) as f32;
        camera.z = (center[2] - 700.0) as f32;
        let (width, height) = (1280.0_f32, 720.0_f32);
        let Some(pixel) = project(&camera, (0, 0), width, height, center) else {
            continue;
        };
        if !(0.0..width).contains(&pixel[0]) || !(0.0..height).contains(&pixel[1]) {
            continue;
        }
        let ray = ray_through_pixel(&camera, (0, 0), width, height, pixel[0], pixel[1]);
        let pick = pick(&infos, &ray, 0, &osrs_render::gpu::RoofView::hidden_above());
        total += 1;
        let ok = pick.object.as_ref().is_some_and(|object| {
            let hit = &infos[&object.region].locs[object.index];
            hit.slots
                .iter()
                .any(|s| f64::from(s.min[0]) - 4.0 <= center[0] + 700.0)
                && (object.index == index || ray_box_any(&ray, hit))
        });
        if !ok {
            missed += 1;
            if missed <= 8 {
                println!(
                    "missed: object {} at {:?} (picked {:?})",
                    loc.definition.identity.id.get(),
                    loc.tile,
                    pick.object.as_ref().map(|o| infos[&o.region].locs[o.index]
                        .definition
                        .identity
                        .id
                        .get())
                );
            }
        }
    }
    println!("{missed} of {total} sampled objects not pickable");
    assert!(
        missed * 20 <= total,
        "too many unpickable objects: {missed}/{total}"
    );
    Ok(())
}

fn ray_box_any(ray: &Ray, loc: &osrs_world::LocInfo) -> bool {
    loc.slots
        .iter()
        .any(|slot| ray_box(ray, slot.min, slot.max).is_some())
}

/// Needs the pinned cache (`RUSTOSRS_TARGET_CACHE_DIR`).
#[test]
#[ignore = "requires the pinned build-241 cache"]
fn a_drawn_roof_is_reported_and_a_hidden_one_is_not() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::var("RUSTOSRS_TARGET_CACHE_DIR")?;
    let mut definitions = osrs_world::WorldDefinitions::open(dir)?;
    let region = osrs_core::coords::RegionCoord::new(50, 50);
    let built = osrs_world::build_region_geometry(
        &mut definitions,
        region,
        osrs_world::TerrainPresentation::default(),
    )?
    .ok_or("region 50,50 missing")?;
    let mut infos = Infos::new();
    infos.insert((50, 50), built.info.clone());
    let roof_loc = built
        .info
        .locs
        .iter()
        .find(|loc| loc.min_plane > 0 && loc.roof != 0 && !loc.slots.is_empty())
        .ok_or("no roof piece in region 50,50")?;
    let slot = &roof_loc.slots[0];
    let centre = [
        f64::from(slot.min[0] + slot.max[0]) / 2.0,
        f64::from(slot.min[2] + slot.max[2]) / 2.0,
    ];
    let ray = Ray {
        origin: [centre[0], -20_000.0, centre[1]],
        dir: [0.0, 1.0, 0.0],
    };
    let drawn = pick(
        &infos,
        &ray,
        0,
        &osrs_render::gpu::RoofView::all_but(Default::default()),
    );
    assert!(
        drawn.roof.is_some(),
        "the roof over the ray column is not reported"
    );
    let cut = osrs_render::gpu::RoofView::all_but(drawn.roof.into_iter().collect());
    assert!(
        pick(&infos, &ray, 0, &cut).roof.is_none(),
        "a hidden roof must not be picked"
    );
    let hidden = pick(&infos, &ray, 0, &osrs_render::gpu::RoofView::hidden_above());
    assert!(hidden.roof.is_none());
    Ok(())
}
