//! How far the legacy frames move the vertices of an animated model.
//! usage: pose_probe <cache> <base-x> <base-y> <sequence-id>
use osrs_core::animation_pose::pose_legacy_object_model;
use osrs_world::{SceneWindow, TerrainPresentation, WorldDefinitions, build_world_scene};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    let mut defs = WorldDefinitions::open(&a[1])?;
    let window = SceneWindow::new(a[2].parse()?, a[3].parse()?).ok_or("align")?;
    let seq: u32 = a[4].parse()?;
    let world = build_world_scene(&mut defs, window, TerrainPresentation::default())?;
    let Some(model) = world
        .animated
        .iter()
        .find(|m| m.sequence.identity.id.get() == seq)
    else {
        return Err("no model".into());
    };
    println!(
        "sequence {seq}: {} frames, delays {:?}, step {:?}, base vertices {}",
        model.frames.len(),
        model.sequence.frame_delays,
        model.sequence.frame_step,
        model.base.vertices.len()
    );
    println!(
        "vertex_skins present: {}",
        model.base.vertex_skins.is_some()
    );
    for (i, frame) in model.frames.iter().enumerate().take(8) {
        let posed = pose_legacy_object_model(&model.base, frame, model.orientation)?;
        let mut moved_low = 0;
        let mut moved_high = 0;
        let mut max_low = 0;
        for (b, p) in model.base.vertices.iter().zip(&posed.vertices) {
            let d = (b.x - p.x)
                .abs()
                .max((b.y - p.y).abs())
                .max((b.z - p.z).abs());
            if d > 0 {
                if b.y > -60 {
                    moved_low += 1;
                    max_low = max_low.max(d);
                } else {
                    moved_high += 1;
                }
            }
        }
        println!(
            "frame {i}: moved low(pole) {moved_low} (max {max_low}), moved high {moved_high}; transforms {}",
            frame.transforms.len()
        );
    }
    Ok(())
}
