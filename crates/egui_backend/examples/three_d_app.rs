use egui::Window;
use egui_backend::gfx_backends::three_d_backend::{
    three_d::{ClearState, RenderTarget},
    ThreeDBackend,
};
use egui_backend::{BackendSettings, GfxBackend, OpenGLWindowContext, UserApp, WindowBackend};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};
struct App {
    frame_count: usize,
    bg_color: egui::Color32,
}
impl App {
    pub fn new(_gl: &ThreeDBackend) -> Self {
        Self {
            frame_count: 0,
            bg_color: egui::Color32::LIGHT_BLUE,
        }
    }
}
impl<W: WindowBackend + OpenGLWindowContext> UserApp<W, ThreeDBackend> for App {
    fn run(
        &mut self,
        egui_context: &egui::Context,
        _window_backend: &mut W,
        gfx_backend: &mut ThreeDBackend,
    ) {
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

        let screen = RenderTarget::screen(
            &gfx_backend.context,
            gfx_backend.glow_backend.framebuffer_size[0],
            gfx_backend.glow_backend.framebuffer_size[1],
        );

        screen.clear(ClearState::color(rgba[0], rgba[1], rgba[2], rgba[3]));
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
    let glow_backend = ThreeDBackend::new(&mut window_backend, ());
    let app = App::new(&glow_backend);
    window_backend.run_event_loop(glow_backend, app);
}
#[cfg(feature = "winit")]
type WB = egui_backend::window_backends::winit_backend::WinitBackend;
#[cfg(feature = "glfw")]
type WB = egui_backend::window_backends::glfw_backend::GlfwBackend;
#[cfg(feature = "sdl2")]
type WB = egui_backend::window_backends::sdl2_backend::Sdl2Backend;
fn main() {
    fake_main::<WB>();
}
