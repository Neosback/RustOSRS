use crate::draw::{DrawRange, DrawStats, parse_draw_ranges};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{
    HtmlCanvasElement, WebGl2RenderingContext as Gl, WebGlBuffer, WebGlProgram, WebGlShader,
    WebGlUniformLocation, WebGlVertexArrayObject,
};

const VERTEX_SHADER: &str = include_str!("shaders/reference.vert.glsl");
const FRAGMENT_SHADER: &str = include_str!("shaders/reference.frag.glsl");

/// First executable Rust/WASM renderer stage.
///
/// This owns WebGL2 state, shader lifecycle, VAO/VBO/IBO, geometry uploads,
/// draw ranges and submission. It lives beside the production PicoGL renderer
/// until static-scene parity is proven.
#[wasm_bindgen]
pub struct RustWebGlRenderer {
    canvas: HtmlCanvasElement,
    gl: Gl,
    program: WebGlProgram,
    vertex_buffer: WebGlBuffer,
    index_buffer: WebGlBuffer,
    vao: WebGlVertexArrayObject,
    view_proj_location: WebGlUniformLocation,
    brightness_location: WebGlUniformLocation,
    draw_ranges: Vec<DrawRange>,
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

        let vertex = compile_shader(&gl, Gl::VERTEX_SHADER, VERTEX_SHADER)?;
        let fragment = compile_shader(&gl, Gl::FRAGMENT_SHADER, FRAGMENT_SHADER)?;
        let program = link_program(&gl, &vertex, &fragment)?;

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

        let view_proj_location = gl
            .get_uniform_location(&program, "u_viewProj")
            .ok_or_else(|| JsValue::from_str("u_viewProj uniform was optimized out"))?;
        let brightness_location = gl
            .get_uniform_location(&program, "u_brightness")
            .ok_or_else(|| JsValue::from_str("u_brightness uniform was optimized out"))?;

        gl.enable(Gl::DEPTH_TEST);
        gl.depth_func(Gl::LEQUAL);
        gl.enable(Gl::CULL_FACE);
        gl.cull_face(Gl::BACK);

        Ok(Self {
            canvas,
            gl,
            program,
            vertex_buffer,
            index_buffer,
            vao,
            view_proj_location,
            brightness_location,
            draw_ranges: Vec::new(),
            index_count: 0,
            last_stats: DrawStats::default(),
        })
    }

    pub fn abi_version(&self) -> u32 {
        crate::RENDERER_ABI_VERSION
    }

    /// Uploads the current TypeScript packed-vertex packet without repacking.
    ///
    /// packed_vertices is [v0,v1,v2, v0,v1,v2, ...].
    pub fn upload_geometry(
        &mut self,
        packed_vertices: &[u32],
        indices: &[u32],
    ) -> Result<(), JsValue> {
        if packed_vertices.len() % 3 != 0 {
            return Err(JsValue::from_str(
                "packed vertex packet must contain groups of three u32 values",
            ));
        }

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
        Ok(())
    }

    /// Existing TypeScript wire layout:
    /// [offsetBytes,elements,instances, ...].
    pub fn set_draw_ranges(&mut self, flat_ranges: &[u32]) -> Result<(), JsValue> {
        self.draw_ranges = parse_draw_ranges(flat_ranges).map_err(JsValue::from_str)?;
        Ok(())
    }

    pub fn clear_draw_ranges(&mut self) {
        self.draw_ranges.clear();
    }

    /// Renders the Stage 0 geometry/HSL reference pass.
    pub fn render(
        &mut self,
        view_projection: &[f32],
        clear_rgba: &[f32],
        brightness: f32,
    ) -> Result<(), JsValue> {
        if view_projection.len() != 16 {
            return Err(JsValue::from_str(
                "view_projection must contain 16 f32 values",
            ));
        }
        if clear_rgba.len() != 4 {
            return Err(JsValue::from_str("clear_rgba must contain 4 f32 values"));
        }

        let width = self.canvas.width() as i32;
        let height = self.canvas.height() as i32;
        self.gl.viewport(0, 0, width.max(1), height.max(1));
        self.gl
            .clear_color(clear_rgba[0], clear_rgba[1], clear_rgba[2], clear_rgba[3]);
        self.gl.clear(Gl::COLOR_BUFFER_BIT | Gl::DEPTH_BUFFER_BIT);

        self.gl.use_program(Some(&self.program));
        self.gl.uniform_matrix4fv_with_f32_array(
            Some(&self.view_proj_location),
            false,
            view_projection,
        );
        self.gl
            .uniform1f(Some(&self.brightness_location), brightness.max(0.0001));
        self.gl.bind_vertex_array(Some(&self.vao));

        let mut stats = DrawStats::default();
        if self.draw_ranges.is_empty() {
            if self.index_count > 0 {
                self.gl.draw_elements_with_i32(
                    Gl::TRIANGLES,
                    self.index_count as i32,
                    Gl::UNSIGNED_INT,
                    0,
                );
                stats.draw_calls = 1;
                stats.submitted_indices = self.index_count as u64;
            }
        } else {
            for range in self
                .draw_ranges
                .iter()
                .copied()
                .filter(|range| !range.is_empty())
            {
                if range.instances <= 1 {
                    self.gl.draw_elements_with_i32(
                        Gl::TRIANGLES,
                        range.elements as i32,
                        Gl::UNSIGNED_INT,
                        range.offset_bytes as i32,
                    );
                } else {
                    self.gl.draw_elements_instanced_with_i32(
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
        }

        self.gl.bind_vertex_array(None);
        self.last_stats = stats;
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
        self.gl.delete_program(Some(&self.program));
        self.draw_ranges.clear();
        self.index_count = 0;
    }
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
