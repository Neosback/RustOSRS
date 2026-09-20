use crate::draw::{DrawRange, DrawStats, draw_range_is_visible, parse_draw_ranges};
use crate::packet::{validate_draw_ranges, validate_geometry};
use crate::static_scene::StaticMapState;
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{
    HtmlCanvasElement, WebGl2RenderingContext as Gl, WebGlBuffer, WebGlProgram, WebGlShader,
    WebGlTexture, WebGlUniformLocation, WebGlVertexArrayObject,
};

const REFERENCE_VERTEX_SHADER: &str = include_str!("shaders/reference.vert.glsl");
const REFERENCE_FRAGMENT_SHADER: &str = include_str!("shaders/reference.frag.glsl");
const STATIC_VERTEX_SHADER: &str = include_str!("shaders/static.vert.glsl");
const STATIC_FRAGMENT_SHADER: &str = include_str!("shaders/static.frag.glsl");

struct StaticProgram {
    program: WebGlProgram,
    view_matrix: WebGlUniformLocation,
    projection_matrix: WebGlUniformLocation,
    world_entity_transform: WebGlUniformLocation,
    world_entity_opacity: WebGlUniformLocation,
    scene_hsl_override: WebGlUniformLocation,
    player_pos: WebGlUniformLocation,
    render_distance: WebGlUniformLocation,
    fog_depth: WebGlUniformLocation,
    current_time: WebGlUniformLocation,
    brightness: WebGlUniformLocation,
    is_new_texture_anim: WebGlUniformLocation,
    color_banding: WebGlUniformLocation,
    draw_id: WebGlUniformLocation,
    map_pos: WebGlUniformLocation,
    time_loaded: WebGlUniformLocation,
    roof_plane_limit: WebGlUniformLocation,
    scene_border_size: WebGlUniformLocation,
    model_info_sampler: WebGlUniformLocation,
    height_map_sampler: WebGlUniformLocation,
    texture_sampler: WebGlUniformLocation,
    material_sampler: WebGlUniformLocation,
    water_texture_sampler: WebGlUniformLocation,
    water_mask_sampler: WebGlUniformLocation,
    texture_layer_count: WebGlUniformLocation,
    material_count: WebGlUniformLocation,
    discard_alpha: WebGlUniformLocation,
    sky_color: WebGlUniformLocation,
}

struct StaticPass {
    model_info_texture: WebGlTexture,
    draw_ranges: Vec<DrawRange>,
    range_planes: Vec<u8>,
}

impl StaticPass {
    fn new(gl: &Gl) -> Result<Self, JsValue> {
        Ok(Self {
            model_info_texture: create_nearest_texture(gl, Gl::TEXTURE_2D)?,
            draw_ranges: Vec::new(),
            range_planes: Vec::new(),
        })
    }

    fn clear(&mut self) {
        self.draw_ranges.clear();
        self.range_planes.clear();
    }

    fn delete(&self, gl: &Gl) {
        gl.delete_texture(Some(&self.model_info_texture));
    }
}

struct StaticGeometryBatch {
    vertex_buffer: WebGlBuffer,
    index_buffer: WebGlBuffer,
    vao: WebGlVertexArrayObject,
    opaque_pass: StaticPass,
    alpha_pass: StaticPass,
    lod_opaque_pass: StaticPass,
    lod_alpha_pass: StaticPass,
    index_count: u32,
}

impl StaticGeometryBatch {
    fn new(gl: &Gl) -> Result<Self, JsValue> {
        let vertex_buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("failed to create static batch vertex buffer"))?;
        let index_buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("failed to create static batch index buffer"))?;
        let vao = gl
            .create_vertex_array()
            .ok_or_else(|| JsValue::from_str("failed to create static batch vertex array"))?;

        gl.bind_vertex_array(Some(&vao));
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&vertex_buffer));
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_i_pointer_with_i32(0, 3, Gl::UNSIGNED_INT, 12, 0);
        gl.bind_buffer(Gl::ELEMENT_ARRAY_BUFFER, Some(&index_buffer));
        gl.bind_vertex_array(None);

        Ok(Self {
            vertex_buffer,
            index_buffer,
            vao,
            opaque_pass: StaticPass::new(gl)?,
            alpha_pass: StaticPass::new(gl)?,
            lod_opaque_pass: StaticPass::new(gl)?,
            lod_alpha_pass: StaticPass::new(gl)?,
            index_count: 0,
        })
    }

    fn upload_geometry(
        &mut self,
        gl: &Gl,
        packed_vertices: &[u32],
        indices: &[u32],
    ) -> Result<(), JsValue> {
        validate_geometry(packed_vertices, indices)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;

        let vertices = js_sys::Uint32Array::from(packed_vertices);
        let index_data = js_sys::Uint32Array::from(indices);

        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.vertex_buffer));
        gl.buffer_data_with_opt_array_buffer(
            Gl::ARRAY_BUFFER,
            Some(&vertices.buffer()),
            Gl::STATIC_DRAW,
        );
        gl.bind_buffer(Gl::ELEMENT_ARRAY_BUFFER, Some(&self.index_buffer));
        gl.buffer_data_with_opt_array_buffer(
            Gl::ELEMENT_ARRAY_BUFFER,
            Some(&index_data.buffer()),
            Gl::STATIC_DRAW,
        );

        self.index_count = indices.len() as u32;
        self.opaque_pass.clear();
        self.alpha_pass.clear();
        self.lod_opaque_pass.clear();
        self.lod_alpha_pass.clear();
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn upload_passes(
        &mut self,
        gl: &Gl,
        lod: bool,
        model_info_opaque: &[u16],
        opaque_ranges: &[u32],
        opaque_range_planes: &[u8],
        model_info_alpha: &[u16],
        alpha_ranges: &[u32],
        alpha_range_planes: &[u8],
    ) -> Result<(), JsValue> {
        let opaque = parse_draw_ranges(opaque_ranges).map_err(JsValue::from_str)?;
        validate_draw_ranges(&opaque, self.index_count as usize)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let alpha = parse_draw_ranges(alpha_ranges).map_err(JsValue::from_str)?;
        validate_draw_ranges(&alpha, self.index_count as usize)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;

        let (opaque_pass, alpha_pass) = if lod {
            (&mut self.lod_opaque_pass, &mut self.lod_alpha_pass)
        } else {
            (&mut self.opaque_pass, &mut self.alpha_pass)
        };

        if !opaque.is_empty() {
            upload_model_info_texture(
                gl,
                &opaque_pass.model_info_texture,
                model_info_opaque,
                "static batch opaque model-info",
            )?;
        }
        if !alpha.is_empty() {
            upload_model_info_texture(
                gl,
                &alpha_pass.model_info_texture,
                model_info_alpha,
                "static batch alpha model-info",
            )?;
        }

        opaque_pass.draw_ranges = opaque;
        opaque_pass.range_planes = opaque_range_planes.to_vec();
        alpha_pass.draw_ranges = alpha;
        alpha_pass.range_planes = alpha_range_planes.to_vec();
        Ok(())
    }

    fn pass(&self, lod: bool, alpha: bool) -> &StaticPass {
        match (lod, alpha) {
            (false, false) => &self.opaque_pass,
            (false, true) => &self.alpha_pass,
            (true, false) => &self.lod_opaque_pass,
            (true, true) => &self.lod_alpha_pass,
        }
    }

    fn delete(&self, gl: &Gl) {
        gl.delete_vertex_array(Some(&self.vao));
        gl.delete_buffer(Some(&self.vertex_buffer));
        gl.delete_buffer(Some(&self.index_buffer));
        self.opaque_pass.delete(gl);
        self.alpha_pass.delete(gl);
        self.lod_opaque_pass.delete(gl);
        self.lod_alpha_pass.delete(gl);
    }
}

const AUX_BATCH_LOC: u32 = 0;
const AUX_BATCH_DOOR: u32 = 1;

/// Rust/WASM rendering backend.
///
/// Stage 0 remains available through `render_reference`. Stage 1 adds the
/// actual static-map transform contract used by the TypeScript/PicoGL renderer:
/// model-info instancing, map placement, contour-ground height sampling,
/// roof-plane culling, fog and model/face priority depth.
#[wasm_bindgen]
pub struct RustWebGlRenderer {
    canvas: HtmlCanvasElement,
    gl: Gl,

    reference_program: WebGlProgram,
    reference_view_proj: WebGlUniformLocation,
    reference_brightness: WebGlUniformLocation,

    static_program: StaticProgram,

    vertex_buffer: WebGlBuffer,
    index_buffer: WebGlBuffer,
    vao: WebGlVertexArrayObject,

    static_opaque_pass: StaticPass,
    static_alpha_pass: StaticPass,
    static_lod_opaque_pass: StaticPass,
    static_lod_alpha_pass: StaticPass,
    loc_batch: Option<StaticGeometryBatch>,
    door_batch: Option<StaticGeometryBatch>,
    height_map_texture: WebGlTexture,
    texture_array: WebGlTexture,
    material_texture: WebGlTexture,
    water_texture_array: WebGlTexture,
    water_mask_texture: WebGlTexture,
    texture_layer_count: i32,
    material_count: i32,
    static_state: Option<StaticMapState>,

    index_count: u32,
    last_stats: DrawStats,
}

#[wasm_bindgen]
impl RustWebGlRenderer {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas: HtmlCanvasElement) -> Result<RustWebGlRenderer, JsValue> {
        console_error_panic_hook::set_once();

        let gl = canvas
            .get_context("webgl2")?
            .ok_or_else(|| JsValue::from_str("WebGL2 is unavailable"))?
            .dyn_into::<Gl>()?;

        let reference_program =
            create_program(&gl, REFERENCE_VERTEX_SHADER, REFERENCE_FRAGMENT_SHADER)?;
        let static_program_raw = create_program(&gl, STATIC_VERTEX_SHADER, STATIC_FRAGMENT_SHADER)?;

        let vertex_buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("failed to create renderer vertex buffer"))?;
        let index_buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("failed to create renderer index buffer"))?;
        let vao = gl
            .create_vertex_array()
            .ok_or_else(|| JsValue::from_str("failed to create renderer vertex array"))?;

        gl.bind_vertex_array(Some(&vao));
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&vertex_buffer));
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_i_pointer_with_i32(0, 3, Gl::UNSIGNED_INT, 12, 0);
        gl.bind_buffer(Gl::ELEMENT_ARRAY_BUFFER, Some(&index_buffer));
        gl.bind_vertex_array(None);

        let static_opaque_pass = StaticPass::new(&gl)?;
        let static_alpha_pass = StaticPass::new(&gl)?;
        let static_lod_opaque_pass = StaticPass::new(&gl)?;
        let static_lod_alpha_pass = StaticPass::new(&gl)?;
        let height_map_texture = create_nearest_texture(&gl, Gl::TEXTURE_2D_ARRAY)?;
        let texture_array = create_nearest_texture(&gl, Gl::TEXTURE_2D_ARRAY)?;
        let material_texture = create_nearest_texture(&gl, Gl::TEXTURE_2D)?;
        let water_texture_array = create_nearest_texture(&gl, Gl::TEXTURE_2D_ARRAY)?;
        let water_mask_texture = create_nearest_texture(&gl, Gl::TEXTURE_2D_ARRAY)?;
        initialize_fallback_texture_array(&gl, &texture_array)?;
        initialize_fallback_materials(&gl, &material_texture)?;
        initialize_fallback_water_textures(&gl, &water_texture_array)?;
        initialize_fallback_water_mask(&gl, &water_mask_texture)?;

        let reference_view_proj = required_uniform(&gl, &reference_program, "u_viewProj")?;
        let reference_brightness = required_uniform(&gl, &reference_program, "u_brightness")?;

        let static_program = StaticProgram {
            view_matrix: required_uniform(&gl, &static_program_raw, "u_viewMatrix")?,
            projection_matrix: required_uniform(&gl, &static_program_raw, "u_projectionMatrix")?,
            world_entity_transform: required_uniform(
                &gl,
                &static_program_raw,
                "u_worldEntityTransform",
            )?,
            world_entity_opacity: required_uniform(
                &gl,
                &static_program_raw,
                "u_worldEntityOpacity",
            )?,
            scene_hsl_override: required_uniform(&gl, &static_program_raw, "u_sceneHslOverride")?,
            player_pos: required_uniform(&gl, &static_program_raw, "u_playerPos")?,
            render_distance: required_uniform(&gl, &static_program_raw, "u_renderDistance")?,
            fog_depth: required_uniform(&gl, &static_program_raw, "u_fogDepth")?,
            current_time: required_uniform(&gl, &static_program_raw, "u_currentTime")?,
            brightness: required_uniform(&gl, &static_program_raw, "u_brightness")?,
            is_new_texture_anim: required_uniform(&gl, &static_program_raw, "u_isNewTextureAnim")?,
            color_banding: required_uniform(&gl, &static_program_raw, "u_colorBanding")?,
            draw_id: required_uniform(&gl, &static_program_raw, "u_drawId")?,
            map_pos: required_uniform(&gl, &static_program_raw, "u_mapPos")?,
            time_loaded: required_uniform(&gl, &static_program_raw, "u_timeLoaded")?,
            roof_plane_limit: required_uniform(&gl, &static_program_raw, "u_roofPlaneLimit")?,
            scene_border_size: required_uniform(&gl, &static_program_raw, "u_sceneBorderSize")?,
            model_info_sampler: required_uniform(&gl, &static_program_raw, "u_modelInfoTexture")?,
            height_map_sampler: required_uniform(&gl, &static_program_raw, "u_heightMap")?,
            texture_sampler: required_uniform(&gl, &static_program_raw, "u_textures")?,
            material_sampler: required_uniform(&gl, &static_program_raw, "u_textureMaterials")?,
            water_texture_sampler: required_uniform(&gl, &static_program_raw, "u_waterTextures")?,
            water_mask_sampler: required_uniform(&gl, &static_program_raw, "u_waterMask")?,
            texture_layer_count: required_uniform(&gl, &static_program_raw, "u_textureLayerCount")?,
            material_count: required_uniform(&gl, &static_program_raw, "u_materialCount")?,
            discard_alpha: required_uniform(&gl, &static_program_raw, "u_discardAlpha")?,
            sky_color: required_uniform(&gl, &static_program_raw, "u_skyColor")?,
            program: static_program_raw,
        };

        gl.enable(Gl::DEPTH_TEST);
        gl.depth_func(Gl::LEQUAL);
        gl.enable(Gl::CULL_FACE);
        gl.cull_face(Gl::BACK);
        gl.enable(Gl::BLEND);
        gl.blend_func(Gl::SRC_ALPHA, Gl::ONE_MINUS_SRC_ALPHA);

        Ok(Self {
            canvas,
            gl,
            reference_program,
            reference_view_proj,
            reference_brightness,
            static_program,
            vertex_buffer,
            index_buffer,
            vao,
            static_opaque_pass,
            static_alpha_pass,
            static_lod_opaque_pass,
            static_lod_alpha_pass,
            loc_batch: None,
            door_batch: None,
            height_map_texture,
            texture_array,
            material_texture,
            water_texture_array,
            water_mask_texture,
            texture_layer_count: 1,
            material_count: 1,
            static_state: None,
            index_count: 0,
            last_stats: DrawStats::default(),
        })
    }

    pub fn abi_version(&self) -> u32 {
        crate::RENDERER_ABI_VERSION
    }

    /// Uploads the current TypeScript packed-vertex/index packet unchanged.
    ///
    /// packed_vertices is [v0,v1,v2, v0,v1,v2, ...].
    pub fn upload_geometry(
        &mut self,
        packed_vertices: &[u32],
        indices: &[u32],
    ) -> Result<(), JsValue> {
        validate_geometry(packed_vertices, indices)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;

        let vertices = js_sys::Uint32Array::from(packed_vertices);
        let index_data = js_sys::Uint32Array::from(indices);

        self.gl
            .bind_buffer(Gl::ARRAY_BUFFER, Some(&self.vertex_buffer));
        self.gl.buffer_data_with_opt_array_buffer(
            Gl::ARRAY_BUFFER,
            Some(&vertices.buffer()),
            Gl::STATIC_DRAW,
        );

        self.gl
            .bind_buffer(Gl::ELEMENT_ARRAY_BUFFER, Some(&self.index_buffer));
        self.gl.buffer_data_with_opt_array_buffer(
            Gl::ELEMENT_ARRAY_BUFFER,
            Some(&index_data.buffer()),
            Gl::STATIC_DRAW,
        );

        self.index_count = indices.len() as u32;
        self.static_opaque_pass.clear();
        self.static_alpha_pass.clear();
        self.static_lod_opaque_pass.clear();
        self.static_lod_alpha_pass.clear();
        Ok(())
    }

    /// Uploads one RGBA16UI model-info packet produced by SceneBuffer.
    ///
    /// The first texels hold per-draw instance offsets; remaining texels hold
    /// encoded ModelInfo instances. Width is fixed at 16 to match main.vert.
    pub fn upload_model_info(&mut self, model_info: &[u16]) -> Result<(), JsValue> {
        if model_info.is_empty() || model_info.len() % (16 * 4) != 0 {
            return Err(JsValue::from_str(
                "model-info packet must contain complete 16-wide RGBA16UI rows",
            ));
        }

        let rows = (model_info.len() / (16 * 4)) as i32;
        let data = js_sys::Uint16Array::from(model_info);

        self.gl.active_texture(Gl::TEXTURE0);
        self.gl.bind_texture(
            Gl::TEXTURE_2D,
            Some(&self.static_opaque_pass.model_info_texture),
        );
        self.gl
            .tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_array_buffer_view(
                Gl::TEXTURE_2D,
                0,
                Gl::RGBA16UI as i32,
                16,
                rows,
                0,
                Gl::RGBA_INTEGER,
                Gl::UNSIGNED_SHORT,
                Some(data.unchecked_ref()),
            )?;
        Ok(())
    }

    /// Uploads the RGBA16UI model-info packet for the alpha static pass.
    pub fn upload_model_info_alpha(&mut self, model_info: &[u16]) -> Result<(), JsValue> {
        if model_info.is_empty() || model_info.len() % (16 * 4) != 0 {
            return Err(JsValue::from_str(
                "alpha model-info packet must contain complete 16-wide RGBA16UI rows",
            ));
        }

        let rows = (model_info.len() / (16 * 4)) as i32;
        let data = js_sys::Uint16Array::from(model_info);

        self.gl.active_texture(Gl::TEXTURE0);
        self.gl.bind_texture(
            Gl::TEXTURE_2D,
            Some(&self.static_alpha_pass.model_info_texture),
        );
        self.gl
            .tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_array_buffer_view(
                Gl::TEXTURE_2D,
                0,
                Gl::RGBA16UI as i32,
                16,
                rows,
                0,
                Gl::RGBA_INTEGER,
                Gl::UNSIGNED_SHORT,
                Some(data.unchecked_ref()),
            )?;
        Ok(())
    }

    /// Uploads the exact R16I height-map array used by the current main shader.
    pub fn upload_height_map(
        &mut self,
        height_map: &[i16],
        size: u32,
        planes: u32,
    ) -> Result<(), JsValue> {
        if size == 0 || planes == 0 {
            return Err(JsValue::from_str("height-map dimensions must be positive"));
        }
        let expected = (size as usize)
            .checked_mul(size as usize)
            .and_then(|value| value.checked_mul(planes as usize))
            .ok_or_else(|| JsValue::from_str("height-map dimensions overflow"))?;
        if height_map.len() != expected {
            return Err(JsValue::from_str(&format!(
                "height-map packet has {} samples, expected {expected}",
                height_map.len()
            )));
        }

        let data = js_sys::Int16Array::from(height_map);
        self.gl.active_texture(Gl::TEXTURE1);
        self.gl
            .bind_texture(Gl::TEXTURE_2D_ARRAY, Some(&self.height_map_texture));
        self.gl.tex_image_3d_with_opt_array_buffer_view(
            Gl::TEXTURE_2D_ARRAY,
            0,
            Gl::R16I as i32,
            size as i32,
            size as i32,
            planes as i32,
            0,
            Gl::RED_INTEGER,
            Gl::SHORT,
            Some(data.unchecked_ref()),
        )?;

        if let Some(mut state) = self.static_state {
            state.height_map_size = size;
            state.height_map_planes = planes;
            self.static_state = Some(state);
        }
        Ok(())
    }

    /// Uploads the renderer's RGBA8 texture array.
    ///
    /// The byte order is intentionally unchanged from the TypeScript
    /// Int32Array/Uint8Array packet. The static fragment shader performs the
    /// same BGRA swizzle as main.frag.glsl.
    pub fn upload_texture_array(
        &mut self,
        pixels: &[u8],
        width: u32,
        height: u32,
        layers: u32,
    ) -> Result<(), JsValue> {
        if width == 0 || height == 0 || layers == 0 {
            return Err(JsValue::from_str(
                "texture-array dimensions must be positive",
            ));
        }
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|value| value.checked_mul(layers as usize))
            .and_then(|value| value.checked_mul(4))
            .ok_or_else(|| JsValue::from_str("texture-array dimensions overflow"))?;
        if pixels.len() != expected {
            return Err(JsValue::from_str(&format!(
                "texture-array packet has {} bytes, expected {expected}",
                pixels.len()
            )));
        }

        let data = js_sys::Uint8Array::from(pixels);
        self.gl.active_texture(Gl::TEXTURE2);
        self.gl
            .bind_texture(Gl::TEXTURE_2D_ARRAY, Some(&self.texture_array));
        self.gl.tex_image_3d_with_opt_array_buffer_view(
            Gl::TEXTURE_2D_ARRAY,
            0,
            Gl::RGBA8 as i32,
            width as i32,
            height as i32,
            layers as i32,
            0,
            Gl::RGBA,
            Gl::UNSIGNED_BYTE,
            Some(data.unchecked_ref()),
        )?;
        self.texture_layer_count = layers as i32;
        Ok(())
    }

    /// Uploads the six-row RGBA8I material table produced by initMaterialsTexture.
    pub fn upload_materials(
        &mut self,
        materials: &[i8],
        texture_count: u32,
    ) -> Result<(), JsValue> {
        const ROWS: usize = crate::material::MATERIAL_TEXTURE_ROWS;
        if texture_count == 0 {
            return Err(JsValue::from_str("material texture width must be positive"));
        }
        let expected = (texture_count as usize)
            .checked_mul(ROWS)
            .and_then(|value| value.checked_mul(4))
            .ok_or_else(|| JsValue::from_str("material texture dimensions overflow"))?;
        if materials.len() != expected {
            return Err(JsValue::from_str(&format!(
                "material packet has {} bytes, expected {expected}",
                materials.len()
            )));
        }

        let data = js_sys::Int8Array::from(materials);
        self.gl.active_texture(Gl::TEXTURE3);
        self.gl
            .bind_texture(Gl::TEXTURE_2D, Some(&self.material_texture));
        self.gl
            .tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_array_buffer_view(
                Gl::TEXTURE_2D,
                0,
                Gl::RGBA8I as i32,
                texture_count as i32,
                ROWS as i32,
                0,
                Gl::RGBA_INTEGER,
                Gl::BYTE,
                Some(data.unchecked_ref()),
            )?;
        self.material_count = texture_count as i32;
        Ok(())
    }

    /// Uploads the five auxiliary water texture layers used by the current
    /// main fragment shader.
    pub fn upload_water_textures(
        &mut self,
        pixels: &[u8],
        width: u32,
        height: u32,
        layers: u32,
    ) -> Result<(), JsValue> {
        if width == 0 || height == 0 || layers == 0 {
            return Err(JsValue::from_str(
                "water texture dimensions must be positive",
            ));
        }
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|value| value.checked_mul(layers as usize))
            .and_then(|value| value.checked_mul(4))
            .ok_or_else(|| JsValue::from_str("water texture dimensions overflow"))?;
        if pixels.len() != expected {
            return Err(JsValue::from_str(&format!(
                "water texture packet has {} bytes, expected {expected}",
                pixels.len()
            )));
        }

        let data = js_sys::Uint8Array::from(pixels);
        self.gl.active_texture(Gl::TEXTURE4);
        self.gl
            .bind_texture(Gl::TEXTURE_2D_ARRAY, Some(&self.water_texture_array));
        configure_water_texture_sampling(&self.gl);
        self.gl.tex_image_3d_with_opt_array_buffer_view(
            Gl::TEXTURE_2D_ARRAY,
            0,
            Gl::RGBA8 as i32,
            width as i32,
            height as i32,
            layers as i32,
            0,
            Gl::RGBA,
            Gl::UNSIGNED_BYTE,
            Some(data.unchecked_ref()),
        )?;
        self.gl.generate_mipmap(Gl::TEXTURE_2D_ARRAY);
        Ok(())
    }

    /// Uploads the per-map RGBA8 water mask generated by SdMapDataLoader.
    pub fn upload_water_mask(
        &mut self,
        pixels: &[u8],
        size: u32,
        planes: u32,
    ) -> Result<(), JsValue> {
        if size == 0 || planes == 0 {
            return Err(JsValue::from_str("water-mask dimensions must be positive"));
        }
        let expected = (size as usize)
            .checked_mul(size as usize)
            .and_then(|value| value.checked_mul(planes as usize))
            .and_then(|value| value.checked_mul(4))
            .ok_or_else(|| JsValue::from_str("water-mask dimensions overflow"))?;
        if pixels.len() != expected {
            return Err(JsValue::from_str(&format!(
                "water-mask packet has {} bytes, expected {expected}",
                pixels.len()
            )));
        }

        let data = js_sys::Uint8Array::from(pixels);
        self.gl.active_texture(Gl::TEXTURE5);
        self.gl
            .bind_texture(Gl::TEXTURE_2D_ARRAY, Some(&self.water_mask_texture));
        self.gl.tex_image_3d_with_opt_array_buffer_view(
            Gl::TEXTURE_2D_ARRAY,
            0,
            Gl::RGBA8 as i32,
            size as i32,
            size as i32,
            planes as i32,
            0,
            Gl::RGBA,
            Gl::UNSIGNED_BYTE,
            Some(data.unchecked_ref()),
        )?;
        Ok(())
    }

    /// Sets the static map-square placement and height-map metadata.
    pub fn set_static_map_state(
        &mut self,
        map_x: f32,
        map_y: f32,
        border_size: i32,
        height_map_size: u32,
        height_map_planes: u32,
        time_loaded: f32,
    ) -> Result<(), JsValue> {
        let state = StaticMapState {
            map_x,
            map_y,
            border_size,
            height_map_size,
            height_map_planes,
            time_loaded,
        };
        state
            .validate()
            .map_err(|message| JsValue::from_str(message))?;
        self.static_state = Some(state);
        Ok(())
    }

    /// Existing TypeScript wire layout:
    /// [offsetBytes,elements,instances, ...].
    pub fn set_draw_ranges(&mut self, flat_ranges: &[u32]) -> Result<(), JsValue> {
        let ranges = parse_draw_ranges(flat_ranges).map_err(JsValue::from_str)?;
        validate_draw_ranges(&ranges, self.index_count as usize)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        self.static_opaque_pass.draw_ranges = ranges;
        self.static_opaque_pass.range_planes.clear();
        Ok(())
    }

    pub fn set_draw_ranges_alpha(&mut self, flat_ranges: &[u32]) -> Result<(), JsValue> {
        let ranges = parse_draw_ranges(flat_ranges).map_err(JsValue::from_str)?;
        validate_draw_ranges(&ranges, self.index_count as usize)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        self.static_alpha_pass.draw_ranges = ranges;
        self.static_alpha_pass.range_planes.clear();
        Ok(())
    }

    /// Uploads both static map passes once so frame rendering stays Rust-owned.
    pub fn upload_static_passes(
        &mut self,
        model_info_opaque: &[u16],
        opaque_ranges: &[u32],
        opaque_range_planes: &[u8],
        model_info_alpha: &[u16],
        alpha_ranges: &[u32],
        alpha_range_planes: &[u8],
    ) -> Result<(), JsValue> {
        let opaque = parse_draw_ranges(opaque_ranges).map_err(JsValue::from_str)?;
        validate_draw_ranges(&opaque, self.index_count as usize)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let alpha = parse_draw_ranges(alpha_ranges).map_err(JsValue::from_str)?;
        validate_draw_ranges(&alpha, self.index_count as usize)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;

        if !opaque.is_empty() {
            self.upload_model_info(model_info_opaque)?;
        }
        if !alpha.is_empty() {
            self.upload_model_info_alpha(model_info_alpha)?;
        }

        self.static_opaque_pass.draw_ranges = opaque;
        self.static_alpha_pass.draw_ranges = alpha;
        self.static_opaque_pass.range_planes = opaque_range_planes.to_vec();
        self.static_alpha_pass.range_planes = alpha_range_planes.to_vec();
        Ok(())
    }

    /// Uploads the LOD static passes once so Rust can choose detail level per frame.
    pub fn upload_static_lod_passes(
        &mut self,
        model_info_opaque: &[u16],
        opaque_ranges: &[u32],
        opaque_range_planes: &[u8],
        model_info_alpha: &[u16],
        alpha_ranges: &[u32],
        alpha_range_planes: &[u8],
    ) -> Result<(), JsValue> {
        let opaque = parse_draw_ranges(opaque_ranges).map_err(JsValue::from_str)?;
        validate_draw_ranges(&opaque, self.index_count as usize)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let alpha = parse_draw_ranges(alpha_ranges).map_err(JsValue::from_str)?;
        validate_draw_ranges(&alpha, self.index_count as usize)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;

        if !opaque.is_empty() {
            upload_model_info_texture(
                &self.gl,
                &self.static_lod_opaque_pass.model_info_texture,
                model_info_opaque,
                "static LOD opaque model-info",
            )?;
        }

        if !alpha.is_empty() {
            upload_model_info_texture(
                &self.gl,
                &self.static_lod_alpha_pass.model_info_texture,
                model_info_alpha,
                "static LOD alpha model-info",
            )?;
        }

        self.static_lod_opaque_pass.draw_ranges = opaque;
        self.static_lod_alpha_pass.draw_ranges = alpha;
        self.static_lod_opaque_pass.range_planes = opaque_range_planes.to_vec();
        self.static_lod_alpha_pass.range_planes = alpha_range_planes.to_vec();
        Ok(())
    }

    pub fn clear_draw_ranges(&mut self) {
        self.static_opaque_pass.clear();
        self.static_alpha_pass.clear();
        self.static_lod_opaque_pass.clear();
        self.static_lod_alpha_pass.clear();
    }

    /// Uploads geometry for an auxiliary static map batch.
    ///
    /// kind 0 = non-door loc geometry, kind 1 = door geometry.
    pub fn upload_aux_geometry(
        &mut self,
        kind: u32,
        packed_vertices: &[u32],
        indices: &[u32],
    ) -> Result<(), JsValue> {
        if packed_vertices.is_empty() && indices.is_empty() {
            let existing = match kind {
                AUX_BATCH_LOC => self.loc_batch.take(),
                AUX_BATCH_DOOR => self.door_batch.take(),
                _ => return Err(JsValue::from_str("unknown auxiliary static batch kind")),
            };
            if let Some(batch) = existing {
                batch.delete(&self.gl);
            }
            return Ok(());
        }

        let mut batch = match kind {
            AUX_BATCH_LOC => self.loc_batch.take(),
            AUX_BATCH_DOOR => self.door_batch.take(),
            _ => return Err(JsValue::from_str("unknown auxiliary static batch kind")),
        }
        .unwrap_or(StaticGeometryBatch::new(&self.gl)?);

        if let Err(error) = batch.upload_geometry(&self.gl, packed_vertices, indices) {
            match kind {
                AUX_BATCH_LOC => self.loc_batch = Some(batch),
                AUX_BATCH_DOOR => self.door_batch = Some(batch),
                _ => unreachable!(),
            }
            return Err(error);
        }

        match kind {
            AUX_BATCH_LOC => self.loc_batch = Some(batch),
            AUX_BATCH_DOOR => self.door_batch = Some(batch),
            _ => unreachable!(),
        }
        Ok(())
    }

    /// Uploads full-detail pass state for an auxiliary static batch.
    #[allow(clippy::too_many_arguments)]
    pub fn upload_aux_passes(
        &mut self,
        kind: u32,
        model_info_opaque: &[u16],
        opaque_ranges: &[u32],
        opaque_range_planes: &[u8],
        model_info_alpha: &[u16],
        alpha_ranges: &[u32],
        alpha_range_planes: &[u8],
    ) -> Result<(), JsValue> {
        self.upload_aux_pass_set(
            kind,
            false,
            model_info_opaque,
            opaque_ranges,
            opaque_range_planes,
            model_info_alpha,
            alpha_ranges,
            alpha_range_planes,
        )
    }

    /// Uploads LOD pass state for an auxiliary static batch.
    #[allow(clippy::too_many_arguments)]
    pub fn upload_aux_lod_passes(
        &mut self,
        kind: u32,
        model_info_opaque: &[u16],
        opaque_ranges: &[u32],
        opaque_range_planes: &[u8],
        model_info_alpha: &[u16],
        alpha_ranges: &[u32],
        alpha_range_planes: &[u8],
    ) -> Result<(), JsValue> {
        self.upload_aux_pass_set(
            kind,
            true,
            model_info_opaque,
            opaque_ranges,
            opaque_range_planes,
            model_info_alpha,
            alpha_ranges,
            alpha_range_planes,
        )
    }

    /// Stage-0 geometry/HSL reference pass retained as an A/B diagnostic.
    pub fn render_reference(
        &mut self,
        view_projection: &[f32],
        clear_rgba: &[f32],
        brightness: f32,
    ) -> Result<(), JsValue> {
        require_matrix(view_projection, "view_projection")?;
        require_vec4(clear_rgba, "clear_rgba")?;
        self.prepare_default_frame(clear_rgba);

        self.gl.use_program(Some(&self.reference_program));
        self.gl.uniform_matrix4fv_with_f32_array(
            Some(&self.reference_view_proj),
            false,
            view_projection,
        );
        self.gl
            .uniform1f(Some(&self.reference_brightness), brightness.max(0.0001));
        self.gl.bind_vertex_array(Some(&self.vao));

        let stats = submit_draw_ranges(
            &self.gl,
            &self.static_opaque_pass.draw_ranges,
            self.index_count,
            None,
            None,
            3,
            true,
        );
        self.gl.bind_vertex_array(None);
        self.last_stats = stats;
        Ok(())
    }

    /// Backwards-compatible Stage-0 entry point.
    pub fn render(
        &mut self,
        view_projection: &[f32],
        clear_rgba: &[f32],
        brightness: f32,
    ) -> Result<(), JsValue> {
        self.render_reference(view_projection, clear_rgba, brightness)
    }

    /// Renders the resident opaque and alpha static passes as one Rust-owned frame.
    ///
    /// Full-detail and LOD resources remain resident at the same time. Frame
    /// selection is explicit and does not mutate or swap renderer ownership.
    #[allow(clippy::too_many_arguments)]
    pub fn render_static_frame(
        &mut self,
        view_matrix: &[f32],
        projection_matrix: &[f32],
        world_entity_transform: &[f32],
        world_entity_opacity: f32,
        sky_rgba: &[f32],
        scene_hsl_override: &[f32],
        player_pos: &[f32],
        render_distance: f32,
        fog_depth: f32,
        current_time: f32,
        brightness: f32,
        roof_plane_limit: f32,
        use_lod: bool,
        is_new_texture_anim: bool,
        color_banding: f32,
    ) -> Result<(), JsValue> {
        self.gl.disable(Gl::BLEND);
        self.render_static(
            view_matrix,
            projection_matrix,
            world_entity_transform,
            world_entity_opacity,
            sky_rgba,
            scene_hsl_override,
            player_pos,
            render_distance,
            fog_depth,
            current_time,
            brightness,
            roof_plane_limit,
            use_lod,
            is_new_texture_anim,
            color_banding,
            false,
            true,
        )?;

        self.gl.enable(Gl::BLEND);
        self.gl.blend_func(Gl::SRC_ALPHA, Gl::ONE_MINUS_SRC_ALPHA);
        self.render_static(
            view_matrix,
            projection_matrix,
            world_entity_transform,
            world_entity_opacity,
            sky_rgba,
            scene_hsl_override,
            player_pos,
            render_distance,
            fog_depth,
            current_time,
            brightness,
            roof_plane_limit,
            use_lod,
            is_new_texture_anim,
            color_banding,
            true,
            false,
        )
    }

    /// Stage-1 static-scene pass matching the current map-square transform path.
    #[allow(clippy::too_many_arguments)]
    pub fn render_static(
        &mut self,
        view_matrix: &[f32],
        projection_matrix: &[f32],
        world_entity_transform: &[f32],
        world_entity_opacity: f32,
        sky_rgba: &[f32],
        scene_hsl_override: &[f32],
        player_pos: &[f32],
        render_distance: f32,
        fog_depth: f32,
        current_time: f32,
        brightness: f32,
        roof_plane_limit: f32,
        use_lod: bool,
        is_new_texture_anim: bool,
        color_banding: f32,
        discard_alpha: bool,
        clear_frame: bool,
    ) -> Result<(), JsValue> {
        require_matrix(view_matrix, "view_matrix")?;
        require_matrix(projection_matrix, "projection_matrix")?;
        require_matrix(world_entity_transform, "world_entity_transform")?;
        require_vec4(sky_rgba, "sky_rgba")?;
        require_vec4(scene_hsl_override, "scene_hsl_override")?;
        if player_pos.len() != 2 {
            return Err(JsValue::from_str("player_pos must contain two f32 values"));
        }
        let state = self
            .static_state
            .ok_or_else(|| JsValue::from_str("static map state has not been configured"))?;
        let pass = match (use_lod, discard_alpha) {
            (false, false) => &self.static_opaque_pass,
            (false, true) => &self.static_alpha_pass,
            (true, false) => &self.static_lod_opaque_pass,
            (true, true) => &self.static_lod_alpha_pass,
        };

        if clear_frame {
            self.prepare_default_frame(sky_rgba);
        } else {
            self.prepare_viewport();
        }
        self.gl.use_program(Some(&self.static_program.program));

        self.gl.uniform_matrix4fv_with_f32_array(
            Some(&self.static_program.view_matrix),
            false,
            view_matrix,
        );
        self.gl.uniform_matrix4fv_with_f32_array(
            Some(&self.static_program.projection_matrix),
            false,
            projection_matrix,
        );
        self.gl.uniform_matrix4fv_with_f32_array(
            Some(&self.static_program.world_entity_transform),
            false,
            world_entity_transform,
        );
        self.gl.uniform1f(
            Some(&self.static_program.world_entity_opacity),
            world_entity_opacity,
        );
        self.gl.uniform4fv_with_f32_array(
            Some(&self.static_program.scene_hsl_override),
            scene_hsl_override,
        );
        self.gl.uniform2f(
            Some(&self.static_program.player_pos),
            player_pos[0],
            player_pos[1],
        );
        self.gl.uniform1f(
            Some(&self.static_program.render_distance),
            render_distance.max(0.0001),
        );
        self.gl
            .uniform1f(Some(&self.static_program.fog_depth), fog_depth.max(0.0));
        self.gl
            .uniform1f(Some(&self.static_program.current_time), current_time);
        self.gl.uniform1f(
            Some(&self.static_program.brightness),
            brightness.max(0.0001),
        );
        self.gl.uniform1f(
            Some(&self.static_program.is_new_texture_anim),
            if is_new_texture_anim { 1.0 } else { 0.0 },
        );
        self.gl.uniform1f(
            Some(&self.static_program.color_banding),
            color_banding.max(1.0),
        );
        self.gl.uniform1i(
            Some(&self.static_program.discard_alpha),
            i32::from(discard_alpha),
        );
        self.gl.uniform1i(
            Some(&self.static_program.texture_layer_count),
            self.texture_layer_count.max(1),
        );
        self.gl.uniform1i(
            Some(&self.static_program.material_count),
            self.material_count.max(1),
        );
        self.gl
            .uniform2f(Some(&self.static_program.map_pos), state.map_x, state.map_y);
        self.gl
            .uniform1f(Some(&self.static_program.time_loaded), state.time_loaded);
        self.gl.uniform1f(
            Some(&self.static_program.roof_plane_limit),
            roof_plane_limit,
        );
        self.gl.uniform1i(
            Some(&self.static_program.scene_border_size),
            state.border_size,
        );
        self.gl
            .uniform4fv_with_f32_array(Some(&self.static_program.sky_color), sky_rgba);

        self.gl.active_texture(Gl::TEXTURE0);
        self.gl
            .bind_texture(Gl::TEXTURE_2D, Some(&pass.model_info_texture));
        self.gl
            .uniform1i(Some(&self.static_program.model_info_sampler), 0);

        self.gl.active_texture(Gl::TEXTURE1);
        self.gl
            .bind_texture(Gl::TEXTURE_2D_ARRAY, Some(&self.height_map_texture));
        self.gl
            .uniform1i(Some(&self.static_program.height_map_sampler), 1);

        self.gl.active_texture(Gl::TEXTURE2);
        self.gl
            .bind_texture(Gl::TEXTURE_2D_ARRAY, Some(&self.texture_array));
        self.gl
            .uniform1i(Some(&self.static_program.texture_sampler), 2);

        self.gl.active_texture(Gl::TEXTURE3);
        self.gl
            .bind_texture(Gl::TEXTURE_2D, Some(&self.material_texture));
        self.gl
            .uniform1i(Some(&self.static_program.material_sampler), 3);

        self.gl.active_texture(Gl::TEXTURE4);
        self.gl
            .bind_texture(Gl::TEXTURE_2D_ARRAY, Some(&self.water_texture_array));
        self.gl
            .uniform1i(Some(&self.static_program.water_texture_sampler), 4);

        self.gl.active_texture(Gl::TEXTURE5);
        self.gl
            .bind_texture(Gl::TEXTURE_2D_ARRAY, Some(&self.water_mask_texture));
        self.gl
            .uniform1i(Some(&self.static_program.water_mask_sampler), 5);

        let roof_limit = roof_plane_limit.clamp(0.0, 3.0) as u8;

        self.gl.bind_vertex_array(Some(&self.vao));
        let mut stats = submit_draw_ranges(
            &self.gl,
            &pass.draw_ranges,
            self.index_count,
            Some(&self.static_program.draw_id),
            Some(&pass.range_planes),
            roof_limit,
            false,
        );

        for batch in [&self.loc_batch, &self.door_batch].into_iter().flatten() {
            let batch_pass = batch.pass(use_lod, discard_alpha);
            self.gl.bind_vertex_array(Some(&batch.vao));
            let batch_stats = submit_draw_ranges(
                &self.gl,
                &batch_pass.draw_ranges,
                batch.index_count,
                Some(&self.static_program.draw_id),
                Some(&batch_pass.range_planes),
                roof_limit,
                false,
            );
            stats.draw_calls += batch_stats.draw_calls;
            stats.submitted_indices += batch_stats.submitted_indices;
        }

        self.gl.bind_vertex_array(None);
        if clear_frame {
            self.last_stats = stats;
        } else {
            self.last_stats.draw_calls += stats.draw_calls;
            self.last_stats.submitted_indices += stats.submitted_indices;
        }
        Ok(())
    }

    pub fn last_draw_calls(&self) -> u32 {
        self.last_stats.draw_calls
    }

    pub fn last_submitted_indices(&self) -> f64 {
        self.last_stats.submitted_indices as f64
    }

    pub fn dispose(&mut self) {
        self.gl.delete_vertex_array(Some(&self.vao));
        self.gl.delete_buffer(Some(&self.vertex_buffer));
        self.gl.delete_buffer(Some(&self.index_buffer));
        self.static_opaque_pass.delete(&self.gl);
        self.static_alpha_pass.delete(&self.gl);
        self.static_lod_opaque_pass.delete(&self.gl);
        self.static_lod_alpha_pass.delete(&self.gl);
        if let Some(batch) = self.loc_batch.take() {
            batch.delete(&self.gl);
        }
        if let Some(batch) = self.door_batch.take() {
            batch.delete(&self.gl);
        }
        self.gl.delete_texture(Some(&self.height_map_texture));
        self.gl.delete_texture(Some(&self.texture_array));
        self.gl.delete_texture(Some(&self.material_texture));
        self.gl.delete_texture(Some(&self.water_texture_array));
        self.gl.delete_texture(Some(&self.water_mask_texture));
        self.gl.delete_program(Some(&self.reference_program));
        self.gl.delete_program(Some(&self.static_program.program));
        self.static_opaque_pass.clear();
        self.static_alpha_pass.clear();
        self.static_lod_opaque_pass.clear();
        self.static_lod_alpha_pass.clear();
        self.index_count = 0;
        self.static_state = None;
    }

    #[allow(clippy::too_many_arguments)]
    fn upload_aux_pass_set(
        &mut self,
        kind: u32,
        lod: bool,
        model_info_opaque: &[u16],
        opaque_ranges: &[u32],
        opaque_range_planes: &[u8],
        model_info_alpha: &[u16],
        alpha_ranges: &[u32],
        alpha_range_planes: &[u8],
    ) -> Result<(), JsValue> {
        let mut batch = match kind {
            AUX_BATCH_LOC => self
                .loc_batch
                .take()
                .ok_or_else(|| JsValue::from_str("loc geometry has not been uploaded"))?,
            AUX_BATCH_DOOR => self
                .door_batch
                .take()
                .ok_or_else(|| JsValue::from_str("door geometry has not been uploaded"))?,
            _ => return Err(JsValue::from_str("unknown auxiliary static batch kind")),
        };

        let result = batch.upload_passes(
            &self.gl,
            lod,
            model_info_opaque,
            opaque_ranges,
            opaque_range_planes,
            model_info_alpha,
            alpha_ranges,
            alpha_range_planes,
        );

        match kind {
            AUX_BATCH_LOC => self.loc_batch = Some(batch),
            AUX_BATCH_DOOR => self.door_batch = Some(batch),
            _ => unreachable!(),
        }
        result
    }

    fn prepare_viewport(&self) {
        let width = self.canvas.width() as i32;
        let height = self.canvas.height() as i32;
        self.gl.viewport(0, 0, width.max(1), height.max(1));
    }

    fn prepare_default_frame(&self, clear_rgba: &[f32]) {
        self.prepare_viewport();
        self.gl
            .clear_color(clear_rgba[0], clear_rgba[1], clear_rgba[2], clear_rgba[3]);
        self.gl.clear(Gl::COLOR_BUFFER_BIT | Gl::DEPTH_BUFFER_BIT);
    }
}

fn submit_draw_ranges(
    gl: &Gl,
    ranges: &[DrawRange],
    index_count: u32,
    draw_id: Option<&WebGlUniformLocation>,
    range_planes: Option<&[u8]>,
    roof_plane_limit: u8,
    draw_all_if_empty: bool,
) -> DrawStats {
    let mut stats = DrawStats::default();

    if ranges.is_empty() {
        if draw_all_if_empty && index_count > 0 {
            if let Some(location) = draw_id {
                gl.uniform1i(Some(location), 0);
            }
            gl.draw_elements_with_i32(Gl::TRIANGLES, index_count as i32, Gl::UNSIGNED_INT, 0);
            stats.draw_calls = 1;
            stats.submitted_indices = index_count as u64;
        }
        return stats;
    }

    for (draw_index, range) in ranges.iter().copied().enumerate() {
        let plane = range_planes.and_then(|planes| planes.get(draw_index).copied());
        if !draw_range_is_visible(range, plane, roof_plane_limit) {
            continue;
        }
        if let Some(location) = draw_id {
            gl.uniform1i(Some(location), draw_index as i32);
        }

        if range.instances <= 1 {
            gl.draw_elements_with_i32(
                Gl::TRIANGLES,
                range.elements as i32,
                Gl::UNSIGNED_INT,
                range.offset_bytes as i32,
            );
        } else {
            gl.draw_elements_instanced_with_i32(
                Gl::TRIANGLES,
                range.elements as i32,
                Gl::UNSIGNED_INT,
                range.offset_bytes as i32,
                range.instances as i32,
            );
        }
        stats.draw_calls += 1;
        stats.submitted_indices += range.submitted_indices();
    }

    stats
}

fn upload_model_info_texture(
    gl: &Gl,
    texture: &WebGlTexture,
    model_info: &[u16],
    label: &str,
) -> Result<(), JsValue> {
    if model_info.is_empty() || model_info.len() % (16 * 4) != 0 {
        return Err(JsValue::from_str(&format!(
            "{label} packet must contain complete 16-wide RGBA16UI rows"
        )));
    }

    let rows = (model_info.len() / (16 * 4)) as i32;
    let data = js_sys::Uint16Array::from(model_info);

    gl.active_texture(Gl::TEXTURE0);
    gl.bind_texture(Gl::TEXTURE_2D, Some(texture));
    gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_array_buffer_view(
        Gl::TEXTURE_2D,
        0,
        Gl::RGBA16UI as i32,
        16,
        rows,
        0,
        Gl::RGBA_INTEGER,
        Gl::UNSIGNED_SHORT,
        Some(data.unchecked_ref()),
    )?;
    Ok(())
}

fn create_nearest_texture(gl: &Gl, target: u32) -> Result<WebGlTexture, JsValue> {
    let texture = gl
        .create_texture()
        .ok_or_else(|| JsValue::from_str("failed to create WebGL texture"))?;
    gl.bind_texture(target, Some(&texture));
    gl.tex_parameteri(target, Gl::TEXTURE_MIN_FILTER, Gl::NEAREST as i32);
    gl.tex_parameteri(target, Gl::TEXTURE_MAG_FILTER, Gl::NEAREST as i32);
    gl.tex_parameteri(target, Gl::TEXTURE_WRAP_S, Gl::CLAMP_TO_EDGE as i32);
    gl.tex_parameteri(target, Gl::TEXTURE_WRAP_T, Gl::CLAMP_TO_EDGE as i32);
    if target == Gl::TEXTURE_2D_ARRAY {
        gl.tex_parameteri(target, Gl::TEXTURE_WRAP_R, Gl::CLAMP_TO_EDGE as i32);
    }
    gl.bind_texture(target, None);
    Ok(texture)
}

fn initialize_fallback_texture_array(gl: &Gl, texture: &WebGlTexture) -> Result<(), JsValue> {
    let pixels = js_sys::Uint8Array::from(&[255u8, 255, 255, 255][..]);
    gl.bind_texture(Gl::TEXTURE_2D_ARRAY, Some(texture));
    gl.tex_image_3d_with_opt_array_buffer_view(
        Gl::TEXTURE_2D_ARRAY,
        0,
        Gl::RGBA8 as i32,
        1,
        1,
        1,
        0,
        Gl::RGBA,
        Gl::UNSIGNED_BYTE,
        Some(pixels.unchecked_ref()),
    )?;
    gl.bind_texture(Gl::TEXTURE_2D_ARRAY, None);
    Ok(())
}

fn initialize_fallback_materials(gl: &Gl, texture: &WebGlTexture) -> Result<(), JsValue> {
    let mut values = [0i8; crate::material::MATERIAL_TEXTURE_ROWS * 4];
    values[3] = 1;
    let data = js_sys::Int8Array::from(&values[..]);
    gl.bind_texture(Gl::TEXTURE_2D, Some(texture));
    gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_array_buffer_view(
        Gl::TEXTURE_2D,
        0,
        Gl::RGBA8I as i32,
        1,
        crate::material::MATERIAL_TEXTURE_ROWS as i32,
        0,
        Gl::RGBA_INTEGER,
        Gl::BYTE,
        Some(data.unchecked_ref()),
    )?;
    gl.bind_texture(Gl::TEXTURE_2D, None);
    Ok(())
}

fn initialize_fallback_water_textures(gl: &Gl, texture: &WebGlTexture) -> Result<(), JsValue> {
    const LAYERS: usize = 5;
    let mut values = [0u8; LAYERS * 4];
    for layer in 0..LAYERS {
        let base = layer * 4;
        values[base] = 128;
        values[base + 1] = 128;
        values[base + 2] = 255;
        values[base + 3] = 255;
    }
    let data = js_sys::Uint8Array::from(&values[..]);
    gl.bind_texture(Gl::TEXTURE_2D_ARRAY, Some(texture));
    configure_water_texture_sampling(gl);
    gl.tex_image_3d_with_opt_array_buffer_view(
        Gl::TEXTURE_2D_ARRAY,
        0,
        Gl::RGBA8 as i32,
        1,
        1,
        LAYERS as i32,
        0,
        Gl::RGBA,
        Gl::UNSIGNED_BYTE,
        Some(data.unchecked_ref()),
    )?;
    gl.generate_mipmap(Gl::TEXTURE_2D_ARRAY);
    gl.bind_texture(Gl::TEXTURE_2D_ARRAY, None);
    Ok(())
}

fn initialize_fallback_water_mask(gl: &Gl, texture: &WebGlTexture) -> Result<(), JsValue> {
    let values = js_sys::Uint8Array::from(&[0u8, 0, 0, 0][..]);
    gl.bind_texture(Gl::TEXTURE_2D_ARRAY, Some(texture));
    gl.tex_image_3d_with_opt_array_buffer_view(
        Gl::TEXTURE_2D_ARRAY,
        0,
        Gl::RGBA8 as i32,
        1,
        1,
        1,
        0,
        Gl::RGBA,
        Gl::UNSIGNED_BYTE,
        Some(values.unchecked_ref()),
    )?;
    gl.bind_texture(Gl::TEXTURE_2D_ARRAY, None);
    Ok(())
}

fn configure_water_texture_sampling(gl: &Gl) {
    gl.tex_parameteri(
        Gl::TEXTURE_2D_ARRAY,
        Gl::TEXTURE_MIN_FILTER,
        Gl::LINEAR_MIPMAP_LINEAR as i32,
    );
    gl.tex_parameteri(
        Gl::TEXTURE_2D_ARRAY,
        Gl::TEXTURE_MAG_FILTER,
        Gl::LINEAR as i32,
    );
    gl.tex_parameteri(Gl::TEXTURE_2D_ARRAY, Gl::TEXTURE_WRAP_S, Gl::REPEAT as i32);
    gl.tex_parameteri(Gl::TEXTURE_2D_ARRAY, Gl::TEXTURE_WRAP_T, Gl::REPEAT as i32);
    gl.tex_parameteri(
        Gl::TEXTURE_2D_ARRAY,
        Gl::TEXTURE_WRAP_R,
        Gl::CLAMP_TO_EDGE as i32,
    );
}

fn require_matrix(value: &[f32], name: &str) -> Result<(), JsValue> {
    if value.len() == 16 {
        Ok(())
    } else {
        Err(JsValue::from_str(&format!(
            "{name} must contain 16 f32 values"
        )))
    }
}

fn require_vec4(value: &[f32], name: &str) -> Result<(), JsValue> {
    if value.len() == 4 {
        Ok(())
    } else {
        Err(JsValue::from_str(&format!(
            "{name} must contain four f32 values"
        )))
    }
}

fn required_uniform(
    gl: &Gl,
    program: &WebGlProgram,
    name: &str,
) -> Result<WebGlUniformLocation, JsValue> {
    gl.get_uniform_location(program, name)
        .ok_or_else(|| JsValue::from_str(&format!("{name} uniform was optimized out")))
}

fn create_program(
    gl: &Gl,
    vertex_source: &str,
    fragment_source: &str,
) -> Result<WebGlProgram, JsValue> {
    let vertex = compile_shader(gl, Gl::VERTEX_SHADER, vertex_source)?;
    let fragment = compile_shader(gl, Gl::FRAGMENT_SHADER, fragment_source)?;
    link_program(gl, &vertex, &fragment)
}

fn compile_shader(gl: &Gl, shader_type: u32, source: &str) -> Result<WebGlShader, JsValue> {
    let shader = gl
        .create_shader(shader_type)
        .ok_or_else(|| JsValue::from_str("failed to create WebGL shader"))?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);

    if gl
        .get_shader_parameter(&shader, Gl::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        Err(JsValue::from_str(
            &gl.get_shader_info_log(&shader)
                .unwrap_or_else(|| "unknown shader compilation failure".to_string()),
        ))
    }
}

fn link_program(
    gl: &Gl,
    vertex: &WebGlShader,
    fragment: &WebGlShader,
) -> Result<WebGlProgram, JsValue> {
    let program = gl
        .create_program()
        .ok_or_else(|| JsValue::from_str("failed to create WebGL program"))?;

    gl.attach_shader(&program, vertex);
    gl.attach_shader(&program, fragment);
    gl.link_program(&program);

    if gl
        .get_program_parameter(&program, Gl::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        gl.detach_shader(&program, vertex);
        gl.detach_shader(&program, fragment);
        gl.delete_shader(Some(vertex));
        gl.delete_shader(Some(fragment));
        Ok(program)
    } else {
        Err(JsValue::from_str(
            &gl.get_program_info_log(&program)
                .unwrap_or_else(|| "unknown program link failure".to_string()),
        ))
    }
}
