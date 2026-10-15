//! Bounded, optional observations shared by the main-thread runtime owners. No observer can
//! mutate product state. Workers never receive this handle or acquire a lock on the document.
use crate::app::edit_runtime::EditFootprint;
use osrs_core::coords::{MapTile, RegionCoord};
use osrs_edit::ChangeImpact;
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

const TRACE_CAPACITY: usize = 256;

#[derive(Debug, Clone)]
pub(in crate::app) enum TraceEvent {
    Edit {
        operation: &'static str,
        before: u64,
        after: u64,
        outcome: &'static str,
        impact: ChangeImpact,
        footprint: EditFootprint,
    },
    Invalidation {
        revision: u64,
        regions: Vec<RegionCoord>,
        reason: &'static str,
    },
    Build {
        source: u64,
        request: u64,
        region: RegionCoord,
        revision: u64,
        epoch: u64,
        generation: u32,
        outcome: &'static str,
    },
}

#[derive(Debug, Clone, Default)]
pub(in crate::app) struct DiagnosticSink(Option<Rc<RefCell<VecDeque<TraceEvent>>>>);

impl DiagnosticSink {
    pub(in crate::app) fn enabled() -> Self {
        Self(Some(Rc::new(RefCell::new(VecDeque::with_capacity(
            TRACE_CAPACITY,
        )))))
    }
    pub(in crate::app) fn clear(&self) {
        if let Some(log) = &self.0 {
            log.borrow_mut().clear();
        }
    }
    pub(in crate::app) fn is_enabled(&self) -> bool {
        self.0.is_some()
    }
    pub(in crate::app) fn record(&self, make: impl FnOnce() -> TraceEvent) {
        if let Some(log) = &self.0 {
            let mut log = log.borrow_mut();
            if log.len() == TRACE_CAPACITY {
                log.pop_front();
            }
            log.push_back(make());
        }
    }
    /// Exact tile edits are distinguished from region-wide rebuild observations. A missing entry
    /// means no matching event is retained, not that the source tile has never been edited.
    pub(in crate::app) fn explain(&self, tile: MapTile, plane: u8) -> Vec<String> {
        let Some(log) = &self.0 else {
            return Vec::new();
        };
        let region = tile.split_region().0;
        let mut lines = Vec::new();
        for event in log.borrow().iter().rev() {
            match event {
                TraceEvent::Edit {
                    operation,
                    before,
                    after,
                    outcome,
                    impact,
                    footprint,
                } if footprint.touches(tile, plane) => {
                    lines.push(format!(
                        "{operation}: {outcome}; revision {before} → {after}; impact {:02x}",
                        impact.bits()
                    ));
                }
                TraceEvent::Invalidation {
                    revision,
                    regions,
                    reason,
                } if regions.contains(&region) => {
                    lines.push(format!("revision {revision}: {reason}"));
                }
                TraceEvent::Build {
                    source,
                    request,
                    region: built,
                    revision,
                    epoch,
                    generation,
                    outcome,
                } if *built == region => {
                    lines.push(format!("build {source}/{request}: {outcome}; revision {revision}, epoch {epoch}, presentation {generation}"));
                }
                _ => {}
            }
            if lines.len() == 12 {
                break;
            }
        }
        lines
    }
}

impl EditFootprint {
    fn touches(&self, tile: MapTile, plane: u8) -> bool {
        self.terrain_positions
            .iter()
            .any(|position| position.tile == tile && position.plane.index().get() == plane)
            || self
                .height_vertices
                .iter()
                .any(|v| v.corner == tile && v.plane.index().get() == plane)
            || self
                .object_removed
                .iter()
                .chain(&self.object_added)
                .any(|p| p.position.tile == tile && p.position.plane.index().get() == plane)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn build(request: u64) -> TraceEvent {
        TraceEvent::Build {
            source: 0,
            request,
            region: RegionCoord::new(0, 0),
            revision: 7,
            epoch: 2,
            generation: 3,
            outcome: "stale: newer edit",
        }
    }
    #[test]
    fn disabled_sink_does_not_evaluate_or_allocate_event() {
        DiagnosticSink::default().record(|| panic!("disabled diagnostics must be lazy"));
    }
    #[test]
    fn trace_is_bounded_and_shared_handles_observe_the_same_log() {
        let sink = DiagnosticSink::enabled();
        let producer = sink.clone();
        for request in 0..300 {
            producer.record(|| build(request));
        }
        let log = sink.0.as_ref().unwrap_or_else(|| unreachable!()).borrow();
        assert_eq!(log.len(), TRACE_CAPACITY);
        assert!(matches!(
            log.front(),
            Some(TraceEvent::Build { request: 44, .. })
        ));
        assert!(matches!(
            log.back(),
            Some(TraceEvent::Build { request: 299, .. })
        ));
    }
    #[test]
    fn tile_explanation_keeps_region_and_transaction_correlation() {
        let sink = DiagnosticSink::enabled();
        let tile = MapTile::new(1, 2);
        sink.record(|| TraceEvent::Edit {
            operation: "paint",
            before: 6,
            after: 7,
            outcome: "committed",
            impact: ChangeImpact::TERRAIN_MATERIAL,
            footprint: EditFootprint::new([tile], [], []).with_terrain_positions([
                osrs_edit::MapPosition::new(
                    tile,
                    osrs_core::coords::SourcePlane::new(0).unwrap_or_else(|| unreachable!()),
                ),
            ]),
        });
        sink.record(|| build(12));
        let lines = sink.explain(tile, 0);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("build 0/12: stale: newer edit"));
        assert!(lines[1].contains("paint: committed; revision 6 → 7"));
        assert!(sink.explain(MapTile::new(64, 0), 0).is_empty());
        assert!(
            sink.explain(tile, 1)
                .iter()
                .all(|line| !line.contains("paint:"))
        );
    }
}
