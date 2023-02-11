use egui::Window;
use egui_backend::{egui, BackendConfig, EguiUserApp, GfxBackend, WindowBackend};
use egui_render_glow::{glow::HasContext, GlowBackend};
use egui_window_glfw_passthrough::GlfwBackend;
struct App {
    frame_count: usize,
    bg_color: egui::Color32,
    egui_context: egui::Context,
    glow_backend: GlowBackend,
    glfw_backend: GlfwBackend,
}
impl App {
    pub fn new(tdb: GlowBackend, glfw_backend: GlfwBackend) -> Self {
        Self {
            frame_count: 0,
            bg_color: egui::Color32::LIGHT_BLUE,
            egui_context: Default::default(),
            glow_backend: tdb,
            glfw_backend,
        }
    }
}
impl EguiUserApp for App {
    type UserGfxBackend = GlowBackend;

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
            &mut self.glow_backend,
            &self.egui_context,
        )
    }

    fn resize_framebuffer(&mut self) {
        let (wb, gb, _) = self.get_all();
        gb.resize_framebuffer(wb);
    }

    fn resume(&mut self) {
        let (wb, gb, _) = self.get_all();
        gb.resume(wb);
    }

    fn suspend(&mut self) {
        let (wb, gb, _) = self.get_all();
        gb.suspend(wb);
    }

    fn run(
        &mut self,
        logical_size: [f32; 2],
    ) -> Option<(egui::PlatformOutput, std::time::Duration)> {
        let (wb, gb, egui_context) = self.get_all();
        let egui_context = egui_context.clone();
        // don't bother doing anything if there's no window
        if let Some(full_output) = if wb.get_window().is_some() {
            let input = wb.get_raw_input();
            gb.prepare_frame(wb);
            egui_context.begin_frame(input);
            self.gui_run();
            Some(egui_context.end_frame())
        } else {
            None
        } {
            let egui::FullOutput {
                platform_output,
                repaint_after,
                textures_delta,
                shapes,
            } = full_output;
            let (wb, gb, egui_context) = self.get_all();
            let egui_context = egui_context.clone();

            gb.render_egui(
                egui_context.tessellate(shapes),
                textures_delta,
                logical_size,
            );
            gb.present(wb);
            return Some((platform_output, repaint_after));
        }
        None
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
    let app = App::new(glow_backend, window_backend);
    <App as EguiUserApp>::UserWindowBackend::run_event_loop(app);
}

fn main() {
    fake_main()
}
