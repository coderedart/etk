use egui::Window;
use egui_backend::{egui, BackendConfig, EguiUserApp, GfxBackend, WindowBackend};
use egui_render_glow::{glow::HasContext, GlowBackend};
use egui_window_sdl2::Sdl2Backend;
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
impl EguiUserApp<Sdl2Backend> for App {
    type UserGfxBackend = GlowBackend;

    fn get_gfx_backend(&mut self) -> &mut Self::UserGfxBackend {
        &mut self.glow_backend
    }

    fn get_egui_context(&mut self) -> egui::Context {
        self.egui_context.clone()
    }

    fn gui_run(&mut self, egui_context: &egui::Context, _window_backend: &mut Sdl2Backend) {
        Window::new("egui user window").show(egui_context, |ui| {
            ui.label("hello");
            ui.label(format!("frame number: {}", self.frame_count));
            ui.label(format!("{:#?}", egui_context.pointer_latest_pos()));
            self.frame_count += 1;
            ui.color_edit_button_srgba(&mut self.bg_color);
            // let input = egui_context.input().clone();
            // input.ui(ui);
        });
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
    tracing_subscriber::fmt().init();
    let config = Default::default();
    let mut window_backend = Sdl2Backend::new(
        config,
        BackendConfig {
            gfx_api_type: egui_backend::GfxApiType::GL,
        },
    );
    let glow_backend = GlowBackend::new(&mut window_backend, Default::default());
    let app = App::new(glow_backend);
    window_backend.run_event_loop(app);
}

fn main() {
    fake_main()
}
