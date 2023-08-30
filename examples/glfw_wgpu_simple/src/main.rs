use egui_backend::{
    egui::{self, TextureHandle, TextureOptions, Window},
    BackendConfig, GfxBackend, UserApp, WindowBackend,
};
use egui_render_wgpu::{wgpu::PowerPreference, WgpuBackend, WgpuConfig};
use egui_window_glfw_passthrough::{GlfwBackend, GlfwConfig};
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
struct App {
    frame_count: usize,
    fps_reset: std::time::Instant,
    fps: usize,
    previous_frame_count: usize,
    egui_wants_input: bool,
    is_window_receiving_events: bool,
    image_handle: Option<TextureHandle>,
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
        if self.fps_reset.elapsed().as_secs_f32() > 1.0 {
            self.fps_reset = std::time::Instant::now();
            self.fps = self.frame_count - self.previous_frame_count;
            self.previous_frame_count = self.frame_count;
        }
        Window::new("egui user window").show(egui_context, |ui| {
            ui.label(format!("frame number: {}", self.frame_count));
            ui.label(format!("fps: {}", self.fps));
            ui.label(format!("{:#?}", egui_context.pointer_latest_pos()));
            ui.checkbox(
                &mut self.is_window_receiving_events,
                "Is Window receiving events?",
            );
            ui.checkbox(&mut self.egui_wants_input, "Does egui want input?");
            let handle = self
                .image_handle
                .get_or_insert_with(|| {
                    egui_context.load_texture(
                        "cat texture",
                        egui_extras::image::load_image_bytes(include_bytes!("../../cat.jpg"))
                            .expect("cat image is invalid jpg"),
                        TextureOptions {
                            magnification: egui::TextureFilter::Linear,
                            minification: egui::TextureFilter::Linear,
                        },
                    )
                })
                .id();
            ui.image(handle, [620.0, 427.0]);
        });
        let cursor_pos = egui_context.pointer_latest_pos().unwrap_or_default();
        // just some controls to show how you can use glfw_backend
        egui_backend::egui::Window::new("controls").show(egui_context, |ui| {
            // sometimes, you want to see the borders to understand where the overlay is.
            let mut borders = self.glfw_backend.window.is_decorated();
            if ui.checkbox(&mut borders, "window borders").changed() {
                self.glfw_backend.window.set_decorated(borders);
            }
            let window_pos = self.glfw_backend.get_window_position().unwrap();
            ui.label(format!(
                "window pos: x: {}, y: {}",
                window_pos[0], window_pos[1]
            ));
            ui.label(format!("window scale: {}", self.glfw_backend.scale));
            ui.label(format!(
                "cursor pos: x: {}, y: {}",
                self.glfw_backend.cursor_pos[0], self.glfw_backend.cursor_pos[1]
            ));
            ui.label(format!(
                "egui cursor pos: x: {}, y: {}",
                cursor_pos.x, cursor_pos.y
            ));
            ui.label(format!(
                "passthrough: {}",
                self.glfw_backend.get_passthrough().unwrap()
            ));
            // how to change size.
            // WARNING: don't use drag value, because window size changing while dragging ui messes things up.
            let mut size = self.glfw_backend.window_size_logical;
            let mut changed = false;
            ui.horizontal(|ui| {
                ui.label("width: ");
                ui.add_enabled(false, egui::DragValue::new(&mut size[0]));
                if ui.button("inc").clicked() {
                    size[0] += 10.0;
                    changed = true;
                }
                if ui.button("dec").clicked() {
                    size[0] -= 10.0;
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("height: ");
                ui.add_enabled(false, egui::DragValue::new(&mut size[1]));
                if ui.button("inc").clicked() {
                    size[1] += 10.0;
                    changed = true;
                }
                if ui.button("dec").clicked() {
                    size[1] -= 10.0;
                    changed = true;
                }
            });
            if changed {
                self.glfw_backend.set_window_size(size);
            }
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
        let wgpu_backend = WgpuBackend::new(
            &mut glfw_backend,
            WgpuConfig {
                power_preference: PowerPreference::HighPerformance,
                ..Default::default()
            },
        );
        Self {
            frame_count: 0,
            egui_wants_input: false,
            is_window_receiving_events: false,
            egui_context: Default::default(),
            wgpu_backend,
            glfw_backend,
            fps_reset: std::time::Instant::now(),
            fps: 0,
            previous_frame_count: 0,
            image_handle: None,
        }
    }
}

pub fn fake_main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or(
                tracing_subscriber::EnvFilter::new("debug,wgpu=warn,naga=warn"),
            ),
        )
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
