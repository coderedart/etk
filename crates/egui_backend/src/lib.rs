//! `egui_backend` crate primarily provides traits to abstract away Window and Rendering parts of egui backends.
//! this allows us to use any compatible window backend with any gfx backend crate.
//!
//! egui is an immediate mode gui library. The lifecycle of egui in every frame goes like this:
//! 1. takes input from the window backend. eg: mouse position, keyboard events, resize..
//! 2. constructs gui objects like windows / panels / buttons etc..
//! 3. outputs gpu friendly data to be drawn by a gfx backend.
//!
//! So, we need a WindowBackend to provide input to egui and a GfxBackend to draw egui's output.
//! egui project already provides a crate called `eframe` for this pupose using `winit` on desktop, custom backend on web and `wgpu`/`glow` for rendering.
//! But it exposes a very limited api.
//! `egui_backend` crate instead is to enable separation of window + gfx concerns using traits.
//! This allows someone to only work on winit backend, and leave gfx backend work to someone else. And because of these common traits,
//! they will all work without having to write any specific glue code.
//!
//! this crate provides 4 traits:
//! 1. `WindowBackend`: implemented by window backends like winit, glfw, sdl2 etc..
//! 2. `GfxBackend<W: WindowBackend>`: implemented by rendering backends for particular or any window backends
//! 3. `UserApp<W: WindowBackend, G: GfxBackend>`: implemented by egui users for a particular combination or any combination of Window / Gfx Backends
//!
//! look at the docs of the relevant trait to learn more.
//!
//! reminder: https://developer.chrome.com/en/docs/web-platform/webgpu/ origin trials of webgpu in chrome ends on 1st Feb, 2023.

pub use egui;
pub use raw_window_handle;

use egui::{ClippedPrimitive, RawInput, TexturesDelta};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

/// Intended to provide a common struct which all window backends accept as their configuration.
/// In future, might add more options like initial window size/title etc..
#[derive(Debug, Clone, Default)]
pub struct BackendConfig {
    /// The kind of graphics api that we plan to use the window with
    pub gfx_api_type: GfxApiType,
}
/// Gfx Apis like Opengl (Gl-es) require some special config while creating a window.
/// OTOH, modern APIs like metal/vk/dx deal with configuration themselves after creating a window.
/// So, we need to tell window backend to choose whether we want a Gl or Non-GL kinda variant.
#[derive(Debug, Clone)]
pub enum GfxApiType {
    /// when we want the gfx backend to decide the api.
    /// usually, this means that we don't want a opengl window and the renderer will choose the right api (vk/dx/mtl etc..)
    NoApi,
    /// This means that we require a GL api.
    /// on glfw/sdl2, it means they will create the necessary opengl contexts and make them current.
    /// the renderer will use the functions `get_proc_address` or `swap_buffers`.
    GL,
}

impl Default for GfxApiType {
    fn default() -> Self {
        #[cfg(target = "wasm32-unknown-unknown")]
        return Self::GL;
        #[cfg(not(target = "wasm32-unknown-unknown"))]
        return Self::NoApi;
    }
}

/// This is the output from egui that renderer needs.
/// meshes and textures_delta come from egui directly.
/// window backend needs to also provide screensize in logical coords, scale and physical framebuffer
/// size in pixels.
pub struct EguiGfxData {
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
pub trait WindowBackend: Sized {
    /// This will be WindowBackend's configuration. if necessary, just add Boxed closures as its
    /// fields and run them before window creation, after window creation etc.. to provide maximum
    /// configurability to users
    type Configuration: Default + Sized;
    type WindowType: HasRawDisplayHandle + HasRawWindowHandle + Sized;
    /// Create a new window backend. and return info needed for the GfxBackend creation and rendering
    /// config is the custom configuration of a specific window backend
    fn new(config: Self::Configuration, backend_config: BackendConfig) -> Self;
    /// This frame's events gather into rawinput and to be presented to egui's context
    fn take_raw_input(&mut self) -> RawInput;
    /// This gives us the "Window" struct of this particular backend. should implement raw window handle apis.
    /// if this is None, it means window hasn't been created, or has been destroyed for some reason.
    /// usually on android, this means the app is suspended.
    fn get_window(&mut self) -> Option<&mut Self::WindowType>;
    /// sometimes, the frame buffer size might have changed and the event is still not received.
    /// in those cases, wgpu / vulkan like render apis will throw an error if you try to acquire swapchain
    /// image with an outdated size. you will need to provide the *latest* size for succesful creation of surface frame.
    /// if the return value is `None`, the window doesn't exist yet. eg: on android, after suspend but before resume event.
    fn get_live_physical_size_framebuffer(&mut self) -> Option<[u32; 2]>;

    /// Run the event loop. different backends run it differently, so they all need to take care and
    /// call the Gfx or UserApp functions at the right time.
    fn run_event_loop<G: GfxBackend<Self> + 'static, U: UserAppData<Self, G> + 'static>(
        self,
        gfx_backend: G,
        user_app: U,
    );
    /// config if GfxBackend needs them. usually tells the GfxBackend whether we have an opengl or non-opengl window.
    /// for example, if a vulkan backend gets a window with opengl, it can gracefully panic instead of probably segfaulting.
    /// this also serves as an indicator for opengl gfx backends, on whether this backend supports `swap_buffers` or `get_proc_address` functions.
    fn get_config(&self) -> &BackendConfig;
    /// optional. only implemented by gl windowing libraries like glfw/sdl2 which hold the gl context with Window
    /// gfx backends like glow (or raw opengl) will call this if needed.
    /// panic! if your WindowBackend doesn't implemented this functionality (eg: winit)
    fn swap_buffers(&mut self) {
        unimplemented!("swap buffers is not implemented for this window backend");
    }
    /// get openGL function addresses. optional, just like `Self::swap_buffers`.
    /// panic! if it doesn't apply to your WindowBackend. eg: winit.
    fn get_proc_address(&mut self, symbol: &str) -> *const core::ffi::c_void {
        unimplemented!(
            "get_proc_address is not implemented for this window backend. called with {symbol}"
        );
    }
}

/// Trait for Gfx backends. these could be Gfx APIs like opengl or vulkan or wgpu etc..
/// or higher level renderers like three-d or rend3 or custom renderers etc..
///
/// This trait is generic over the WindowBackend because some renderers might want to only work for a specific
/// window backend.
///
/// for example, an sdl2_gfx renderer might only want to work with a specific sdl2 window backend. and
/// another person might want to make a different sdl2 renderer, and can reuse the old sdl2 window backend.
pub trait GfxBackend<W: WindowBackend> {
    /// similar to WindowBakendConfig. just make them as complicated or as simple as you want.
    type Configuration: Default;

    /// create a new GfxBackend using info from window backend and custom config struct
    /// `WindowBackend` trait provides the backend config, which can be used by the renderer to check
    /// for compatibility.
    ///
    /// for example, a glow renderer might want an opengl context. but if the window was created without one,
    /// the glow renderer should panic.
    fn new(window_backend: &mut W, config: Self::Configuration) -> Self;

    /// Android only. callend on app suspension, which destroys the window.
    /// so, will need to destroy the `Surface` and recreate during resume event.
    fn suspend(&mut self, _window_backend: &mut W) {
        unimplemented!("This window backend doesn't implement suspend event");
    }
    /// Android Only. called when app is resumed after suspension.
    /// On Android, window can only be created on resume event. so, you cannot create a `Surface` before entering the event loop.
    /// We can now create a new surface (swapchain) for the window.
    /// on other platforms, it **may** be called once at startup after entering eventloop, but we can ignore it.
    fn resume(&mut self, _window_backend: &mut W) {}
    /// prepare the surface / swapchain etc.. by acquiring an image for the current frame.
    /// `framebuffer_needs_resize` indicates a window resize.
    /// use `WindowBackend::get_live_physical_size_framebuffer` fn to resize your swapchain.
    fn prepare_frame(&mut self, framebuffer_needs_resize: bool, window_backend: &mut W);

    /// This is where the renderers will start creating renderpasses, issue draw calls etc.. using the data previously prepared.
    fn render(&mut self, egui_gfx_data: EguiGfxData);

    /// This is called at the end of the frame. after everything is drawn, you can now present
    /// on opengl, you might call `WindowBackend::swap_buffers`.
    /// on wgpu / vulkan, you might submit commands to queues, present swapchain image etc..
    fn present(&mut self, window_backend: &mut W);
}

/// This is the trait most users care about. just implement this trait and you can use any `WindowBackend` or `GfxBackend` to run your egui app.
///
/// if you don't particular care about the window or gfx backends used to run your app, you can just use a generic impl
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
/// if you want to use functionality from a particular Backend like drawing with wgpu, use specific generic types on your impl.
/// ```rust
/// pub struct App;
/// impl UserApp<WinitBackend, WgpuBackend> for App {
///     fn run(&mut self, egui_context: &egui::Context, window_backend: &mut WinitBackend, gfx_backend: &mut WgpuBackend) {
///         /* do something with winit or wgpu */
///     }    
/// }
/// ```
///
/// we might add more functions to this trait in future which will be called between specific functions.
/// like `pre_render` which will be called after `GfxBackend::pre_render` but before `GfxBackend::render`.
/// or `post_render` which will be called after `GfxBackend::render` but before `GfxBackend::present` etc..
///
/// it will all depend on the demands of users and backend implementors who might need more flexibility
pub trait UserAppData<W: WindowBackend, G: GfxBackend<W>> {
    /// This function is provided a
    /// 1. mutable reference to the data/struct which this is implemented for
    /// 2. egui context.
    /// 3. raw_input. use the raw input to start a frame in egui context and draw your gui stuff
    /// 4. window backend. in case you want something like window size or whatever
    /// 5. gfx backend. this is what you use to draw stuff or fill up some data buffers/textures before using them during rendering callbacks etc..
    ///
    /// and this function returns the egui fulloutput which you will get by ending the frame in egui context.
    ///
    /// you can use the rawinput to get events like cursor movement, button presses/releases, keyboard key press/releases, window resize events etc.
    /// and you can filter them out too. like only restricting egui to left half of your window by modifying the resize event before starting egui context.
    /// you can also use the fulloutput to add accesskit or other useful features without support from windowing/gfx backends.
    fn run(
        &mut self,
        egui_context: &egui::Context,
        raw_input: egui::RawInput,
        window_backend: &mut W,
        gfx_backend: &mut G,
    ) -> egui::FullOutput;
}
