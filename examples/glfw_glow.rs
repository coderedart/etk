use egui::Window;
use egui_backend::{GfxApiConfig, GfxBackend, UserApp, WindowBackend};
use egui_render_glow::{glow::HasContext, *};
use egui_window_glfw::{GlfwConfig, GlfwWindow};
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
impl UserApp<GlfwWindow, GlowBackend> for App {
    fn run(
        &mut self,
        egui_context: &egui::Context,
        _window_backend: &mut GlfwWindow,
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

    let glfw_config = GlfwConfig {
        glfw_callback: Some(Box::new(|glfw_context| {
            glfw_context.window_hint(egui_window_glfw::glfw::WindowHint::TransparentFramebuffer(
                true,
            ));
        })),
    };
    let (glfw_backend, window_info_for_gfx) = GlfwWindow::new(
        glfw_config,
        GfxApiConfig::OpenGL {
            version: Some((3, 0)),
            samples: None,
            srgb: Some(true),
            transparent: Some(true),
            debug: None,
        },
    );
    let glow_backend = GlowBackend::new(window_info_for_gfx, ());
    let app = App::new(&glow_backend);
    glfw_backend.run_event_loop(glow_backend, app);
}
