//! Disposable floor palette and texture previews. Source changes replace the receiver and
//! clear cached textures; old workers cannot install into a later source's palette.
use eframe::egui::{self, Color32, Vec2};
use osrs_cache::source::CacheSource;
use osrs_render::{TextureAnimationVector, gpu::TextureLayer};
use osrs_world::{FloorPalette, WorldError};
use std::{
    collections::HashMap,
    sync::{Arc, mpsc::Receiver},
};
#[derive(Default)]
pub(super) struct PaletteRuntime {
    palette: Option<Arc<FloorPalette>>,
    receiver: Option<Receiver<Result<FloorPalette, WorldError>>>,
    floor_textures: Option<Vec<Option<TextureLayer>>>,
    texture_handles: HashMap<u32, (egui::TextureHandle, TextureAnimationVector)>,
    #[cfg(target_arch = "wasm32")]
    job: Option<(
        std::sync::mpsc::Sender<Result<FloorPalette, WorldError>>,
        CacheSource,
    )>,
}
impl PaletteRuntime {
    pub(super) fn palette(&self) -> &Option<Arc<FloorPalette>> {
        &self.palette
    }
    pub(super) fn loading(&self) -> bool {
        self.receiver.is_some()
    }
    pub(super) fn set_textures(&mut self, textures: Vec<Option<TextureLayer>>) {
        self.floor_textures = Some(textures);
        self.texture_handles.clear();
    }
    pub(super) fn reset(&mut self) {
        self.palette = None;
        self.receiver = None;
        self.floor_textures = None;
        self.texture_handles.clear();
        #[cfg(target_arch = "wasm32")]
        {
            self.job = None;
        }
    }
    pub(super) fn load(&mut self, source: CacheSource) {
        let (sender, receiver) = std::sync::mpsc::channel();
        self.receiver = Some(receiver);
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let _ = sender.send(osrs_world::load_floor_palette(source));
        });
        #[cfg(target_arch = "wasm32")]
        {
            self.job = Some((sender, source));
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub(super) fn run_job(&mut self, ready: bool) {
        if ready && let Some((sender, source)) = self.job.take() {
            let _ = sender.send(osrs_world::load_floor_palette(source));
        }
    }
    pub(super) fn poll(&mut self) {
        if let Some(receiver) = &self.receiver
            && let Ok(result) = receiver.try_recv()
        {
            self.palette = result.ok().map(Arc::new);
            self.receiver = None;
        }
    }
    /// The egui texture of a floor texture (made on first use), with its scroll animation.
    pub(in crate::app) fn floor_texture(
        &mut self,
        ctx: &egui::Context,
        texture: u32,
    ) -> Option<(egui::TextureId, TextureAnimationVector)> {
        if let Some((handle, vector)) = self.texture_handles.get(&texture) {
            return Some((handle.id(), *vector));
        }
        let layer = self
            .floor_textures
            .as_ref()?
            .get(texture as usize)?
            .as_ref()?;
        let pixels: Vec<Color32> = layer
            .rgba
            .iter()
            .map(|p| Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
            .collect();
        let size = (layer.rgba.len() as f64).sqrt() as usize;
        let image = egui::ColorImage {
            size: [size, size],
            source_size: Vec2::new(size as f32, size as f32),
            pixels,
        };
        let handle = ctx.load_texture(
            format!("floor-texture-{texture}"),
            image,
            egui::TextureOptions {
                magnification: egui::TextureFilter::Linear,
                minification: egui::TextureFilter::Linear,
                wrap_mode: egui::TextureWrapMode::Repeat,
                mipmap_mode: None,
            },
        );
        let vector = TextureAnimationVector::from_direction_speed(
            layer.animation_direction,
            layer.animation_speed,
        );
        let id = handle.id();
        self.texture_handles.insert(texture, (handle, vector));
        Some((id, vector))
    }
}
