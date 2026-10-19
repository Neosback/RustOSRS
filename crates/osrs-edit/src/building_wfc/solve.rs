//! The shared masked and rectangular solver. Holes never participate in adjacency.
use super::*;

/// Solve a rectangle, reporting the same decisions as the unobserved solver.
pub fn solve_wfc_observed(
    catalog: &WfcCatalog,
    width: u16,
    height: u16,
    seed: u64,
    constraints: &[WfcConstraint],
    max_backtracks: u32,
    observe: &mut dyn FnMut(WfcEvent<'_>),
) -> Result<WfcSolution, WfcSolveError> {
    solve_wfc_masked(
        catalog,
        WfcRepairRequest {
            width,
            height,
            seed,
            max_backtracks,
            max_repairs: 0,
            radius: 2,
        },
        &vec![true; usize::from(width) * usize::from(height)],
        constraints,
        observe,
    )
}

/// Solve only active cells; holes carry no output and never contribute adjacency evidence.
pub fn solve_wfc_masked(
    catalog: &WfcCatalog,
    request: WfcRepairRequest,
    active: &[bool],
    constraints: &[WfcConstraint],
    observe: &mut dyn FnMut(WfcEvent<'_>),
) -> Result<WfcSolution, WfcSolveError> {
    let WfcRepairRequest {
        width,
        height,
        seed,
        max_backtracks,
        ..
    } = request;
    if width == 0 || height == 0 {
        return Err(WfcSolveError::InvalidDimensions { width, height });
    }
    let cell_count = usize::from(width)
        .checked_mul(usize::from(height))
        .ok_or(WfcSolveError::CellCountOverflow)?;
    if active.len() != cell_count || !active.iter().any(|b| *b) {
        return Err(WfcSolveError::InvalidDimensions { width, height });
    }
    let all_patterns = catalog.all_patterns();
    let mut domains = vec![all_patterns; cell_count];

    let filler = all_patterns
        .iter()
        .next()
        .ok_or(WfcSolveError::ConstraintHasNoValidPatterns {
            coord: WfcCoord::new(0, 0),
        })?;
    for (domain, enabled) in domains.iter_mut().zip(active) {
        if !enabled {
            *domain = PatternDomain::singleton(filler);
        }
    }
    for constraint in constraints {
        let Some(index) = grid_index(width, height, constraint.coord) else {
            return Err(WfcSolveError::ConstraintOutsideGrid {
                coord: constraint.coord,
            });
        };
        if !active[index] {
            return Err(WfcSolveError::ConstraintOutsideGrid {
                coord: constraint.coord,
            });
        }
        let narrowed = domains[index].intersect(constraint.allowed.intersect(all_patterns));
        if narrowed.is_empty() {
            return Err(WfcSolveError::ConstraintHasNoValidPatterns {
                coord: constraint.coord,
            });
        }
        domains[index] = narrowed;
    }

    let initial_queue = (0..cell_count).collect::<VecDeque<_>>();
    if let Err(cell) = propagate(catalog, width, height, active, &mut domains, initial_queue) {
        return Err(WfcSolveError::Contradiction {
            coord: coord_from_index(width, cell),
            backtracks: 0,
        });
    }

    observe(WfcEvent::Start { domains: &domains });
    let mut rng = SplitMix64::new(seed);
    let mut decisions = Vec::<DecisionFrame>::new();
    let mut backtracks = 0_u32;

    loop {
        let Some(cell) = select_lowest_entropy_cell(catalog, &domains) else {
            let cells = domains
                .iter()
                .map(|domain| {
                    domain
                        .iter()
                        .next()
                        .unwrap_or_else(|| unreachable!("solved domains are singletons"))
                })
                .collect();
            return Ok(WfcSolution {
                width,
                height,
                cells,
                active: active.to_vec(),
                backtracks,
            });
        };

        let before = domains.clone();
        let options = domains[cell].len();
        let chosen = choose_weighted(catalog, domains[cell], &mut rng);
        let remaining = domains[cell].without(chosen);
        if !remaining.is_empty() {
            decisions.push(DecisionFrame {
                domains: before,
                cell,
                remaining,
            });
        }
        domains[cell] = PatternDomain::singleton(chosen);

        let propagated = propagate(
            catalog,
            width,
            height,
            active,
            &mut domains,
            VecDeque::from([cell]),
        );
        match propagated {
            Ok(()) => observe(WfcEvent::Collapse {
                cell: coord_from_index(width, cell),
                chosen,
                options,
                domains: &domains,
            }),
            Err(contradiction) => observe(WfcEvent::Contradiction {
                cell: coord_from_index(width, contradiction),
            }),
        }
        if let Err(contradiction) = propagated {
            restore_after_contradiction(
                catalog,
                width,
                height,
                active,
                &mut domains,
                &mut decisions,
                &mut rng,
                &mut backtracks,
                max_backtracks,
                contradiction,
            )?;
            observe(WfcEvent::Backtrack { domains: &domains });
        }
    }
}
