#[cfg(feature = "glow")]
mod gate {
    use egui::Window;
    use egui_backend::gfx_backends::glow_backend::{glow::HasContext, *};
    use egui_backend::{BackendSettings, GfxBackend, OpenGLWindowContext, UserApp, WindowBackend};
    use tracing_subscriber::{
        prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt,
    };
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
    impl<W: WindowBackend + OpenGLWindowContext> UserApp<W, GlowBackend> for App {
        fn run(
            &mut self,
            egui_context: &egui::Context,
            _window_backend: &mut W,
            gfx_backend: &mut GlowBackend,
        ) {
            Window::new("egui user window").show(egui_context, |ui| {
                ui.label("hello");
                ui.label(format!("frame number: {}", self.frame_count));
                ui.label(format!("{:#?}", egui_context.pointer_latest_pos()));
                self.frame_count += 1;
                let input = egui_context.input().clone();
                input.ui(ui);
            });
            unsafe {
                gfx_backend.glow_context.clear(glow::COLOR_BUFFER_BIT);
            }
        }
    }

    pub fn fake_main<W: WindowBackend + OpenGLWindowContext>() {
        tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer())
            .with(tracing_subscriber::EnvFilter::from_default_env())
            .init();

        let config = Default::default();
        let mut window_backend = W::new(
            config,
            BackendSettings {
                gfx_api_type: egui_backend::GfxApiType::OpenGL {
                    native_config: Default::default(),
                },
            },
        );
        let glow_backend = GlowBackend::new(&mut window_backend, ());
        let app = App::new(&glow_backend);
        window_backend.run_event_loop(glow_backend, app);
    }
}
#[cfg(feature = "glow")]
pub use gate::*;

fn main() {}
