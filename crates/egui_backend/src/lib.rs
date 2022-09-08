//! `egui_backend` crate primarily provides traits to abstract away Window and Rendering parts of egui backends.
//! this allows us to use any compatible (see `WARNING` below) window backend with any render backend crate.
//!
//! this crate can provides 4 traits:
//! 1. `WindowBackend`: implemented by window backends
//! 2. `OpenGLWindowContext` : specific to Native openGL. Window Backends which support OpengGL can implement this trait
//! 3. `GfxBackend<W: WindowBackend>`: implemented by rendering backends for particular or any window backends
//! 4. `UserApp<W: WindowBackend, G: GfxBackend>`: implemented by egui users for a particular combination or any combination of Window or Gfx Backends
//!
//! look at the docs of the relevant trait to learn more.
//!
//! WARNING:
//! the primary goal was to separate window and rendering completely.
//! It would work for modern graphics api backends like vulkan, metal, dx12 etc..
//! but for opengl, window parts are often mixed with opengl parts.
//! for example, opengl needs functions like `swap_buffers`, `make_context_current` or `get_proc_address` which are provided
//! by the window crates like sdl2 / glfw / glutin. this is made complicated by multi-threading or the fact that opengl context
//! is often created with a window rather than a separate api etc..
//!
//! If we remove support for OpenGL, we can simplify this crate A LOT.
//!
//! TODO: remove OpenGL into a separate crate / trait once webgpu spec is stable
//!
//! <https://developer.chrome.com/en/docs/web-platform/webgpu/> origin trials of webgpu in chrome ends on 1st Feb, 2023.
//!

pub use egui;
pub use raw_window_handle;

mod opengl;
use egui::{ClippedPrimitive, RawInput, TexturesDelta};
pub use opengl::*;
use raw_window_handle::HasRawWindowHandle;

/// Intended to provide a common struct which all window backends accept as their configuration.
/// a lot of the settings are `Option<T>`, so that users can let the window backends choose defaults when user doesn't care.
///
/// After the creation of a window, the backend must set all the options to `Some(T)`. for example, if the user set the
/// srgb field of opengl options to `None`, and the backend must set that field to `Some(true)` or `Some(false)` so that the
/// renderer can know whether the framebuffer (surface) supports srgb or not.
#[derive(Debug, Clone, Default)]
pub struct BackendSettings {
    /// The kind of graphics api that we plan to use the window with
    pub gfx_api_type: GfxApiType,
}
/// Different kinds of gfx APIs and their relevant settings.
#[derive(Debug, Clone)]
pub enum GfxApiType {
    /// when we want toe window to decide.
    /// usually, this means that we don't want a opengl window and the renderer will choose the right api (vk/dx/mtl etc..)
    NoApi,
    /// specifically request vulkan api. just like `NoApi`, it will avoid creation of an opengl context with the window.
    /// people might choose vulkan specifically in certain situations (like transparent framebuffer)
    #[cfg(not(target_arch = "wasm32"))]
    Vulkan,
    /// Tell the window backend to create an OpenGL Window
    /// lots of settings to choose from :)
    #[cfg(not(target_arch = "wasm32"))]
    OpenGL {
        /// contains all the settings that are usually provided by a OpenGL window creation library
        native_config: NativeGlConfig,
    },
    /// intended for WebGL2 + winit combinations. can be used by either wgpu or glow.
    /// until webgpu is available in beta / stable, this is the only api available on web
    #[cfg(target_arch = "wasm32")]
    WebGL2 {
        /// only tested on winit atm
        /// if this is None, window backend will create a canvas and add it to DOM's body
        /// if this is Some(id), we will get the canvas element with this id and use it as the window's backing canvas.
        canvas_id: Option<String>,
        /// settings to use during context creation from a canvas element.
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
///
/// This trait is generic over the WindowBackend because some renderers might want to only work for a specific
/// window backend.
///
/// for example, an sdl2 renderer might only want to work with a specific sdl2 window backend. and
/// another person might want to make a different sdl2 renderer, and can reuse the old sdl2 window backend.
///
///
pub trait GfxBackend<W: WindowBackend> {
    /// similar to WindowBakendSettings. just make them as complicated or as simple as you want.
    type Configuration: Default;

    /// create a new GfxBackend using info from window backend and custom settings struct
    /// `WindowBackend` trait provides the backend settings, which can be used by the renderer to check
    /// for compatibility.
    ///
    /// for example, a glow renderer might want an opengl context. but if the window was created without one,
    /// the glow renderer should panic.
    fn new(window_backend: &mut W, settings: Self::Configuration) -> Self;

    /// prepare the surface / swapchain etc.. for rendering. this should be called right after
    /// WindowBackend has finished processing events so that the Gfx backend could resize the framebuffer
    /// if there's any resize event.
    ///
    /// if the framebuffer needs to resize due to a resize event or scale change event, the bool would be true.
    ///
    /// the gfx backend can get the latest size (in physical pixels) using `WindowBackend::get_live_physical_size_framebuffer` fn.
    fn prepare_frame(&mut self, framebuffer_needs_resize: bool, window_backend: &mut W);

    /// reference : <https://github.com/gfx-rs/wgpu/wiki/Encapsulating-Graphics-Work>
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
    ///
    /// Basically, this is the stage where renderers can sort meshes, upload data / textures, prepare bindgroups, create mipmaps and other such tasks to prepare for the rendering stage.
    fn prepare_render(&mut self, egui_gfx_output: EguiGfxOutput);

    /// This is where the renderers will start creating renderpasses, issue draw calls etc.. using the data previously prepared.
    ///
    fn render(&mut self);

    /// This is called at the end of the frame. after everything is drawn, you can now present
    /// on opengl, you might call `WindowBackend::swap_buffers`.
    /// on wgpu / vulkan, you might submit commands to queues, present swapchain image etc..
    fn present(&mut self, window_backend: &mut W);
}

/// This is the trait most users care about. just implement this trait and you can use any `WindowBackend` or `GfxBackend` to run your egui app.
///
/// First, if you don't particular care about the window or gfx backends used to run your app, you can just use a generic impl
/// ```rust
/// pub struct App;
/// impl<W: WindowBackend, G: GfxBackend<W>> UserApp<W, G> for App {
///     fn run(&mut self, egui_context: &egui::Context, window_backend: &mut W, gfx_backend: &mut G) {
///         egui::Window::new("New Window").show(egui_context, |ui| {
///             ui.label("hello label");
///         });
///     }    
/// }
/// ```
///
/// Second, if you want to use functionality from a particular Backend like drawing with wgpu, use specific generic types on your impl.
/// ```rust
/// pub struct App;
/// impl UserApp<WinitBackend, WgpuBackend> for App {
///     fn run(&mut self, egui_context: &egui::Context, window_backend: &mut WinitBackend, gfx_backend: &mut WgpuBackend) {
///         egui::Window::new("New Window").show(egui_context, |ui| {
///             ui.label("hello label");
///         });
///         /* do something with window_backend or gfx_backend */
///         // most of the data is public in both of those backends so that user can see and understand exactly what's going on.
///     }    
/// }
/// ```
///
/// we might add more functions to this trait in future which will be called between specific functions.
/// like `pre_render` which will be called after `GfxBackend::pre_render` but before `GfxBackend::render`.
/// or `post_render` which will be called after `GfxBackend::render` but before `GfxBackend::present` etc..
///
/// it will all depend on the demands of users and backend implementors who might need more flexibility
pub trait UserApp<W: WindowBackend, G: GfxBackend<W>>: Sized {
    fn run(&mut self, egui_context: &egui::Context, window_backend: &mut W, gfx_backend: &mut G);
}
