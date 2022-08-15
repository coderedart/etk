use bytemuck::cast_slice;
use egui_backend::{
    egui::{self, TextureId},
    EguiGfxOutput, GfxBackend, OpenGLWindowContext, WindowBackend,
};
use std::sync::Arc;

use glow::{
    Context as GlowContext, HasContext, NativeBuffer, NativeProgram, NativeSampler,
    NativeUniformLocation, NativeVertexArray,
};
use intmap::IntMap;

pub use glow;
pub mod pipeline;
const EGUI_VS: &str = include_str!("../egui.vs");
const EGUI_FS: &str = include_str!("../egui.fs");

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
    pub window_opengl_context: Box<dyn OpenGLWindowContext>,
    pub painter: Painter,
}

pub struct Painter {
    pub glow_context: Arc<GlowContext>,
    pub linear_sampler: NativeSampler,
    pub nearest_sampler: NativeSampler,
    managed_textures: IntMap<GpuTexture>,
    egui_program: NativeProgram,
    vao: NativeVertexArray,
    vbo: NativeBuffer,
    ebo: NativeBuffer,
    u_screen_size: NativeUniformLocation,
    u_sampler: NativeUniformLocation,
}

impl Drop for Painter {
    fn drop(&mut self) {
        unsafe {
            self.glow_context.delete_sampler(self.linear_sampler);
            self.glow_context.delete_sampler(self.nearest_sampler);
            self.glow_context.delete_program(self.egui_program);
            self.glow_context.delete_vertex_array(self.vao);
            self.glow_context.delete_buffer(self.vbo);
            self.glow_context.delete_buffer(self.ebo);
        }
    }
}
unsafe fn create_program_from_src(
    glow_context: &glow::Context,
    vertex_src: &str,
    frag_src: &str,
) -> NativeProgram {
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
        eprintln!("vertex shader info log: {info_log}")
    }
    if !glow_context.get_shader_compile_status(vs) {
        panic!("failed to compile vertex shader. info_log: {}", info_log);
    }
    glow_error!(glow_context);
    glow_context.compile_shader(fs);
    let info_log = glow_context.get_shader_info_log(fs);
    if !info_log.is_empty() {
        eprintln!("fragment shader info log: {info_log}")
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
        eprintln!("egui program info log: {info_log}")
    }
    if !glow_context.get_program_link_status(egui_program) {
        panic!("failed to link egui glow program. info_log: {}", info_log);
    }
    glow_error!(glow_context);
    // no need for shaders anymore after linking
    glow_context.detach_shader(egui_program, vs);
    glow_context.detach_shader(egui_program, fs);
    glow_context.delete_shader(vs);
    glow_context.delete_shader(fs);
    egui_program
}

unsafe fn create_egui_vao_buffers(
    glow_context: &glow::Context,
    program: NativeProgram,
) -> (NativeVertexArray, NativeBuffer, NativeBuffer) {
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
    glow_context.enable_vertex_attrib_array(location);
    glow_context.vertex_attrib_pointer_f32(location, 2, glow::FLOAT, false, 20, 0);
    let location = glow_context
        .get_attrib_location(program, "vin_tc")
        .expect("failed to get vin_tc location");
    assert_eq!(location, 1, "vin_tc");
    glow_context.enable_vertex_attrib_array(location);
    glow_context.vertex_attrib_pointer_f32(location, 2, glow::FLOAT, false, 20, 8);
    let location = glow_context
        .get_attrib_location(program, "vin_sc")
        .expect("failed to get vin_sc location");
    assert_eq!(location, 2, "vin_sc");
    glow_context.enable_vertex_attrib_array(location);
    glow_context.vertex_attrib_pointer_f32(location, 4, glow::UNSIGNED_BYTE, false, 20, 16);
    glow_error!(glow_context);
    (vao, vbo, ebo)
}

unsafe fn create_samplers(glow_context: &glow::Context) -> (NativeSampler, NativeSampler) {
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
impl Painter {
    pub fn new(glow_context: Arc<GlowContext>) -> Self {
        unsafe {
            glow_error!(glow_context);
            // compile shaders
            let egui_program = create_program_from_src(&glow_context, EGUI_VS, EGUI_FS);
            // shader verification
            glow_error!(glow_context);
            let u_screen_size = glow_context
                .get_uniform_location(egui_program, "u_screen_size")
                .expect("failed to find u_screen_size");
            let u_sampler = glow_context
                .get_uniform_location(egui_program, "u_sampler")
                .expect("failed to find u_sampler");
            glow_context.use_program(Some(egui_program));
            let (vao, vbo, ebo) = create_egui_vao_buffers(&glow_context, egui_program);
            let (linear_sampler, nearest_sampler) = create_samplers(&glow_context);
            Self {
                managed_textures: Default::default(),
                glow_context,
                egui_program,
                vao,
                vbo,
                ebo,
                linear_sampler,
                nearest_sampler,
                u_screen_size,
                u_sampler,
            }
        }
    }
}
pub struct GpuTexture {
    handle: glow::NativeTexture,
    width: u32,
    height: u32,
    sampler: NativeSampler,
}

impl GfxBackend for GlowBackend {
    type GfxBackendSettings = ();

    fn new(
        window_info_for_gfx: egui_backend::WindowInfoForGfx,
        _settings: Self::GfxBackendSettings,
    ) -> Self {
        let mut window_opengl_context = window_info_for_gfx
            .opengl_context
            .expect("window backend doesn't support opengl window context trait");
        window_opengl_context.make_context_current();
        let glow_context = Arc::new(unsafe {
            glow::Context::from_loader_function(|s| window_opengl_context.get_proc_address(s))
        });
        unsafe {
            let gl = glow_context.clone();
            match window_info_for_gfx.gfx_api_config {
                egui_backend::GfxApiConfig::OpenGL { debug, .. } => {
                    if let Some(debug) = debug {
                        if debug {
                            gl.enable(glow::DEBUG_OUTPUT);
                            gl.enable(glow::DEBUG_OUTPUT_SYNCHRONOUS);
                            assert!(gl.supports_debug());
                            gl.debug_message_callback(
                                |source, error_type, error_id, severity, error_str| {
                                    let severity = match severity {
                                        glow::DEBUG_SEVERITY_NOTIFICATION => {
                                            // "notification"
                                            return;
                                        }
                                        glow::DEBUG_SEVERITY_LOW => "low",
                                        glow::DEBUG_SEVERITY_MEDIUM => "medium",
                                        glow::DEBUG_SEVERITY_HIGH => "high",
                                        rest => panic!("unknown severity {rest}"),
                                    };
                                    dbg!(source, error_type, error_id, severity, error_str);
                                },
                            );
                            glow_error!(glow_context);
                        }
                    }
                }
                _ => unimplemented!(),
            }
        }
        let painter = Painter::new(glow_context.clone());
        Self {
            glow_context,
            painter,
            window_opengl_context,
        }
    }

    fn prepare_frame<W: WindowBackend>(
        &mut self,
        framebuffer_size_update: Option<[u32; 2]>,
        _window_backend: &W,
    ) {
        if let Some(fb_size) = framebuffer_size_update {
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
        let EguiGfxOutput {
            meshes,
            textures_delta,
            screen_size_logical,
            screen_size_physical,
            scale,
        } = egui_gfx_output;
        unsafe {
            let glow_context = self.glow_context.clone();
            glow_error!(glow_context);

            // update textures
            for (texture_id, delta) in textures_delta.set {
                match texture_id {
                    TextureId::Managed(managed) => {
                        glow_context.bind_texture(
                            glow::TEXTURE_2D,
                            Some(match self.painter.managed_textures.entry(managed) {
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
                                            self.painter.nearest_sampler
                                        } else {
                                            self.painter.linear_sampler
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
                                .painter
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
            // // setup egui configuration
            glow_context.enable(glow::SCISSOR_TEST);
            glow_context.disable(glow::DEPTH_TEST);
            glow_context.enable(glow::FRAMEBUFFER_SRGB);
            glow_context.active_texture(glow::TEXTURE0);
            glow_context.bind_buffer(glow::ARRAY_BUFFER, Some(self.painter.vbo));
            glow_context.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.painter.ebo));
            glow_context.bind_vertex_array(Some(self.painter.vao));
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
            glow_context.use_program(Some(self.painter.egui_program));
            glow_context.active_texture(glow::TEXTURE0);

            glow_context.uniform_1_i32(Some(&self.painter.u_sampler), 0);
            glow_context
                .uniform_2_f32_slice(Some(&self.painter.u_screen_size), &screen_size_logical);

            for clipped_primitive in meshes {
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

                glow_context.scissor(
                    clip_min_x,
                    screen_size_physical[1] as i32 - clip_max_y, // NOTE: Y coordinate must be flipped inside the cliprect relative to screen height
                    clip_max_x - clip_min_x,
                    clip_max_y - clip_min_y,
                );
                glow_error!(glow_context);
                match clipped_primitive.primitive {
                    egui::epaint::Primitive::Mesh(mesh) => {
                        glow_context.bind_buffer(glow::ARRAY_BUFFER, Some(self.painter.vbo));
                        glow_context
                            .bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.painter.ebo));
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
                                    .painter
                                    .managed_textures
                                    .get(managed)
                                    .expect("managed texture cannot be found");
                                glow_context
                                    .bind_texture(glow::TEXTURE_2D, Some(managed_tex.handle));

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
                        glow_context.draw_elements(
                            glow::TRIANGLES,
                            indices_len,
                            glow::UNSIGNED_INT,
                            0,
                        );

                        glow_error!(glow_context);
                    }

                    egui::epaint::Primitive::Callback(_) => todo!(),
                }
            }
            glow_error!(glow_context);

            for tid in textures_delta.free {
                match tid {
                    TextureId::Managed(managed) => {
                        glow_context.delete_texture(
                            self.painter
                                .managed_textures
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
    }

    fn present(&mut self) {
        self.window_opengl_context.swap_buffers();
    }

    fn render(&mut self) {}
}
