use egui::Window;
use egui_backend::{BackendSettings, GfxBackend, UserApp, WindowBackend};

use egui_window_winit::*;

type WB = WinitBackend;
type GB = egui_render_wgpu::WgpuBackend;
// type GB = egui_render_glow::GlowBackend;
struct App;

impl<W: WindowBackend, G: GfxBackend<W>> UserApp<W, G> for App {
    fn run(&mut self, egui_context: &egui::Context, _window_backend: &mut W, _gfx_backend: &mut G) {
        Window::new("egui user window").show(egui_context, |ui| {
            ui.label("hello");
        });
    }
}
fn main() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();

    let config = Default::default();
    let mut window_backend = WB::new(config, BackendSettings::default());
    let gfx_backend = GB::new(&mut window_backend, Default::default());
    let app = App;
    window_backend.run_event_loop(gfx_backend, app);
}
