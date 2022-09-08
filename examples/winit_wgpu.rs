mod wgpu_app;
fn main() {
    wgpu_app::fake_main::<egui_window_winit::WinitBackend>();
}
