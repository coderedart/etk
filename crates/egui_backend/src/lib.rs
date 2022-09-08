//! `egui_backend` crate primarily provides traits to abstract away Window and Rendering parts of egui backends.
//! this allows us to use any compatible (see `WARNING` below) window backend with any render backend crate.
//!
//! this crate can provides 4 traits:
//! 1. `WindowBackend`: implemented by window backends
//! 2. `OpenGLWindowContext` : specific to openGL.
//! 3. `GfxBackend<W: WindowBackend>`: implemented by rendering backends for particular or any window backends
//! 4. `UserApp<W: WindowBackend, G: GfxBackend>`: implemented by egui users for a particular combination or any combination of Window or Gfx Backends
//!
//!
//! WARNING:
//! the primary goal was to separate window and rendering completely.
//! It would work for modern graphics api backends like vulkan, metal, dx12 etc..
//! but for opengl, window parts are often mixed with opengl parts.
//! for example, opengl needs functions like `swap_buffers`, `make_context_current` or `get_proc_address` which are provided
//! by the window crates like sdl2 / glfw / glutin. this is made complicated by multi-threading or the fact that opengl context
//! is often created with a window etc..
//!
//! so, this turned out to be slighly more complex than i hoped for.
//!

pub use egui;
pub use raw_window_handle;

mod opengl;
use egui::{ClippedPrimitive, RawInput, TexturesDelta};
pub use opengl::*;
use raw_window_handle::HasRawWindowHandle;

/// This configuration can be used by both Gfx and Window backends to set the right configuration
/// OpenGL obviously has many more settings than vulkan as a lot of Window backends set these
/// attributes at the time of window creation.
///
#[derive(Debug, Clone, Default)]
pub struct BackendSettings {
    pub gfx_api_type: GfxApiType,
}

#[derive(Debug, Clone)]
pub enum GfxApiType {
    NoApi,
    Vulkan,
    #[cfg(not(target_arch = "wasm32"))]
    OpenGL {
        native_config: NativeGlConfig,
    },
    #[cfg(target_arch = "wasm32")]
    WebGL2 {
        /// only tested on winit atm
        /// if this is None, window backend will create a canvas and add it to DOM's body
        /// if this is Some(id), we will get the canvas element with this id and use it as the window's backing canvas.
        canvas_id: Option<String>,
        webgl_config: WebGlConfig,
    },
}
#[cfg(target_arch = "wasm32")]
impl Default for GfxApiType {
    fn default() -> Self {
        Self::WebGL2 {
            canvas_id: Some("egui_winit_canvas".to_string()),
            webgl_config: Default::default(),
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl Default for GfxApiType {
    fn default() -> Self {
        Self::Vulkan
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
}

/// Implement this trait for your windowing backend. the main responsibility of a
/// Windowing Backend is to
/// 1. poll and gather events
/// 2. convert events to egui raw input and give it to egui context's begin_frame
/// 3. provide framebuffer resize (optional) details to Gfx Backend when preparing the frame (surface / swapchain etc..)
/// 4. run event loop and call the necessary functions of Gfx and UserApp
pub trait WindowBackend: Sized + HasRawWindowHandle {
    /// This will be WindowBackend's configuration. if necessary, just add Boxed closures as its
    /// fields and run them before window creation, after window creation etc.. to provide maximum
    /// configurability to users
    type Configuration: Default;

    /// Create a new window backend. and return info needed for the GfxBackend creation and rendering
    /// config is the custom configuration of a specific window backend
    fn new(config: Self::Configuration, backend_settings: BackendSettings) -> Self;
    /// This frame's events gather into rawinput and to be presented to egui's context
    fn take_raw_input(&mut self) -> RawInput;
    /// return Some(size) if there's been a framebuffer resize event recently. once we take it, it should
    /// return None until there's a fresh resize event.
    fn take_latest_size_update(&mut self) -> Option<[u32; 2]>;

    /// sometimes, the frame buffer size might have changed and the event is still not received.
    /// in those cases, wgpu / vulkan like render apis will throw an error if you try to acquire swapchain
    /// image with an outdated size. you will need to provide the *latest* size for succesful creation of surface frame.
    fn get_live_physical_size_framebuffer(&mut self) -> [u32; 2];

    /// Run the event loop. different backends run it differently, so they all need to take care and
    /// call the Gfx or UserApp functions at the right time.
    fn run_event_loop<G: GfxBackend<Self> + 'static, U: UserApp<Self, G> + 'static>(
        self,
        gfx_backend: G,
        user_app: U,
    );

    fn get_settings(&self) -> &BackendSettings;
}

/// This is the trait to implement for Gfx backends. these could be Gfx APIs like opengl or vulkan or wgpu etc..
/// or higher level renderers like three-d or rend3 or custom renderers etc..
pub trait GfxBackend<W: WindowBackend> {
    /// similar to WindowBakendSettings. just make them as complicated or as simple as you want.
    type Configuration: Default;

    /// create a new GfxBackend using info from window backend and custom settings struct
    fn new(window_backend: &mut W, settings: Self::Configuration) -> Self;
    /// prepare the surface / swapchain etc.. for rendering. this should be called right after
    /// WindowBackend has finished processing events so that the Gfx backend could resize
    /// if there's any resize event.
    fn prepare_frame(&mut self, framebuffer_size_update: Option<[u32; 2]>, window_backend: &mut W);

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
    fn present(&mut self, window_backend: &mut W);
}

/// implement this trait for your struct and just use any Window or Gfx backends you want.
pub trait UserApp<W: WindowBackend, G: GfxBackend<W>>: Sized {
    fn run(&mut self, egui_context: &egui::Context, window_backend: &mut W, gfx_backend: &mut G);
}
