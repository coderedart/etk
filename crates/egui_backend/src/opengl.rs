/// This is something transferred from window backend to gfx backends.
/// making sure that renderers can decide everything like when to swap buffers as well as multi-threading etc..
///
///
pub trait OpenGLWindowContext {
    /// Swaps buffers (swapchain) when we are using double buffering (99% of the time, double buffering is the default)
    /// this also flushes the opengl commands and blocks until the swapchain image is presented.
    fn swap_buffers(&mut self);
    /// for single threading, we should only call this once at the creation of our renderer. as we will only
    /// use main thread most of the time
    fn make_context_current(&mut self);
    /// check if this is current. mostly useless if we just use single main thread.
    fn is_current(&mut self) -> bool;
    /// get openGL function addresses.
    fn get_proc_address(&mut self, symbol: &str) -> *const core::ffi::c_void;
}

/// OpenGL Profile
/// most of the time, we would just use Core
#[derive(Debug, Default, Clone, Copy)]
pub enum GlProfile {
    #[default]
    Core,
    Compat,
}
