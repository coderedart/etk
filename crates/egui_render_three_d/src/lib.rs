use egui_backend::{GfxBackend, WindowBackend};
use egui_render_glow::GlowBackend;
pub use three_d;
use three_d::Context;
pub struct ThreeDBackend {
    pub context: Context,
    pub glow_backend: GlowBackend,
}

impl Drop for ThreeDBackend {
    fn drop(&mut self) {
        unsafe {
            self.glow_backend.destroy();
        }
    }
}

impl<
        #[cfg(not(target_arch = "wasm32"))] W: WindowBackend + egui_backend::OpenGLWindowContext,
        #[cfg(target_arch = "wasm32")] W: WindowBackend,
    > GfxBackend<W> for ThreeDBackend
{
    type Configuration = ();

    fn new(window_backend: &mut W, settings: Self::Configuration) -> Self {
        let glow_backend = egui_render_glow::GlowBackend::new(window_backend, settings);
        Self {
            context: Context::from_gl_context(glow_backend.glow_context.clone())
                .expect("failed to create threed context"),
            glow_backend,
        }
    }

    fn prepare_frame(&mut self, framebuffer_size_update: bool, window_backend: &mut W) {
        self.glow_backend
            .prepare_frame(framebuffer_size_update, window_backend);
    }

    fn prepare_render(&mut self, egui_gfx_output: egui_backend::EguiGfxOutput) {
        <GlowBackend as GfxBackend<W>>::prepare_render(&mut self.glow_backend, egui_gfx_output);
    }

    fn render(&mut self) {
        <GlowBackend as GfxBackend<W>>::render(&mut self.glow_backend);
    }

    fn present(&mut self, window_backend: &mut W) {
        self.glow_backend.present(window_backend);
    }
}
