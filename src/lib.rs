#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: egui_window_winit::winit::platform::android::activity::AndroidApp) {
    use egui_window_winit::WinitConfig;
    use tracing_subscriber::{
        prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt,
    };
    let layer = tracing_android::layer("etktrace").unwrap();
    let filter = tracing_subscriber::filter::LevelFilter::from_level(tracing::Level::WARN);
    tracing_subscriber::registry()
        .with(filter)
        .with(layer)
        .init();
    // android_logger::init_once(android_logger::Config::default().with_min_level(log::Level::Info));
    tracing::warn!("initiated logging");
    let winit_backend = egui_window_winit::WinitBackend::new(
        WinitConfig {
            android_app: Some(app),
            title: String::new(),
            dom_element_id: String::new(),
        },
        Default::default(),
    );
    tracing::warn!(
        "created window backend. does window exist? {}",
        winit_backend.window.is_some()
    );
    fake_main(winit_backend);
}
use egui::Window;
use egui_backend::{GfxBackend, UserAppData, WindowBackend};
use egui_render_wgpu::WgpuBackend;
type GB = WgpuBackend;
pub fn fake_main<W: WindowBackend>(mut window_backend: W) {
    let gfx_backend = GB::new(&mut window_backend, Default::default());

    window_backend.run_event_loop(gfx_backend, App { check: false });
}

// // type GB = egui_render_glow::GlowBackend;
// // type GB = egui_render_three_d::ThreeDBackend;
pub struct App {
    check: bool,
}
#[cfg(not(feature = "passthrough"))]
impl<W: WindowBackend, G: GfxBackend<W>> UserAppData<W, G> for App {
    fn run(
        &mut self,
        egui_context: &egui::Context,
        raw_input: egui::RawInput,
        _window_backend: &mut W,
        _gfx_backend: &mut G,
    ) -> egui::FullOutput {
        // do something with raw_input like filtering certain events or some custom usecase.
        // and then begin frame
        egui_context.begin_frame(raw_input);
        // do the egui stuff
        Window::new("egui user window").show(egui_context, |ui| {
            ui.label("hello");
            let checkbox = ui.checkbox(&mut self.check, "this is a checkbox");
            ui.label(format!("checkbox location: {:?}", checkbox.rect));
            let button = ui.button("click me ");
            if button.clicked() {
                tracing::warn!("touched me");
            }
            ui.label(format!("button rect: {:?}", button.rect));
            let input = egui_context.input().clone();
            input.ui(ui);
        });
        // end frame
        let output = egui_context.end_frame();
        // do something custom like accesskit stuff using fulloutput before forwarding it back to window backend.
        output
    }
}
#[cfg(feature = "passthrough")]
impl<G: GfxBackend<GlfwBackend>> UserAppData<GlfwBackend, G> for App {
    fn run(
        &mut self,
        egui_context: &egui::Context,
        raw_input: egui::RawInput,
        window_backend: &mut GlfwBackend,
        _gfx_backend: &mut G,
    ) -> egui::FullOutput {
        // do something with raw_input like filtering certain events or some custom usecase.
        // and then begin frame
        egui_context.begin_frame(raw_input);
        // do the egui stuff
        Window::new("egui user window").show(egui_context, |ui| {
            ui.label("hello");
            if window_backend.window.is_mouse_passthrough() {
                ui.label("passthrough enabled. mouse position: ");
                ui.label(format!("{:#?}", window_backend.cursor_pos_physical_pixels));
            }
        });
        // end frame
        let output = egui_context.end_frame();
        let window = window_backend.get_window().unwrap();
        if egui_context.wants_pointer_input() || egui_context.wants_keyboard_input() {
            window.set_mouse_passthrough(false);
        } else {
            window.set_mouse_passthrough(true);
        }
        // do something custom like accesskit stuff using fulloutput before forwarding it back to window backend.
        output
    }
}
