use egui_backend::{EguiGfxOutput, GfxBackend, WindowBackend};
use tracing::{info, warn};

use std::sync::Arc;

pub use glow;
use glow::{Context as GlowContext, HasContext};
mod painter;
pub use painter::*;

#[macro_export]
macro_rules! glow_error {
    ($glow_context: ident) => {
        let error_code = $glow_context.get_error();
        if error_code != glow::NO_ERROR {
            panic!("glow error: {} at line {}", error_code, line!());
        }
    };
}
pub struct GlowBackend {
    pub glow_context: Arc<GlowContext>,
    pub framebuffer_size: [u32; 2],
    pub painter: Painter,
}

impl Drop for GlowBackend {
    fn drop(&mut self) {
        unsafe {
            self.painter.destroy(&self.glow_context);
        }
    }
}

impl<
        #[cfg(not(target_arch = "wasm32"))] W: WindowBackend + egui_backend::OpenGLWindowContext,
        #[cfg(target_arch = "wasm32")] W: WindowBackend,
    > GfxBackend<W> for GlowBackend
{
    type Configuration = ();

    fn new(window_backend: &mut W, _settings: Self::Configuration) -> Self {
        let glow_context = Arc::new(unsafe {
            match window_backend.get_settings().gfx_api_type.clone() {
                #[cfg(not(target_arch = "wasm32"))]
                egui_backend::GfxApiType::OpenGL { native_config } => {
                    let gl =
                        glow::Context::from_loader_function(|s| window_backend.get_proc_address(s));

                    let gl_version = gl.version();
                    info!("glow using gl version: {gl_version:?}");
                    assert!(
                        gl_version.major >= 3,
                        "egui glow only supports opengl major version 3 or above {gl_version:?}"
                    );

                    assert_eq!(
                        native_config.double_buffer,
                        Some(true),
                        "egui glow only supports double buffer"
                    );

                    // assert!(native_config.minor.unwrap() >= 0, "egui glow only supports opengl minor version ???");
                    assert_eq!(
                        native_config.samples, None,
                        "egui glow doesn't support multi sampling"
                    );
                    assert_eq!(
                        native_config.srgb,
                        Some(true),
                        "egui glow only supports srgb compatible surface/ framebuffers"
                    );

                    if let Some(debug) = native_config.debug {
                        if debug {
                            gl.enable(glow::DEBUG_OUTPUT);
                            gl.enable(glow::DEBUG_OUTPUT_SYNCHRONOUS);
                            assert!(gl.supports_debug());
                            gl.debug_message_callback(
                                |source, error_type, error_id, severity, error_str| {
                                    match severity {
                                        glow::DEBUG_SEVERITY_NOTIFICATION => tracing::debug!(
                                            source, error_type, error_id, severity, error_str
                                        ),
                                        glow::DEBUG_SEVERITY_LOW => {
                                            tracing::info!(
                                                source, error_type, error_id, severity, error_str
                                            )
                                        }
                                        glow::DEBUG_SEVERITY_MEDIUM => {
                                            tracing::warn!(
                                                source, error_type, error_id, severity, error_str
                                            )
                                        }
                                        glow::DEBUG_SEVERITY_HIGH => tracing::error!(
                                            source, error_type, error_id, severity, error_str
                                        ),
                                        rest => panic!("unknown severity {rest}"),
                                    };
                                },
                            );
                            glow_error!(gl);
                        }
                    }
                    gl
                }
                #[cfg(target_arch = "wasm32")]
                egui_backend::GfxApiType::WebGL2 {
                    canvas_id,
                    webgl_config,
                } => {
                    use wasm_bindgen::JsCast;

                    let handle_id = match window_backend.raw_window_handle() {
                        egui_backend::raw_window_handle::RawWindowHandle::Web(handle_id) => {
                            handle_id.id
                        }
                        _ => {
                            unimplemented!("non web raw window handles are not supported on wasm32")
                        }
                    };
                    let canvas_node: wasm_bindgen::JsValue = web_sys::window()
                        .and_then(|win| win.document())
                        .and_then(|doc: web_sys::Document| {
                            doc.query_selector(&format!("[data-raw-handle=\"{}\"]", handle_id))
                                .ok()
                        })
                        .expect("expected to find single canvas")
                        .into();
                    let canvas_element: web_sys::HtmlCanvasElement = canvas_node.into();
                    let context_options = create_context_options_from_webgl_config(webgl_config);
                    let context = canvas_element
                        .get_context_with_context_options("webgl2", &context_options)
                        .unwrap()
                        .unwrap()
                        .dyn_into()
                        .unwrap();
                    glow::Context::from_webgl2_context(context)
                }
                _ => {
                    unimplemented!("egui glow only supports WebGL2 or OpenGL gfx types ")
                }
            }
        });
        if glow_context.supported_extensions().contains("EXT_sRGB")
            || glow_context.supported_extensions().contains("GL_EXT_sRGB")
            || glow_context
                .supported_extensions()
                .contains("GL_ARB_framebuffer_sRGB")
        {
            warn!("srgb support detected by egui glow");
        } else {
            warn!("no srgb support detected by egui glow");
        }

        let painter = Painter::new(&glow_context);
        Self {
            glow_context,
            painter,
            framebuffer_size: window_backend.get_live_physical_size_framebuffer(),
        }
    }

    fn prepare_frame(
        &mut self,
        framebuffer_size_update: Option<[u32; 2]>,
        _window_backend: &mut W,
    ) {
        if let Some(fb_size) = framebuffer_size_update {
            unsafe {
                self.glow_context
                    .viewport(0, 0, fb_size[0] as i32, fb_size[1] as i32);
            }
        }
        unsafe {
            self.glow_context.disable(glow::SCISSOR_TEST);
            self.glow_context.clear(glow::COLOR_BUFFER_BIT);
        }
    }

    fn prepare_render(&mut self, egui_gfx_output: EguiGfxOutput) {
        unsafe {
            self.painter
                .prepare_render(&self.glow_context, egui_gfx_output, self.framebuffer_size)
        };
    }

    fn render(&mut self) {
        unsafe {
            self.painter.render(&self.glow_context);
        }
    }

    fn present(&mut self, window_backend: &mut W) {
        // on wasm, there's no swap buffers.. the browser takes care of it automatically.
        #[cfg(not(target_arch = "wasm32"))]
        {
            egui_backend::OpenGLWindowContext::swap_buffers(window_backend);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn create_context_options_from_webgl_config(
    webgl_config: egui_backend::WebGlConfig,
) -> js_sys::Object {
    use wasm_bindgen::JsValue;

    let context_options = js_sys::Object::new();
    if let Some(alpha) = webgl_config.alpha {
        js_sys::Reflect::set(
            &context_options,
            &"alpha".into(),
            &if alpha { JsValue::TRUE } else { JsValue::FALSE },
        )
        .expect("Cannot create context options");
    }
    if let Some(antialias) = webgl_config.antialias {
        js_sys::Reflect::set(
            &context_options,
            &"antialias".into(),
            &if antialias {
                JsValue::TRUE
            } else {
                JsValue::FALSE
            },
        )
        .expect("Cannot create context options");
    }
    if let Some(depth) = webgl_config.depth {
        js_sys::Reflect::set(
            &context_options,
            &"depth".into(),
            &if depth { JsValue::TRUE } else { JsValue::FALSE },
        )
        .expect("Cannot create context options");
    }
    if let Some(value) = webgl_config.desynchronized {
        js_sys::Reflect::set(
            &context_options,
            &"desynchronized".into(),
            &if value { JsValue::TRUE } else { JsValue::FALSE },
        )
        .expect("Cannot create context options");
    }
    if let Some(value) = webgl_config.fail_if_major_performance_caveat {
        js_sys::Reflect::set(
            &context_options,
            &"failIfMajorPerformanceCaveat".into(),
            &if value { JsValue::TRUE } else { JsValue::FALSE },
        )
        .expect("Cannot create context options");
    }
    if let Some(value) = webgl_config.low_power {
        js_sys::Reflect::set(
            &context_options,
            &"powerPreference".into(),
            &if value {
                JsValue::from_str("low-power")
            } else {
                JsValue::from_str("high-performance")
            },
        )
        .expect("Cannot create context options");
    }
    if let Some(value) = webgl_config.premultiplied_alpha {
        js_sys::Reflect::set(
            &context_options,
            &"premultipliedAlpha".into(),
            &if value { JsValue::TRUE } else { JsValue::FALSE },
        )
        .expect("Cannot create context options");
    }
    if let Some(value) = webgl_config.preserve_drawing_buffer {
        js_sys::Reflect::set(
            &context_options,
            &"preserveDrawingBuffer".into(),
            &if value { JsValue::TRUE } else { JsValue::FALSE },
        )
        .expect("Cannot create context options");
    }
    if let Some(value) = webgl_config.stencil {
        js_sys::Reflect::set(
            &context_options,
            &"stencil".into(),
            &if value { JsValue::TRUE } else { JsValue::FALSE },
        )
        .expect("Cannot create context options");
    }
    context_options
}
