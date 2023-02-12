mod input;
mod mouse;
mod touch;
pub use input::*;
// use raw_window_handle::RawWindowHandle;
pub use mouse::*;
pub use touch::*;
pub struct WindowInput {
    /// in points.
    pub window_size: [f32; 2],
    /// number of points per pixel. multiplying window size with scale gives framebuffer_size
    pub scale: f32,
    /// in pixels
    pub framebuffer_size: [u32; 2],
    /// in point space
    pub cursor_position: [f32; 2],
}

