//! Bounded repair distinguishes user locks from disposable solver suggestions.
use super::*;

#[derive(Debug, Clone, Copy)]
pub struct WfcRepairRequest {
    pub width: u16,
    pub height: u16,
    pub seed: u64,
    pub max_backtracks: u32,
    pub max_repairs: u32,
    pub radius: u16,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WfcRepairReport {
    pub solution: WfcSolution,
    /// Exact suggestions relaxed during this solve. Author locks never appear here.
    pub relaxed: Vec<WfcConstraint>,
}

pub fn repair_wfc(
    catalog: &WfcCatalog,
    request: WfcRepairRequest,
    locks: &[WfcConstraint],
    suggestions: &[WfcConstraint],
) -> Result<WfcRepairReport, WfcSolveError> {
    repair_wfc_masked(
        catalog,
        request,
        locks,
        suggestions,
        &vec![true; usize::from(request.width) * usize::from(request.height)],
    )
}

pub fn repair_wfc_masked(
    catalog: &WfcCatalog,
    request: WfcRepairRequest,
    locks: &[WfcConstraint],
    suggestions: &[WfcConstraint],
    mask: &[bool],
) -> Result<WfcRepairReport, WfcSolveError> {
    let mut active = suggestions.to_vec();
    let mut relaxed = Vec::new();
    let mut seed = request.seed;
    let mut attempt = 0;
    loop {
        let constraints: Vec<_> = locks.iter().chain(&active).copied().collect();
        let result = solve_wfc_masked(
            catalog,
            WfcRepairRequest { seed, ..request },
            mask,
            &constraints,
            &mut |_| {},
        );
        let error = match result {
            Ok(solution) => return Ok(WfcRepairReport { solution, relaxed }),
            Err(error) => error,
        };
        let coord = match error {
            WfcSolveError::Contradiction { coord, .. }
            | WfcSolveError::ConstraintHasNoValidPatterns { coord } => coord,
            _ => return Err(error),
        };
        if attempt == request.max_repairs {
            return Err(error);
        }
        let before = active.len();
        active.retain(|c| {
            if c.coord.x.abs_diff(coord.x) <= request.radius
                && c.coord.y.abs_diff(coord.y) <= request.radius
            {
                relaxed.push(*c);
                false
            } else {
                true
            }
        });
        if active.len() == before {
            return Err(error);
        }
        attempt += 1;
        seed = SplitMix64::new(seed).next_u64();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn catalog() -> WfcCatalog {
        WfcCatalog::new([0, 1].map(|socket| WfcPatternSpec {
            weight: 1,
            sockets: [WfcSocket(socket); 4],
        }))
        .unwrap()
    }
    fn pin(x: u16, pattern: u16) -> WfcConstraint {
        WfcConstraint {
            coord: WfcCoord::new(x, 0),
            allowed: PatternDomain::singleton(WfcPatternId::new(pattern).unwrap()),
        }
    }
    fn request() -> WfcRepairRequest {
        WfcRepairRequest {
            width: 6,
            height: 1,
            seed: 5,
            max_backtracks: 8,
            max_repairs: 2,
            radius: 2,
        }
    }
    #[test]
    fn conflicting_author_locks_fail_instead_of_disappearing() {
        assert!(repair_wfc(&catalog(), request(), &[pin(0, 0), pin(1, 1)], &[]).is_err());
    }
    #[test]
    fn repair_reports_only_soft_relaxations_and_preserves_every_lock() {
        let report =
            repair_wfc(&catalog(), request(), &[pin(5, 0)], &[pin(0, 0), pin(1, 1)]).unwrap();
        assert!(!report.relaxed.is_empty());
        assert!(report.relaxed.iter().all(|p| p.coord.x < 3));
        assert_eq!(
            report.solution.at(WfcCoord::new(5, 0)),
            WfcPatternId::new(0)
        );
        assert_eq!(
            report,
            repair_wfc(&catalog(), request(), &[pin(5, 0)], &[pin(0, 0), pin(1, 1)]).unwrap()
        );
    }
    #[test]
    fn sparse_holes_disconnect_domains_and_never_become_floor_output() {
        let request = WfcRepairRequest {
            width: 3,
            height: 1,
            ..request()
        };
        let mask = [true, false, true];
        let result =
            repair_wfc_masked(&catalog(), request, &[pin(0, 0), pin(2, 1)], &[], &mask).unwrap();
        assert_eq!(
            result.solution.at(WfcCoord::new(0, 0)),
            WfcPatternId::new(0)
        );
        assert_eq!(result.solution.at(WfcCoord::new(1, 0)), None);
        assert_eq!(
            result.solution.at(WfcCoord::new(2, 0)),
            WfcPatternId::new(1)
        );
        assert!(repair_wfc_masked(&catalog(), request, &[pin(1, 0)], &[], &mask).is_err());
    }
}
