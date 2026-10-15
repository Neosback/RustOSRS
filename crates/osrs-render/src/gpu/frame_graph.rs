//! A small frame graph: passes declare the resources they read and write, the graph drops passes
//! that do not contribute to an imported output, checks that every transient is written before it
//! is read, allocates the transients (pooled across frames) and records the surviving passes in
//! declaration order.
//!
//! Resources are textures the size of the frame. Imported ("external") resources are supplied by
//! the caller at execution time (the viewport colour view); transients live in a [`TransientPool`]
//! and are reallocated only when their size or description changes.

use std::{collections::HashMap, error::Error, fmt};

/// Handle of a graph resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceId(usize);

/// Description of a frame-sized transient texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransientDesc {
    pub format: wgpu::TextureFormat,
    pub samples: u32,
    /// Also usable as a shader input (`TEXTURE_BINDING`).
    pub sampled: bool,
}

#[derive(Debug, Clone, Copy)]
enum Kind {
    External,
    Transient(TransientDesc),
    /// A dependency only: the resource lives outside the graph (a persistent shadow map) and the
    /// graph just orders the passes that write and read it.
    Token,
}

struct Resource {
    name: &'static str,
    kind: Kind,
}

/// Views of the resources a pass touches.
pub struct PassResources<'a> {
    views: &'a HashMap<ResourceId, &'a wgpu::TextureView>,
}

impl<'a> PassResources<'a> {
    /// The view bound to `id`, when the pass declared it.
    pub fn view(&self, id: ResourceId) -> Option<&'a wgpu::TextureView> {
        self.views.get(&id).copied()
    }
}

type PassFn<'a> = Box<dyn FnOnce(&mut wgpu::CommandEncoder, &PassResources<'_>) + 'a>;

struct Pass<'a> {
    name: &'static str,
    reads: Vec<ResourceId>,
    writes: Vec<ResourceId>,
    run: PassFn<'a>,
}

/// Why a graph cannot be executed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameGraphError {
    /// A pass reads a transient that no earlier surviving pass writes.
    ReadBeforeWrite {
        pass: &'static str,
        resource: &'static str,
    },
    /// An imported resource was not supplied.
    MissingExternal(&'static str),
}

impl fmt::Display for FrameGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadBeforeWrite { pass, resource } => {
                write!(
                    formatter,
                    "pass `{pass}` reads `{resource}` before it is written"
                )
            }
            Self::MissingExternal(name) => {
                write!(formatter, "external resource `{name}` not bound")
            }
        }
    }
}

impl Error for FrameGraphError {}

struct PooledTexture {
    desc: TransientDesc,
    size: (u32, u32),
    view: wgpu::TextureView,
}

/// Transient textures kept between frames.
#[derive(Default)]
pub struct TransientPool {
    textures: HashMap<&'static str, PooledTexture>,
}

impl TransientPool {
    /// Storage owned by pooled render targets, excluding backend allocation overhead.
    pub fn allocated_bytes(&self) -> u64 {
        self.textures
            .values()
            .map(|pooled| {
                super::texture_storage_bytes(
                    pooled.size.0,
                    pooled.size.1,
                    1,
                    1,
                    pooled.desc.samples,
                    pooled.desc.format.block_copy_size(None).unwrap_or(0),
                )
            })
            .sum()
    }
    fn ensure(
        &mut self,
        device: &wgpu::Device,
        name: &'static str,
        desc: TransientDesc,
        size: (u32, u32),
    ) {
        if self
            .textures
            .get(name)
            .is_some_and(|pooled| pooled.desc == desc && pooled.size == size)
        {
            return;
        }
        let mut usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
        if desc.sampled {
            usage |= wgpu::TextureUsages::TEXTURE_BINDING;
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(name),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: desc.samples,
            dimension: wgpu::TextureDimension::D2,
            format: desc.format,
            usage,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.textures
            .insert(name, PooledTexture { desc, size, view });
    }

    /// Number of live transient textures (diagnostics and tests).
    pub fn len(&self) -> usize {
        self.textures.len()
    }

    pub fn is_empty(&self) -> bool {
        self.textures.is_empty()
    }
}

/// One frame's pass list.
pub struct FrameGraph<'a> {
    size: (u32, u32),
    resources: Vec<Resource>,
    passes: Vec<Pass<'a>>,
}

impl<'a> FrameGraph<'a> {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            size: (width, height),
            resources: Vec::new(),
            passes: Vec::new(),
        }
    }

    fn add_resource(&mut self, name: &'static str, kind: Kind) -> ResourceId {
        self.resources.push(Resource { name, kind });
        ResourceId(self.resources.len() - 1)
    }

    /// A resource the caller binds at execution time (for example the viewport colour view).
    pub fn import(&mut self, name: &'static str) -> ResourceId {
        self.add_resource(name, Kind::External)
    }

    /// A frame-sized texture owned by the graph.
    pub fn transient(&mut self, name: &'static str, desc: TransientDesc) -> ResourceId {
        self.add_resource(name, Kind::Transient(desc))
    }

    /// An ordering-only resource for data the passes keep outside the graph, such as a shadow map
    /// the caller owns: a pass that reads it runs after the pass that writes it, and the writer
    /// is kept alive by the readers.
    pub fn token(&mut self, name: &'static str) -> ResourceId {
        self.add_resource(name, Kind::Token)
    }

    /// Append a pass. Passes run in the order they were added.
    pub fn add_pass(
        &mut self,
        name: &'static str,
        reads: &[ResourceId],
        writes: &[ResourceId],
        run: impl FnOnce(&mut wgpu::CommandEncoder, &PassResources<'_>) + 'a,
    ) {
        self.passes.push(Pass {
            name,
            reads: reads.to_vec(),
            writes: writes.to_vec(),
            run: Box::new(run),
        });
    }

    fn is_external(&self, id: ResourceId) -> bool {
        matches!(self.resources[id.0].kind, Kind::External)
    }

    /// Indices of the passes that run: those writing an imported resource and, transitively, the
    /// passes producing what they read. Errors when a transient is read before any pass writes it.
    fn compile(&self) -> Result<Vec<usize>, FrameGraphError> {
        let mut live = vec![false; self.passes.len()];
        let mut pending: Vec<usize> = Vec::new();
        for (index, pass) in self.passes.iter().enumerate() {
            if pass.writes.iter().any(|&id| self.is_external(id)) {
                live[index] = true;
                pending.push(index);
            }
        }
        while let Some(index) = pending.pop() {
            let pass = &self.passes[index];
            for &read in &pass.reads {
                // The latest earlier pass that writes the resource produces what this pass reads.
                let producer = self.passes[..index]
                    .iter()
                    .rposition(|earlier| earlier.writes.contains(&read));
                match producer {
                    Some(producer) => {
                        if !live[producer] {
                            live[producer] = true;
                            pending.push(producer);
                        }
                    }
                    None if self.is_external(read) => {}
                    None => {
                        return Err(FrameGraphError::ReadBeforeWrite {
                            pass: pass.name,
                            resource: self.resources[read.0].name,
                        });
                    }
                }
            }
        }
        Ok((0..self.passes.len())
            .filter(|&index| live[index])
            .collect())
    }

    /// Names of the passes [`Self::execute`] would run, in order.
    pub fn plan(&self) -> Result<Vec<&'static str>, FrameGraphError> {
        Ok(self
            .compile()?
            .into_iter()
            .map(|index| self.passes[index].name)
            .collect())
    }

    /// Allocate the transients the surviving passes use and record those passes into `encoder`.
    pub fn execute(
        self,
        device: &wgpu::Device,
        pool: &mut TransientPool,
        encoder: &mut wgpu::CommandEncoder,
        externals: &[(ResourceId, &wgpu::TextureView)],
    ) -> Result<(), FrameGraphError> {
        let order = self.compile()?;
        let mut used: Vec<ResourceId> = Vec::new();
        for &index in &order {
            let pass = &self.passes[index];
            for &id in pass.reads.iter().chain(&pass.writes) {
                if !used.contains(&id) {
                    used.push(id);
                }
            }
        }
        for &id in &used {
            if let Kind::Transient(desc) = self.resources[id.0].kind {
                pool.ensure(device, self.resources[id.0].name, desc, self.size);
            }
        }
        let mut views: HashMap<ResourceId, &wgpu::TextureView> = HashMap::new();
        for &id in &used {
            let resource = &self.resources[id.0];
            let view = match resource.kind {
                Kind::Token => continue,
                Kind::External => externals
                    .iter()
                    .find(|(external, _)| *external == id)
                    .map(|(_, view)| *view)
                    .ok_or(FrameGraphError::MissingExternal(resource.name))?,
                Kind::Transient(_) => pool
                    .textures
                    .get(resource.name)
                    .map(|pooled| &pooled.view)
                    .ok_or(FrameGraphError::MissingExternal(resource.name))?,
            };
            views.insert(id, view);
        }
        let resources = PassResources { views: &views };
        let mut passes: Vec<Option<Pass<'a>>> = self.passes.into_iter().map(Some).collect();
        for index in order {
            if let Some(pass) = passes[index].take() {
                (pass.run)(encoder, &resources);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HDR: TransientDesc = TransientDesc {
        format: wgpu::TextureFormat::Rgba16Float,
        samples: 1,
        sampled: true,
    };

    fn nop(_: &mut wgpu::CommandEncoder, _: &PassResources<'_>) {}

    #[test]
    fn passes_run_in_declaration_order() -> Result<(), FrameGraphError> {
        let mut graph = FrameGraph::new(8, 8);
        let output = graph.import("output");
        let hdr = graph.transient("hdr", HDR);
        graph.add_pass("scene", &[], &[hdr], nop);
        graph.add_pass("resolve", &[hdr], &[output], nop);
        assert_eq!(graph.plan()?, ["scene", "resolve"]);
        Ok(())
    }

    #[test]
    fn passes_not_feeding_an_output_are_dropped() -> Result<(), FrameGraphError> {
        let mut graph = FrameGraph::new(8, 8);
        let output = graph.import("output");
        let hdr = graph.transient("hdr", HDR);
        let unused = graph.transient("unused", HDR);
        graph.add_pass("scene", &[], &[hdr], nop);
        graph.add_pass("shadow", &[], &[unused], nop);
        graph.add_pass("resolve", &[hdr], &[output], nop);
        assert_eq!(graph.plan()?, ["scene", "resolve"]);
        Ok(())
    }

    #[test]
    fn tokens_order_passes_without_textures() -> Result<(), FrameGraphError> {
        let mut graph = FrameGraph::new(8, 8);
        let output = graph.import("output");
        let shadow = graph.token("shadow-map");
        graph.add_pass("shadow", &[], &[shadow], nop);
        graph.add_pass("scene", &[shadow], &[output], nop);
        assert_eq!(graph.plan()?, ["shadow", "scene"]);
        // Without a reader the writer is dropped.
        let mut graph = FrameGraph::new(8, 8);
        let output = graph.import("output");
        let shadow = graph.token("shadow-map");
        graph.add_pass("shadow", &[], &[shadow], nop);
        graph.add_pass("scene", &[], &[output], nop);
        assert_eq!(graph.plan()?, ["scene"]);
        Ok(())
    }

    #[test]
    fn a_single_pass_may_write_the_output_directly() -> Result<(), FrameGraphError> {
        let mut graph = FrameGraph::new(8, 8);
        let output = graph.import("output");
        graph.add_pass("scene", &[], &[output], nop);
        assert_eq!(graph.plan()?, ["scene"]);
        Ok(())
    }

    #[test]
    fn reading_an_unwritten_transient_is_an_error() {
        let mut graph = FrameGraph::new(8, 8);
        let output = graph.import("output");
        let hdr = graph.transient("hdr", HDR);
        graph.add_pass("resolve", &[hdr], &[output], nop);
        assert_eq!(
            graph.plan(),
            Err(FrameGraphError::ReadBeforeWrite {
                pass: "resolve",
                resource: "hdr"
            })
        );
    }

    #[test]
    fn a_producer_declared_after_its_consumer_does_not_count() {
        let mut graph = FrameGraph::new(8, 8);
        let output = graph.import("output");
        let hdr = graph.transient("hdr", HDR);
        graph.add_pass("resolve", &[hdr], &[output], nop);
        graph.add_pass("scene", &[], &[hdr], nop);
        assert!(graph.plan().is_err());
    }

    #[test]
    fn the_latest_writer_is_the_producer() -> Result<(), FrameGraphError> {
        let mut graph = FrameGraph::new(8, 8);
        let output = graph.import("output");
        let hdr = graph.transient("hdr", HDR);
        graph.add_pass("clear", &[], &[hdr], nop);
        graph.add_pass("scene", &[hdr], &[hdr], nop);
        graph.add_pass("resolve", &[hdr], &[output], nop);
        assert_eq!(graph.plan()?, ["clear", "scene", "resolve"]);
        Ok(())
    }
}
