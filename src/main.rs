use egui_backend::{BackendConfig, WindowBackend};
use egui_window_winit::WinitBackend;
use etk::fake_main;
type WB = WinitBackend;
#[cfg(feature = "passthrough")]
use egui_window_glfw_passthrough::GlfwBackend;

fn main() {
    console_error_panic_hook::set_once();
    // #[cfg(target = "wasm32-unknown-unknown")]
    tracing_wasm::set_as_global_default();
    #[cfg(not(feature = "passthrough"))]
    let window_backend = WB::new(Default::default(), BackendConfig::default());
    #[cfg(feature = "passthrough")]
    let window_backend = GlfwBackend::new(
        egui_window_glfw_passthrough::GlfwConfig {
            glfw_callback: Some(Box::new(|gtx| {
                gtx.window_hint(
                    egui_window_glfw_passthrough::glfw::WindowHint::TransparentFramebuffer(true),
                );
            })),
            window_callback: None,
        },
        Default::default(),
    );
    fake_main(window_backend);
}
