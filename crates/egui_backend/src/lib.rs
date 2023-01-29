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

use std::time::Duration;

pub use egui;
pub use raw_window_handle;

use egui::{ClippedPrimitive, FullOutput, PlatformOutput, RawInput, TexturesDelta};
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
        #[cfg(target_arch = "wasm32")]
        return Self::GL;
        #[cfg(not(target_arch = "wasm32"))]
        return Self::NoApi;
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
    /// sometimes, the frame buffer size might have changed and the resize event is still not received.
    /// in those cases, wgpu / vulkan like render apis will throw an error if you try to acquire swapchain
    /// image with an outdated size. you will need to provide the *latest* size for succesful creation of surface frame.
    /// if the return value is `None`, the window doesn't exist yet. eg: on android, after suspend but before resume event.
    fn get_live_physical_size_framebuffer(&mut self) -> Option<[u32; 2]>;
    fn get_raw_input(&mut self) -> RawInput;
    /// Run the event loop. different backends run it differently, so they all need to take care and
    /// call the Gfx or UserApp functions at the right time.
    fn run_event_loop<U: EguiUserApp<Self> + 'static>(self, user_app: U);
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
pub trait GfxBackend {
    /// similar to WindowBakendConfig. just make them as complicated or as simple as you want.
    type Configuration: Default;

    /// create a new GfxBackend using info from window backend and custom config struct
    /// `WindowBackend` trait provides the backend config, which can be used by the renderer to check
    /// for compatibility.
    ///
    /// for example, a glow renderer might want an opengl context. but if the window was created without one,
    /// the glow renderer should panic.
    fn new(window_backend: &mut impl WindowBackend, config: Self::Configuration) -> Self;

    /// Android only. callend on app suspension, which destroys the window.
    /// so, will need to destroy the `Surface` and recreate during resume event.
    fn suspend(&mut self, _window_backend: &mut impl WindowBackend) {
        unimplemented!("This window backend doesn't implement suspend event");
    }
    /// Android Only. called when app is resumed after suspension.
    /// On Android, window can only be created on resume event. so, you cannot create a `Surface` before entering the event loop.
    /// when this fn is called, we can create a new surface (swapchain) for the window.
    /// doesn't apply on other platforms.
    fn resume(&mut self, _window_backend: &mut impl WindowBackend) {}
    /// prepare the surface / swapchain etc.. by acquiring an image for the current frame.
    /// use `WindowBackend::get_live_physical_size_framebuffer` fn to resize your swapchain if it is out of date.
    fn prepare_frame(&mut self, window_backend: &mut impl WindowBackend);

    /// This is where the renderers will start creating renderpasses, issue draw calls etc.. using the data previously prepared.
    fn render_egui(
        &mut self,
        meshes: Vec<ClippedPrimitive>,
        textures_delta: TexturesDelta,
        logical_screen_size: [f32; 2],
    );

    /// This is called at the end of the frame. after everything is drawn, you can now present
    /// on opengl, renderer might call `WindowBackend::swap_buffers`.
    /// on wgpu / vulkan, renderer might submit commands to queues, present swapchain image etc..
    fn present(&mut self, window_backend: &mut impl WindowBackend);
    /// called if framebuffer has been resized. use this to reconfigure your swapchain/surface/viewport..
    fn resize_framebuffer(&mut self, window_backend: &mut impl WindowBackend);
}

/// This is the trait most users care about. we already have a bunch of default implementations. override them for more advanced usage.
pub trait EguiUserApp<WB: WindowBackend> {
    type UserGfxBackend: GfxBackend;

    fn get_gfx_backend(&mut self) -> &mut Self::UserGfxBackend;
    fn get_egui_context(&mut self) -> egui::Context;
    fn resize_framebuffer(&mut self, window_backend: &mut WB) {
        self.get_gfx_backend().resize_framebuffer(window_backend);
    }
    fn resume(&mut self, window_backend: &mut WB) {
        self.get_gfx_backend().resume(window_backend);
    }
    fn suspend(&mut self, window_backend: &mut WB) {
        self.get_gfx_backend().suspend(window_backend);
    }
    fn run(
        &mut self,
        logical_size: [f32; 2],
        window_backend: &mut WB,
    ) -> Option<(PlatformOutput, Duration)> {
        // don't bother doing anything if there's no window
        if window_backend.get_window().is_some() {
            let egui_context = self.get_egui_context();
            let input = window_backend.get_raw_input();
            self.get_gfx_backend().prepare_frame(window_backend);
            egui_context.begin_frame(input);
            self.gui_run(&egui_context, window_backend);
            let FullOutput {
                platform_output,
                repaint_after,
                textures_delta,
                shapes,
            } = egui_context.end_frame();
            self.get_gfx_backend().render_egui(
                egui_context.tessellate(shapes),
                textures_delta,
                logical_size,
            );
            self.get_gfx_backend().present(window_backend);
            return Some((platform_output, repaint_after));
        }
        None
    }
    /// This is the only function user needs to implement. this function will be called every frame.
    fn gui_run(&mut self, egui_context: &egui::Context, window_backend: &mut WB);
}

/// Some nice util functions commonly used by egui backends.
pub mod util {
    /// input: clip rectangle, scale and framebuffer size in physical pixels
    /// we will get [x, y, width, height] of the scissor rectangle.
    ///
    /// internally, it will
    /// 1. multiply clip rect and scale  to convert the logical rectangle to a physical rectangle in framebuffer space.
    /// 2. clamp the rectangle between 0..width and 0..height of the frambuffer.
    /// 3. round the rectangle into the nearest u32 (within the above frambuffer bounds ofc)
    /// 4. return Some only if width/height of scissor region are not zero.
    pub fn scissor_from_clip_rect(
        clip_rect: &egui::Rect,
        scale: f32,
        physical_framebuffer_size: [u32; 2],
    ) -> Option<[u32; 4]> {
        // copy paste from official egui impl because i have no idea what this is :D
        let clip_min_x = scale * clip_rect.min.x;
        let clip_min_y = scale * clip_rect.min.y;
        let clip_max_x = scale * clip_rect.max.x;
        let clip_max_y = scale * clip_rect.max.y;
        let clip_min_x = clip_min_x.clamp(0.0, physical_framebuffer_size[0] as f32);
        let clip_min_y = clip_min_y.clamp(0.0, physical_framebuffer_size[1] as f32);
        let clip_max_x = clip_max_x.clamp(clip_min_x, physical_framebuffer_size[0] as f32);
        let clip_max_y = clip_max_y.clamp(clip_min_y, physical_framebuffer_size[1] as f32);

        let clip_min_x = clip_min_x.round() as u32;
        let clip_min_y = clip_min_y.round() as u32;
        let clip_max_x = clip_max_x.round() as u32;
        let clip_max_y = clip_max_y.round() as u32;

        let width = (clip_max_x - clip_min_x).max(1);
        let height = (clip_max_y - clip_min_y).max(1);

        // Clip scissor rectangle to target size.
        let clip_x = clip_min_x.min(physical_framebuffer_size[0]);
        let clip_y = clip_min_y.min(physical_framebuffer_size[1]);
        let clip_width = width.min(physical_framebuffer_size[0] - clip_x);
        let clip_height = height.min(physical_framebuffer_size[1] - clip_y);
        // return none if scissor width/height are zero
        (clip_height != 0 && clip_width != 0).then_some([clip_x, clip_y, clip_width, clip_height])
    }
}
