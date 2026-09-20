use crate::draw::{
    DRAW_HASH_OFFSET_BASIS, DrawRange, DrawStats, draw_range_is_visible, hash_visible_draw_ranges,
    parse_draw_range_patches, parse_draw_ranges,
};
use crate::packet::{validate_draw_range_planes, validate_draw_ranges, validate_geometry};
use crate::static_scene::StaticMapState;
use std::collections::HashMap;
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{
    HtmlCanvasElement, WebGl2RenderingContext as Gl, WebGlBuffer, WebGlFramebuffer, WebGlProgram,
    WebGlRenderbuffer, WebGlShader, WebGlTexture, WebGlUniformLocation, WebGlVertexArrayObject,
};

const REFERENCE_VERTEX_SHADER: &str = include_str!("shaders/reference.vert.glsl");
const REFERENCE_FRAGMENT_SHADER: &str = include_str!("shaders/reference.frag.glsl");
const STATIC_VERTEX_SHADER: &str = include_str!("shaders/static.vert.glsl");
const STATIC_FRAGMENT_SHADER: &str = include_str!("shaders/static.frag.glsl");
const NPC_VERTEX_SHADER: &str = include_str!("shaders/npc.vert.glsl");
const PLAYER_VERTEX_SHADER: &str = include_str!("shaders/player.vert.glsl");
const PLAYER_FRAGMENT_SHADER: &str = include_str!("shaders/player.frag.glsl");
const PROJECTILE_VERTEX_SHADER: &str = include_str!("shaders/projectile.vert.glsl");
const PRESENT_VERTEX_SHADER: &str = include_str!("shaders/present.vert.glsl");
const PRESENT_FXAA_FRAGMENT_SHADER: &str = include_str!("shaders/present-fxaa.frag.glsl");
const SCENE_OVERLAY_VERTEX_SHADER: &str = include_str!("shaders/scene-overlay.vert.glsl");
const SCENE_OVERLAY_FRAGMENT_SHADER: &str = include_str!("shaders/scene-overlay.frag.glsl");

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

struct NpcProgram {
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
    npc_data_offset: WebGlUniformLocation,
    map_pos: WebGlUniformLocation,
    time_loaded: WebGlUniformLocation,
    scene_border_size: WebGlUniformLocation,
    model_y_offset: WebGlUniformLocation,
    actor_data_sampler: WebGlUniformLocation,
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

struct PlayerProgram {
    program: WebGlProgram,
    view_matrix: WebGlUniformLocation,
    projection_matrix: WebGlUniformLocation,
    world_entity_transform: WebGlUniformLocation,
    scene_hsl_override: WebGlUniformLocation,
    player_pos: WebGlUniformLocation,
    render_distance: WebGlUniformLocation,
    fog_depth: WebGlUniformLocation,
    current_time: WebGlUniformLocation,
    brightness: WebGlUniformLocation,
    is_new_texture_anim: WebGlUniformLocation,
    color_banding: WebGlUniformLocation,
    player_data_offset: WebGlUniformLocation,
    use_player_slot_attribute: WebGlUniformLocation,
    map_pos: WebGlUniformLocation,
    time_loaded: WebGlUniformLocation,
    scene_border_size: WebGlUniformLocation,
    model_y_offset: WebGlUniformLocation,
    actor_data_sampler: WebGlUniformLocation,
    height_map_sampler: WebGlUniformLocation,
    texture_sampler: WebGlUniformLocation,
    material_sampler: WebGlUniformLocation,
    material_count: WebGlUniformLocation,
    discard_alpha: WebGlUniformLocation,
    sky_color: WebGlUniformLocation,
}

struct ProjectileProgram {
    program: WebGlProgram,
    view_matrix: WebGlUniformLocation,
    projection_matrix: WebGlUniformLocation,
    scene_hsl_override: WebGlUniformLocation,
    player_pos: WebGlUniformLocation,
    render_distance: WebGlUniformLocation,
    fog_depth: WebGlUniformLocation,
    current_time: WebGlUniformLocation,
    brightness: WebGlUniformLocation,
    is_new_texture_anim: WebGlUniformLocation,
    color_banding: WebGlUniformLocation,
    projectile_data_offset: WebGlUniformLocation,
    map_pos: WebGlUniformLocation,
    time_loaded: WebGlUniformLocation,
    scene_border_size: WebGlUniformLocation,
    model_y_offset: WebGlUniformLocation,
    projectile_sub_offset: WebGlUniformLocation,
    actor_data_sampler: WebGlUniformLocation,
    height_map_sampler: WebGlUniformLocation,
    texture_sampler: WebGlUniformLocation,
    material_sampler: WebGlUniformLocation,
    water_texture_sampler: WebGlUniformLocation,
    water_mask_sampler: WebGlUniformLocation,
    texture_layer_count: WebGlUniformLocation,
    material_count: WebGlUniformLocation,
    discard_alpha: WebGlUniformLocation,
    sky_color: WebGlUniformLocation,
    world_entity_opacity: WebGlUniformLocation,
}

struct PresentProgram {
    program: WebGlProgram,
    frame_sampler: WebGlUniformLocation,
    resolution: WebGlUniformLocation,
}

struct SceneOverlayProgram {
    program: WebGlProgram,
    view_matrix: WebGlUniformLocation,
    projection_matrix: WebGlUniformLocation,
    color: WebGlUniformLocation,
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
        validate_draw_range_planes(&opaque, opaque_range_planes)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        validate_draw_range_planes(&alpha, alpha_range_planes)
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

    fn patch_draw_ranges(
        &mut self,
        lod: bool,
        alpha: bool,
        flat_patches: &[u32],
    ) -> Result<(), JsValue> {
        let pass = match (lod, alpha) {
            (false, false) => &mut self.opaque_pass,
            (false, true) => &mut self.alpha_pass,
            (true, false) => &mut self.lod_opaque_pass,
            (true, true) => &mut self.lod_alpha_pass,
        };

        let updates = parse_draw_range_patches(flat_patches, pass.draw_ranges.len())
            .map_err(|error| JsValue::from_str(&error))?;

        for (_, candidate) in &updates {
            validate_draw_ranges(&[*candidate], self.index_count as usize)
                .map_err(|error| JsValue::from_str(&error.to_string()))?;
        }

        for (range_index, candidate) in updates {
            pass.draw_ranges[range_index] = candidate;
        }
        Ok(())
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

struct IndexedGeometryBatch {
    vertex_buffer: WebGlBuffer,
    index_buffer: WebGlBuffer,
    vao: WebGlVertexArrayObject,
    index_count: u32,
}

impl IndexedGeometryBatch {
    fn new(gl: &Gl) -> Result<Self, JsValue> {
        let vertex_buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("failed to create indexed vertex buffer"))?;
        let index_buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("failed to create indexed index buffer"))?;
        let vao = gl
            .create_vertex_array()
            .ok_or_else(|| JsValue::from_str("failed to create indexed vertex array"))?;

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
            index_count: 0,
        })
    }

    fn upload_geometry(
        &mut self,
        gl: &Gl,
        packed_vertices: &[u32],
        indices: &[u32],
    ) -> Result<(), JsValue> {
        self.upload_geometry_with_usage(gl, packed_vertices, indices, Gl::STATIC_DRAW)
    }

    fn upload_geometry_with_usage(
        &mut self,
        gl: &Gl,
        packed_vertices: &[u32],
        indices: &[u32],
        usage: u32,
    ) -> Result<(), JsValue> {
        validate_geometry(packed_vertices, indices)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;

        let vertices = js_sys::Uint32Array::from(packed_vertices);
        let index_data = js_sys::Uint32Array::from(indices);

        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.vertex_buffer));
        gl.buffer_data_with_opt_array_buffer(Gl::ARRAY_BUFFER, Some(&vertices.buffer()), usage);
        gl.bind_buffer(Gl::ELEMENT_ARRAY_BUFFER, Some(&self.index_buffer));
        gl.buffer_data_with_opt_array_buffer(
            Gl::ELEMENT_ARRAY_BUFFER,
            Some(&index_data.buffer()),
            usage,
        );

        self.index_count = indices.len() as u32;
        Ok(())
    }

    fn delete(&self, gl: &Gl) {
        gl.delete_vertex_array(Some(&self.vao));
        gl.delete_buffer(Some(&self.vertex_buffer));
        gl.delete_buffer(Some(&self.index_buffer));
    }
}

const AUX_BATCH_LOC: u32 = 0;
const AUX_BATCH_DOOR: u32 = 1;
const AUX_BATCH_GROUND: u32 = 2;
const NPC_BATCH_KIND: u32 = 5;
const DYNAMIC_NPC_BATCH_KIND: u32 = 6;
const DYNAMIC_PLAYER_BATCH_KIND: u32 = 7;
const DYNAMIC_GFX_BATCH_KIND: u32 = 8;
const DYNAMIC_PROJECTILE_BATCH_KIND: u32 = 9;

struct StaticMapResources {
    terrain_batch: StaticGeometryBatch,
    loc_batch: Option<StaticGeometryBatch>,
    ground_batch: Option<StaticGeometryBatch>,
    door_batch: Option<StaticGeometryBatch>,
    npc_batch: Option<IndexedGeometryBatch>,
    height_map_texture: WebGlTexture,
    water_mask_texture: WebGlTexture,
    state: Option<StaticMapState>,
}

impl StaticMapResources {
    fn new(gl: &Gl) -> Result<Self, JsValue> {
        let height_map_texture = create_nearest_texture(gl, Gl::TEXTURE_2D_ARRAY)?;
        let water_mask_texture = create_nearest_texture(gl, Gl::TEXTURE_2D_ARRAY)?;
        initialize_fallback_water_mask(gl, &water_mask_texture)?;

        Ok(Self {
            terrain_batch: StaticGeometryBatch::new(gl)?,
            loc_batch: None,
            ground_batch: None,
            door_batch: None,
            npc_batch: None,
            height_map_texture,
            water_mask_texture,
            state: None,
        })
    }

    fn is_empty(&self) -> bool {
        self.state.is_none()
            && self.terrain_batch.index_count == 0
            && self.loc_batch.is_none()
            && self.ground_batch.is_none()
            && self.door_batch.is_none()
            && self.npc_batch.is_none()
    }

    fn delete(&mut self, gl: &Gl) {
        self.terrain_batch.delete(gl);
        if let Some(batch) = self.loc_batch.take() {
            batch.delete(gl);
        }
        if let Some(batch) = self.ground_batch.take() {
            batch.delete(gl);
        }
        if let Some(batch) = self.door_batch.take() {
            batch.delete(gl);
        }
        if let Some(batch) = self.npc_batch.take() {
            batch.delete(gl);
        }
        gl.delete_texture(Some(&self.height_map_texture));
        gl.delete_texture(Some(&self.water_mask_texture));
        self.state = None;
    }
}

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
    npc_program: NpcProgram,
    dynamic_npc_batch: IndexedGeometryBatch,
    dynamic_gfx_batch: IndexedGeometryBatch,
    projectile_program: ProjectileProgram,
    dynamic_projectile_batch: IndexedGeometryBatch,
    player_program: PlayerProgram,
    dynamic_player_batch: IndexedGeometryBatch,
    player_slot_buffer: WebGlBuffer,

    static_map_key: u32,
    static_map: StaticMapResources,
    parked_static_maps: HashMap<u32, StaticMapResources>,
    texture_array: WebGlTexture,
    material_texture: WebGlTexture,
    water_texture_array: WebGlTexture,
    actor_data_texture: WebGlTexture,

    presentation_enabled: bool,
    presentation_framebuffer: WebGlFramebuffer,
    presentation_color_texture: WebGlTexture,
    presentation_depth_renderbuffer: WebGlRenderbuffer,
    presentation_msaa_enabled: bool,
    presentation_msaa_framebuffer: WebGlFramebuffer,
    presentation_msaa_color_renderbuffer: WebGlRenderbuffer,
    presentation_msaa_depth_renderbuffer: WebGlRenderbuffer,
    presentation_msaa_samples: i32,
    presentation_fxaa_enabled: bool,
    present_program: PresentProgram,
    present_vao: WebGlVertexArrayObject,
    scene_overlay_program: SceneOverlayProgram,
    scene_overlay_vertex_buffer: WebGlBuffer,
    scene_overlay_vao: WebGlVertexArrayObject,
    presentation_width: i32,
    presentation_height: i32,

    texture_layer_count: i32,
    material_count: i32,
    last_stats: DrawStats,
    last_draw_hash: u32,
    terrain_only_pass: bool,
    terrain_batch_kind: u32,
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
        let npc_program_raw = create_program(&gl, NPC_VERTEX_SHADER, STATIC_FRAGMENT_SHADER)?;
        let player_program_raw = create_program(&gl, PLAYER_VERTEX_SHADER, PLAYER_FRAGMENT_SHADER)?;
        let projectile_program_raw =
            create_program(&gl, PROJECTILE_VERTEX_SHADER, STATIC_FRAGMENT_SHADER)?;
        let present_program_raw =
            create_program(&gl, PRESENT_VERTEX_SHADER, PRESENT_FXAA_FRAGMENT_SHADER)?;
        let present_vao = gl
            .create_vertex_array()
            .ok_or_else(|| JsValue::from_str("failed to create presentation vertex array"))?;
        let scene_overlay_program_raw = create_program(
            &gl,
            SCENE_OVERLAY_VERTEX_SHADER,
            SCENE_OVERLAY_FRAGMENT_SHADER,
        )?;
        let scene_overlay_vertex_buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("failed to create scene overlay vertex buffer"))?;
        let scene_overlay_vao = gl
            .create_vertex_array()
            .ok_or_else(|| JsValue::from_str("failed to create scene overlay vertex array"))?;
        gl.bind_vertex_array(Some(&scene_overlay_vao));
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&scene_overlay_vertex_buffer));
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_with_i32(0, 3, Gl::FLOAT, false, 12, 0);
        gl.bind_vertex_array(None);

        let static_map = StaticMapResources::new(&gl)?;
        let dynamic_npc_batch = IndexedGeometryBatch::new(&gl)?;
        let dynamic_gfx_batch = IndexedGeometryBatch::new(&gl)?;
        let dynamic_projectile_batch = IndexedGeometryBatch::new(&gl)?;
        let dynamic_player_batch = IndexedGeometryBatch::new(&gl)?;
        let player_slot_buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("failed to create player slot buffer"))?;
        gl.bind_vertex_array(Some(&dynamic_player_batch.vao));
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&player_slot_buffer));
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_i_pointer_with_i32(1, 1, Gl::INT, 4, 0);
        gl.vertex_attrib_divisor(1, 1);
        gl.bind_vertex_array(None);
        let texture_array = create_nearest_texture(&gl, Gl::TEXTURE_2D_ARRAY)?;
        let material_texture = create_nearest_texture(&gl, Gl::TEXTURE_2D)?;
        let water_texture_array = create_nearest_texture(&gl, Gl::TEXTURE_2D_ARRAY)?;
        let actor_data_texture = create_nearest_texture(&gl, Gl::TEXTURE_2D)?;
        let presentation_framebuffer = gl
            .create_framebuffer()
            .ok_or_else(|| JsValue::from_str("failed to create presentation framebuffer"))?;
        let presentation_color_texture = create_linear_texture(&gl, Gl::TEXTURE_2D)?;
        let presentation_depth_renderbuffer = gl
            .create_renderbuffer()
            .ok_or_else(|| JsValue::from_str("failed to create presentation depth renderbuffer"))?;
        let presentation_msaa_framebuffer = gl
            .create_framebuffer()
            .ok_or_else(|| JsValue::from_str("failed to create presentation MSAA framebuffer"))?;
        let presentation_msaa_color_renderbuffer = gl.create_renderbuffer().ok_or_else(|| {
            JsValue::from_str("failed to create presentation MSAA color renderbuffer")
        })?;
        let presentation_msaa_depth_renderbuffer = gl.create_renderbuffer().ok_or_else(|| {
            JsValue::from_str("failed to create presentation MSAA depth renderbuffer")
        })?;
        initialize_fallback_texture_array(&gl, &texture_array)?;
        initialize_fallback_materials(&gl, &material_texture)?;
        initialize_fallback_water_textures(&gl, &water_texture_array)?;

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

        let npc_program = NpcProgram {
            view_matrix: required_uniform(&gl, &npc_program_raw, "u_viewMatrix")?,
            projection_matrix: required_uniform(&gl, &npc_program_raw, "u_projectionMatrix")?,
            world_entity_transform: required_uniform(
                &gl,
                &npc_program_raw,
                "u_worldEntityTransform",
            )?,
            world_entity_opacity: required_uniform(&gl, &npc_program_raw, "u_worldEntityOpacity")?,
            scene_hsl_override: required_uniform(&gl, &npc_program_raw, "u_sceneHslOverride")?,
            player_pos: required_uniform(&gl, &npc_program_raw, "u_playerPos")?,
            render_distance: required_uniform(&gl, &npc_program_raw, "u_renderDistance")?,
            fog_depth: required_uniform(&gl, &npc_program_raw, "u_fogDepth")?,
            current_time: required_uniform(&gl, &npc_program_raw, "u_currentTime")?,
            brightness: required_uniform(&gl, &npc_program_raw, "u_brightness")?,
            is_new_texture_anim: required_uniform(&gl, &npc_program_raw, "u_isNewTextureAnim")?,
            color_banding: required_uniform(&gl, &npc_program_raw, "u_colorBanding")?,
            draw_id: required_uniform(&gl, &npc_program_raw, "u_drawId")?,
            npc_data_offset: required_uniform(&gl, &npc_program_raw, "u_npcDataOffset")?,
            map_pos: required_uniform(&gl, &npc_program_raw, "u_mapPos")?,
            time_loaded: required_uniform(&gl, &npc_program_raw, "u_timeLoaded")?,
            scene_border_size: required_uniform(&gl, &npc_program_raw, "u_sceneBorderSize")?,
            model_y_offset: required_uniform(&gl, &npc_program_raw, "u_modelYOffset")?,
            actor_data_sampler: required_uniform(&gl, &npc_program_raw, "u_npcDataTexture")?,
            height_map_sampler: required_uniform(&gl, &npc_program_raw, "u_heightMap")?,
            texture_sampler: required_uniform(&gl, &npc_program_raw, "u_textures")?,
            material_sampler: required_uniform(&gl, &npc_program_raw, "u_textureMaterials")?,
            water_texture_sampler: required_uniform(&gl, &npc_program_raw, "u_waterTextures")?,
            water_mask_sampler: required_uniform(&gl, &npc_program_raw, "u_waterMask")?,
            texture_layer_count: required_uniform(&gl, &npc_program_raw, "u_textureLayerCount")?,
            material_count: required_uniform(&gl, &npc_program_raw, "u_materialCount")?,
            discard_alpha: required_uniform(&gl, &npc_program_raw, "u_discardAlpha")?,
            sky_color: required_uniform(&gl, &npc_program_raw, "u_skyColor")?,
            program: npc_program_raw,
        };

        let player_program = PlayerProgram {
            view_matrix: required_uniform(&gl, &player_program_raw, "u_viewMatrix")?,
            projection_matrix: required_uniform(&gl, &player_program_raw, "u_projectionMatrix")?,
            world_entity_transform: required_uniform(
                &gl,
                &player_program_raw,
                "u_worldEntityTransform",
            )?,
            scene_hsl_override: required_uniform(&gl, &player_program_raw, "u_sceneHslOverride")?,
            player_pos: required_uniform(&gl, &player_program_raw, "u_playerPos")?,
            render_distance: required_uniform(&gl, &player_program_raw, "u_renderDistance")?,
            fog_depth: required_uniform(&gl, &player_program_raw, "u_fogDepth")?,
            current_time: required_uniform(&gl, &player_program_raw, "u_currentTime")?,
            brightness: required_uniform(&gl, &player_program_raw, "u_brightness")?,
            is_new_texture_anim: required_uniform(&gl, &player_program_raw, "u_isNewTextureAnim")?,
            color_banding: required_uniform(&gl, &player_program_raw, "u_colorBanding")?,
            player_data_offset: required_uniform(&gl, &player_program_raw, "u_playerDataOffset")?,
            use_player_slot_attribute: required_uniform(
                &gl,
                &player_program_raw,
                "u_usePlayerSlotAttribute",
            )?,
            map_pos: required_uniform(&gl, &player_program_raw, "u_mapPos")?,
            time_loaded: required_uniform(&gl, &player_program_raw, "u_timeLoaded")?,
            scene_border_size: required_uniform(&gl, &player_program_raw, "u_sceneBorderSize")?,
            model_y_offset: required_uniform(&gl, &player_program_raw, "u_modelYOffset")?,
            actor_data_sampler: required_uniform(&gl, &player_program_raw, "u_playerDataTexture")?,
            height_map_sampler: required_uniform(&gl, &player_program_raw, "u_heightMap")?,
            texture_sampler: required_uniform(&gl, &player_program_raw, "u_textures")?,
            material_sampler: required_uniform(&gl, &player_program_raw, "u_textureMaterials")?,
            material_count: required_uniform(&gl, &player_program_raw, "u_materialCount")?,
            discard_alpha: required_uniform(&gl, &player_program_raw, "u_discardAlpha")?,
            sky_color: required_uniform(&gl, &player_program_raw, "u_skyColor")?,
            program: player_program_raw,
        };

        let present_program = PresentProgram {
            frame_sampler: required_uniform(&gl, &present_program_raw, "u_frame")?,
            resolution: required_uniform(&gl, &present_program_raw, "u_resolution")?,
            program: present_program_raw,
        };
        let scene_overlay_program = SceneOverlayProgram {
            view_matrix: required_uniform(&gl, &scene_overlay_program_raw, "u_viewMatrix")?,
            projection_matrix: required_uniform(
                &gl,
                &scene_overlay_program_raw,
                "u_projectionMatrix",
            )?,
            color: required_uniform(&gl, &scene_overlay_program_raw, "u_color")?,
            program: scene_overlay_program_raw,
        };

        let projectile_program = ProjectileProgram {
            view_matrix: required_uniform(&gl, &projectile_program_raw, "u_viewMatrix")?,
            projection_matrix: required_uniform(
                &gl,
                &projectile_program_raw,
                "u_projectionMatrix",
            )?,
            scene_hsl_override: required_uniform(
                &gl,
                &projectile_program_raw,
                "u_sceneHslOverride",
            )?,
            player_pos: required_uniform(&gl, &projectile_program_raw, "u_playerPos")?,
            render_distance: required_uniform(&gl, &projectile_program_raw, "u_renderDistance")?,
            fog_depth: required_uniform(&gl, &projectile_program_raw, "u_fogDepth")?,
            current_time: required_uniform(&gl, &projectile_program_raw, "u_currentTime")?,
            brightness: required_uniform(&gl, &projectile_program_raw, "u_brightness")?,
            is_new_texture_anim: required_uniform(
                &gl,
                &projectile_program_raw,
                "u_isNewTextureAnim",
            )?,
            color_banding: required_uniform(&gl, &projectile_program_raw, "u_colorBanding")?,
            projectile_data_offset: required_uniform(
                &gl,
                &projectile_program_raw,
                "u_projectileDataOffset",
            )?,
            map_pos: required_uniform(&gl, &projectile_program_raw, "u_mapPos")?,
            time_loaded: required_uniform(&gl, &projectile_program_raw, "u_timeLoaded")?,
            scene_border_size: required_uniform(&gl, &projectile_program_raw, "u_sceneBorderSize")?,
            model_y_offset: required_uniform(&gl, &projectile_program_raw, "u_modelYOffset")?,
            projectile_sub_offset: required_uniform(
                &gl,
                &projectile_program_raw,
                "u_projectileSubOffset",
            )?,
            actor_data_sampler: required_uniform(&gl, &projectile_program_raw, "u_npcDataTexture")?,
            height_map_sampler: required_uniform(&gl, &projectile_program_raw, "u_heightMap")?,
            texture_sampler: required_uniform(&gl, &projectile_program_raw, "u_textures")?,
            material_sampler: required_uniform(&gl, &projectile_program_raw, "u_textureMaterials")?,
            water_texture_sampler: required_uniform(
                &gl,
                &projectile_program_raw,
                "u_waterTextures",
            )?,
            water_mask_sampler: required_uniform(&gl, &projectile_program_raw, "u_waterMask")?,
            texture_layer_count: required_uniform(
                &gl,
                &projectile_program_raw,
                "u_textureLayerCount",
            )?,
            material_count: required_uniform(&gl, &projectile_program_raw, "u_materialCount")?,
            discard_alpha: required_uniform(&gl, &projectile_program_raw, "u_discardAlpha")?,
            sky_color: required_uniform(&gl, &projectile_program_raw, "u_skyColor")?,
            world_entity_opacity: required_uniform(
                &gl,
                &projectile_program_raw,
                "u_worldEntityOpacity",
            )?,
            program: projectile_program_raw,
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
            npc_program,
            dynamic_npc_batch,
            dynamic_gfx_batch,
            projectile_program,
            dynamic_projectile_batch,
            player_program,
            dynamic_player_batch,
            player_slot_buffer,
            static_map_key: 0,
            static_map,
            parked_static_maps: HashMap::new(),
            texture_array,
            material_texture,
            water_texture_array,
            actor_data_texture,
            presentation_enabled: false,
            presentation_framebuffer,
            presentation_color_texture,
            presentation_depth_renderbuffer,
            presentation_msaa_enabled: false,
            presentation_msaa_framebuffer,
            presentation_msaa_color_renderbuffer,
            presentation_msaa_depth_renderbuffer,
            presentation_msaa_samples: 0,
            presentation_fxaa_enabled: false,
            present_program,
            present_vao,
            scene_overlay_program,
            scene_overlay_vertex_buffer,
            scene_overlay_vao,
            presentation_width: 0,
            presentation_height: 0,
            texture_layer_count: 1,
            material_count: 1,
            last_stats: DrawStats::default(),
            last_draw_hash: DRAW_HASH_OFFSET_BASIS,
            terrain_only_pass: false,
            terrain_batch_kind: 0,
        })
    }

    pub fn abi_version(&self) -> u32 {
        crate::RENDERER_ABI_VERSION
    }

    /// Selects a resident static map slot, preserving the previously active
    /// map's GPU resources for later reuse.
    pub fn select_static_map(&mut self, map_key: u32) -> Result<(), JsValue> {
        if self.static_map_key == map_key {
            return Ok(());
        }

        if self.parked_static_maps.is_empty() && self.static_map.is_empty() {
            let next = StaticMapResources::new(&self.gl)?;
            let mut placeholder = std::mem::replace(&mut self.static_map, next);
            placeholder.delete(&self.gl);
            self.static_map_key = map_key;
            return Ok(());
        }

        let next = match self.parked_static_maps.remove(&map_key) {
            Some(map) => map,
            None => StaticMapResources::new(&self.gl)?,
        };
        let previous = std::mem::replace(&mut self.static_map, next);
        self.parked_static_maps
            .insert(self.static_map_key, previous);
        self.static_map_key = map_key;
        Ok(())
    }

    pub fn active_static_map_key(&self) -> u32 {
        self.static_map_key
    }

    pub fn resident_static_map_count(&self) -> u32 {
        let active_count = usize::from(!self.static_map.is_empty());
        (self.parked_static_maps.len() + active_count) as u32
    }

    pub fn remove_static_map(&mut self, map_key: u32) -> Result<(), JsValue> {
        if map_key != self.static_map_key {
            if let Some(mut map) = self.parked_static_maps.remove(&map_key) {
                map.delete(&self.gl);
            }
            return Ok(());
        }

        if let Some(next_key) = self.parked_static_maps.keys().next().copied() {
            let next = self
                .parked_static_maps
                .remove(&next_key)
                .expect("resident map key disappeared");
            let mut removed = std::mem::replace(&mut self.static_map, next);
            removed.delete(&self.gl);
            self.static_map_key = next_key;
            return Ok(());
        }

        let replacement = StaticMapResources::new(&self.gl)?;
        let mut removed = std::mem::replace(&mut self.static_map, replacement);
        removed.delete(&self.gl);
        self.static_map_key = 0;
        Ok(())
    }

    pub fn clear_static_maps(&mut self) -> Result<(), JsValue> {
        for (_, mut map) in self.parked_static_maps.drain() {
            map.delete(&self.gl);
        }

        let replacement = StaticMapResources::new(&self.gl)?;
        let mut active = std::mem::replace(&mut self.static_map, replacement);
        active.delete(&self.gl);
        self.static_map_key = 0;
        Ok(())
    }

    /// Uploads the current TypeScript packed-vertex/index packet unchanged.
    ///
    /// packed_vertices is [v0,v1,v2, v0,v1,v2, ...].
    pub fn upload_geometry(
        &mut self,
        packed_vertices: &[u32],
        indices: &[u32],
    ) -> Result<(), JsValue> {
        self.static_map
            .terrain_batch
            .upload_geometry(&self.gl, packed_vertices, indices)
    }

    /// Uploads the current map's prebaked NPC packed geometry. The NPC shader
    /// is not Rust-owned yet; Stage 2 mirrors GPU ownership first so draw
    /// behavior can move independently in the next parity step.
    pub fn upload_npc_geometry(
        &mut self,
        packed_vertices: &[u32],
        indices: &[u32],
    ) -> Result<(), JsValue> {
        if packed_vertices.is_empty() && indices.is_empty() {
            if let Some(batch) = self.static_map.npc_batch.take() {
                batch.delete(&self.gl);
            }
            return Ok(());
        }

        let mut batch = self
            .static_map
            .npc_batch
            .take()
            .unwrap_or(IndexedGeometryBatch::new(&self.gl)?);

        if let Err(error) = batch.upload_geometry(&self.gl, packed_vertices, indices) {
            self.static_map.npc_batch = Some(batch);
            return Err(error);
        }

        self.static_map.npc_batch = Some(batch);
        Ok(())
    }

    /// Uploads one current-frame dynamic NPC geometry packet into a
    /// reusable GPU batch. TypeScript still owns animation selection.
    pub fn upload_dynamic_npc_geometry(
        &mut self,
        packed_vertices: &[u32],
        indices: &[u32],
    ) -> Result<(), JsValue> {
        self.dynamic_npc_batch.upload_geometry_with_usage(
            &self.gl,
            packed_vertices,
            indices,
            Gl::DYNAMIC_DRAW,
        )
    }

    /// Uploads one finalized spot-animation/GFX geometry packet into a
    /// reusable batch. TypeScript remains authoritative for effect frame choice.
    pub fn upload_dynamic_gfx_geometry(
        &mut self,
        packed_vertices: &[u32],
        indices: &[u32],
    ) -> Result<(), JsValue> {
        self.dynamic_gfx_batch.upload_geometry_with_usage(
            &self.gl,
            packed_vertices,
            indices,
            Gl::DYNAMIC_DRAW,
        )
    }

    /// Uploads one finalized projectile frame geometry packet into a
    /// reusable batch. TypeScript remains authoritative for trajectory and
    /// animation-frame selection.
    pub fn upload_dynamic_projectile_geometry(
        &mut self,
        packed_vertices: &[u32],
        indices: &[u32],
    ) -> Result<(), JsValue> {
        self.dynamic_projectile_batch.upload_geometry_with_usage(
            &self.gl,
            packed_vertices,
            indices,
            Gl::DYNAMIC_DRAW,
        )
    }

    /// Uploads one finalized player geometry packet into a reusable
    /// dynamic batch. Appearance construction and animation remain in TypeScript.
    pub fn upload_dynamic_player_geometry(
        &mut self,
        packed_vertices: &[u32],
        indices: &[u32],
    ) -> Result<(), JsValue> {
        self.dynamic_player_batch.upload_geometry_with_usage(
            &self.gl,
            packed_vertices,
            indices,
            Gl::DYNAMIC_DRAW,
        )
    }

    /// Uploads one RGBA16UI model-info packet produced by SceneBuffer.
    ///
    /// The first texels hold per-draw instance offsets; remaining texels hold
    /// encoded ModelInfo instances. Width is fixed at 16 to match main.vert.
    pub fn upload_model_info(&mut self, model_info: &[u16]) -> Result<(), JsValue> {
        upload_model_info_texture(
            &self.gl,
            &self.static_map.terrain_batch.opaque_pass.model_info_texture,
            model_info,
            "model-info",
        )
    }

    /// Uploads the RGBA16UI model-info packet for the alpha static pass.
    pub fn upload_model_info_alpha(&mut self, model_info: &[u16]) -> Result<(), JsValue> {
        upload_model_info_texture(
            &self.gl,
            &self.static_map.terrain_batch.alpha_pass.model_info_texture,
            model_info,
            "alpha model-info",
        )
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
        self.gl.bind_texture(
            Gl::TEXTURE_2D_ARRAY,
            Some(&self.static_map.height_map_texture),
        );
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

        if let Some(mut state) = self.static_map.state {
            state.height_map_size = size;
            state.height_map_planes = planes;
            self.static_map.state = Some(state);
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

    /// Uploads the live 16-wide RGBA16UI actor-data texture produced by
    /// TypeScript. Stage 2 initially mirrors this resource without changing
    /// player/NPC draw ownership.
    pub fn upload_actor_data(
        &mut self,
        values: &[u16],
        width: u32,
        height: u32,
    ) -> Result<(), JsValue> {
        if width == 0 || height == 0 {
            return Err(JsValue::from_str(
                "actor-data texture dimensions must be positive",
            ));
        }

        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|value| value.checked_mul(4))
            .ok_or_else(|| JsValue::from_str("actor-data texture dimensions overflow"))?;
        if values.len() != expected {
            return Err(JsValue::from_str(&format!(
                "actor-data packet has {} u16 values, expected {expected}",
                values.len()
            )));
        }

        let data = js_sys::Uint16Array::from(values);
        self.gl.active_texture(Gl::TEXTURE6);
        self.gl
            .bind_texture(Gl::TEXTURE_2D, Some(&self.actor_data_texture));
        self.gl
            .tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_array_buffer_view(
                Gl::TEXTURE_2D,
                0,
                Gl::RGBA16UI as i32,
                width as i32,
                height as i32,
                0,
                Gl::RGBA_INTEGER,
                Gl::UNSIGNED_SHORT,
                Some(data.unchecked_ref()),
            )?;
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
        self.gl.bind_texture(
            Gl::TEXTURE_2D_ARRAY,
            Some(&self.static_map.water_mask_texture),
        );
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
        self.static_map.state = Some(state);
        Ok(())
    }

    /// Existing TypeScript wire layout:
    /// [offsetBytes,elements,instances, ...].
    pub fn set_draw_ranges(&mut self, flat_ranges: &[u32]) -> Result<(), JsValue> {
        let ranges = parse_draw_ranges(flat_ranges).map_err(JsValue::from_str)?;
        validate_draw_ranges(&ranges, self.static_map.terrain_batch.index_count as usize)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        self.static_map.terrain_batch.opaque_pass.draw_ranges = ranges;
        self.static_map
            .terrain_batch
            .opaque_pass
            .range_planes
            .clear();
        Ok(())
    }

    pub fn set_draw_ranges_alpha(&mut self, flat_ranges: &[u32]) -> Result<(), JsValue> {
        let ranges = parse_draw_ranges(flat_ranges).map_err(JsValue::from_str)?;
        validate_draw_ranges(&ranges, self.static_map.terrain_batch.index_count as usize)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        self.static_map.terrain_batch.alpha_pass.draw_ranges = ranges;
        self.static_map
            .terrain_batch
            .alpha_pass
            .range_planes
            .clear();
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
        validate_draw_ranges(&opaque, self.static_map.terrain_batch.index_count as usize)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let alpha = parse_draw_ranges(alpha_ranges).map_err(JsValue::from_str)?;
        validate_draw_ranges(&alpha, self.static_map.terrain_batch.index_count as usize)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;

        if !opaque.is_empty() {
            self.upload_model_info(model_info_opaque)?;
        }
        if !alpha.is_empty() {
            self.upload_model_info_alpha(model_info_alpha)?;
        }

        self.static_map.terrain_batch.opaque_pass.draw_ranges = opaque;
        self.static_map.terrain_batch.alpha_pass.draw_ranges = alpha;
        self.static_map.terrain_batch.opaque_pass.range_planes = opaque_range_planes.to_vec();
        self.static_map.terrain_batch.alpha_pass.range_planes = alpha_range_planes.to_vec();
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
        validate_draw_ranges(&opaque, self.static_map.terrain_batch.index_count as usize)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let alpha = parse_draw_ranges(alpha_ranges).map_err(JsValue::from_str)?;
        validate_draw_ranges(&alpha, self.static_map.terrain_batch.index_count as usize)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;

        if !opaque.is_empty() {
            upload_model_info_texture(
                &self.gl,
                &self
                    .static_map
                    .terrain_batch
                    .lod_opaque_pass
                    .model_info_texture,
                model_info_opaque,
                "static LOD opaque model-info",
            )?;
        }

        if !alpha.is_empty() {
            upload_model_info_texture(
                &self.gl,
                &self
                    .static_map
                    .terrain_batch
                    .lod_alpha_pass
                    .model_info_texture,
                model_info_alpha,
                "static LOD alpha model-info",
            )?;
        }

        self.static_map.terrain_batch.lod_opaque_pass.draw_ranges = opaque;
        self.static_map.terrain_batch.lod_alpha_pass.draw_ranges = alpha;
        self.static_map.terrain_batch.lod_opaque_pass.range_planes = opaque_range_planes.to_vec();
        self.static_map.terrain_batch.lod_alpha_pass.range_planes = alpha_range_planes.to_vec();
        Ok(())
    }

    pub fn clear_draw_ranges(&mut self) {
        self.static_map.terrain_batch.opaque_pass.clear();
        self.static_map.terrain_batch.alpha_pass.clear();
        self.static_map.terrain_batch.lod_opaque_pass.clear();
        self.static_map.terrain_batch.lod_alpha_pass.clear();
    }

    /// Uploads geometry for an auxiliary static map batch.
    ///
    /// kind 0 = non-door loc geometry, kind 1 = door geometry, kind 2 = ground items.
    pub fn upload_aux_geometry(
        &mut self,
        kind: u32,
        packed_vertices: &[u32],
        indices: &[u32],
    ) -> Result<(), JsValue> {
        if packed_vertices.is_empty() && indices.is_empty() {
            let existing = match kind {
                AUX_BATCH_LOC => self.static_map.loc_batch.take(),
                AUX_BATCH_DOOR => self.static_map.door_batch.take(),
                AUX_BATCH_GROUND => self.static_map.ground_batch.take(),
                _ => return Err(JsValue::from_str("unknown auxiliary static batch kind")),
            };
            if let Some(batch) = existing {
                batch.delete(&self.gl);
            }
            return Ok(());
        }

        let mut batch = match kind {
            AUX_BATCH_LOC => self.static_map.loc_batch.take(),
            AUX_BATCH_DOOR => self.static_map.door_batch.take(),
            AUX_BATCH_GROUND => self.static_map.ground_batch.take(),
            _ => return Err(JsValue::from_str("unknown auxiliary static batch kind")),
        }
        .unwrap_or(StaticGeometryBatch::new(&self.gl)?);

        if let Err(error) = batch.upload_geometry(&self.gl, packed_vertices, indices) {
            match kind {
                AUX_BATCH_LOC => self.static_map.loc_batch = Some(batch),
                AUX_BATCH_DOOR => self.static_map.door_batch = Some(batch),
                AUX_BATCH_GROUND => self.static_map.ground_batch = Some(batch),
                _ => unreachable!(),
            }
            return Err(error);
        }

        match kind {
            AUX_BATCH_LOC => self.static_map.loc_batch = Some(batch),
            AUX_BATCH_DOOR => self.static_map.door_batch = Some(batch),
            AUX_BATCH_GROUND => self.static_map.ground_batch = Some(batch),
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

    /// Applies per-frame animated-loc draw-range changes without re-uploading
    /// geometry or model-info textures.
    pub fn patch_aux_draw_ranges(
        &mut self,
        kind: u32,
        lod: bool,
        alpha: bool,
        flat_patches: &[u32],
    ) -> Result<(), JsValue> {
        if flat_patches.is_empty() {
            return Ok(());
        }

        let batch = match kind {
            AUX_BATCH_LOC => self
                .static_map
                .loc_batch
                .as_mut()
                .ok_or_else(|| JsValue::from_str("loc geometry has not been uploaded"))?,
            AUX_BATCH_DOOR => self
                .static_map
                .door_batch
                .as_mut()
                .ok_or_else(|| JsValue::from_str("door geometry has not been uploaded"))?,
            AUX_BATCH_GROUND => self
                .static_map
                .ground_batch
                .as_mut()
                .ok_or_else(|| JsValue::from_str("ground geometry has not been uploaded"))?,
            _ => return Err(JsValue::from_str("unknown auxiliary static batch kind")),
        };

        batch.patch_draw_ranges(lod, alpha, flat_patches)
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
        self.gl
            .bind_vertex_array(Some(&self.static_map.terrain_batch.vao));

        let stats = submit_draw_ranges(
            &self.gl,
            &self.static_map.terrain_batch.opaque_pass.draw_ranges,
            self.static_map.terrain_batch.index_count,
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

    pub fn begin_static_frame(&mut self, sky_rgba: &[f32]) -> Result<(), JsValue> {
        require_vec4(sky_rgba, "sky_rgba")?;
        if self.presentation_enabled {
            self.ensure_presentation_target()?;
            let framebuffer = if self.presentation_msaa_enabled {
                &self.presentation_msaa_framebuffer
            } else {
                &self.presentation_framebuffer
            };
            self.gl.bind_framebuffer(Gl::FRAMEBUFFER, Some(framebuffer));
        } else {
            self.gl.bind_framebuffer(Gl::FRAMEBUFFER, None);
        }
        self.prepare_default_frame(sky_rgba);
        self.last_stats = DrawStats::default();
        self.last_draw_hash = DRAW_HASH_OFFSET_BASIS;
        Ok(())
    }

    /// Renders one pass for the currently selected resident map without
    /// clearing the framebuffer. This is the building block for a visible-map
    /// frame that draws all opaque maps first and transparent maps in reverse.
    #[allow(clippy::too_many_arguments)]
    pub fn render_active_static_map_pass(
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
        transparent: bool,
    ) -> Result<(), JsValue> {
        if transparent {
            self.gl.enable(Gl::BLEND);
            self.gl.blend_func(Gl::SRC_ALPHA, Gl::ONE_MINUS_SRC_ALPHA);
        } else {
            self.gl.disable(Gl::BLEND);
        }

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
            transparent,
            false,
        )
    }

    /// Mirrors the PicoGL Mode-1 overlapping world-entity ghost redraw.
    ///
    /// The live renderer redraws only the opaque terrain batch with blending,
    /// a packed-HSL override and very low world-entity opacity. Auxiliary loc,
    /// ground-item and door batches are intentionally excluded.
    #[allow(clippy::too_many_arguments)]
    pub fn render_active_static_terrain_ghost_pass(
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
        self.gl.enable(Gl::BLEND);
        self.gl.blend_func(Gl::SRC_ALPHA, Gl::ONE_MINUS_SRC_ALPHA);

        self.terrain_only_pass = true;
        self.terrain_batch_kind = 4;
        let result = self.render_static(
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
            false,
        );
        self.terrain_only_pass = false;
        self.terrain_batch_kind = 0;
        self.gl.disable(Gl::BLEND);
        result
    }

    /// Backwards-compatible one-map frame wrapper.
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
        self.begin_static_frame(sky_rgba)?;
        self.render_active_static_map_pass(
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
        )?;
        self.render_active_static_map_pass(
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
            .static_map
            .state
            .ok_or_else(|| JsValue::from_str("static map state has not been configured"))?;
        let pass = match (use_lod, discard_alpha) {
            (false, false) => &self.static_map.terrain_batch.opaque_pass,
            (false, true) => &self.static_map.terrain_batch.alpha_pass,
            (true, false) => &self.static_map.terrain_batch.lod_opaque_pass,
            (true, true) => &self.static_map.terrain_batch.lod_alpha_pass,
        };

        if clear_frame {
            self.prepare_default_frame(sky_rgba);
            self.last_draw_hash = DRAW_HASH_OFFSET_BASIS;
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
        self.gl.bind_texture(
            Gl::TEXTURE_2D_ARRAY,
            Some(&self.static_map.height_map_texture),
        );
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
        self.gl.bind_texture(
            Gl::TEXTURE_2D_ARRAY,
            Some(&self.static_map.water_mask_texture),
        );
        self.gl
            .uniform1i(Some(&self.static_program.water_mask_sampler), 5);

        let roof_limit = roof_plane_limit.clamp(0.0, 3.0) as u8;

        let flags = u32::from(discard_alpha) | (u32::from(use_lod) << 1);
        let mut draw_hash = hash_visible_draw_ranges(
            self.last_draw_hash,
            self.static_map_key,
            flags,
            self.terrain_batch_kind,
            &pass.draw_ranges,
            Some(&pass.range_planes),
            roof_limit,
        );

        self.gl
            .bind_vertex_array(Some(&self.static_map.terrain_batch.vao));
        let mut stats = submit_draw_ranges(
            &self.gl,
            &pass.draw_ranges,
            self.static_map.terrain_batch.index_count,
            Some(&self.static_program.draw_id),
            Some(&pass.range_planes),
            roof_limit,
            false,
        );

        if !self.terrain_only_pass {
            for (batch_kind, batch) in [
                (1, self.static_map.loc_batch.as_ref()),
                (2, self.static_map.ground_batch.as_ref()),
                (3, self.static_map.door_batch.as_ref()),
            ] {
                let Some(batch) = batch else {
                    continue;
                };
                let batch_pass = batch.pass(use_lod, discard_alpha);
                draw_hash = hash_visible_draw_ranges(
                    draw_hash,
                    self.static_map_key,
                    flags,
                    batch_kind,
                    &batch_pass.draw_ranges,
                    Some(&batch_pass.range_planes),
                    roof_limit,
                );
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
        }

        self.last_draw_hash = draw_hash;
        self.gl.bind_vertex_array(None);
        if clear_frame {
            self.last_stats = stats;
        } else {
            self.last_stats.draw_calls += stats.draw_calls;
            self.last_stats.submitted_indices += stats.submitted_indices;
        }
        Ok(())
    }

    /// Renders one prebaked NPC pass for the currently selected map.
    ///
    /// TypeScript remains responsible for simulation and animation-frame
    /// selection. The ABI receives only the final draw ranges plus numeric
    /// renderer state, keeping ECS objects out of Rust.
    #[allow(clippy::too_many_arguments)]
    pub fn render_active_npc_pass(
        &mut self,
        flat_ranges: &[u32],
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
        is_new_texture_anim: bool,
        color_banding: f32,
        npc_data_offset: i32,
        model_y_offset: f32,
        transparent: bool,
    ) -> Result<(), JsValue> {
        let (npc_vao, npc_index_count) = match self.static_map.npc_batch.as_ref() {
            Some(batch) => (batch.vao.clone(), batch.index_count),
            None if flat_ranges.is_empty() => return Ok(()),
            None => {
                return Err(JsValue::from_str(
                    "NPC geometry has not been uploaded for the active map",
                ));
            }
        };

        self.render_npc_geometry_pass(
            flat_ranges,
            npc_vao,
            npc_index_count,
            NPC_BATCH_KIND,
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
            is_new_texture_anim,
            color_banding,
            npc_data_offset,
            model_y_offset,
            transparent,
            None,
            None,
            None,
        )
    }

    /// Renders the currently uploaded dynamic NPC fallback geometry for the
    /// selected map. The actor-data offset already points at the one actor
    /// represented by this temporary geometry packet.
    #[allow(clippy::too_many_arguments)]
    pub fn render_active_dynamic_npc_pass(
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
        is_new_texture_anim: bool,
        color_banding: f32,
        npc_data_offset: i32,
        model_y_offset: f32,
        transparent: bool,
    ) -> Result<(), JsValue> {
        let npc_index_count = self.dynamic_npc_batch.index_count;
        if npc_index_count == 0 {
            return Ok(());
        }
        let npc_vao = self.dynamic_npc_batch.vao.clone();
        let range = [0, npc_index_count, 1];

        self.render_npc_geometry_pass(
            &range,
            npc_vao,
            npc_index_count,
            DYNAMIC_NPC_BATCH_KIND,
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
            is_new_texture_anim,
            color_banding,
            npc_data_offset,
            model_y_offset,
            transparent,
            None,
            None,
            None,
        )
    }

    /// Renders one finalized attached/world spot-animation geometry packet
    /// using the NPC actor transform contract. Production GFX deliberately
    /// disables map-load fade and back-face culling.
    #[allow(clippy::too_many_arguments)]
    pub fn render_active_gfx_pass(
        &mut self,
        view_matrix: &[f32],
        projection_matrix: &[f32],
        sky_rgba: &[f32],
        scene_hsl_override: &[f32],
        player_pos: &[f32],
        render_distance: f32,
        fog_depth: f32,
        current_time: f32,
        brightness: f32,
        is_new_texture_anim: bool,
        color_banding: f32,
        actor_data_offset: i32,
        model_y_offset: f32,
        map_x: f32,
        map_y: f32,
        transparent: bool,
        restore_cull_back_face: bool,
    ) -> Result<(), JsValue> {
        let index_count = self.dynamic_gfx_batch.index_count;
        if index_count == 0 {
            return Ok(());
        }
        let vao = self.dynamic_gfx_batch.vao.clone();
        let range = [0, index_count, 1];
        let identity = [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];

        let result = self.render_npc_geometry_pass(
            &range,
            vao,
            index_count,
            DYNAMIC_GFX_BATCH_KIND,
            view_matrix,
            projection_matrix,
            &identity,
            1.0,
            sky_rgba,
            scene_hsl_override,
            player_pos,
            render_distance,
            fog_depth,
            current_time,
            brightness,
            is_new_texture_anim,
            color_banding,
            actor_data_offset,
            model_y_offset,
            transparent,
            Some((map_x, map_y)),
            Some(-1.0),
            Some(false),
        );

        if restore_cull_back_face {
            self.gl.enable(Gl::CULL_FACE);
            self.gl.cull_face(Gl::BACK);
        } else {
            self.gl.disable(Gl::CULL_FACE);
        }
        result
    }

    /// Renders one finalized projectile frame. TypeScript remains
    /// authoritative for trajectory, orientation packing and animation frame
    /// choice; Rust owns the GPU upload/state/submission boundary.
    #[allow(clippy::too_many_arguments)]
    pub fn render_active_projectile_pass(
        &mut self,
        view_matrix: &[f32],
        projection_matrix: &[f32],
        sky_rgba: &[f32],
        scene_hsl_override: &[f32],
        player_pos: &[f32],
        render_distance: f32,
        fog_depth: f32,
        current_time: f32,
        brightness: f32,
        is_new_texture_anim: bool,
        color_banding: f32,
        projectile_data_offset: i32,
        model_y_offset: f32,
        projectile_sub_offset: &[f32],
        map_x: f32,
        map_y: f32,
        transparent: bool,
        cull_back_face: bool,
    ) -> Result<(), JsValue> {
        require_matrix(view_matrix, "view_matrix")?;
        require_matrix(projection_matrix, "projection_matrix")?;
        require_vec4(sky_rgba, "sky_rgba")?;
        require_vec4(scene_hsl_override, "scene_hsl_override")?;
        if player_pos.len() != 2 {
            return Err(JsValue::from_str("player_pos must contain two f32 values"));
        }
        if projectile_sub_offset.len() != 2 {
            return Err(JsValue::from_str(
                "projectile_sub_offset must contain two f32 values",
            ));
        }
        if projectile_data_offset < 0 {
            return Err(JsValue::from_str(
                "projectile_data_offset must be non-negative",
            ));
        }

        let state = self
            .static_map
            .state
            .ok_or_else(|| JsValue::from_str("static map state has not been configured"))?;
        let index_count = self.dynamic_projectile_batch.index_count;
        if index_count == 0 {
            return Ok(());
        }

        if transparent {
            self.gl.enable(Gl::BLEND);
            self.gl.blend_func(Gl::SRC_ALPHA, Gl::ONE_MINUS_SRC_ALPHA);
            self.gl.depth_mask(false);
        } else {
            self.gl.disable(Gl::BLEND);
            self.gl.depth_mask(true);
        }
        if cull_back_face {
            self.gl.enable(Gl::CULL_FACE);
            self.gl.cull_face(Gl::BACK);
        } else {
            self.gl.disable(Gl::CULL_FACE);
        }

        self.prepare_viewport();
        self.gl.use_program(Some(&self.projectile_program.program));

        self.gl.uniform_matrix4fv_with_f32_array(
            Some(&self.projectile_program.view_matrix),
            false,
            view_matrix,
        );
        self.gl.uniform_matrix4fv_with_f32_array(
            Some(&self.projectile_program.projection_matrix),
            false,
            projection_matrix,
        );
        self.gl.uniform4fv_with_f32_array(
            Some(&self.projectile_program.scene_hsl_override),
            scene_hsl_override,
        );
        self.gl.uniform2f(
            Some(&self.projectile_program.player_pos),
            player_pos[0],
            player_pos[1],
        );
        self.gl.uniform1f(
            Some(&self.projectile_program.render_distance),
            render_distance.max(0.0001),
        );
        self.gl
            .uniform1f(Some(&self.projectile_program.fog_depth), fog_depth.max(0.0));
        self.gl
            .uniform1f(Some(&self.projectile_program.current_time), current_time);
        self.gl.uniform1f(
            Some(&self.projectile_program.brightness),
            brightness.max(0.0001),
        );
        self.gl.uniform1f(
            Some(&self.projectile_program.is_new_texture_anim),
            if is_new_texture_anim { 1.0 } else { 0.0 },
        );
        self.gl.uniform1f(
            Some(&self.projectile_program.color_banding),
            color_banding.max(1.0),
        );
        self.gl.uniform1i(
            Some(&self.projectile_program.projectile_data_offset),
            projectile_data_offset,
        );
        self.gl
            .uniform2f(Some(&self.projectile_program.map_pos), map_x, map_y);
        self.gl
            .uniform1f(Some(&self.projectile_program.time_loaded), -1.0);
        self.gl.uniform1i(
            Some(&self.projectile_program.scene_border_size),
            state.border_size,
        );
        self.gl.uniform1f(
            Some(&self.projectile_program.model_y_offset),
            model_y_offset,
        );
        self.gl.uniform2f(
            Some(&self.projectile_program.projectile_sub_offset),
            projectile_sub_offset[0],
            projectile_sub_offset[1],
        );
        self.gl.uniform1i(
            Some(&self.projectile_program.texture_layer_count),
            self.texture_layer_count.max(1),
        );
        self.gl.uniform1i(
            Some(&self.projectile_program.material_count),
            self.material_count.max(1),
        );
        self.gl.uniform1i(
            Some(&self.projectile_program.discard_alpha),
            i32::from(transparent),
        );
        self.gl
            .uniform4fv_with_f32_array(Some(&self.projectile_program.sky_color), sky_rgba);
        self.gl
            .uniform1f(Some(&self.projectile_program.world_entity_opacity), 1.0);

        self.gl.active_texture(Gl::TEXTURE6);
        self.gl
            .bind_texture(Gl::TEXTURE_2D, Some(&self.actor_data_texture));
        self.gl
            .uniform1i(Some(&self.projectile_program.actor_data_sampler), 6);

        self.gl.active_texture(Gl::TEXTURE1);
        self.gl.bind_texture(
            Gl::TEXTURE_2D_ARRAY,
            Some(&self.static_map.height_map_texture),
        );
        self.gl
            .uniform1i(Some(&self.projectile_program.height_map_sampler), 1);

        self.gl.active_texture(Gl::TEXTURE2);
        self.gl
            .bind_texture(Gl::TEXTURE_2D_ARRAY, Some(&self.texture_array));
        self.gl
            .uniform1i(Some(&self.projectile_program.texture_sampler), 2);

        self.gl.active_texture(Gl::TEXTURE3);
        self.gl
            .bind_texture(Gl::TEXTURE_2D, Some(&self.material_texture));
        self.gl
            .uniform1i(Some(&self.projectile_program.material_sampler), 3);

        self.gl.active_texture(Gl::TEXTURE4);
        self.gl
            .bind_texture(Gl::TEXTURE_2D_ARRAY, Some(&self.water_texture_array));
        self.gl
            .uniform1i(Some(&self.projectile_program.water_texture_sampler), 4);

        self.gl.active_texture(Gl::TEXTURE5);
        self.gl.bind_texture(
            Gl::TEXTURE_2D_ARRAY,
            Some(&self.static_map.water_mask_texture),
        );
        self.gl
            .uniform1i(Some(&self.projectile_program.water_mask_sampler), 5);

        let ranges = [DrawRange::new(0, index_count, 1)];
        let flags = u32::from(transparent);
        self.last_draw_hash = hash_visible_draw_ranges(
            self.last_draw_hash,
            self.static_map_key,
            flags,
            DYNAMIC_PROJECTILE_BATCH_KIND,
            &ranges,
            None,
            3,
        );

        self.gl
            .bind_vertex_array(Some(&self.dynamic_projectile_batch.vao));
        let stats = submit_draw_ranges(&self.gl, &ranges, index_count, None, None, 3, false);
        self.gl.bind_vertex_array(None);
        self.gl.depth_mask(true);

        self.last_stats.draw_calls += stats.draw_calls;
        self.last_stats.submitted_indices += stats.submitted_indices;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn render_npc_geometry_pass(
        &mut self,
        flat_ranges: &[u32],
        npc_vao: WebGlVertexArrayObject,
        npc_index_count: u32,
        batch_kind: u32,
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
        is_new_texture_anim: bool,
        color_banding: f32,
        npc_data_offset: i32,
        model_y_offset: f32,
        transparent: bool,
        map_pos_override: Option<(f32, f32)>,
        time_loaded_override: Option<f32>,
        cull_back_face_override: Option<bool>,
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
            .static_map
            .state
            .ok_or_else(|| JsValue::from_str("static map state has not been configured"))?;
        let ranges = parse_draw_ranges(flat_ranges).map_err(JsValue::from_str)?;
        validate_draw_ranges(&ranges, npc_index_count as usize)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;

        if transparent {
            self.gl.enable(Gl::BLEND);
            self.gl.blend_func(Gl::SRC_ALPHA, Gl::ONE_MINUS_SRC_ALPHA);
        } else {
            self.gl.disable(Gl::BLEND);
        }
        if let Some(cull_back_face) = cull_back_face_override {
            if cull_back_face {
                self.gl.enable(Gl::CULL_FACE);
                self.gl.cull_face(Gl::BACK);
            } else {
                self.gl.disable(Gl::CULL_FACE);
            }
        }

        self.prepare_viewport();
        self.gl.use_program(Some(&self.npc_program.program));

        self.gl.uniform_matrix4fv_with_f32_array(
            Some(&self.npc_program.view_matrix),
            false,
            view_matrix,
        );
        self.gl.uniform_matrix4fv_with_f32_array(
            Some(&self.npc_program.projection_matrix),
            false,
            projection_matrix,
        );
        self.gl.uniform_matrix4fv_with_f32_array(
            Some(&self.npc_program.world_entity_transform),
            false,
            world_entity_transform,
        );
        self.gl.uniform1f(
            Some(&self.npc_program.world_entity_opacity),
            world_entity_opacity,
        );
        self.gl.uniform4fv_with_f32_array(
            Some(&self.npc_program.scene_hsl_override),
            scene_hsl_override,
        );
        self.gl.uniform2f(
            Some(&self.npc_program.player_pos),
            player_pos[0],
            player_pos[1],
        );
        self.gl.uniform1f(
            Some(&self.npc_program.render_distance),
            render_distance.max(0.0001),
        );
        self.gl
            .uniform1f(Some(&self.npc_program.fog_depth), fog_depth.max(0.0));
        self.gl
            .uniform1f(Some(&self.npc_program.current_time), current_time);
        self.gl
            .uniform1f(Some(&self.npc_program.brightness), brightness.max(0.0001));
        self.gl.uniform1f(
            Some(&self.npc_program.is_new_texture_anim),
            if is_new_texture_anim { 1.0 } else { 0.0 },
        );
        self.gl.uniform1f(
            Some(&self.npc_program.color_banding),
            color_banding.max(1.0),
        );
        self.gl.uniform1i(Some(&self.npc_program.draw_id), 0);
        self.gl
            .uniform1i(Some(&self.npc_program.npc_data_offset), npc_data_offset);
        let (map_x, map_y) = map_pos_override.unwrap_or((state.map_x, state.map_y));
        self.gl
            .uniform2f(Some(&self.npc_program.map_pos), map_x, map_y);
        self.gl.uniform1f(
            Some(&self.npc_program.time_loaded),
            time_loaded_override.unwrap_or(state.time_loaded),
        );
        self.gl
            .uniform1i(Some(&self.npc_program.scene_border_size), state.border_size);
        self.gl
            .uniform1f(Some(&self.npc_program.model_y_offset), model_y_offset);
        self.gl.uniform1i(
            Some(&self.npc_program.texture_layer_count),
            self.texture_layer_count.max(1),
        );
        self.gl.uniform1i(
            Some(&self.npc_program.material_count),
            self.material_count.max(1),
        );
        self.gl.uniform1i(
            Some(&self.npc_program.discard_alpha),
            i32::from(transparent),
        );
        self.gl
            .uniform4fv_with_f32_array(Some(&self.npc_program.sky_color), sky_rgba);

        self.gl.active_texture(Gl::TEXTURE6);
        self.gl
            .bind_texture(Gl::TEXTURE_2D, Some(&self.actor_data_texture));
        self.gl
            .uniform1i(Some(&self.npc_program.actor_data_sampler), 6);

        self.gl.active_texture(Gl::TEXTURE1);
        self.gl.bind_texture(
            Gl::TEXTURE_2D_ARRAY,
            Some(&self.static_map.height_map_texture),
        );
        self.gl
            .uniform1i(Some(&self.npc_program.height_map_sampler), 1);

        self.gl.active_texture(Gl::TEXTURE2);
        self.gl
            .bind_texture(Gl::TEXTURE_2D_ARRAY, Some(&self.texture_array));
        self.gl
            .uniform1i(Some(&self.npc_program.texture_sampler), 2);

        self.gl.active_texture(Gl::TEXTURE3);
        self.gl
            .bind_texture(Gl::TEXTURE_2D, Some(&self.material_texture));
        self.gl
            .uniform1i(Some(&self.npc_program.material_sampler), 3);

        self.gl.active_texture(Gl::TEXTURE4);
        self.gl
            .bind_texture(Gl::TEXTURE_2D_ARRAY, Some(&self.water_texture_array));
        self.gl
            .uniform1i(Some(&self.npc_program.water_texture_sampler), 4);

        self.gl.active_texture(Gl::TEXTURE5);
        self.gl.bind_texture(
            Gl::TEXTURE_2D_ARRAY,
            Some(&self.static_map.water_mask_texture),
        );
        self.gl
            .uniform1i(Some(&self.npc_program.water_mask_sampler), 5);

        let flags = u32::from(transparent);
        self.last_draw_hash = hash_visible_draw_ranges(
            self.last_draw_hash,
            self.static_map_key,
            flags,
            batch_kind,
            &ranges,
            None,
            3,
        );

        self.gl.bind_vertex_array(Some(&npc_vao));
        let stats = submit_draw_ranges(
            &self.gl,
            &ranges,
            npc_index_count,
            Some(&self.npc_program.draw_id),
            None,
            3,
            false,
        );
        self.gl.bind_vertex_array(None);
        self.last_stats.draw_calls += stats.draw_calls;
        self.last_stats.submitted_indices += stats.submitted_indices;
        Ok(())
    }

    /// Renders the currently uploaded finalized player geometry.
    ///
    /// Single-player draws resolve the actor slot directly. Remote batches use
    /// the same instanced integer slot attribute contract as PicoGL so draw
    /// structure and actor-data addressing remain parity-testable.
    #[allow(clippy::too_many_arguments)]
    pub fn render_active_player_pass(
        &mut self,
        view_matrix: &[f32],
        projection_matrix: &[f32],
        world_entity_transform: &[f32],
        sky_rgba: &[f32],
        scene_hsl_override: &[f32],
        player_pos: &[f32],
        render_distance: f32,
        fog_depth: f32,
        current_time: f32,
        brightness: f32,
        is_new_texture_anim: bool,
        color_banding: f32,
        player_data_offset: i32,
        player_slots: &[i32],
        model_y_offset: f32,
        transparent: bool,
        cull_back_face: bool,
        restore_cull_back_face: bool,
    ) -> Result<(), JsValue> {
        require_matrix(view_matrix, "view_matrix")?;
        require_matrix(projection_matrix, "projection_matrix")?;
        require_matrix(world_entity_transform, "world_entity_transform")?;
        require_vec4(sky_rgba, "sky_rgba")?;
        require_vec4(scene_hsl_override, "scene_hsl_override")?;
        if player_pos.len() != 2 {
            return Err(JsValue::from_str("player_pos must contain two f32 values"));
        }
        if player_data_offset < 0 {
            return Err(JsValue::from_str("player_data_offset must be non-negative"));
        }
        if player_slots.iter().any(|slot| *slot < 0) {
            return Err(JsValue::from_str("player slots must be non-negative"));
        }

        let state = self
            .static_map
            .state
            .ok_or_else(|| JsValue::from_str("static map state has not been configured"))?;
        let index_count = self.dynamic_player_batch.index_count;
        if index_count == 0 {
            return Ok(());
        }

        if transparent {
            self.gl.enable(Gl::BLEND);
            self.gl.blend_func(Gl::SRC_ALPHA, Gl::ONE_MINUS_SRC_ALPHA);
        } else {
            self.gl.disable(Gl::BLEND);
        }
        if cull_back_face {
            self.gl.enable(Gl::CULL_FACE);
            self.gl.cull_face(Gl::BACK);
        } else {
            self.gl.disable(Gl::CULL_FACE);
        }

        self.prepare_viewport();
        self.gl.use_program(Some(&self.player_program.program));

        self.gl.uniform_matrix4fv_with_f32_array(
            Some(&self.player_program.view_matrix),
            false,
            view_matrix,
        );
        self.gl.uniform_matrix4fv_with_f32_array(
            Some(&self.player_program.projection_matrix),
            false,
            projection_matrix,
        );
        self.gl.uniform_matrix4fv_with_f32_array(
            Some(&self.player_program.world_entity_transform),
            false,
            world_entity_transform,
        );
        self.gl.uniform4fv_with_f32_array(
            Some(&self.player_program.scene_hsl_override),
            scene_hsl_override,
        );
        self.gl.uniform2f(
            Some(&self.player_program.player_pos),
            player_pos[0],
            player_pos[1],
        );
        self.gl.uniform1f(
            Some(&self.player_program.render_distance),
            render_distance.max(0.0001),
        );
        self.gl
            .uniform1f(Some(&self.player_program.fog_depth), fog_depth.max(0.0));
        self.gl
            .uniform1f(Some(&self.player_program.current_time), current_time);
        self.gl.uniform1f(
            Some(&self.player_program.brightness),
            brightness.max(0.0001),
        );
        self.gl.uniform1f(
            Some(&self.player_program.is_new_texture_anim),
            if is_new_texture_anim { 1.0 } else { 0.0 },
        );
        self.gl.uniform1f(
            Some(&self.player_program.color_banding),
            color_banding.max(1.0),
        );
        let (resolved_player_data_offset, use_player_slot_attribute, instance_count) =
            match player_slots {
                [] => (player_data_offset, false, 1),
                [slot] => (
                    player_data_offset
                        .checked_add(*slot)
                        .ok_or_else(|| JsValue::from_str("player actor-data offset overflow"))?,
                    false,
                    1,
                ),
                slots => {
                    let slot_data = js_sys::Int32Array::from(slots);
                    self.gl
                        .bind_buffer(Gl::ARRAY_BUFFER, Some(&self.player_slot_buffer));
                    self.gl.buffer_data_with_opt_array_buffer(
                        Gl::ARRAY_BUFFER,
                        Some(&slot_data.buffer()),
                        Gl::DYNAMIC_DRAW,
                    );
                    (
                        player_data_offset,
                        true,
                        u32::try_from(slots.len())
                            .map_err(|_| JsValue::from_str("too many player slots"))?,
                    )
                }
            };
        self.gl.uniform1i(
            Some(&self.player_program.player_data_offset),
            resolved_player_data_offset,
        );
        self.gl.uniform1i(
            Some(&self.player_program.use_player_slot_attribute),
            i32::from(use_player_slot_attribute),
        );
        self.gl
            .uniform2f(Some(&self.player_program.map_pos), state.map_x, state.map_y);
        self.gl
            .uniform1f(Some(&self.player_program.time_loaded), state.time_loaded);
        self.gl.uniform1i(
            Some(&self.player_program.scene_border_size),
            state.border_size,
        );
        self.gl
            .uniform1f(Some(&self.player_program.model_y_offset), model_y_offset);
        self.gl.uniform1i(
            Some(&self.player_program.material_count),
            self.material_count.max(1),
        );
        self.gl.uniform1i(
            Some(&self.player_program.discard_alpha),
            i32::from(transparent),
        );
        self.gl
            .uniform4fv_with_f32_array(Some(&self.player_program.sky_color), sky_rgba);

        self.gl.active_texture(Gl::TEXTURE6);
        self.gl
            .bind_texture(Gl::TEXTURE_2D, Some(&self.actor_data_texture));
        self.gl
            .uniform1i(Some(&self.player_program.actor_data_sampler), 6);

        self.gl.active_texture(Gl::TEXTURE1);
        self.gl.bind_texture(
            Gl::TEXTURE_2D_ARRAY,
            Some(&self.static_map.height_map_texture),
        );
        self.gl
            .uniform1i(Some(&self.player_program.height_map_sampler), 1);

        self.gl.active_texture(Gl::TEXTURE2);
        self.gl
            .bind_texture(Gl::TEXTURE_2D_ARRAY, Some(&self.texture_array));
        self.gl
            .uniform1i(Some(&self.player_program.texture_sampler), 2);

        self.gl.active_texture(Gl::TEXTURE3);
        self.gl
            .bind_texture(Gl::TEXTURE_2D, Some(&self.material_texture));
        self.gl
            .uniform1i(Some(&self.player_program.material_sampler), 3);

        let range = [DrawRange::new(0, index_count, instance_count)];
        let flags = u32::from(transparent);
        self.last_draw_hash = hash_visible_draw_ranges(
            self.last_draw_hash,
            self.static_map_key,
            flags,
            DYNAMIC_PLAYER_BATCH_KIND,
            &range,
            None,
            3,
        );

        self.gl
            .bind_vertex_array(Some(&self.dynamic_player_batch.vao));
        let stats = submit_draw_ranges(&self.gl, &range, index_count, None, None, 3, false);
        self.gl.bind_vertex_array(None);
        if restore_cull_back_face {
            self.gl.enable(Gl::CULL_FACE);
            self.gl.cull_face(Gl::BACK);
        } else {
            self.gl.disable(Gl::CULL_FACE);
        }

        self.last_stats.draw_calls += stats.draw_calls;
        self.last_stats.submitted_indices += stats.submitted_indices;
        Ok(())
    }

    /// Enables the Stage-3 offscreen scene target. When enabled, subsequent
    /// frames render into a Rust-owned RGBA8 + depth framebuffer until
    /// `present_frame` resolves that image to the detached canvas.
    pub fn set_presentation_enabled(&mut self, enabled: bool) -> Result<(), JsValue> {
        self.presentation_enabled = enabled;
        if enabled {
            self.ensure_presentation_target()?;
        } else {
            self.gl.bind_framebuffer(Gl::FRAMEBUFFER, None);
        }
        Ok(())
    }

    pub fn presentation_enabled(&self) -> bool {
        self.presentation_enabled
    }

    pub fn set_presentation_msaa_enabled(&mut self, enabled: bool) -> Result<(), JsValue> {
        if self.presentation_msaa_enabled == enabled {
            return Ok(());
        }
        self.presentation_msaa_enabled = enabled;
        self.presentation_width = 0;
        self.presentation_height = 0;
        if self.presentation_enabled {
            self.ensure_presentation_target()?;
        }
        Ok(())
    }

    pub fn presentation_msaa_enabled(&self) -> bool {
        self.presentation_msaa_enabled
    }

    pub fn presentation_msaa_samples(&self) -> i32 {
        self.presentation_msaa_samples
    }

    pub fn set_presentation_fxaa_enabled(&mut self, enabled: bool) {
        self.presentation_fxaa_enabled = enabled;
    }

    pub fn presentation_fxaa_enabled(&self) -> bool {
        self.presentation_fxaa_enabled
    }

    /// Presents the Rust-owned scene target to the canvas.
    ///
    /// MSAA, when enabled, resolves into the single-sample presentation
    /// texture first. FXAA then samples that exact resolved texture into the
    /// default framebuffer; otherwise the resolved texture is blitted raw.
    pub fn present_frame(&mut self) -> Result<(), JsValue> {
        if !self.presentation_enabled {
            return Ok(());
        }

        self.ensure_presentation_target()?;
        let width = self.presentation_width.max(1);
        let height = self.presentation_height.max(1);

        if self.presentation_msaa_enabled {
            self.gl.bind_framebuffer(
                Gl::READ_FRAMEBUFFER,
                Some(&self.presentation_msaa_framebuffer),
            );
            self.gl
                .bind_framebuffer(Gl::DRAW_FRAMEBUFFER, Some(&self.presentation_framebuffer));
            self.gl.blit_framebuffer(
                0,
                0,
                width,
                height,
                0,
                0,
                width,
                height,
                Gl::COLOR_BUFFER_BIT,
                Gl::NEAREST,
            );
        }

        if self.presentation_fxaa_enabled {
            self.gl.bind_framebuffer(Gl::FRAMEBUFFER, None);
            self.prepare_viewport();

            self.gl.disable(Gl::DEPTH_TEST);
            self.gl.disable(Gl::BLEND);

            self.gl.use_program(Some(&self.present_program.program));
            self.gl.uniform2f(
                Some(&self.present_program.resolution),
                width as f32,
                height as f32,
            );
            self.gl.active_texture(Gl::TEXTURE0);
            self.gl
                .bind_texture(Gl::TEXTURE_2D, Some(&self.presentation_color_texture));
            self.gl
                .uniform1i(Some(&self.present_program.frame_sampler), 0);

            self.gl.bind_vertex_array(Some(&self.present_vao));
            self.gl.draw_arrays(Gl::TRIANGLES, 0, 3);
            self.gl.bind_vertex_array(None);
            self.gl.bind_texture(Gl::TEXTURE_2D, None);
            self.gl.use_program(None);

            // Scene rendering expects depth testing on at the beginning of the
            // next frame. Blending intentionally remains disabled so the next
            // opaque pass starts from the same state as the PicoGL renderer.
            self.gl.enable(Gl::DEPTH_TEST);
        } else {
            self.gl
                .bind_framebuffer(Gl::READ_FRAMEBUFFER, Some(&self.presentation_framebuffer));
            self.gl.bind_framebuffer(Gl::DRAW_FRAMEBUFFER, None);
            self.gl.blit_framebuffer(
                0,
                0,
                width,
                height,
                0,
                0,
                width,
                height,
                Gl::COLOR_BUFFER_BIT,
                Gl::NEAREST,
            );
        }

        self.gl.bind_framebuffer(Gl::READ_FRAMEBUFFER, None);
        self.gl.bind_framebuffer(Gl::DRAW_FRAMEBUFFER, None);
        self.gl.bind_framebuffer(Gl::FRAMEBUFFER, None);
        self.prepare_viewport();
        Ok(())
    }

    /// Draws one depth-aware world-space overlay primitive after the scene
    /// geometry/actors and before presentation. Overlay draws intentionally do
    /// not participate in the static/dynamic structural draw fingerprint.
    pub fn render_scene_overlay(
        &mut self,
        vertices: &[f32],
        color: &[f32],
        view_matrix: &[f32],
        projection_matrix: &[f32],
        filled: bool,
    ) -> Result<(), JsValue> {
        require_matrix(view_matrix, "view_matrix")?;
        require_matrix(projection_matrix, "projection_matrix")?;
        require_vec4(color, "color")?;
        if vertices.len() % 3 != 0 {
            return Err(JsValue::from_str(
                "scene overlay vertices must contain xyz triples",
            ));
        }
        let vertex_count = vertices.len() / 3;
        let minimum = if filled { 3 } else { 2 };
        if vertex_count < minimum {
            return Ok(());
        }
        let vertex_count = i32::try_from(vertex_count)
            .map_err(|_| JsValue::from_str("scene overlay vertex count overflow"))?;

        let cull_was_enabled = self.gl.is_enabled(Gl::CULL_FACE);
        let blend_was_enabled = self.gl.is_enabled(Gl::BLEND);

        self.prepare_viewport();
        self.gl.enable(Gl::DEPTH_TEST);
        self.gl.depth_mask(false);
        self.gl.disable(Gl::CULL_FACE);
        if filled {
            self.gl.enable(Gl::BLEND);
            self.gl.blend_func(Gl::SRC_ALPHA, Gl::ONE_MINUS_SRC_ALPHA);
        } else {
            self.gl.disable(Gl::BLEND);
        }

        self.gl
            .use_program(Some(&self.scene_overlay_program.program));
        self.gl.uniform_matrix4fv_with_f32_array(
            Some(&self.scene_overlay_program.view_matrix),
            false,
            view_matrix,
        );
        self.gl.uniform_matrix4fv_with_f32_array(
            Some(&self.scene_overlay_program.projection_matrix),
            false,
            projection_matrix,
        );
        self.gl
            .uniform4fv_with_f32_array(Some(&self.scene_overlay_program.color), color);

        let vertex_data = js_sys::Float32Array::from(vertices);
        self.gl
            .bind_buffer(Gl::ARRAY_BUFFER, Some(&self.scene_overlay_vertex_buffer));
        self.gl.buffer_data_with_opt_array_buffer(
            Gl::ARRAY_BUFFER,
            Some(&vertex_data.buffer()),
            Gl::DYNAMIC_DRAW,
        );
        self.gl.bind_vertex_array(Some(&self.scene_overlay_vao));
        self.gl.draw_arrays(
            if filled {
                Gl::TRIANGLE_FAN
            } else {
                Gl::LINE_STRIP
            },
            0,
            vertex_count,
        );
        self.gl.bind_vertex_array(None);
        self.gl.use_program(None);

        self.gl.depth_mask(true);
        if cull_was_enabled {
            self.gl.enable(Gl::CULL_FACE);
        } else {
            self.gl.disable(Gl::CULL_FACE);
        }
        if blend_was_enabled {
            self.gl.enable(Gl::BLEND);
        } else {
            self.gl.disable(Gl::BLEND);
        }
        Ok(())
    }

    pub fn last_draw_calls(&self) -> u32 {
        self.last_stats.draw_calls
    }

    pub fn last_submitted_indices(&self) -> f64 {
        self.last_stats.submitted_indices as f64
    }

    pub fn last_draw_hash(&self) -> u32 {
        self.last_draw_hash
    }

    pub fn dispose(&mut self) {
        self.static_map.delete(&self.gl);
        for (_, mut map) in self.parked_static_maps.drain() {
            map.delete(&self.gl);
        }
        self.gl.delete_texture(Some(&self.texture_array));
        self.gl.delete_texture(Some(&self.material_texture));
        self.gl.delete_texture(Some(&self.water_texture_array));
        self.gl.delete_texture(Some(&self.actor_data_texture));
        self.gl
            .delete_framebuffer(Some(&self.presentation_framebuffer));
        self.gl
            .delete_texture(Some(&self.presentation_color_texture));
        self.gl
            .delete_renderbuffer(Some(&self.presentation_depth_renderbuffer));
        self.gl
            .delete_framebuffer(Some(&self.presentation_msaa_framebuffer));
        self.gl
            .delete_renderbuffer(Some(&self.presentation_msaa_color_renderbuffer));
        self.gl
            .delete_renderbuffer(Some(&self.presentation_msaa_depth_renderbuffer));
        self.dynamic_npc_batch.delete(&self.gl);
        self.dynamic_gfx_batch.delete(&self.gl);
        self.dynamic_projectile_batch.delete(&self.gl);
        self.dynamic_player_batch.delete(&self.gl);
        self.gl.delete_buffer(Some(&self.player_slot_buffer));
        self.gl.delete_program(Some(&self.reference_program));
        self.gl.delete_program(Some(&self.static_program.program));
        self.gl.delete_program(Some(&self.npc_program.program));
        self.gl
            .delete_program(Some(&self.projectile_program.program));
        self.gl.delete_program(Some(&self.player_program.program));
        self.gl.delete_program(Some(&self.present_program.program));
        self.gl.delete_vertex_array(Some(&self.present_vao));
        self.gl
            .delete_program(Some(&self.scene_overlay_program.program));
        self.gl
            .delete_buffer(Some(&self.scene_overlay_vertex_buffer));
        self.gl.delete_vertex_array(Some(&self.scene_overlay_vao));
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
                .static_map
                .loc_batch
                .take()
                .ok_or_else(|| JsValue::from_str("loc geometry has not been uploaded"))?,
            AUX_BATCH_DOOR => self
                .static_map
                .door_batch
                .take()
                .ok_or_else(|| JsValue::from_str("door geometry has not been uploaded"))?,
            AUX_BATCH_GROUND => self
                .static_map
                .ground_batch
                .take()
                .ok_or_else(|| JsValue::from_str("ground geometry has not been uploaded"))?,
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
            AUX_BATCH_LOC => self.static_map.loc_batch = Some(batch),
            AUX_BATCH_DOOR => self.static_map.door_batch = Some(batch),
            AUX_BATCH_GROUND => self.static_map.ground_batch = Some(batch),
            _ => unreachable!(),
        }
        result
    }

    fn ensure_presentation_target(&mut self) -> Result<(), JsValue> {
        let width = (self.canvas.width() as i32).max(1);
        let height = (self.canvas.height() as i32).max(1);
        if self.presentation_width == width && self.presentation_height == height {
            return Ok(());
        }

        self.gl
            .bind_texture(Gl::TEXTURE_2D, Some(&self.presentation_color_texture));
        self.gl
            .tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_array_buffer_view(
                Gl::TEXTURE_2D,
                0,
                Gl::RGBA8 as i32,
                width,
                height,
                0,
                Gl::RGBA,
                Gl::UNSIGNED_BYTE,
                None,
            )?;

        self.gl.bind_renderbuffer(
            Gl::RENDERBUFFER,
            Some(&self.presentation_depth_renderbuffer),
        );
        self.gl
            .renderbuffer_storage(Gl::RENDERBUFFER, Gl::DEPTH_COMPONENT24, width, height);

        self.gl
            .bind_framebuffer(Gl::FRAMEBUFFER, Some(&self.presentation_framebuffer));
        self.gl.framebuffer_texture_2d(
            Gl::FRAMEBUFFER,
            Gl::COLOR_ATTACHMENT0,
            Gl::TEXTURE_2D,
            Some(&self.presentation_color_texture),
            0,
        );
        self.gl.framebuffer_renderbuffer(
            Gl::FRAMEBUFFER,
            Gl::DEPTH_ATTACHMENT,
            Gl::RENDERBUFFER,
            Some(&self.presentation_depth_renderbuffer),
        );

        let status = self.gl.check_framebuffer_status(Gl::FRAMEBUFFER);
        if status != Gl::FRAMEBUFFER_COMPLETE {
            self.gl.bind_framebuffer(Gl::FRAMEBUFFER, None);
            self.gl.bind_renderbuffer(Gl::RENDERBUFFER, None);
            self.gl.bind_texture(Gl::TEXTURE_2D, None);
            return Err(JsValue::from_str(&format!(
                "presentation framebuffer incomplete: 0x{status:04x}",
            )));
        }

        self.presentation_msaa_samples = 0;
        if self.presentation_msaa_enabled {
            let max_samples = self
                .gl
                .get_parameter(Gl::MAX_SAMPLES)?
                .as_f64()
                .unwrap_or(1.0)
                .floor() as i32;
            let samples = max_samples.max(1);

            self.gl.bind_renderbuffer(
                Gl::RENDERBUFFER,
                Some(&self.presentation_msaa_color_renderbuffer),
            );
            self.gl.renderbuffer_storage_multisample(
                Gl::RENDERBUFFER,
                samples,
                Gl::RGBA8,
                width,
                height,
            );
            self.gl.bind_renderbuffer(
                Gl::RENDERBUFFER,
                Some(&self.presentation_msaa_depth_renderbuffer),
            );
            self.gl.renderbuffer_storage_multisample(
                Gl::RENDERBUFFER,
                samples,
                Gl::DEPTH_COMPONENT24,
                width,
                height,
            );

            self.gl
                .bind_framebuffer(Gl::FRAMEBUFFER, Some(&self.presentation_msaa_framebuffer));
            self.gl.framebuffer_renderbuffer(
                Gl::FRAMEBUFFER,
                Gl::COLOR_ATTACHMENT0,
                Gl::RENDERBUFFER,
                Some(&self.presentation_msaa_color_renderbuffer),
            );
            self.gl.framebuffer_renderbuffer(
                Gl::FRAMEBUFFER,
                Gl::DEPTH_ATTACHMENT,
                Gl::RENDERBUFFER,
                Some(&self.presentation_msaa_depth_renderbuffer),
            );

            let msaa_status = self.gl.check_framebuffer_status(Gl::FRAMEBUFFER);
            if msaa_status != Gl::FRAMEBUFFER_COMPLETE {
                self.gl.bind_framebuffer(Gl::FRAMEBUFFER, None);
                self.gl.bind_renderbuffer(Gl::RENDERBUFFER, None);
                self.gl.bind_texture(Gl::TEXTURE_2D, None);
                return Err(JsValue::from_str(&format!(
                    "presentation MSAA framebuffer incomplete: 0x{msaa_status:04x}",
                )));
            }
            self.presentation_msaa_samples = samples;
        }

        self.gl.bind_framebuffer(Gl::FRAMEBUFFER, None);
        self.gl.bind_renderbuffer(Gl::RENDERBUFFER, None);
        self.gl.bind_texture(Gl::TEXTURE_2D, None);

        self.presentation_width = width;
        self.presentation_height = height;
        Ok(())
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

fn create_linear_texture(gl: &Gl, target: u32) -> Result<WebGlTexture, JsValue> {
    let texture = gl
        .create_texture()
        .ok_or_else(|| JsValue::from_str("failed to create WebGL texture"))?;
    gl.bind_texture(target, Some(&texture));
    gl.tex_parameteri(target, Gl::TEXTURE_MIN_FILTER, Gl::LINEAR as i32);
    gl.tex_parameteri(target, Gl::TEXTURE_MAG_FILTER, Gl::LINEAR as i32);
    gl.tex_parameteri(target, Gl::TEXTURE_WRAP_S, Gl::CLAMP_TO_EDGE as i32);
    gl.tex_parameteri(target, Gl::TEXTURE_WRAP_T, Gl::CLAMP_TO_EDGE as i32);
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
