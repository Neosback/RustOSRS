//! Material texture array: one `texture_2d_array` layer per texture-owning material, every layer
//! resampled to one square size, with a full mip chain.
//!
//! Adapted from the 117 HD plugin. Copyright (c) 2018, Adam <Adam@sigterm.info>; (c) 2021, 117 and Hooder (BSD 2-Clause, see THIRD_PARTY.md).
//!
//! Ported from `MaterialManager.swapMaterials` / `TextureManager.uploadTexture` of the 117 HD
//! plugin (BSD-2, see THIRD_PARTY.md): textures are the PNG/JPG files named after their material
//! (lower case), flipped horizontally to match the vanilla UV orientation, resampled bicubically to
//! the configured resolution (128 or 256 px) and stored as sRGB. Mip levels are averaged in linear
//! light, like `glGenerateMipmap` on an sRGB texture.
//!
//! Decoding is pure CPU work ([`DecodeJob`], `Send`) so a caller can run it on a worker thread and
//! hand the result to [`MaterialTextureArray::upload`] on the render thread.

use super::layout::srgb_to_linear;
use image::imageops::FilterType;
use osrs_hd::Material;
use std::{
    collections::HashMap,
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
};

/// Extensions tried in order (`TextureManager.SUPPORTED_IMAGE_EXTENSIONS`).
const EXTENSIONS: [&str; 2] = ["png", "jpg"];

/// Texture resolutions the plugin offers (`TextureResolution`).
pub const DEFAULT_RESOLUTION: u32 = 256;

/// Where a texture-array layer gets its pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerSource {
    /// Name of the owning material (original case).
    pub material: String,
    /// `vanillaTextureIndex` fallback when no image file exists.
    pub vanilla_index: Option<i32>,
}

impl LayerSource {
    /// File stem of the texture image (`Material.getTextureName`).
    pub fn file_stem(&self) -> String {
        self.material.to_lowercase()
    }
}

/// Which layer each material samples.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LayerPlan {
    pub layers: Vec<LayerSource>,
    layer_of: HashMap<String, i32>,
}

impl LayerPlan {
    /// One layer per distinct texture owner (`Material.resolveTextureOwner`), `NONE` excluded.
    /// A material inherits its parent's layer; a parent that is not in the list (`NONE`) means the
    /// material has no texture of its own.
    pub fn from_materials(materials: &[Material]) -> Self {
        Self::from_table(materials, materials)
    }

    /// Like [`Self::from_materials`] for a material table whose parent chains may lead to
    /// definitions outside it (`definitions` lists every resolved definition by name).
    pub fn from_table(materials: &[Material], definitions: &[Material]) -> Self {
        let by_name: HashMap<&str, &Material> = definitions
            .iter()
            .map(|material| (material.name.as_str(), material))
            .collect();
        let owner_of = |start: &Material| -> Option<String> {
            let mut current = start;
            // Bounded walk: a malformed parent loop must not hang the loader.
            for _ in 0..=definitions.len() {
                match current.parent.as_deref() {
                    None => return Some(current.name.clone()),
                    Some(parent) => current = by_name.get(parent)?,
                }
            }
            None
        };
        let mut plan = Self::default();
        let mut owner_layer: HashMap<String, i32> = HashMap::new();
        for material in materials {
            let Some(owner) = owner_of(material) else {
                continue;
            };
            let layer = match owner_layer.get(&owner) {
                Some(&layer) => layer,
                None => {
                    let layer = plan.layers.len() as i32;
                    let vanilla_index = by_name
                        .get(owner.as_str())
                        .and_then(|owner| owner.vanilla_texture_index)
                        .map(|index| index as i32);
                    plan.layers.push(LayerSource {
                        material: owner.clone(),
                        vanilla_index,
                    });
                    owner_layer.insert(owner, layer);
                    layer
                }
            };
            plan.layer_of.insert(material.name.clone(), layer);
        }
        plan
    }

    /// Layer sampled by `material`, `-1` when it has none.
    pub fn layer_of(&self, material: &str) -> i32 {
        self.layer_of.get(material).copied().unwrap_or(-1)
    }
}

/// Texture loading or upload failure.
#[derive(Debug)]
pub enum MaterialArrayError {
    /// The device cannot hold the array.
    Limits(String),
    /// An image file exists but cannot be decoded.
    Image(PathBuf, String),
    /// The requested resolution is not a power of two.
    Resolution(u32),
}

impl fmt::Display for MaterialArrayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limits(detail) => write!(formatter, "material texture array too large: {detail}"),
            Self::Image(path, detail) => write!(formatter, "texture {}: {detail}", path.display()),
            Self::Resolution(size) => {
                write!(formatter, "texture resolution {size} is not a power of two")
            }
        }
    }
}

impl Error for MaterialArrayError {}

/// One layer's pixels: RGBA8 sRGB, mip level 0 first.
pub type MipChain = Vec<Vec<u8>>;

/// Decoded layers ready for upload (CPU only, `Send`).
pub struct DecodedLayers {
    pub size: u32,
    pub mip_levels: u32,
    /// `None`: no image file (a vanilla fallback or empty layer).
    pub layers: Vec<Option<MipChain>>,
    /// Layers without an image file, by material name.
    pub missing: Vec<String>,
}

impl DecodedLayers {
    /// Decode every layer of `plan` from `<textures>/<name>.{png,jpg}` on all cores.
    pub fn load(textures: &Path, plan: &LayerPlan, size: u32) -> Result<Self, MaterialArrayError> {
        if !size.is_power_of_two() {
            return Err(MaterialArrayError::Resolution(size));
        }
        let workers = std::thread::available_parallelism().map_or(1, usize::from);
        let chunk = plan.layers.len().div_ceil(workers).max(1);
        let results: Vec<Result<Option<MipChain>, MaterialArrayError>> =
            std::thread::scope(|scope| {
                let handles: Vec<_> = plan
                    .layers
                    .chunks(chunk)
                    .map(|sources| {
                        scope.spawn(move || {
                            sources
                                .iter()
                                .map(|source| decode_layer(textures, &source.file_stem(), size))
                                .collect::<Vec<_>>()
                        })
                    })
                    .collect();
                handles
                    .into_iter()
                    .flat_map(|handle| {
                        handle
                            .join()
                            .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
                    })
                    .collect()
            });
        let mut layers = Vec::with_capacity(plan.layers.len());
        let mut missing = Vec::new();
        for (source, result) in plan.layers.iter().zip(results) {
            let layer = result?;
            if layer.is_none() {
                missing.push(source.material.clone());
            }
            layers.push(layer);
        }
        Ok(Self {
            size,
            mip_levels: size.ilog2() + 1,
            layers,
            missing,
        })
    }
}

/// Decode, flip and resample one texture, then build its mip chain. `None` when no file exists.
fn decode_layer(
    textures: &Path,
    stem: &str,
    size: u32,
) -> Result<Option<MipChain>, MaterialArrayError> {
    for extension in EXTENSIONS {
        let path = textures.join(format!("{stem}.{extension}"));
        if !path.is_file() {
            continue;
        }
        let bytes = fs::read(&path)
            .map_err(|error| MaterialArrayError::Image(path.clone(), error.to_string()))?;
        let image = image::load_from_memory(&bytes)
            .map_err(|error| MaterialArrayError::Image(path.clone(), error.to_string()))?
            .to_rgba8();
        let base = prepare_base(&image, size);
        return Ok(Some(build_mips(base, size)));
    }
    Ok(None)
}

/// Flip horizontally (vanilla UV orientation) and resample to `size x size` with a bicubic filter.
pub fn prepare_base(image: &image::RgbaImage, size: u32) -> Vec<u8> {
    let flipped = image::imageops::flip_horizontal(image);
    if flipped.dimensions() == (size, size) {
        return flipped.into_raw();
    }
    image::imageops::resize(&flipped, size, size, FilterType::CatmullRom).into_raw()
}

/// Full mip chain of a `size x size` RGBA8 sRGB image: level 0 is `base`.
pub fn build_mips(base: Vec<u8>, size: u32) -> MipChain {
    let table = SrgbTable::new();
    let mut chain = vec![base];
    let mut level_size = size as usize;
    while level_size > 1 {
        let Some(previous) = chain.last() else { break };
        let next = downsample(previous, level_size, &table);
        chain.push(next);
        level_size /= 2;
    }
    chain
}

/// sRGB decode table and nearest-code encoder.
struct SrgbTable {
    linear: [f32; 256],
}

impl SrgbTable {
    fn new() -> Self {
        let mut linear = [0.0; 256];
        for (code, value) in linear.iter_mut().enumerate() {
            *value = srgb_to_linear(code as f32 / 255.0);
        }
        Self { linear }
    }

    fn encode(&self, value: f32) -> u8 {
        let above = self.linear.partition_point(|&entry| entry < value);
        if above == 0 {
            return 0;
        }
        if above >= 256 {
            return 255;
        }
        let below = above - 1;
        if value - self.linear[below] <= self.linear[above] - value {
            below as u8
        } else {
            above as u8
        }
    }
}

/// 2x2 box filter in linear light (alpha averaged directly).
fn downsample(source: &[u8], size: usize, table: &SrgbTable) -> Vec<u8> {
    let target = (size / 2).max(1);
    let mut out = Vec::with_capacity(target * target * 4);
    for row in 0..target {
        for column in 0..target {
            let mut color = [0.0_f32; 3];
            let mut alpha = 0_u32;
            for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                let y = (row * 2 + dy).min(size - 1);
                let x = (column * 2 + dx).min(size - 1);
                let texel = &source[(y * size + x) * 4..][..4];
                for channel in 0..3 {
                    color[channel] += table.linear[usize::from(texel[channel])];
                }
                alpha += u32::from(texel[3]);
            }
            for value in color {
                out.push(table.encode(value / 4.0));
            }
            out.push(((alpha + 2) / 4) as u8);
        }
    }
    out
}

/// Mip chain of a vanilla texture (RGBA texels, zero alpha = cutout) resampled to `size`: the
/// layer a material without an image file samples (`TextureManager.loadTexture` fallback).
/// Vanilla textures keep their orientation; only plugin image files are flipped.
pub fn vanilla_mip_chain(texels: &[[u8; 4]], source_size: u32, size: u32) -> MipChain {
    let flat: Vec<u8> = texels.iter().flatten().copied().collect();
    let base = match image::RgbaImage::from_raw(source_size, source_size, flat) {
        Some(image) if source_size == size => image.into_raw(),
        Some(image) => {
            image::imageops::resize(&image, size, size, FilterType::CatmullRom).into_raw()
        }
        None => vec![0; (size * size * 4) as usize],
    };
    build_mips(base, size)
}

/// The uploaded `texture_2d_array` and its sampler.
pub struct MaterialTextureArray {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    size: u32,
    layers: u32,
    mip_levels: u32,
}

impl MaterialTextureArray {
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

    /// Allocate the array for `decoded` and upload every decoded layer, checking the device limits
    /// first (Metal: 2048 array layers, 16384 px).
    pub fn upload(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        decoded: &DecodedLayers,
    ) -> Result<Self, MaterialArrayError> {
        let layers = (decoded.layers.len() as u32).max(1);
        let limits = device.limits();
        if decoded.size > limits.max_texture_dimension_2d {
            return Err(MaterialArrayError::Limits(format!(
                "{} px exceeds the device limit of {} px",
                decoded.size, limits.max_texture_dimension_2d
            )));
        }
        if layers > limits.max_texture_array_layers {
            return Err(MaterialArrayError::Limits(format!(
                "{layers} layers exceed the device limit of {}",
                limits.max_texture_array_layers
            )));
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hd-material-array"),
            size: wgpu::Extent3d {
                width: decoded.size,
                height: decoded.size,
                depth_or_array_layers: layers,
            },
            mip_level_count: decoded.mip_levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let array = Self {
            view: texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            }),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("hd-material-sampler"),
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                anisotropy_clamp: 16,
                ..Default::default()
            }),
            texture,
            size: decoded.size,
            layers,
            mip_levels: decoded.mip_levels,
        };
        for (layer, chain) in decoded.layers.iter().enumerate() {
            if let Some(chain) = chain {
                array.write_layer(queue, layer as u32, chain);
            }
        }
        Ok(array)
    }

    /// Replace one layer (for example a vanilla texture) with a prepared mip chain.
    pub fn write_layer(&self, queue: &wgpu::Queue, layer: u32, chain: &MipChain) {
        for (mip, pixels) in chain.iter().enumerate().take(self.mip_levels as usize) {
            let level_size = (self.size >> mip).max(1);
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: mip as u32,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(level_size * 4),
                    rows_per_image: Some(level_size),
                },
                wgpu::Extent3d {
                    width: level_size,
                    height: level_size,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    pub const fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    pub const fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub const fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    pub const fn size(&self) -> u32 {
        self.size
    }

    pub const fn layer_count(&self) -> u32 {
        self.layers
    }

    pub const fn mip_levels(&self) -> u32 {
        self.mip_levels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn material(name: &str, parent: Option<&str>, vanilla: Option<f32>) -> Material {
        Material {
            name: name.to_owned(),
            parent: parent.map(str::to_owned),
            vanilla_texture_index: vanilla,
            ..Material::default()
        }
    }

    #[test]
    fn children_share_their_owners_layer() {
        let plan = LayerPlan::from_materials(&[
            material("BRICK", None, Some(2.0)),
            material("BRICK_DARK", Some("BRICK"), None),
            material("WOOD", None, None),
            material("UNLIT_125", Some("NONE"), None),
        ]);
        assert_eq!(plan.layers.len(), 2);
        assert_eq!(plan.layer_of("BRICK"), 0);
        assert_eq!(plan.layer_of("BRICK_DARK"), 0);
        assert_eq!(plan.layer_of("WOOD"), 1);
        assert_eq!(plan.layer_of("UNLIT_125"), -1);
        assert_eq!(plan.layer_of("MISSING"), -1);
        assert_eq!(plan.layers[0].vanilla_index, Some(2));
        assert_eq!(plan.layers[0].file_stem(), "brick");
    }

    #[test]
    fn parent_loops_do_not_hang() {
        let plan = LayerPlan::from_materials(&[
            material("A", Some("B"), None),
            material("B", Some("A"), None),
        ]);
        assert!(plan.layers.is_empty());
    }

    #[test]
    fn mip_chain_reaches_one_texel_and_averages_linear_light() {
        // A 2x2 base of black and white texels averages to 50% *linear*, i.e. sRGB code 188.
        let base = vec![
            0, 0, 0, 255, 255, 255, 255, 255, //
            255, 255, 255, 255, 0, 0, 0, 255,
        ];
        let chain = build_mips(base, 2);
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[1], vec![188, 188, 188, 255]);
    }

    #[test]
    fn solid_colours_survive_the_mip_chain() {
        let base: Vec<u8> = (0..16).flat_map(|_| [10, 120, 250, 77]).collect();
        let chain = build_mips(base, 4);
        assert_eq!(chain.len(), 3);
        assert!(
            chain
                .iter()
                .all(|level| level.chunks(4).all(|t| t == [10, 120, 250, 77]))
        );
    }

    #[test]
    fn odd_resolutions_are_rejected() {
        let plan = LayerPlan::default();
        assert!(matches!(
            DecodedLayers::load(Path::new("."), &plan, 100),
            Err(MaterialArrayError::Resolution(100))
        ));
    }
}
