use egui::Window;
use egui_backend::{GfxApiConfig, GfxBackend, UserApp, WindowBackend};
use egui_render_wgpu::*;
use egui_window_winit::*;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};
type WB = WinitBackend;
struct App {
    frame_count: usize,
}

impl App {
    pub fn new(_gfx_backend: &WgpuBackend) -> Self {
        Self { frame_count: 0 }
    }
}
impl UserApp<WB, WgpuBackend> for App {
    fn run(
        &mut self,
        egui_context: &egui::Context,
        _window_backend: &mut WB,
        _gfx_backend: &mut WgpuBackend,
    ) {
        Window::new("egui user window").show(egui_context, |ui| {
            ui.label("hello");
            ui.label(format!("frame number: {}", self.frame_count));
            ui.label(format!("{:#?}", egui_context.pointer_latest_pos()));
            self.frame_count += 1;
        });
    }
}

fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = Default::default();
    let (window_backend, window_info_for_gfx) = WB::new(config, GfxApiConfig::Vulkan {});
    let gfx_backend = WgpuBackend::new(window_info_for_gfx, Default::default());
    let app = App::new(&gfx_backend);
    window_backend.run_event_loop(gfx_backend, app);
}
