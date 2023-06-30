use egui_backend::{
    egui::{self, Window},
    BackendConfig, GfxBackend, UserApp, WindowBackend,
};
use egui_render_wgpu::WgpuBackend;
use egui_window_glfw_passthrough::{GlfwBackend, GlfwConfig};
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
struct App {
    frame_count: usize,
    egui_wants_input: bool,
    is_window_receiving_events: bool,
    egui_context: egui::Context,
    wgpu_backend: WgpuBackend,
    glfw_backend: GlfwBackend,
}

impl UserApp for App {
    type UserWindowBackend = GlfwBackend;

    fn get_all(
        &mut self,
    ) -> (
        &mut Self::UserWindowBackend,
        &mut Self::UserGfxBackend,
        &egui::Context,
    ) {
        (
            &mut self.glfw_backend,
            &mut self.wgpu_backend,
            &self.egui_context,
        )
    }
    fn gui_run(&mut self) {
        self.frame_count += 1;
        let egui_context = self.egui_context.clone();
        let egui_context = &&egui_context;
        // draw a triangle
        Window::new("egui user window").show(egui_context, |ui| {
            ui.label(format!("frame number: {}", self.frame_count));
            ui.label(format!("{:#?}", egui_context.pointer_latest_pos()));
            ui.checkbox(
                &mut self.is_window_receiving_events,
                "Is Window receiving events?",
            );
            ui.checkbox(&mut self.egui_wants_input, "Does egui want input?");
        });

        self.is_window_receiving_events = !self.glfw_backend.window.is_mouse_passthrough();
        if !self.is_window_receiving_events {
            egui_context.request_repaint();
        }
        // don't forget to only ask egui if it wants input AFTER ending the frame
        self.egui_wants_input =
            egui_context.wants_pointer_input() || egui_context.wants_keyboard_input();
        // if window is receiving events when egui doesn't want input. or if window not receiving events when egui wants input.
        if self.is_window_receiving_events != self.egui_wants_input {
            self.glfw_backend
                .window
                .set_mouse_passthrough(!self.egui_wants_input); // passthrough means not receiving events. so, if egui wants input, we set passthrough to false. otherwise true.
        }
    }

    type UserGfxBackend = WgpuBackend;
}
impl App {
    pub fn new(mut glfw_backend: GlfwBackend) -> Self {
        let wgpu_backend = WgpuBackend::new(&mut glfw_backend, Default::default());
        Self {
            frame_count: 0,
            egui_wants_input: false,
            is_window_receiving_events: false,
            egui_context: Default::default(),
            wgpu_backend,
            glfw_backend,
        }
    }
}

pub fn fake_main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let window_backend = GlfwBackend::new(
        GlfwConfig {
            glfw_callback: Box::new(|glfw_context| {
                glfw_context.window_hint(egui_window_glfw_passthrough::glfw::WindowHint::Floating(
                    true,
                ));
            }),
            ..Default::default()
        },
        BackendConfig {
            is_opengl: false,
            opengl_config: Default::default(),
            transparent: true.into(),
        },
    );

    let app = App::new(window_backend);
    <App as UserApp>::UserWindowBackend::run_event_loop(app);
}

fn main() {
    fake_main();
}
