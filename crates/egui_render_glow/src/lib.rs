use bytemuck::cast_slice;
use egui_backend::{egui, EguiGfxOutput, GfxBackend, WindowBackend};
use intmap::IntMap;
use tracing::{info, warn};

use std::sync::Arc;

use egui_backend::egui::*;
pub use glow;
use glow::{Context as GlowContext, HasContext, *};

#[macro_export]
macro_rules! glow_error {
    ($glow_context: ident) => {
        let error_code = $glow_context.get_error();
        if error_code != glow::NO_ERROR {
            panic!("glow error: {} at line {}", error_code, line!());
        }
    };
}
pub struct GlowBackend {
    pub glow_context: Arc<GlowContext>,
    pub framebuffer_size: [u32; 2],
    pub painter: Painter,
}

impl GlowBackend {
    pub unsafe fn destroy(&mut self) {
        self.painter.destroy(&self.glow_context);
    }
}

impl<
        #[cfg(not(target_arch = "wasm32"))] W: WindowBackend + egui_backend::OpenGLWindowContext,
        #[cfg(target_arch = "wasm32")] W: WindowBackend,
    > GfxBackend<W> for GlowBackend
{
    type Configuration = ();

    fn new(window_backend: &mut W, _settings: Self::Configuration) -> Self {
        let glow_context = Arc::new(unsafe {
            match window_backend.get_settings().gfx_api_type.clone() {
                #[cfg(not(target_arch = "wasm32"))]
                egui_backend::GfxApiType::OpenGL { native_config } => {
                    let gl =
                        glow::Context::from_loader_function(|s| window_backend.get_proc_address(s));

                    let gl_version = gl.version();
                    info!("glow using gl version: {gl_version:?}");
                    assert!(
                        gl_version.major >= 3,
                        "egui glow only supports opengl major version 3 or above {gl_version:?}"
                    );

                    assert_eq!(
                        native_config.double_buffer,
                        Some(true),
                        "egui glow only supports double buffer"
                    );

                    // assert!(native_config.minor.unwrap() >= 0, "egui glow only supports opengl minor version ???");
                    assert_eq!(
                        native_config.samples, None,
                        "egui glow doesn't support multi sampling"
                    );
                    assert_eq!(
                        native_config.srgb,
                        Some(true),
                        "egui glow only supports srgb compatible surface/ framebuffers"
                    );

                    if let Some(debug) = native_config.debug {
                        if debug {
                            gl.enable(glow::DEBUG_OUTPUT);
                            gl.enable(glow::DEBUG_OUTPUT_SYNCHRONOUS);
                            assert!(gl.supports_debug());
                            gl.debug_message_callback(
                                |source, error_type, error_id, severity, error_str| {
                                    match severity {
                                        glow::DEBUG_SEVERITY_NOTIFICATION => tracing::debug!(
                                            source, error_type, error_id, severity, error_str
                                        ),
                                        glow::DEBUG_SEVERITY_LOW => {
                                            tracing::info!(
                                                source, error_type, error_id, severity, error_str
                                            )
                                        }
                                        glow::DEBUG_SEVERITY_MEDIUM => {
                                            tracing::warn!(
                                                source, error_type, error_id, severity, error_str
                                            )
                                        }
                                        glow::DEBUG_SEVERITY_HIGH => tracing::error!(
                                            source, error_type, error_id, severity, error_str
                                        ),
                                        rest => panic!("unknown severity {rest}"),
                                    };
                                },
                            );
                            glow_error!(gl);
                        }
                    }
                    gl
                }
                #[cfg(target_arch = "wasm32")]
                egui_backend::GfxApiType::WebGL2 {
                    canvas_id,
                    webgl_config,
                } => {
                    use wasm_bindgen::JsCast;

                    let handle_id = match window_backend.raw_window_handle() {
                        egui_backend::raw_window_handle::RawWindowHandle::Web(handle_id) => {
                            handle_id.id
                        }
                        _ => {
                            unimplemented!("non web raw window handles are not supported on wasm32")
                        }
                    };
                    let canvas_node: wasm_bindgen::JsValue = web_sys::window()
                        .and_then(|win| win.document())
                        .and_then(|doc: web_sys::Document| {
                            doc.query_selector(&format!("[data-raw-handle=\"{}\"]", handle_id))
                                .ok()
                        })
                        .expect("expected to find single canvas")
                        .into();
                    let canvas_element: web_sys::HtmlCanvasElement = canvas_node.into();
                    let context_options = create_context_options_from_webgl_config(webgl_config);
                    let context = canvas_element
                        .get_context_with_context_options("webgl2", &context_options)
                        .unwrap()
                        .unwrap()
                        .dyn_into()
                        .unwrap();
                    glow::Context::from_webgl2_context(context)
                }
                _ => {
                    unimplemented!("egui glow only supports WebGL2 or OpenGL gfx types ")
                }
            }
        });
        if glow_context.supported_extensions().contains("EXT_sRGB")
            || glow_context.supported_extensions().contains("GL_EXT_sRGB")
            || glow_context
                .supported_extensions()
                .contains("GL_ARB_framebuffer_sRGB")
        {
            warn!("srgb support detected by egui glow");
        } else {
            warn!("no srgb support detected by egui glow");
        }

        let painter = Painter::new(&glow_context);
        Self {
            glow_context,
            painter,
            framebuffer_size: window_backend.get_live_physical_size_framebuffer(),
        }
    }

    fn prepare_frame(&mut self, framebuffer_size_update: bool, window_backend: &mut W) {
        if framebuffer_size_update {
            let fb_size = window_backend.get_live_physical_size_framebuffer();
            unsafe {
                self.glow_context
                    .viewport(0, 0, fb_size[0] as i32, fb_size[1] as i32);
            }
        }
        unsafe {
            self.glow_context.disable(glow::SCISSOR_TEST);
            self.glow_context.clear(glow::COLOR_BUFFER_BIT);
        }
    }

    fn prepare_render(&mut self, egui_gfx_output: EguiGfxOutput) {
        unsafe {
            self.painter
                .prepare_render(&self.glow_context, egui_gfx_output, self.framebuffer_size)
        };
    }

    fn render(&mut self) {
        unsafe {
            self.painter.render(&self.glow_context);
        }
    }

    fn present(&mut self, window_backend: &mut W) {
        // on wasm, there's no swap buffers.. the browser takes care of it automatically.
        #[cfg(not(target_arch = "wasm32"))]
        {
            egui_backend::OpenGLWindowContext::swap_buffers(window_backend);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn create_context_options_from_webgl_config(
    webgl_config: egui_backend::WebGlConfig,
) -> js_sys::Object {
    use wasm_bindgen::JsValue;

    let context_options = js_sys::Object::new();
    if let Some(alpha) = webgl_config.alpha {
        js_sys::Reflect::set(
            &context_options,
            &"alpha".into(),
            &if alpha { JsValue::TRUE } else { JsValue::FALSE },
        )
        .expect("Cannot create context options");
    }
    if let Some(antialias) = webgl_config.antialias {
        js_sys::Reflect::set(
            &context_options,
            &"antialias".into(),
            &if antialias {
                JsValue::TRUE
            } else {
                JsValue::FALSE
            },
        )
        .expect("Cannot create context options");
    }
    if let Some(depth) = webgl_config.depth {
        js_sys::Reflect::set(
            &context_options,
            &"depth".into(),
            &if depth { JsValue::TRUE } else { JsValue::FALSE },
        )
        .expect("Cannot create context options");
    }
    if let Some(value) = webgl_config.desynchronized {
        js_sys::Reflect::set(
            &context_options,
            &"desynchronized".into(),
            &if value { JsValue::TRUE } else { JsValue::FALSE },
        )
        .expect("Cannot create context options");
    }
    if let Some(value) = webgl_config.fail_if_major_performance_caveat {
        js_sys::Reflect::set(
            &context_options,
            &"failIfMajorPerformanceCaveat".into(),
            &if value { JsValue::TRUE } else { JsValue::FALSE },
        )
        .expect("Cannot create context options");
    }
    if let Some(value) = webgl_config.low_power {
        js_sys::Reflect::set(
            &context_options,
            &"powerPreference".into(),
            &if value {
                JsValue::from_str("low-power")
            } else {
                JsValue::from_str("high-performance")
            },
        )
        .expect("Cannot create context options");
    }
    if let Some(value) = webgl_config.premultiplied_alpha {
        js_sys::Reflect::set(
            &context_options,
            &"premultipliedAlpha".into(),
            &if value { JsValue::TRUE } else { JsValue::FALSE },
        )
        .expect("Cannot create context options");
    }
    if let Some(value) = webgl_config.preserve_drawing_buffer {
        js_sys::Reflect::set(
            &context_options,
            &"preserveDrawingBuffer".into(),
            &if value { JsValue::TRUE } else { JsValue::FALSE },
        )
        .expect("Cannot create context options");
    }
    if let Some(value) = webgl_config.stencil {
        js_sys::Reflect::set(
            &context_options,
            &"stencil".into(),
            &if value { JsValue::TRUE } else { JsValue::FALSE },
        )
        .expect("Cannot create context options");
    }
    context_options
}
const EGUI_VS: &str = include_str!("../egui.vs");
#[cfg(not(target_arch = "wasm32"))]
const EGUI_FS: &str = include_str!("../egui.fs");
#[cfg(target_arch = "wasm32")]
const EGUI_FS: &str = include_str!("../egui_webgl.fs");
pub struct GpuTexture {
    handle: glow::Texture,
    width: u32,
    height: u32,
    sampler: Sampler,
}

/// Egui Painter using glow::Context
/// unlike
pub struct Painter {
    pub linear_sampler: Sampler,
    pub nearest_sampler: Sampler,
    pub managed_textures: IntMap<GpuTexture>,
    pub egui_program: Program,
    pub vao: VertexArray,
    pub vbo: Buffer,
    pub ebo: Buffer,
    pub u_screen_size: UniformLocation,
    pub u_sampler: UniformLocation,
    pub clipped_primitives: Vec<ClippedPrimitive>,
    pub textures_to_delete: Vec<TextureId>,
    pub screen_size_logical: [f32; 2],
    pub screen_size_physical: [u32; 2],
}

impl Painter {
    pub fn new(glow_context: &glow::Context) -> Self {
        info!("creating glow egui painter");
        unsafe {
            glow_error!(glow_context);
            // compile shaders
            let egui_program = create_program_from_src(&glow_context, EGUI_VS, EGUI_FS);
            // shader verification
            glow_error!(glow_context);
            let u_screen_size = glow_context
                .get_uniform_location(egui_program, "u_screen_size")
                .expect("failed to find u_screen_size");
            info!("location of uniform u_screen_size is {u_screen_size:?}");
            let u_sampler = glow_context
                .get_uniform_location(egui_program, "u_sampler")
                .expect("failed to find u_sampler");
            info!("location of uniform u_sampler is {u_sampler:?}");
            glow_context.use_program(Some(egui_program));
            let (vao, vbo, ebo) = create_egui_vao_buffers(&glow_context, egui_program);
            info!("created egui vao, vbo, ebo");
            let (linear_sampler, nearest_sampler) = create_samplers(&glow_context);
            info!("created linear and nearest samplers");
            Self {
                managed_textures: Default::default(),
                egui_program,
                vao,
                vbo,
                ebo,
                linear_sampler,
                nearest_sampler,
                u_screen_size,
                u_sampler,
                clipped_primitives: Vec::new(),
                textures_to_delete: Vec::new(),
                screen_size_logical: [0.0; 2],
                screen_size_physical: [0; 2],
            }
        }
    }
    pub unsafe fn prepare_render(
        &mut self,
        glow_context: &glow::Context,
        egui_gfx_output: EguiGfxOutput,
        screen_size_physical: [u32; 2],
    ) {
        let EguiGfxOutput {
            meshes,
            textures_delta,
            screen_size_logical,
        } = egui_gfx_output;
        self.textures_to_delete = textures_delta.free;
        self.clipped_primitives = meshes;
        self.screen_size_logical = screen_size_logical;
        self.screen_size_physical = screen_size_physical;
        glow_error!(glow_context);

        // update textures
        for (texture_id, delta) in textures_delta.set {
            match texture_id {
                TextureId::Managed(managed) => {
                    glow_context.bind_texture(
                        glow::TEXTURE_2D,
                        Some(match self.managed_textures.entry(managed) {
                            intmap::Entry::Occupied(o) => o.get().handle,
                            intmap::Entry::Vacant(v) => {
                                let handle = glow_context
                                    .create_texture()
                                    .expect("failed to create texture");
                                v.insert(GpuTexture {
                                    handle,
                                    width: 0,
                                    height: 0,
                                    sampler: if managed == 0 {
                                        self.nearest_sampler
                                    } else {
                                        self.linear_sampler
                                    },
                                })
                                .handle
                            }
                        }),
                    );
                }
                TextureId::User(_) => todo!(),
            }
            glow_error!(glow_context);

            let (pixels, size): (Vec<u8>, [usize; 2]) = match delta.image {
                egui::ImageData::Color(_) => todo!(),
                egui::ImageData::Font(font_image) => (
                    font_image
                        .srgba_pixels(1.0)
                        .flat_map(|c| c.to_array())
                        .collect(),
                    font_image.size,
                ),
            };
            if let Some(pos) = delta.pos {
                glow_context.tex_sub_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    pos[0] as i32,
                    pos[1] as i32,
                    size[0] as i32,
                    size[1] as i32,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(&pixels),
                )
            } else {
                match texture_id {
                    TextureId::Managed(key) => {
                        let gpu_tex = self
                            .managed_textures
                            .get_mut(key)
                            .expect("failed to find texture with key");
                        gpu_tex.width = size[0] as u32;
                        gpu_tex.height = size[1] as u32;
                    }
                    TextureId::User(_) => todo!(),
                }
                glow_context.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::SRGB8_ALPHA8 as i32,
                    size[0] as i32,
                    size[1] as i32,
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    Some(&pixels),
                );
            }
            glow_error!(glow_context);
        }
    }
    pub unsafe fn render(&mut self, glow_context: &glow::Context) {
        let screen_size_physical = self.screen_size_physical;
        let screen_size_logical = self.screen_size_logical;
        let scale = screen_size_physical[0] as f32 / screen_size_logical[0];

        // setup egui configuration
        glow_context.enable(glow::SCISSOR_TEST);
        glow_context.disable(glow::DEPTH_TEST);
        glow_error!(glow_context);
        #[cfg(not(target_arch = "wasm32"))]
        glow_context.enable(glow::FRAMEBUFFER_SRGB);

        glow_error!(glow_context);
        glow_context.active_texture(glow::TEXTURE0);
        glow_error!(glow_context);

        glow_context.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
        glow_context.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ebo));
        glow_context.bind_vertex_array(Some(self.vao));
        glow_context.enable(glow::BLEND);
        glow_context.blend_equation_separate(glow::FUNC_ADD, glow::FUNC_ADD);
        glow_context.blend_func_separate(
            // egui outputs colors with premultiplied alpha:
            glow::ONE,
            glow::ONE_MINUS_SRC_ALPHA,
            // Less important, but this is technically the correct alpha blend function
            // when you want to make use of the framebuffer alpha (for screenshots, compositing, etc).
            glow::ONE_MINUS_DST_ALPHA,
            glow::ONE,
        );
        glow_context.use_program(Some(self.egui_program));
        glow_context.active_texture(glow::TEXTURE0);
        glow_context.uniform_1_i32(Some(&self.u_sampler), 0);
        glow_context.uniform_2_f32_slice(Some(&self.u_screen_size), &screen_size_logical);
        for clipped_primitive in &self.clipped_primitives {
            let clip_rect = clipped_primitive.clip_rect;
            let clip_min_x = scale * clip_rect.min.x;
            let clip_min_y = scale * clip_rect.min.y;
            let clip_max_x = scale * clip_rect.max.x;
            let clip_max_y = scale * clip_rect.max.y;

            // Round to integer:
            let clip_min_x = clip_min_x.round() as i32;
            let clip_min_y = clip_min_y.round() as i32;
            let clip_max_x = clip_max_x.round() as i32;
            let clip_max_y = clip_max_y.round() as i32;

            // Clamp:
            let clip_min_x = clip_min_x.clamp(0, screen_size_physical[0] as i32);
            let clip_min_y = clip_min_y.clamp(0, screen_size_physical[1] as i32);
            let clip_max_x = clip_max_x.clamp(clip_min_x, screen_size_physical[0] as i32);
            let clip_max_y = clip_max_y.clamp(clip_min_y, screen_size_physical[1] as i32);
            let clip_x = clip_min_x;
            let clip_y = screen_size_physical[1] as i32 - clip_max_y; // NOTE: Y coordinate must be flipped inside the cliprect relative to screen height
            let width = clip_max_x - clip_min_x;
            let height = clip_max_y - clip_min_y;
            glow_context.scissor(clip_x, clip_y, width, height);
            match clipped_primitive.primitive {
                egui::epaint::Primitive::Mesh(ref mesh) => {
                    glow_context.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
                    glow_context.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ebo));
                    glow_context.buffer_data_u8_slice(
                        glow::ARRAY_BUFFER,
                        cast_slice(&mesh.vertices),
                        glow::STATIC_DRAW,
                    );
                    glow_context.buffer_data_u8_slice(
                        glow::ELEMENT_ARRAY_BUFFER,
                        cast_slice(&mesh.indices),
                        glow::STATIC_DRAW,
                    );
                    glow_error!(glow_context);
                    match mesh.texture_id {
                        TextureId::Managed(managed) => {
                            let managed_tex = self
                                .managed_textures
                                .get(managed)
                                .expect("managed texture cannot be found");
                            glow_context.bind_texture(glow::TEXTURE_2D, Some(managed_tex.handle));

                            glow_context.bind_sampler(0, Some(managed_tex.sampler));
                        }
                        TextureId::User(_) => todo!(),
                    }
                    glow_error!(glow_context);

                    let indices_len: i32 = mesh
                        .indices
                        .len()
                        .try_into()
                        .expect("failed to fit indices length into i32");

                    glow_error!(glow_context);
                    glow_context.draw_elements(glow::TRIANGLES, indices_len, glow::UNSIGNED_INT, 0);

                    glow_error!(glow_context);
                }

                egui::epaint::Primitive::Callback(_) => todo!(),
            }
        }
        glow_error!(glow_context);
        let textures_to_delete = std::mem::take(&mut self.textures_to_delete);
        for tid in textures_to_delete {
            match tid {
                TextureId::Managed(managed) => {
                    glow_context.delete_texture(
                        self.managed_textures
                            .remove(managed)
                            .expect("can't find texture to delete")
                            .handle,
                    );
                }
                TextureId::User(_) => todo!(),
            }
        }
        glow_error!(glow_context);
    }
    pub unsafe fn destroy(&mut self, glow_context: &glow::Context) {
        glow_context.delete_sampler(self.linear_sampler);
        glow_context.delete_sampler(self.nearest_sampler);
        for (_, texture) in std::mem::take(&mut self.managed_textures) {
            glow_context.delete_texture(texture.handle);
        }
        glow_context.delete_program(self.egui_program);
        glow_context.delete_vertex_array(self.vao);
        glow_context.delete_buffer(self.vbo);
        glow_context.delete_buffer(self.ebo);
    }
}
unsafe fn create_program_from_src(
    glow_context: &glow::Context,
    vertex_src: &str,
    frag_src: &str,
) -> Program {
    let vs = glow_context
        .create_shader(glow::VERTEX_SHADER)
        .expect("shader creation failed");
    let fs = glow_context
        .create_shader(glow::FRAGMENT_SHADER)
        .expect("failed to create frag shader");
    glow_context.shader_source(vs, vertex_src);
    glow_context.shader_source(fs, frag_src);
    glow_context.compile_shader(vs);
    let info_log = glow_context.get_shader_info_log(vs);
    if !info_log.is_empty() {
        warn!("vertex shader info log: {info_log}")
    }
    if !glow_context.get_shader_compile_status(vs) {
        panic!("failed to compile vertex shader. info_log: {}", info_log);
    }
    glow_error!(glow_context);
    glow_context.compile_shader(fs);
    let info_log = glow_context.get_shader_info_log(fs);
    if !info_log.is_empty() {
        warn!("fragment shader info log: {info_log}")
    }
    if !glow_context.get_shader_compile_status(fs) {
        panic!("failed to compile fragment shader. info_log: {}", info_log);
    }
    glow_error!(glow_context);

    let egui_program = glow_context
        .create_program()
        .expect("failed to create glow program");
    glow_context.attach_shader(egui_program, vs);
    glow_context.attach_shader(egui_program, fs);
    glow_context.link_program(egui_program);
    let info_log = glow_context.get_program_info_log(egui_program);
    if !info_log.is_empty() {
        warn!("egui program info log: {info_log}")
    }
    if !glow_context.get_program_link_status(egui_program) {
        panic!("failed to link egui glow program. info_log: {}", info_log);
    }
    glow_error!(glow_context);
    info!("egui shader program successfully compiled and linked");
    // no need for shaders anymore after linking
    glow_context.detach_shader(egui_program, vs);
    glow_context.detach_shader(egui_program, fs);
    glow_context.delete_shader(vs);
    glow_context.delete_shader(fs);
    egui_program
}

unsafe fn create_egui_vao_buffers(
    glow_context: &glow::Context,
    program: Program,
) -> (VertexArray, Buffer, Buffer) {
    let vao = glow_context
        .create_vertex_array()
        .expect("failed to create egui vao");
    glow_context.bind_vertex_array(Some(vao));
    glow_error!(glow_context);

    // buffers
    let vbo = glow_context
        .create_buffer()
        .expect("failed to create array buffer");
    glow_context.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
    glow_error!(glow_context);

    let ebo = glow_context
        .create_buffer()
        .expect("failed to create element buffer");
    glow_context.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
    glow_error!(glow_context);

    // enable position, tex coords and color attributes. this will bind vbo to the vao
    let location = glow_context
        .get_attrib_location(program, "vin_pos")
        .expect("failed to get vin_pos location");
    assert_eq!(location, 0, "vin_pos");
    info!("vin_pos vertex attribute location is {location}");
    glow_context.enable_vertex_attrib_array(location);
    glow_context.vertex_attrib_pointer_f32(location, 2, glow::FLOAT, false, 20, 0);
    let location = glow_context
        .get_attrib_location(program, "vin_tc")
        .expect("failed to get vin_tc location");
    assert_eq!(location, 1, "vin_tc");
    info!("vin_tc vertex attribute location is {location}");
    glow_context.enable_vertex_attrib_array(location);
    glow_context.vertex_attrib_pointer_f32(location, 2, glow::FLOAT, false, 20, 8);
    let location = glow_context
        .get_attrib_location(program, "vin_sc")
        .expect("failed to get vin_sc location");
    assert_eq!(location, 2, "vin_sc");
    info!("vin_sc vertex attribute location is {location}");
    glow_context.enable_vertex_attrib_array(location);
    glow_context.vertex_attrib_pointer_f32(location, 4, glow::UNSIGNED_BYTE, false, 20, 16);
    glow_error!(glow_context);
    (vao, vbo, ebo)
}

unsafe fn create_samplers(glow_context: &glow::Context) -> (Sampler, Sampler) {
    let nearest_sampler = glow_context
        .create_sampler()
        .expect("failed to create nearest sampler");
    glow_context.bind_sampler(0, Some(nearest_sampler));
    glow_error!(glow_context);

    glow_context.sampler_parameter_i32(
        nearest_sampler,
        glow::TEXTURE_MAG_FILTER,
        glow::NEAREST
            .try_into()
            .expect("failed to fit NEAREST in i32"),
    );
    glow_error!(glow_context);

    glow_context.sampler_parameter_i32(
        nearest_sampler,
        glow::TEXTURE_MIN_FILTER,
        glow::NEAREST
            .try_into()
            .expect("failed to fit NEAREST in i32"),
    );
    glow_error!(glow_context);

    let linear_sampler = glow_context
        .create_sampler()
        .expect("failed to create linear sampler");
    glow_context.bind_sampler(0, Some(linear_sampler));
    glow_error!(glow_context);

    glow_context.sampler_parameter_i32(
        linear_sampler,
        glow::TEXTURE_MAG_FILTER,
        glow::LINEAR
            .try_into()
            .expect("failed to fit LINEAR MIPMAP NEAREST in i32"),
    );
    glow_error!(glow_context);

    glow_context.sampler_parameter_i32(
        linear_sampler,
        glow::TEXTURE_MIN_FILTER,
        glow::LINEAR
            .try_into()
            .expect("failed to fit LINEAR MIPMAP NEAREST in i32"),
    );
    glow_error!(glow_context);
    (linear_sampler, nearest_sampler)
}
