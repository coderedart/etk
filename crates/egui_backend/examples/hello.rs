mod glow_app;
mod three_d_app;
mod wgpu_app;

#[cfg(feature = "glow")]
use glow_app::fake_main;

#[cfg(feature = "wgpu")]
use wgpu_app::fake_main;

#[cfg(feature = "three-d")]
use three_d_app::fake_main;

#[cfg(feature = "winit")]
type WB = egui_backend::window_backends::winit_backend::WinitBackend;
#[cfg(feature = "glfw")]
type WB = egui_backend::window_backends::glfw_backend::GlfwBackend;
#[cfg(feature = "sdl2")]
type WB = egui_backend::window_backends::sdl2_backend::Sdl2Backend;

fn main() {
    #[cfg(any(feature = "glow", feature = "wgpu", feature = "three-d"))]
    fake_main::<WB>();
}
