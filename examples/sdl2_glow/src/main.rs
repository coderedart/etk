use egui::Window;
use egui_backend::{egui, BackendConfig, EguiUserApp, GfxBackend, WindowBackend};
use egui_render_glow::{glow::HasContext, GlowBackend};
use egui_window_sdl2::Sdl2Backend;
struct App {
    frame_count: usize,
    bg_color: egui::Color32,
    egui_context: egui::Context,
    glow_backend: GlowBackend,
    window_backend: Sdl2Backend,
}
impl App {
    pub fn new(tdb: GlowBackend, window_backend: Sdl2Backend) -> Self {
        Self {
            frame_count: 0,
            bg_color: egui::Color32::LIGHT_BLUE,
            egui_context: Default::default(),
            glow_backend: tdb,
            window_backend,
        }
    }
}
impl EguiUserApp for App {
    type UserGfxBackend = GlowBackend;

    type UserWindowBackend = Sdl2Backend;

    fn get_all(
        &mut self,
    ) -> (
        &mut Self::UserWindowBackend,
        &mut Self::UserGfxBackend,
        &egui::Context,
    ) {
        (
            &mut self.window_backend,
            &mut self.glow_backend,
            &self.egui_context,
        )
    }
    fn gui_run(&mut self) {
        let egui_context = self.egui_context.clone();
        let egui_context = &egui_context;
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
    use tracing_subscriber::prelude::*;
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let config = egui_window_sdl2::SDL2Config {
        window_creator_callback: Box::new(|video_subsystem| {
            // renderdoc only works with opengl 3.3+
            video_subsystem.gl_attr().set_context_major_version(3);
            video_subsystem.gl_attr().set_context_minor_version(3);
            let mut window_builder = video_subsystem.window("sdl2 creator", 800, 600);
            window_builder.opengl();
            window_builder.allow_highdpi();
            window_builder.resizable();

            window_builder.build().expect("failed to create a window")
        }),
    };
    let mut window_backend = Sdl2Backend::new(config, BackendConfig {});
    let glow_backend = GlowBackend::new(&mut window_backend, Default::default());
    let app = App::new(glow_backend, window_backend);
    <App as EguiUserApp>::UserWindowBackend::run_event_loop(app);
}

fn main() {
    fake_main()
}
