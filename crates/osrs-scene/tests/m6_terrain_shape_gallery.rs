use osrs_core::coords::SceneTile;
use osrs_scene::{
    ShapedTerrainInput, ShapedTerrainSurface, TerrainCorners, TerrainFace, TerrainVertex,
};

const HISTORICAL_GOLDEN: &str = include_str!("../../../reference-fixtures/deob_golden.txt");

#[test]
fn all_13_shapes_and_4_rotations_match_historical_golden_exactly() {
    let expected: Vec<&str> = HISTORICAL_GOLDEN
        .lines()
        .filter(|line| line.starts_with("tri shape="))
        .collect();
    assert_eq!(expected.len(), 52, "historical terrain gallery must contain 13x4 cases");

    let mut actual = Vec::with_capacity(52);
    for shape in 0..=12 {
        for rotation in 0..=3 {
            let surface = ShapedTerrainSurface::build(ShapedTerrainInput {
                shape,
                rotation,
                texture_id: None,
                tile: SceneTile::new(0, 0),
                heights: TerrainCorners::new(64, 64, 64, 64),
                underlay_colors: TerrainCorners::new(1000, 1001, 1002, 1003),
                overlay_colors: TerrainCorners::new(2000, 2001, 2002, 2003),
                underlay_rgb: 0,
                overlay_rgb: 0,
            })
            .expect("canonical gallery input is valid");
            actual.push(format_surface(&surface));
        }
    }

    assert_eq!(actual.iter().map(String::as_str).collect::<Vec<_>>(), expected);
}

fn format_surface(surface: &ShapedTerrainSurface) -> String {
    format!(
        "tri shape={} rot={} nverts={} vx={} vy={} vz={} faces={} fx={} fy={} fz={} ca={} cb={} cc={} tex={} flat={}",
        surface.shape,
        surface.rotation,
        surface.vertices.len(),
        join_vertices(&surface.vertices, |vertex| vertex.position.x.units()),
        join_vertices(&surface.vertices, |vertex| vertex.position.y.units()),
        join_vertices(&surface.vertices, |vertex| vertex.position.z.units()),
        surface.faces.len(),
        join_faces(&surface.faces, |face| face.indices[0].to_string()),
        join_faces(&surface.faces, |face| face.indices[1].to_string()),
        join_faces(&surface.faces, |face| face.indices[2].to_string()),
        join_faces(&surface.faces, |face| face.colors[0].to_string()),
        join_faces(&surface.faces, |face| face.colors[1].to_string()),
        join_faces(&surface.faces, |face| face.colors[2].to_string()),
        if surface.faces.iter().all(|face| face.texture_id.is_none()) {
            "null".to_owned()
        } else {
            join_faces(&surface.faces, |face| {
                face.texture_id.map_or_else(|| "-1".to_owned(), |value| value.to_string())
            })
        },
        surface.is_flat
    )
}

fn join_vertices(
    vertices: &[TerrainVertex],
    value: impl Fn(&TerrainVertex) -> i32,
) -> String {
    vertices
        .iter()
        .map(|vertex| value(vertex).to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn join_faces(faces: &[TerrainFace], value: impl Fn(&TerrainFace) -> String) -> String {
    faces.iter().map(value).collect::<Vec<_>>().join(",")
}
