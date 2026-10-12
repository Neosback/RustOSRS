from pathlib import Path

path = Path('crates/osrs-editor/src/terrain_path/planner.rs')
text = path.read_text()

text = text.replace(
    '/// A tile that is part of the plan, with the cell that won it.\nstruct Chosen {',
    '/// A tile that is part of the plan, with the cell that won it.\n#[derive(Clone)]\nstruct Chosen {',
    1,
)

needle = '''fn seam_mismatch(a: u32, b: u32) -> bool {
    a != 0 && b != 0 && a & b == 0
}

'''
assert needle in text, 'seam_mismatch insertion point changed'
helpers = r'''fn seam_mismatch(a: u32, b: u32) -> bool {
    a != 0 && b != 0 && a & b == 0
}

/// Label cardinally connected selected tiles, keeping profiles separate.
fn coverage_component_labels(
    strong: &BTreeMap<(u8, i32, i32), Chosen>,
    weak: &BTreeMap<(u8, i32, i32), Chosen>,
) -> HashMap<Tile, usize> {
    let mut by_tile: HashMap<Tile, String> = HashMap::new();
    for (key, chosen) in strong.iter().chain(weak.iter()) {
        by_tile.insert((key.1, key.2, key.0), chosen.key.3.clone());
    }
    let mut order: Vec<Tile> = by_tile.keys().copied().collect();
    order.sort_by_key(tile_order);
    let mut labels = HashMap::new();
    let mut component = 0;
    for seed in order {
        if labels.contains_key(&seed) {
            continue;
        }
        let profile = by_tile[&seed].clone();
        let mut queue = VecDeque::from([seed]);
        labels.insert(seed, component);
        while let Some(current) = queue.pop_front() {
            for side in SIDES {
                let next = side_step(current, side);
                if labels.contains_key(&next)
                    || by_tile.get(&next).is_none_or(|candidate| candidate != &profile)
                {
                    continue;
                }
                labels.insert(next, component);
                queue.push_back(next);
            }
        }
        component += 1;
    }
    labels
}

/// Restore only low-coverage tiles that actually join two otherwise separate generated pieces.
/// This runs after the normal Terraini coverage rule, so already-connected parity cases are left
/// untouched.
fn repair_coverage_continuity(
    strong: &BTreeMap<(u8, i32, i32), Chosen>,
    weak: &mut BTreeMap<(u8, i32, i32), Chosen>,
    candidates: &BTreeMap<(u8, i32, i32), Chosen>,
) -> usize {
    let mut added = 0;
    loop {
        let labels = coverage_component_labels(strong, weak);
        let mut best: Option<((u8, i32, i32), Chosen)> = None;
        for (key, chosen) in candidates {
            if strong.contains_key(key) || weak.contains_key(key) {
                continue;
            }
            let tile = (key.1, key.2, key.0);
            let mut components = Vec::new();
            for side in SIDES {
                let next = side_step(tile, side);
                let next_key = tile_order(&next);
                let neighbour = strong.get(&next_key).or_else(|| weak.get(&next_key));
                let Some(neighbour) = neighbour else {
                    continue;
                };
                if neighbour.key.3 != chosen.key.3 {
                    continue;
                }
                if let Some(component) = labels.get(&next)
                    && !components.contains(component)
                {
                    components.push(*component);
                }
            }
            if components.len() < 2 {
                continue;
            }
            let replace = best.as_ref().is_none_or(|(best_key, best_chosen)| {
                chosen.coverage > best_chosen.coverage
                    || (chosen.coverage == best_chosen.coverage && key < best_key)
            });
            if replace {
                best = Some((*key, chosen.clone()));
            }
        }
        let Some((key, chosen)) = best else {
            break;
        };
        weak.insert(key, chosen);
        added += 1;
    }
    added
}

fn shape_component_labels(
    merged: &[(Tile, Chosen)],
    index_of: &HashMap<Tile, usize>,
    shapes: &[u32],
    atlas: &OverlayShapeAtlas,
) -> Vec<usize> {
    let mut labels = vec![usize::MAX; merged.len()];
    let mut component = 0;
    for seed in 0..merged.len() {
        if labels[seed] != usize::MAX {
            continue;
        }
        labels[seed] = component;
        let mut queue = VecDeque::from([seed]);
        while let Some(current) = queue.pop_front() {
            let (tile, chosen) = &merged[current];
            for side in SIDES {
                let next = side_step(*tile, side);
                let Some(&neighbour) = index_of.get(&next) else {
                    continue;
                };
                if labels[neighbour] != usize::MAX
                    || merged[neighbour].1.key.3 != chosen.key.3
                {
                    continue;
                }
                let mine = atlas.edge_mask(
                    (shapes[current] >> 2) as u8,
                    (shapes[current] & 3) as u8,
                    side,
                );
                let theirs = atlas.edge_mask(
                    (shapes[neighbour] >> 2) as u8,
                    (shapes[neighbour] & 3) as u8,
                    opposite(side),
                );
                if mine & theirs != 0 {
                    labels[neighbour] = component;
                    queue.push_back(neighbour);
                }
            }
        }
        component += 1;
    }
    labels
}

/// If side-adjacent generated tiles still belong to different shape components after normal seam
/// repair, make the boundary tiles whole. Distant components remain warnings rather than being
/// joined across empty or obstacle-masked space.
fn repair_shape_continuity(
    merged: &[(Tile, Chosen)],
    index_of: &HashMap<Tile, usize>,
    shapes: &mut [u32],
    atlas: &OverlayShapeAtlas,
) -> usize {
    let mut repaired = 0;
    loop {
        let labels = shape_component_labels(merged, index_of, shapes, atlas);
        let mut bridge = None;
        'outer: for (i, (tile, chosen)) in merged.iter().enumerate() {
            for side in [EAST, SOUTH] {
                let next = side_step(*tile, side);
                let Some(&j) = index_of.get(&next) else {
                    continue;
                };
                if merged[j].1.key.3 == chosen.key.3 && labels[i] != labels[j] {
                    bridge = Some((i, j));
                    break 'outer;
                }
            }
        }
        let Some((a, b)) = bridge else {
            break;
        };
        if shapes[a] == 0 && shapes[b] == 0 {
            break;
        }
        shapes[a] = 0;
        shapes[b] = 0;
        repaired += 1;
    }
    repaired
}

'''
text = text.replace(needle, helpers, 1)

old = '''    // A thinly covered tile only counts when three of its four sides are path (it fills a notch).
    let strong_keys: HashSet<Tile> = strong.keys().map(|k| (k.1, k.2, k.0)).collect();
    weak.retain(|k, _| {
        let tile = (k.1, k.2, k.0);
        SIDES
            .iter()
            .filter(|side| strong_keys.contains(&side_step(tile, **side)))
            .count()
            >= 3
    });
    // The plan's tiles, in plane, x, y order.
'''
new = '''    // A thinly covered tile only counts when three of its four sides are path (it fills a notch).
    // Keep the discarded candidates around so a single low-coverage bridge can repair an actual
    // break without changing normal connected Terraini output.
    let continuity_candidates = weak.clone();
    let strong_keys: HashSet<Tile> = strong.keys().map(|k| (k.1, k.2, k.0)).collect();
    weak.retain(|k, _| {
        let tile = (k.1, k.2, k.0);
        SIDES
            .iter()
            .filter(|side| strong_keys.contains(&side_step(tile, **side)))
            .count()
            >= 3
    });
    repair_coverage_continuity(&strong, &mut weak, &continuity_candidates);
    // The plan's tiles, in plane, x, y order.
'''
assert old in text, 'coverage filtering block changed'
text = text.replace(old, new, 1)

old = '''    for i in whole {
        shapes[i] = 0;
    }

    // The paints.
'''
new = '''    for i in whole {
        shapes[i] = 0;
    }
    repair_shape_continuity(&merged, &index_of, &mut shapes, atlas);

    // The paints.
'''
assert old in text, 'shape repair insertion point changed'
text = text.replace(old, new, 1)

old = '''    #[test]
    fn network_policy_adds_an_interior_connector_without_bending_the_main_line() {
'''
assert old in text, 'planner test insertion point changed'
extra_tests = r'''    #[test]
    fn continuity_repair_promotes_a_single_weak_cardinal_bridge() {
        let chosen = |x: i32, y: i32, coverage: f64| Chosen {
            key: (x, y, 0, "p".to_owned()),
            cell_index: 0,
            coverage,
        };
        let mut strong = BTreeMap::new();
        strong.insert((0, 0, 0), chosen(0, 0, 1.0));
        strong.insert((0, 1, 1), chosen(1, 1, 1.0));
        let weak = BTreeMap::new();
        let mut candidates = BTreeMap::new();
        candidates.insert((0, 1, 0), chosen(1, 0, 0.08));
        candidates.insert((0, 0, 1), chosen(0, 1, 0.06));

        let mut repaired = weak;
        assert_eq!(repair_coverage_continuity(&strong, &mut repaired, &candidates), 1);
        assert!(repaired.contains_key(&(0, 1, 0)));
        let labels = coverage_component_labels(&strong, &repaired);
        let unique: HashSet<_> = labels.values().copied().collect();
        assert_eq!(unique.len(), 1);
    }

    #[test]
    fn shape_continuity_repair_joins_side_adjacent_disconnected_shapes() {
        let atlas = OverlayShapeAtlas::new(8);
        let mut pair = None;
        'find: for a_shape in 0..12_u8 {
            for a_orientation in 0..4_u8 {
                for b_shape in 0..12_u8 {
                    for b_orientation in 0..4_u8 {
                        if atlas.edge_mask(a_shape, a_orientation, EAST)
                            & atlas.edge_mask(b_shape, b_orientation, WEST)
                            == 0
                        {
                            pair = Some((a_shape, a_orientation, b_shape, b_orientation));
                            break 'find;
                        }
                    }
                }
            }
        }
        let (a_shape, a_orientation, b_shape, b_orientation) = pair.expect("shape pair");
        let chosen = |x: i32| Chosen {
            key: (x, 0, 0, "p".to_owned()),
            cell_index: 0,
            coverage: 1.0,
        };
        let merged = vec![((0, 0, 0), chosen(0)), ((1, 0, 0), chosen(1))];
        let index_of = HashMap::from([((0, 0, 0), 0), ((1, 0, 0), 1)]);
        let mut shapes = vec![
            (u32::from(a_shape) << 2) | u32::from(a_orientation),
            (u32::from(b_shape) << 2) | u32::from(b_orientation),
        ];
        let before = shape_component_labels(&merged, &index_of, &shapes, &atlas);
        assert_ne!(before[0], before[1]);
        assert_eq!(
            repair_shape_continuity(&merged, &index_of, &mut shapes, &atlas),
            1
        );
        let after = shape_component_labels(&merged, &index_of, &shapes, &atlas);
        assert_eq!(after[0], after[1]);
    }

    #[test]
    fn network_policy_adds_an_interior_connector_without_bending_the_main_line() {
'''
text = text.replace(old, extra_tests, 1)

path.write_text(text)
