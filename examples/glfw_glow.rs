use egui::Window;
use egui_backend::{BackendSettings, GfxApiType, GfxBackend, UserApp, WindowBackend};
use egui_render_glow::{glow::HasContext, *};
use egui_window_glfw::{GlfwBackend, GlfwConfig};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};
struct App;
impl UserApp<GlfwBackend, GlowBackend> for App {
    fn run(
        &mut self,
        egui_context: &egui::Context,
        _window_backend: &mut GlfwBackend,
        gfx_backend: &mut GlowBackend,
    ) {
        Window::new("egui user window").show(egui_context, |ui| {
            ui.label("hello");
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

    let glfw_config = GlfwConfig {
        glfw_callback: Some(Box::new(|glfw_context| {
            glfw_context.window_hint(egui_window_glfw::glfw::WindowHint::TransparentFramebuffer(
                true,
            ));
        })),
    };
    let mut glfw_backend = GlfwBackend::new(
        glfw_config,
        BackendSettings {
            gfx_api_type: GfxApiType::OpenGL {
                native_config: Default::default(),
            },
        },
    );
    let glow_backend = GlowBackend::new(&mut glfw_backend, ());
    let app = App;
    glfw_backend.run_event_loop(glow_backend, app);
}
