mod glow_app;

fn main() {
    glow_app::fake_main::<egui_window_winit::WinitBackend>();
}
