use egui::Window;
use egui_backend::{GfxApiType, GfxBackend, UserApp, WindowBackend};
use egui_render_glow::{glow::HasContext, *};
use egui_window_sdl2::*;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};
struct App {
    frame_count: usize,
}
impl App {
    pub fn new(gl: &GlowBackend) -> Self {
        let gl = gl.glow_context.clone();
        unsafe {
            gl.clear_color(0.0, 0.0, 0.0, 0.0);
        }
        Self { frame_count: 0 }
    }
}
impl UserApp<SDL2Backend, GlowBackend> for App {
    fn run(
        &mut self,
        egui_context: &egui::Context,
        _window_backend: &mut SDL2Backend,
        gfx_backend: &mut GlowBackend,
    ) {
        Window::new("egui user window").show(egui_context, |ui| {
            ui.label("hello");
            ui.label(format!("frame number: {}", self.frame_count));
            ui.label(format!("{:#?}", egui_context.pointer_latest_pos()));
            self.frame_count += 1;
        });
        unsafe {
            gfx_backend.glow_context.clear(glow::COLOR_BUFFER_BIT);
        }
    }
}

fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = Default::default();
    let mut window_backend = SDL2Backend::new(
        config,
        egui_backend::BackendSettings {
            gfx_api_type: GfxApiType::OpenGL {
                native_config: Default::default(),
            },
        },
    );
    let glow_backend = GlowBackend::new(&mut window_backend, ());
    let app = App::new(&glow_backend);
    window_backend.run_event_loop(glow_backend, app);
}
