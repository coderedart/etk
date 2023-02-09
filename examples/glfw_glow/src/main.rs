use egui::Window;
use egui_backend::{egui, BackendConfig, EguiUserApp, GfxBackend, WindowBackend};
use egui_render_glow::{glow::HasContext, GlowBackend};
use egui_window_glfw_passthrough::GlfwBackend;
struct App {
    frame_count: usize,
    bg_color: egui::Color32,
    egui_context: egui::Context,
    glow_backend: GlowBackend,
}
impl App {
    pub fn new(tdb: GlowBackend) -> Self {
        Self {
            frame_count: 0,
            bg_color: egui::Color32::LIGHT_BLUE,
            egui_context: Default::default(),
            glow_backend: tdb,
        }
    }
}
impl EguiUserApp<GlfwBackend> for App {
    type UserGfxBackend = GlowBackend;

    fn get_gfx_backend(&mut self) -> &mut Self::UserGfxBackend {
        &mut self.glow_backend
    }

    fn get_egui_context(&mut self) -> egui::Context {
        self.egui_context.clone()
    }

    fn gui_run(&mut self, egui_context: &egui::Context, _window_backend: &mut GlfwBackend) {
        Window::new("egui user window").show(egui_context, |ui| {
            ui.label("hello");
            ui.label(format!("frame number: {}", self.frame_count));
            ui.label(format!("{:#?}", egui_context.pointer_latest_pos()));
            self.frame_count += 1;
            ui.color_edit_button_srgba(&mut self.bg_color);
            // let input = egui_context.input().clone();
            // input.ui(ui);
        });
        egui_context.request_repaint();
        let rgba = self.bg_color.to_array();
        let rgba = rgba.map(|component| component as f32 / 255.0);

        unsafe {
            self.glow_backend
                .glow_context
                .clear_color(rgba[0], rgba[1], rgba[2], rgba[3]);
            self.glow_backend
                .glow_context
                .clear(egui_render_glow::glow::COLOR_BUFFER_BIT);
        }
    }
}

pub fn fake_main() {
    use tracing_subscriber::prelude::*;
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let config = egui_window_glfw_passthrough::GlfwConfig {
        glfw_callback: Box::new(|gtx| {
            gtx.window_hint(egui_window_glfw_passthrough::glfw::WindowHint::ClientApi(
                egui_window_glfw_passthrough::glfw::ClientApiHint::OpenGl,
            ));
        }),
        ..Default::default()
    };
    let mut window_backend = GlfwBackend::new(config, BackendConfig {});
    let glow_backend = GlowBackend::new(&mut window_backend, Default::default());
    let app = App::new(glow_backend);
    window_backend.run_event_loop(app);
}

fn main() {
    fake_main()
}
