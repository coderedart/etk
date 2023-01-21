use egui_backend::{EguiGfxData, GfxBackend, WindowBackend};
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

impl<W: WindowBackend> GfxBackend<W> for ThreeDBackend {
    type Configuration = ThreeDConfig;

    fn new(window_backend: &mut W, _config: Self::Configuration) -> Self {
        let glow_backend = GlowBackend::new(window_backend, _config.glow_config);

        #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
        {
            use three_d::HasContext;
            let supported_extension = (&glow_backend.glow_context).supported_extensions();

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

    fn suspend(&mut self, _window_backend: &mut W) {}

    fn resume(&mut self, _window_backend: &mut W) {}

    fn prepare_frame(&mut self, framebuffer_size_update: bool, window_backend: &mut W) {
        self.glow_backend
            .prepare_frame(framebuffer_size_update, window_backend);
    }

    fn render(&mut self, egui_gfx_data: EguiGfxData) {
        <GlowBackend as GfxBackend<W>>::render(&mut self.glow_backend, egui_gfx_data);
    }

    fn present(&mut self, window_backend: &mut W) {
        self.glow_backend.present(window_backend);
    }
}
