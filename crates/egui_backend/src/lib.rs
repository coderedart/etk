//! `egui_backend` crate primarily provides traits to abstract away Window and Rendering parts of egui backends.
//! this allows us to use any window backend with any gfx backend crate.
//!
//! egui is an immediate mode gui library. The lifecycle of egui in every frame goes like this:
//! 1. takes input from the window backend. eg: mouse position, keyboard events, resize..
//! 2. constructs gui objects like windows / panels / buttons etc.. and deals with any input interactions.
//! 3. outputs those gui objects as gpu friendly data to be drawn by a gfx backend.
//!
//! So, we need a WindowBackend to provide input to egui and a GfxBackend to draw egui's output.
//! egui already provides an official backends for wgpu, winit and glow, along with a higher level wrapper crate called `eframe`
//! eframe uses `winit` on desktop, custom backend on web and `wgpu`/`glow` for rendering.
//! If that serves your usecase, then it is recommended to keep using that.
//!
//! `egui_backend` crate instead tries to enable separation of window + gfx concerns using traits.
//!
//! this crate provides 3 traits:
//! 1. [`WindowBackend`]: implemented by window backends like [winit](https://docs.rs/winit), [glfw](https://docs.rs/glfw), [sdl2](https://docs.rs/sdl2) etc..
//! 2. [`GfxBackend`]: implemented by rendering backends like [wgpu](https://docs.rs/wgpu), [glow](https://docs.rs/glow), [three-d](https://docs.rs/three-d),
//! 3. [`EguiUserApp`]: implemented by end user's struct which holds the app data as well as egui context and the renderer.
//!
//! This crate will also try to provide functions or structs which are useful across all backends.
//! 1. [`BackendConfig`]: has some configuration which needs to be provided at startup.
//!
//! look at the docs of the relevant trait to learn more.

// #[cfg(target_feature = "egui")]
pub use egui;
// #[cfg(target_feature = "egui")]
use egui::{ClippedPrimitive, FullOutput, PlatformOutput, RawInput, TexturesDelta};
pub use raw_window_handle;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use std::time::Duration;

/// Intended to provide a common struct which all window backends accept as their configuration.
/// To set size/position/title etc.. just use the windowbackend trait functions after you created the window.
/// This struct is primarily intended for settings which are to be specified *before* creating a window like opengl or transparency etc..
#[derive(Debug, Clone)]
pub struct BackendConfig {
    /// true by default
    pub is_opengl: bool,
    pub opengl_config: Option<OpenGlConfig>,
    pub transparent: Option<bool>,
}

impl Default for BackendConfig {
    fn default() -> Self {
        // let is_opengl = cfg!(target_arch = "wasm32");
        let is_opengl = true;
        Self {
            is_opengl,
            transparent: None,
            opengl_config: Default::default(),
        }
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
    /// Run the event loop. different backends run it differently, so they all need to take care and
    /// call the Gfx or UserApp functions at the right time.
    fn run_event_loop<U: UserApp<UserWindowBackend = Self> + 'static>(user_app: U);
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
    fn is_opengl(&self) -> bool;
    /// get openGL function addresses. optional, just like `Self::swap_buffers`.
    /// panic! if it doesn't apply to your WindowBackend. eg: winit.
    fn get_proc_address(&mut self, symbol: &str) -> *const core::ffi::c_void {
        unimplemented!(
            "get_proc_address is not implemented for this window backend. called with {symbol}"
        );
    }
    fn set_window_title(&mut self, title: &str);
    fn get_window_position(&mut self) -> Option<[f32; 2]>;
    fn set_window_position(&mut self, pos: [f32; 2]);
    fn get_window_size(&mut self) -> Option<[f32; 2]>;
    fn set_window_size(&mut self, size: [f32; 2]);
    fn get_window_minimized(&mut self) -> Option<bool>;
    fn set_minimize_window(&mut self, min: bool);
    fn get_window_maximized(&mut self) -> Option<bool>;
    fn set_maximize_window(&mut self, max: bool);
    fn get_window_visibility(&mut self) -> Option<bool>;
    fn set_window_visibility(&mut self, vis: bool);
    fn get_always_on_top(&mut self) -> Option<bool>;
    fn set_always_on_top(&mut self, always_on_top: bool);
    fn get_passthrough(&mut self) -> Option<bool>;
    fn set_passthrough(&mut self, passthrough: bool);
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
/// We assume that user will provide egui context as well as the gfx backend. This allows user to have maximum control on how they behave.
///
pub trait UserApp {
    ///
    type UserGfxBackend: GfxBackend;
    type UserWindowBackend: WindowBackend;
    /// A shortcut function to get windodw, gfx backends as well as egui context.
    fn get_all(
        &mut self,
    ) -> (
        &mut Self::UserWindowBackend,
        &mut Self::UserGfxBackend,
        &egui::Context,
    );

    fn resize_framebuffer(&mut self) {
        let (wb, gb, _) = self.get_all();
        gb.resize_framebuffer(wb);
    }
    fn resume(&mut self) {
        let (wb, gb, _) = self.get_all();
        gb.resume(wb);
    }
    fn suspend(&mut self) {
        let (wb, gb, _) = self.get_all();
        gb.suspend(wb);
    }
    fn run(&mut self, logical_size: [f32; 2]) -> Option<(PlatformOutput, Duration)> {
        let (wb, gb, egui_context) = self.get_all();
        let egui_context = egui_context.clone();
        // don't bother doing anything if there's no window
        if let Some(full_output) = if wb.get_window().is_some() {
            let input = wb.take_raw_input();
            gb.prepare_frame(wb);
            egui_context.begin_frame(input);
            self.gui_run();
            Some(egui_context.end_frame())
        } else {
            None
        } {
            let FullOutput {
                platform_output,
                repaint_after,
                textures_delta,
                shapes,
            } = full_output;
            let (wb, gb, egui_context) = self.get_all();
            let egui_context = egui_context.clone();

            gb.render_egui(
                egui_context.tessellate(shapes),
                textures_delta,
                logical_size,
            );
            gb.present(wb);
            return Some((platform_output, repaint_after));
        }
        None
    }
    /// This is the only function user needs to implement. this function will be called every frame.
    fn gui_run(&mut self);
}

/// Some nice util functions commonly used by egui backends.
pub mod util {

    /// input: clip rectangle in logical pixels, scale and framebuffer size in physical pixels
    /// we will get [x, y, width, height] of the scissor rectangle.
    ///
    /// internally, it will
    /// 1. multiply clip rect and scale  to convert the logical rectangle to a physical rectangle in framebuffer space.
    /// 2. clamp the rectangle between 0..width and 0..height of the frambuffer. make sure that width/height are positive/zero.
    /// 3. return Some only if width/height of scissor region are not zero.
    ///
    /// This fn is for wgpu/metal/directx.
    /// For opengl, use [`scissor_from_clip_rect_opengl`].
    pub fn scissor_from_clip_rect(
        clip_rect: &egui::Rect,
        scale: f32,
        physical_framebuffer_size: [u32; 2],
    ) -> Option<[u32; 4]> {
        // copy paste from official egui impl because i have no idea what this is :D

        // first, we turn the clip rectangle into physical framebuffer coordinates
        // clip_min is top left point and clip_max is bottom right.
        let clip_min_x = scale * clip_rect.min.x;
        let clip_min_y = scale * clip_rect.min.y;
        let clip_max_x = scale * clip_rect.max.x;
        let clip_max_y = scale * clip_rect.max.y;

        // round to integers
        let clip_min_x = clip_min_x.round() as i32;
        let clip_min_y = clip_min_y.round() as i32;
        let clip_max_x = clip_max_x.round() as i32;
        let clip_max_y = clip_max_y.round() as i32;

        // clamp top_left of clip rect to be within framebuffer bounds
        let clip_min_x = clip_min_x.clamp(0, physical_framebuffer_size[0] as i32);
        let clip_min_y = clip_min_y.clamp(0, physical_framebuffer_size[1] as i32);
        // clamp bottom right of clip rect to be between top_left of clip rect and framebuffer bottom right bounds
        let clip_max_x = clip_max_x.clamp(clip_min_x, physical_framebuffer_size[0] as i32);
        let clip_max_y = clip_max_y.clamp(clip_min_y, physical_framebuffer_size[1] as i32);
        // x,y are simply top left coords
        let x = clip_min_x as u32;
        let y = clip_min_y as u32;
        // width height by subtracting bottom right with top left coords.
        let width = (clip_max_x - clip_min_x) as u32;
        let height = (clip_max_y - clip_min_y) as u32;
        // return only if scissor width/height are not zero. otherwise, no need for a scissor rect at all
        (width != 0 && height != 0).then_some([x, y, width, height])
    }
    /// For wgpu, dx, metal, use [`scissor_from_clip_rect`]..
    ///
    /// **NOTE**:
    /// egui coordinates are in logical window space with top left being [0, 0].
    /// In opengl, bottom left is [0, 0].
    /// so, we need to use bottom left clip-rect coordinate as x,y instead of top left.
    /// 1. bottom left corner's y coordinate is simply top left corner's y added with clip rect height
    /// 2. but this `y` is represents top border + y units. in opengl, we need units from bottom border  
    /// 3. we know that for any point y, distance between top and y + distance between bottom and y gives us total height
    /// 4. so, height - y units from top gives us y units from bottom.
    /// math is suprisingly hard to write down.. just draw it on a paper, it makes sense.
    pub fn scissor_from_clip_rect_opengl(
        clip_rect: &egui::Rect,
        scale: f32,
        physical_framebuffer_size: [u32; 2],
    ) -> Option<[u32; 4]> {
        scissor_from_clip_rect(clip_rect, scale, physical_framebuffer_size).map(|mut arr| {
            arr[1] = physical_framebuffer_size[1] - (arr[1] + arr[3]);
            arr
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct OpenGlConfig {
    /// minimum major opengl version
    /// 2 or 3 is common
    pub major: Option<u8>,
    /// minor version.
    pub minor: Option<u8>,
    /// If we want an ES context
    /// false is default
    pub es: Option<bool>,
    /// try creating srgb surface for window
    pub srgb: Option<bool>,
    /// depth bits
    pub depth: Option<u8>,
    /// stencil bits
    pub stencil: Option<u8>,
    /// The number of bits per each color channel.
    /// default should be rgba with 8 bits each.
    pub color_bits: Option<[u8; 4]>,
    /// Must be a power of 2
    pub multi_samples: Option<u8>,
    /// If false, we request a compatible context.
    /// If true, core context.
    /// true is default.
    pub core: Option<bool>,
}
