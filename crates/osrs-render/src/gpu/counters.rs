//! Per-frame renderer counters for diagnostics. They record what the renderer did; no rendering
//! decision reads them.
use std::cell::Cell;

/// Work counted since the previous [`SceneRenderer::take_counters`](super::SceneRenderer::take_counters).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderCounters {
    /// `draw` and `draw_indexed` calls across the shadow and colour passes.
    pub draw_calls: u32,
    /// Vertices (or indices, for the sorted alpha pass) submitted by those calls.
    pub vertices: u64,
    /// Bytes written to scene buffers (zone vertex streams and alpha indices).
    pub upload_bytes: u64,
    /// Frames that re-rendered the shadow maps.
    pub shadow_redraws: u32,
    /// Zones that passed frustum and distance culling in the last encoded frame.
    pub visible_zones: u32,
}

#[derive(Default)]
pub(super) struct FrameCounters {
    draw_calls: Cell<u32>,
    vertices: Cell<u64>,
    upload_bytes: Cell<u64>,
    shadow_redraws: Cell<u32>,
    visible_zones: Cell<u32>,
}

impl FrameCounters {
    pub(super) fn draw(&self, vertices: u32) {
        self.draw_calls.set(self.draw_calls.get().saturating_add(1));
        self.vertices
            .set(self.vertices.get().saturating_add(u64::from(vertices)));
    }

    pub(super) fn upload(&self, bytes: usize) {
        self.upload_bytes
            .set(self.upload_bytes.get().saturating_add(bytes as u64));
    }

    pub(super) fn shadow_redraw(&self) {
        self.shadow_redraws
            .set(self.shadow_redraws.get().saturating_add(1));
    }

    pub(super) fn set_visible_zones(&self, zones: usize) {
        self.visible_zones.set(zones.min(u32::MAX as usize) as u32);
    }

    pub(super) fn take(&self) -> RenderCounters {
        RenderCounters {
            draw_calls: self.draw_calls.take(),
            vertices: self.vertices.take(),
            upload_bytes: self.upload_bytes.take(),
            shadow_redraws: self.shadow_redraws.take(),
            visible_zones: self.visible_zones.get(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_resets_accumulators_and_keeps_the_last_visible_zone_count() {
        let counters = FrameCounters::default();
        counters.draw(30);
        counters.draw(12);
        counters.upload(64);
        counters.shadow_redraw();
        counters.set_visible_zones(7);
        let first = counters.take();
        assert_eq!(
            first,
            RenderCounters {
                draw_calls: 2,
                vertices: 42,
                upload_bytes: 64,
                shadow_redraws: 1,
                visible_zones: 7,
            }
        );
        let second = counters.take();
        assert_eq!(
            (second.draw_calls, second.vertices, second.upload_bytes),
            (0, 0, 0)
        );
        assert_eq!(second.visible_zones, 7);
    }
}
