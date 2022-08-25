pub use egui;
pub use raw_window_handle;

mod opengl;
use egui::{ClippedPrimitive, RawInput, TexturesDelta};
pub use opengl::*;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};

/// This configuration can be used by both Gfx and Window backends to set the right configuration
/// OpenGL obviously has many more settings than vulkan as a lot of Window backends set these
/// attributes at the time of window creation.
///
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum GfxApiConfig {
    OpenGL {
        version: Option<(u8, u8)>,
        samples: Option<u8>,
        srgb: Option<bool>,
        transparent: Option<bool>,
        debug: Option<bool>,
    },
    Vulkan {},
}

impl Default for GfxApiConfig {
    fn default() -> Self {
        Self::OpenGL {
            version: None,
            samples: None,
            srgb: None,
            transparent: None,
            debug: None,
        }
    }
}

/// This is the output from egui that renderer needs.
/// meshes and textures_delta come from egui directly. but
/// window backend needs to also provide screensize in logical coords, scale and physical framebuffer
/// size in pixels.
///
pub struct EguiGfxOutput {
    /// from output of `Context::end_frame()`
    pub meshes: Vec<ClippedPrimitive>,
    /// from output of `Context::end_frame()`
    pub textures_delta: TexturesDelta,
    /// this is what you provided to `RawInput` for `Context::begin_frame()`
    /// * used for screen_size uniform in shaders
    pub screen_size_logical: [f32; 2],
    /// size in pixels of the current swapchain image (surface or viewport etc..) that we are rendering to.
    /// * used to calculate scissor regions (clip rectangles)
    pub framebuffer_size_physical: [u32; 2],
    /// scale (pixels_per_point) that was used in `RawInput`
    /// * used to calculate scissor regions (clip rectangles)
    pub scale: f32,
}

/// The main rason to have this is to transfer the RawWindowHandle from WindowBackend to GfxBackend.
/// But this is even more important because of the opengl_context. most window backends create the
/// opengl context along with the window (glfw / sdl2) and we need some functions like SwapBuffers
/// or MakeCurrent etc.. for rendering needs.
///
/// This primarily allows for separation of Gfx and Window functions. Modern APIs like vulkan or Wgpu
/// already deal with surface creation or swapping or multi-threading etc.. themselves.
///
///
pub struct WindowInfoForGfx {
    pub gfx_api_config: GfxApiConfig,
    pub window_handle: RawWindowHandle,
    pub opengl_context: Option<Box<dyn OpenGLWindowContext>>,
}
unsafe impl HasRawWindowHandle for WindowInfoForGfx {
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.window_handle
    }
}

/// Implement this trait for your windowing backend. the main responsibility of a
/// Windowing Backend is to
/// 1. poll and gather events
/// 2. convert events to egui raw input and give it to egui context's begin_frame
/// 3. provide framebuffer resize (optional) details to Gfx Backend when preparing the frame (surface / swapchain etc..)
/// 4. run event loop and call the necessary functions of Gfx and UserApp
pub trait WindowBackend: Sized {
    /// This will be WindowBackend's configuration. if necessary, just add Boxed closures as its
    /// fields and run them before window creation, after window creation etc.. to provide maximum
    /// configurability to users
    type Configuration;

    /// Create a new window backend. and return info needed for the GfxBackend creation and rendering
    fn new(config: Self::Configuration, gfx_api_config: GfxApiConfig) -> (Self, WindowInfoForGfx)
    where
        Self: Sized;
    /// This frame's events gather into rawinput and to be presented to egui's context
    fn take_raw_input(&mut self) -> RawInput;
    /// return Some(size) if there's been a framebuffer resize event recently. once we take it, it should
    /// return None until there's a fresh resize event.
    fn take_latest_size_update(&mut self) -> Option<[u32; 2]>;
    /// Run the event loop. different backends run it differently, so they all need to take care and
    /// call the Gfx or UserApp functions at the right time.
    fn run_event_loop<G: GfxBackend + 'static, U: UserApp<Self, G> + 'static>(
        self,
        gfx_backend: G,
        user_app: U,
    );
    fn get_live_physical_size_framebuffer(&mut self) -> [u32; 2];
}

/// This is the trait to implement for Gfx backends. these could be Gfx APIs like opengl or vulkan or wgpu etc..
/// or higher level renderers like three-d or rend3 or custom renderers etc..
///
///
pub trait GfxBackend {
    /// similar to WindowBakendSettings. just make them as complicated or as simple as you want.
    type GfxBackendSettings;

    /// create a new GfxBackend using info from window backend and custom settings struct
    fn new(window_info_for_gfx: WindowInfoForGfx, settings: Self::GfxBackendSettings) -> Self;
    /// prepare the surface / swapchain etc.. for rendering. this should be called right after
    /// WindowBackend has finished processing events so that the Gfx backend could resize
    /// if there's any resize event.
    fn prepare_frame<W: WindowBackend>(
        &mut self,
        framebuffer_size_update: Option<[u32; 2]>,
        window_backend: &mut W,
    );

    /// reference : https://github.com/gfx-rs/wgpu/wiki/Encapsulating-Graphics-Work
    /// specific quote about submitting commands
    /// > use a fewest possible render passes as possible...
    /// > Middleware should not call queue.submit unless absolutely necessary. It is an extremely expensive function and should only be called once per frame.
    /// > If the middleware generates a CommandBuffer, hand that buffer back to the user to submit themselves.
    /// and then we have the reason to "prepare render":
    /// > ... Because render passes need all data they use to last as long as they do, all resources that are going to be used in the render pass need to be created ahead of time.
    ///
    /// The main intent though is that this trait will be implemented by both "low level gfx apis" like wgpu or opengl or vulkan etc.. as well as high level renderers.
    /// the renderers might collect all kinds of commands from `UserApp`'s run function where the user might want to draw things like shapes or objects like circles etc...
    /// the renderers could have just collected them in random order, but decide to batch them up by bindgroups like textures (sprites) or by shaders (shapes / lighting) etc..
    /// this function can be used to do that kind of stuff.
    /// we submit the egui data at this point so the renderer can upload to buffers or prepare bindgroups / uniforms etc...
    fn prepare_render(&mut self, egui_gfx_output: EguiGfxOutput);

    fn render(&mut self);

    /// This is called at the end of the frame. after everything is drawn, you can now present
    /// the frame (swap buffers).
    fn present(&mut self);
}

/// implement this trait for your struct and just use any Window or Gfx backends you want.
pub trait UserApp<W: WindowBackend, G: GfxBackend>: Sized {
    fn run(&mut self, egui_context: &egui::Context, window_backend: &mut W, gfx_backend: &mut G);
}
