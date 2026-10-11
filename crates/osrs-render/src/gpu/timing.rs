//! GPU time of the scene pass, measured with timestamp queries.
//!
//! The pass writes a timestamp at its start and end; the two values are resolved into a buffer,
//! copied to one of a few readback buffers, and mapped after the frame is submitted. Results
//! arrive a frame or two later, so the readback buffers form a small ring and the newest finished
//! measurement (smoothed) is published through an atomic.

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, Ordering},
};

const SLOTS: usize = 3;
const SMOOTHING: f32 = 0.1;

struct Slot {
    buffer: wgpu::Buffer,
    busy: Arc<AtomicBool>,
}

pub(super) struct GpuTimer {
    query_set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    slots: Vec<Slot>,
    period_ns: f32,
    result_bits: Arc<AtomicU32>,
    next: usize,
    active: Option<usize>,
}

impl GpuTimer {
    /// `None` when the device was created without `TIMESTAMP_QUERY`.
    pub(super) fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("scene-timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: 2,
        });
        let make = |label: &str, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: 16,
                usage,
                mapped_at_creation: false,
            })
        };
        let resolve = make(
            "scene-timestamps-resolve",
            wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        );
        let slots = (0..SLOTS)
            .map(|_| Slot {
                buffer: make(
                    "scene-timestamps-readback",
                    wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                ),
                busy: Arc::new(AtomicBool::new(false)),
            })
            .collect();
        Some(Self {
            query_set,
            resolve,
            slots,
            period_ns: queue.get_timestamp_period(),
            result_bits: Arc::new(AtomicU32::new(0)),
            next: 0,
            active: None,
        })
    }

    /// Reserve a readback slot for this frame; `false` when all are still in flight.
    pub(super) fn begin(&mut self) -> bool {
        for offset in 0..SLOTS {
            let index = (self.next + offset) % SLOTS;
            if !self.slots[index].busy.load(Ordering::Acquire) {
                self.next = (index + 1) % SLOTS;
                self.active = Some(index);
                return true;
            }
        }
        self.active = None;
        false
    }

    pub(super) fn pass_writes(&self) -> wgpu::RenderPassTimestampWrites<'_> {
        wgpu::RenderPassTimestampWrites {
            query_set: &self.query_set,
            beginning_of_pass_write_index: Some(0),
            end_of_pass_write_index: Some(1),
        }
    }

    /// Resolve the pass timestamps into the frame's readback slot.
    pub(super) fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        let Some(index) = self.active else { return };
        encoder.resolve_query_set(&self.query_set, 0..2, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.slots[index].buffer, 0, 16);
    }

    /// Start mapping the frame's slot (call after the encoder was submitted) and collect results.
    pub(super) fn finish(&mut self, device: &wgpu::Device) {
        if let Some(index) = self.active.take() {
            let slot = &self.slots[index];
            slot.busy.store(true, Ordering::Release);
            let busy = slot.busy.clone();
            let buffer = slot.buffer.clone();
            let result = self.result_bits.clone();
            let period = self.period_ns;
            slot.buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |status| {
                    if status.is_ok() {
                        let ms = buffer
                            .slice(..)
                            .get_mapped_range()
                            .ok()
                            .map_or(0.0, |view| {
                                let ticks: &[u64] = bytemuck::cast_slice(&view);
                                ticks[1].saturating_sub(ticks[0]) as f32 * period / 1.0e6
                            });
                        buffer.unmap();
                        let previous = f32::from_bits(result.load(Ordering::Relaxed));
                        let smoothed = if previous == 0.0 {
                            ms
                        } else {
                            previous + (ms - previous) * SMOOTHING
                        };
                        result.store(smoothed.to_bits(), Ordering::Relaxed);
                    }
                    busy.store(false, Ordering::Release);
                });
        }
        let _ = device.poll(wgpu::PollType::Poll);
    }

    pub(super) fn scene_ms(&self) -> Option<f32> {
        let ms = f32::from_bits(self.result_bits.load(Ordering::Relaxed));
        (ms > 0.0).then_some(ms)
    }
}
