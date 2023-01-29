use egui_backend::{
    egui::{ClippedPrimitive, TexturesDelta},
    GfxBackend, WindowBackend,
};
use egui_render_glow::{GlowBackend, GlowConfig};
pub use three_d;
use three_d::Context;
pub struct ThreeDBackend {
    pub context: Context,
    pub glow_backend: GlowBackend,
}

#[derive(Default)]
pub struct ThreeDConfig {
    glow_config: GlowConfig,
}

impl GfxBackend for ThreeDBackend {
    type Configuration = ThreeDConfig;

    fn new(window_backend: &mut impl WindowBackend, _config: Self::Configuration) -> Self {
        let glow_backend = GlowBackend::new(window_backend, _config.glow_config);

        #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
        {
            use three_d::HasContext;
            let supported_extension = (glow_backend.glow_context).supported_extensions();

            assert!(supported_extension.contains("EXT_color_buffer_float"));

            assert!(supported_extension.contains("OES_texture_float"));

            assert!(supported_extension.contains("OES_texture_float_linear"));
        }

        Self {
            context: Context::from_gl_context(glow_backend.glow_context.clone())
                .expect("failed to create threed context"),
            glow_backend,
        }
    }

    fn suspend(&mut self, _window_backend: &mut impl WindowBackend) {}

    fn resume(&mut self, _window_backend: &mut impl WindowBackend) {}

    fn prepare_frame(&mut self, window_backend: &mut impl WindowBackend) {
        self.glow_backend.prepare_frame(window_backend);
    }

    fn render_egui(
        &mut self,
        meshes: Vec<ClippedPrimitive>,
        textures_delta: TexturesDelta,
        logical_screen_size: [f32; 2],
    ) {
        self.glow_backend
            .render_egui(meshes, textures_delta, logical_screen_size);
    }

    fn present(&mut self, window_backend: &mut impl WindowBackend) {
        self.glow_backend.present(window_backend);
    }

    fn resize_framebuffer(&mut self, window_backend: &mut impl WindowBackend) {
        self.glow_backend.resize_framebuffer(window_backend);
    }
}
