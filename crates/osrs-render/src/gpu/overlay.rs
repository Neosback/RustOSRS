//! Line overlay for editor guides (tile / zone / region grid, bounds).

use super::{COLOR_FORMAT, DEPTH_FORMAT, MSAA_SAMPLES};
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// One line endpoint: render-local position and straight-alpha colour.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct LineVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

impl LineVertex {
    const SIZE: u64 = std::mem::size_of::<Self>() as u64;
}

pub(super) struct LineOverlay {
    pipeline: wgpu::RenderPipeline,
    buffer: Option<wgpu::Buffer>,
    capacity: u64,
    vertex_count: u32,
}

impl LineOverlay {
    pub(super) fn new(device: &wgpu::Device, globals_layout: &wgpu::BindGroupLayout) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("overlay.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("overlay.wgsl").into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("overlay"),
            bind_group_layouts: &[Some(globals_layout)],
            immediate_size: 0,
        });
        let attributes = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 12,
                shader_location: 1,
            },
        ];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("overlay-lines"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: LineVertex::SIZE,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: COLOR_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Greater),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: MSAA_SAMPLES,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            buffer: None,
            capacity: 0,
            vertex_count: 0,
        }
    }

    /// Replace the lines (pairs of vertices form one segment).
    pub(super) fn set(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, lines: &[LineVertex]) {
        self.vertex_count = (lines.len() & !1) as u32;
        if self.vertex_count == 0 {
            return;
        }
        let bytes: &[u8] = bytemuck::cast_slice(&lines[..self.vertex_count as usize]);
        match &self.buffer {
            Some(buffer) if self.capacity >= bytes.len() as u64 => {
                queue.write_buffer(buffer, 0, bytes);
            }
            _ => {
                self.capacity = (bytes.len() as u64 * 3 / 2).max(4096);
                let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("overlay-lines"),
                    contents: &{
                        let mut padded = bytes.to_vec();
                        padded.resize(self.capacity as usize, 0);
                        padded
                    },
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });
                self.buffer = Some(buffer);
            }
        }
    }

    pub(super) fn draw(&self, pass: &mut wgpu::RenderPass<'_>, globals: &wgpu::BindGroup) {
        let Some(buffer) = &self.buffer else { return };
        if self.vertex_count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, globals, &[]);
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.draw(0..self.vertex_count, 0..1);
    }
}
