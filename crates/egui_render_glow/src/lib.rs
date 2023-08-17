mod helpers;
use bytemuck::cast_slice;
use egui::TextureId;
use egui_backend::{egui::TexturesDelta, *};
pub use glow;
use glow::{Context as GlowContext, HasContext, *};
use helpers::*;
use intmap::IntMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// opengl error checking flushes all commands and forces synchronization
/// so, we should make this feature gated eventually and maybe use debug callbacks (on desktop atleast)
#[macro_export]
macro_rules! glow_error {
    ($glow_context: ident) => {
        let error_code = $glow_context.get_error();
        if error_code != glow::NO_ERROR {
            tracing::error!("glow error: {} at line {}", error_code, line!());
        }
    };
}
/// All shaders are targeting #version 300 es
pub const EGUI_VS: &str = include_str!("../egui.vert");
/// output will be in linear space, so make suer to enable framebuffer srgb
pub const EGUI_LINEAR_OUTPUT_FS: &str = include_str!("../egui_linear_output.frag");
/// the output will be in srgb space, so make sure to disable framebuffer srgb.
pub const EGUI_SRGB_OUTPUT_FS: &str = include_str!("../egui_srgb_output.frag");

/// these are config to be provided to browser when requesting a webgl context
///
/// refer to `WebGL context attributes:` config in the link: <https://developer.mozilla.org/en-US/docs/Web/API/HTMLCanvasElement/getContext>
///
/// alternatively, the spec lists all attributes here <https://registry.khronos.org/webgl/specs/latest/1.0/#5.2>
///
/// ```js
/// WebGLContextAttributes {
///     boolean alpha = true;
///     boolean depth = true;
///     boolean stencil = false;
///     boolean antialias = true;
///     boolean premultipliedAlpha = true;
///     boolean preserveDrawingBuffer = false;
///     WebGLPowerPreference powerPreference = "default";
///     boolean failIfMajorPerformanceCaveat = false;
///     boolean desynchronized = false;
/// };
///
/// ```
///
/// we will only support WebGL2 for now. WebGL2 is available in 90+ % of all active devices according to <https://caniuse.com/?search=webgl2>.
#[derive(Debug, Clone, Default)]
pub struct WebGlConfig {
    pub alpha: Option<bool>,
    pub depth: Option<bool>,
    pub stencil: Option<bool>,
    pub antialias: Option<bool>,
    pub premultiplied_alpha: Option<bool>,
    pub preserve_drawing_buffer: Option<bool>,
    /// possible values are "default", "high-performance", "low-power"
    /// `None`: default.
    /// `Some(true)`: lower power
    /// `Some(false)`: high performance
    pub low_power: Option<bool>,
    pub fail_if_major_performance_caveat: Option<bool>,
    pub desynchronized: Option<bool>,
}

pub struct GlowBackend {
    pub glow_context: Arc<GlowContext>,
    pub framebuffer_size: [u32; 2],
    pub painter: Painter,
}

impl Drop for GlowBackend {
    fn drop(&mut self) {
        unsafe { self.painter.destroy(&self.glow_context) };
    }
}

#[derive(Debug, Default)]
pub struct GlowConfig {
    pub webgl_config: WebGlConfig,
    pub enable_debug: bool,
}

impl GfxBackend for GlowBackend {
    type Configuration = GlowConfig;

    fn new(window_backend: &mut impl WindowBackend, config: Self::Configuration) -> Self {
        let glow_context: Arc<glow::Context> =
            unsafe { create_glow_context(window_backend, config.webgl_config) };

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

        let painter = unsafe { Painter::new(&glow_context) };
        Self {
            glow_context,
            painter,
            framebuffer_size: window_backend.get_live_physical_size_framebuffer().unwrap(),
        }
    }

    fn suspend(&mut self, _window_backend: &mut impl WindowBackend) {
        tracing::warn!("egui glow backend doesn't do anything on suspend");
    }

    fn resume(&mut self, _window_backend: &mut impl WindowBackend) {
        tracing::warn!("resume does nothing on glow backend");
    }

    fn prepare_frame(&mut self, _window_backend: &mut impl WindowBackend) {
        unsafe {
            self.glow_context.disable(glow::SCISSOR_TEST);
            self.glow_context.clear(glow::COLOR_BUFFER_BIT);
        }
    }

    fn present(&mut self, _window_backend: &mut impl WindowBackend) {
        #[cfg(any(not(target_arch = "wasm32"), target_os = "emscripten"))]
        {
            _window_backend.swap_buffers();
        }
        // on wasm, there's no swap buffers.. the browser takes care of it automatically.
    }

    fn resize_framebuffer(&mut self, window_backend: &mut impl WindowBackend) {
        if let Some(fb_size) = window_backend.get_live_physical_size_framebuffer() {
            self.framebuffer_size = fb_size;
            self.painter.screen_size_physical = fb_size;
            unsafe {
                self.glow_context
                    .viewport(0, 0, fb_size[0] as i32, fb_size[1] as i32);
            }
        }
    }

    fn render_egui(
        &mut self,
        meshes: Vec<egui::ClippedPrimitive>,
        textures_delta: egui::TexturesDelta,
        logical_screen_size: [f32; 2],
    ) {
        unsafe {
            self.painter.prepare_render(
                &self.glow_context,
                meshes,
                textures_delta,
                logical_screen_size,
            );
            self.painter.render_egui(&self.glow_context);
        }
    }
}
pub struct GpuTexture {
    handle: glow::Texture,
    width: u32,
    height: u32,
    sampler: Sampler,
}

/// Egui Painter using glow::Context
/// Assumptions:
/// 1. srgb framebuffer
/// 2. opengl 3+ on desktop and webgl2 only on web.
/// 3.
pub struct Painter {
    /// Most of these objects are created at startup
    pub linear_sampler: Sampler,
    pub nearest_sampler: Sampler,
    pub font_sampler: Sampler,
    pub managed_textures: IntMap<GpuTexture>,
    pub egui_program: Program,
    pub vao: VertexArray,
    pub vbo: Buffer,
    pub ebo: Buffer,
    pub u_screen_size: UniformLocation,
    pub u_sampler: UniformLocation,
    pub clipped_primitives: Vec<egui::ClippedPrimitive>,
    pub textures_to_delete: Vec<TextureId>,
    /// updated every frame from the egui gfx output struct
    pub logical_screen_size: [f32; 2],
    /// must update on framebuffer resize.
    pub screen_size_physical: [u32; 2],
}

impl Painter {
    /// # Safety
    /// well, its opengl.. so anything can go wrong. but basicaly, make sure that this opengl context is valid/current
    /// and manually call [`Self::destroy`] before dropping this.
    pub unsafe fn new(gl: &glow::Context) -> Self {
        info!("creating glow egui painter");
        unsafe {
            info!("GL Version: {}", gl.get_parameter_string(glow::VERSION));
            info!("GL Renderer: {}", gl.get_parameter_string(glow::RENDERER));
            info!("Gl Vendor: {}", gl.get_parameter_string(glow::VENDOR));
            if gl.version().major > 1 {
                info!(
                    "GLSL version: {}",
                    gl.get_parameter_string(glow::SHADING_LANGUAGE_VERSION)
                );
            }
            glow_error!(gl);
            // compile shaders
            let egui_program = create_program_from_src(
                gl,
                EGUI_VS,
                if cfg!(wasm32) {
                    // on wasm, we always assume srgb framebuffer
                    EGUI_LINEAR_OUTPUT_FS
                } else {
                    EGUI_SRGB_OUTPUT_FS
                },
            );
            // shader verification
            glow_error!(gl);
            let u_screen_size = gl
                .get_uniform_location(egui_program, "u_screen_size")
                .expect("failed to find u_screen_size");
            debug!("location of uniform u_screen_size is {u_screen_size:?}");
            let u_sampler = gl
                .get_uniform_location(egui_program, "u_sampler")
                .expect("failed to find u_sampler");
            debug!("location of uniform u_sampler is {u_sampler:?}");
            gl.use_program(Some(egui_program));
            let (vao, vbo, ebo) = create_egui_vao_buffers(gl, egui_program);
            debug!("created egui vao, vbo, ebo");
            let (linear_sampler, nearest_sampler, font_sampler) = create_samplers(gl);
            debug!("created linear and nearest samplers");
            Self {
                managed_textures: Default::default(),
                egui_program,
                vao,
                vbo,
                ebo,
                linear_sampler,
                nearest_sampler,
                font_sampler,
                u_screen_size,
                u_sampler,
                clipped_primitives: Vec::new(),
                textures_to_delete: Vec::new(),
                logical_screen_size: [0.0; 2],
                screen_size_physical: [0; 2],
            }
        }
    }
    /// uploads data to opengl buffers / textures
    /// # Safety
    /// make sure that there's no opengl issues and context is still current
    pub unsafe fn prepare_render(
        &mut self,
        glow_context: &glow::Context,

        meshes: Vec<egui::ClippedPrimitive>,
        textures_delta: TexturesDelta,
        logical_screen_size: [f32; 2],
    ) {
        self.textures_to_delete = textures_delta.free;
        self.clipped_primitives = meshes;
        self.logical_screen_size = logical_screen_size;
        glow_error!(glow_context);

        // update textures
        for (texture_id, delta) in textures_delta.set {
            let sampler = match delta.options.minification {
                egui::TextureFilter::Nearest => self.nearest_sampler,
                egui::TextureFilter::Linear => self.linear_sampler,
            };
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
                                        // special sampler for font that would clamp to edge
                                        self.font_sampler
                                    } else {
                                        sampler
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
                egui::ImageData::Color(c) => (
                    c.pixels.iter().flat_map(egui::Color32::to_array).collect(),
                    c.size,
                ),
                egui::ImageData::Font(font_image) => (
                    font_image
                        .srgba_pixels(None)
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
    /// # Safety
    /// uses a bunch of unsfae opengl functions, any of which might segfault.
    pub unsafe fn render_egui(&mut self, glow_context: &glow::Context) {
        let screen_size_physical = self.screen_size_physical;
        let screen_size_logical = self.logical_screen_size;
        let scale = screen_size_physical[0] as f32 / screen_size_logical[0];

        // setup egui configuration
        glow_context.enable(glow::SCISSOR_TEST);
        glow_context.disable(glow::DEPTH_TEST);
        glow_error!(glow_context);
        #[cfg(not(target_arch = "wasm32"))]
        glow_context.disable(glow::FRAMEBUFFER_SRGB);

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
            if let Some(scissor_rect) = egui_backend::util::scissor_from_clip_rect_opengl(
                &clipped_primitive.clip_rect,
                scale,
                screen_size_physical,
            ) {
                glow_context.scissor(
                    scissor_rect[0] as i32,
                    scissor_rect[1] as i32,
                    scissor_rect[2] as i32,
                    scissor_rect[3] as i32,
                );
            } else {
                continue;
            }
            match clipped_primitive.primitive {
                egui::epaint::Primitive::Mesh(ref mesh) => {
                    glow_context.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
                    glow_context.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ebo));
                    glow_context.buffer_data_u8_slice(
                        glow::ARRAY_BUFFER,
                        cast_slice(&mesh.vertices),
                        glow::STREAM_DRAW,
                    );
                    glow_context.buffer_data_u8_slice(
                        glow::ELEMENT_ARRAY_BUFFER,
                        cast_slice(&mesh.indices),
                        glow::STREAM_DRAW,
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
    /// # Safety
    /// This must be called only once.
    /// must not use it again because this destroys all the opengl objects.
    pub unsafe fn destroy(&mut self, glow_context: &glow::Context) {
        tracing::warn!("destroying egui glow painter");
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
