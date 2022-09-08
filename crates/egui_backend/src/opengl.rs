/// most window backends create the opengl context along with the window (glfw / sdl2) and we need some
/// functions like SwapBuffers or MakeCurrent etc.. for rendering needs.
/// Modern APIs like vulkan or Wgpu already deal with surface creation or swapping or multi-threading etc.. themselves.
///
///
#[cfg(not(target_arch = "wasm32"))]
pub trait OpenGLWindowContext {
    /// Swaps buffers (swapchain) when we are using double buffering (99% of the time, double buffering is the default)
    /// this also flushes the opengl commands and blocks until the swapchain image is presented.
    fn swap_buffers(&mut self);
    /// get openGL function addresses.
    fn get_proc_address(&mut self, symbol: &str) -> *const core::ffi::c_void;
}

/// Native settings for OpenGL creation. preferably panic if the settings are not available
/// taken care of by window backends
/// after the creation of the window, the window backend **must** fill the options which are
/// not set by the user.
/// example:
///     if user did not set the major or minor version. after creating window, the backend must
///     get the version of opengl and set the relevant fields before passing this on to the rendering backend.
#[derive(Debug, Clone, Copy)]
pub struct NativeGlConfig {
    /// major opengl version.
    pub major: Option<u8>,
    /// minor opengl version
    pub minor: Option<u8>,
    /// whether it is an ES version. example: GL version ES 3.0
    pub es: Option<bool>,
    /// true: enable core profile. false: use compatibility profile
    pub core: Option<bool>,
    pub depth_bits: Option<u8>,
    pub stencil_bits: Option<u8>,
    /// if this is zero, multi sampling will be disabled
    pub samples: Option<u8>,
    /// framebuffer srgb compatibility
    pub srgb: Option<bool>,
    pub double_buffer: Option<bool>,
    pub vsync: Option<bool>,
    pub debug: Option<bool>,
}
impl Default for NativeGlConfig {
    fn default() -> Self {
        Self {
            major: None,
            minor: None,
            es: None,
            core: None,
            depth_bits: Default::default(),
            stencil_bits: Default::default(),
            samples: Default::default(),
            srgb: Some(true),
            double_buffer: Some(true),
            vsync: Some(true),
            debug: None,
        }
    }
}
/// these are settings to be provided to browser when requesting a webgl context
/// refer to `WebGL context attributes:` settings in the link:
/// https://developer.mozilla.org/en-US/docs/Web/API/HTMLCanvasElement/getContext
/// alternatively, the spec lists all attributes here https://registry.khronos.org/webgl/specs/latest/1.0/#5.2
/// ```js
/// WebGLContextAttributes {
///     boolean alpha = true;
///     boolean depth = true;
///     boolean stencil = false;
///     boolean antialias = true;
///     boolean premultipliedAlpha = true;
///     boolean preserveDrawingBuffer = false;
///     WebGLPowerPreference powerPreference = "default";
///     boolean failIfMajorPerformanceCaveat = false;
///     boolean desynchronized = false;
/// };
///
/// ```
/// we will only support WebGL2 for now. WebGL2 is available in 90+ % of all users
/// according to https://caniuse.com/?search=webgl2
/// we can add support for webgl2 if needed later
#[derive(Debug, Clone)]
pub struct WebGlConfig {
    pub alpha: Option<bool>,
    pub depth: Option<bool>,
    pub stencil: Option<bool>,
    pub antialias: Option<bool>,
    pub premultiplied_alpha: Option<bool>,
    pub preserve_drawing_buffer: Option<bool>,
    /// possible values are "default", "high-performance", "low-power"
    /// `None`: default.
    /// `Some(true)`: lower power
    /// `Some(false)`: high performance
    pub low_power: Option<bool>,
    pub fail_if_major_performance_caveat: Option<bool>,
    pub desynchronized: Option<bool>,
}
impl Default for WebGlConfig {
    fn default() -> Self {
        Self {
            alpha: Default::default(),
            depth: Default::default(),
            stencil: Default::default(),
            antialias: Default::default(),
            premultiplied_alpha: Default::default(),
            preserve_drawing_buffer: Default::default(),
            low_power: Default::default(),
            fail_if_major_performance_caveat: Default::default(),
            desynchronized: Default::default(),
        }
    }
}
