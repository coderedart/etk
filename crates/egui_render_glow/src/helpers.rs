use std::sync::Arc;

use crate::{glow_error, WebGlConfig};
use egui_backend::WindowBackend;
use glow::*;
use tracing::*;

#[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
pub unsafe fn create_glow_wasm32_unknown(
    window_backend: &mut impl WindowBackend,
    webgl_config: WebGlConfig,
) -> glow::Context {
    use egui_backend::raw_window_handle::HasRawWindowHandle;
    use wasm_bindgen::JsCast;

    let handle_id = match window_backend
        .get_window()
        .expect("window backend doesn't have a window yet???")
        .raw_window_handle()
    {
        crate::raw_window_handle::RawWindowHandle::Web(handle_id) => handle_id.id,
        _ => unimplemented!("non web raw window handles are not supported on wasm32"),
    };
    let canvas_node: wasm_bindgen::JsValue = web_sys::window()
        .and_then(|win| win.document())
        .and_then(|doc: web_sys::Document| {
            doc.query_selector(&format!("[data-raw-handle=\"{handle_id}\"]"))
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
pub unsafe fn create_glow_context(
    window_backend: &mut impl WindowBackend,
    _webgl_config: WebGlConfig,
) -> Arc<glow::Context> {
    // for wasm32-unknown-unknown, use glow's own constructor.
    #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
    let glow_context = create_glow_wasm32_unknown(window_backend, _webgl_config);
    // for non-web and emscripten platforms, just use loader fn
    #[cfg(any(not(target_arch = "wasm32"), target_os = "emscripten"))]
    let glow_context = glow::Context::from_loader_function(|s| window_backend.get_proc_address(s));

    tracing::debug!("created glow context");
    let glow_context = Arc::new(glow_context);
    glow_error!(glow_context);
    glow_context
}
pub unsafe fn create_program_from_src(
    glow_context: &glow::Context,
    vertex_src: &str,
    frag_src: &str,
) -> Program {
    let vs = glow_context
        .create_shader(glow::VERTEX_SHADER)
        .expect("shader creation failed");
    let fs = glow_context
        .create_shader(glow::FRAGMENT_SHADER)
        .expect("failed to create frag shader");
    glow_context.shader_source(vs, vertex_src);
    glow_context.shader_source(fs, frag_src);
    glow_context.compile_shader(vs);
    let info_log = glow_context.get_shader_info_log(vs);
    if !info_log.is_empty() {
        warn!("vertex shader info log: {info_log}")
    }
    if !glow_context.get_shader_compile_status(vs) {
        panic!("failed to compile vertex shader. info_log: {info_log}");
    }
    glow_error!(glow_context);
    glow_context.compile_shader(fs);
    let info_log = glow_context.get_shader_info_log(fs);
    if !info_log.is_empty() {
        warn!("fragment shader info log: {info_log}")
    }
    if !glow_context.get_shader_compile_status(fs) {
        panic!("failed to compile fragment shader. info_log: {info_log}");
    }
    glow_error!(glow_context);

    let egui_program = glow_context
        .create_program()
        .expect("failed to create glow program");
    glow_context.attach_shader(egui_program, vs);
    glow_context.attach_shader(egui_program, fs);
    glow_context.link_program(egui_program);
    let info_log = glow_context.get_program_info_log(egui_program);
    if !info_log.is_empty() {
        warn!("egui program info log: {info_log}")
    }
    if !glow_context.get_program_link_status(egui_program) {
        panic!("failed to link egui glow program. info_log: {info_log}");
    }
    glow_error!(glow_context);
    debug!("egui shader program successfully compiled and linked");
    // no need for shaders anymore after linking
    glow_context.detach_shader(egui_program, vs);
    glow_context.detach_shader(egui_program, fs);
    glow_context.delete_shader(vs);
    glow_context.delete_shader(fs);
    egui_program
}

pub unsafe fn create_egui_vao_buffers(
    glow_context: &glow::Context,
    program: Program,
) -> (VertexArray, Buffer, Buffer) {
    let vao = glow_context
        .create_vertex_array()
        .expect("failed to create egui vao");
    glow_context.bind_vertex_array(Some(vao));
    glow_error!(glow_context);

    // buffers
    let vbo = glow_context
        .create_buffer()
        .expect("failed to create array buffer");
    glow_context.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
    glow_error!(glow_context);

    let ebo = glow_context
        .create_buffer()
        .expect("failed to create element buffer");
    glow_context.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
    glow_error!(glow_context);

    // enable position, tex coords and color attributes. this will bind vbo to the vao
    let location = glow_context
        .get_attrib_location(program, "vin_pos")
        .expect("failed to get vin_pos location");
    debug!("vin_pos vertex attribute location is {location}");
    glow_context.enable_vertex_attrib_array(location);
    glow_context.vertex_attrib_pointer_f32(location, 2, glow::FLOAT, false, 20, 0);
    let location = glow_context
        .get_attrib_location(program, "vin_tc")
        .expect("failed to get vin_tc location");
    debug!("vin_tc vertex attribute location is {location}");
    glow_context.enable_vertex_attrib_array(location);
    glow_context.vertex_attrib_pointer_f32(location, 2, glow::FLOAT, false, 20, 8);
    let location = glow_context
        .get_attrib_location(program, "vin_sc")
        .expect("failed to get vin_sc location");
    debug!("vin_sc vertex attribute location is {location}");
    glow_context.enable_vertex_attrib_array(location);
    glow_context.vertex_attrib_pointer_f32(location, 4, glow::UNSIGNED_BYTE, false, 20, 16);

    glow_error!(glow_context);
    (vao, vbo, ebo)
}

pub unsafe fn create_samplers(glow_context: &glow::Context) -> (Sampler, Sampler) {
    let nearest_sampler = glow_context
        .create_sampler()
        .expect("failed to create nearest sampler");
    glow_context.bind_sampler(0, Some(nearest_sampler));
    glow_error!(glow_context);

    glow_context.sampler_parameter_i32(
        nearest_sampler,
        glow::TEXTURE_MAG_FILTER,
        glow::NEAREST
            .try_into()
            .expect("failed to fit NEAREST in i32"),
    );
    glow_error!(glow_context);

    glow_context.sampler_parameter_i32(
        nearest_sampler,
        glow::TEXTURE_MIN_FILTER,
        glow::NEAREST
            .try_into()
            .expect("failed to fit NEAREST in i32"),
    );
    glow_error!(glow_context);

    let linear_sampler = glow_context
        .create_sampler()
        .expect("failed to create linear sampler");
    glow_context.bind_sampler(0, Some(linear_sampler));
    glow_error!(glow_context);

    glow_context.sampler_parameter_i32(
        linear_sampler,
        glow::TEXTURE_MAG_FILTER,
        glow::LINEAR
            .try_into()
            .expect("failed to fit LINEAR MIPMAP NEAREST in i32"),
    );
    glow_error!(glow_context);

    glow_context.sampler_parameter_i32(
        linear_sampler,
        glow::TEXTURE_MIN_FILTER,
        glow::LINEAR
            .try_into()
            .expect("failed to fit LINEAR MIPMAP NEAREST in i32"),
    );
    glow_error!(glow_context);
    (linear_sampler, nearest_sampler)
}

#[allow(unused)]
pub unsafe fn enable_debug(gl: &glow::Context) {
    gl.enable(glow::DEBUG_OUTPUT);
    gl.enable(glow::DEBUG_OUTPUT_SYNCHRONOUS);
    if gl.supports_debug() {
        tracing::info!("opengl supports debug. setting debug callback");
        gl.debug_message_callback(|source, error_type, error_id, severity, error_str| {
            match severity {
                glow::DEBUG_SEVERITY_NOTIFICATION => {
                    tracing::debug!(source, error_type, error_id, severity, error_str)
                }
                glow::DEBUG_SEVERITY_LOW => {
                    tracing::info!(source, error_type, error_id, severity, error_str)
                }
                glow::DEBUG_SEVERITY_MEDIUM => {
                    tracing::warn!(source, error_type, error_id, severity, error_str)
                }
                glow::DEBUG_SEVERITY_HIGH => {
                    tracing::error!(source, error_type, error_id, severity, error_str)
                }
                rest => tracing::error!("unknown severity {rest}"),
            };
        });
    }
    glow_error!(gl);
}

#[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
fn create_context_options_from_webgl_config(webgl_config: crate::WebGlConfig) -> js_sys::Object {
    let context_options = js_sys::Object::new();
    if let Some(value) = webgl_config.alpha {
        js_sys::Reflect::set(&context_options, &"alpha".into(), &value.into()).unwrap();
    }
    if let Some(value) = webgl_config.antialias {
        js_sys::Reflect::set(&context_options, &"antialias".into(), &value.into()).unwrap();
    }
    if let Some(value) = webgl_config.depth {
        js_sys::Reflect::set(&context_options, &"depth".into(), &value.into()).unwrap();
    }
    if let Some(value) = webgl_config.desynchronized {
        js_sys::Reflect::set(&context_options, &"desynchronized".into(), &value.into()).unwrap();
    }
    if let Some(value) = webgl_config.fail_if_major_performance_caveat {
        js_sys::Reflect::set(
            &context_options,
            &"failIfMajorPerformanceCaveat".into(),
            &value.into(),
        )
        .unwrap();
    }
    if let Some(value) = webgl_config.low_power {
        js_sys::Reflect::set(
            &context_options,
            &"powerPreference".into(),
            &if value {
                "low-power"
            } else {
                "high-performance"
            }
            .into(),
        )
        .unwrap();
    }
    if let Some(value) = webgl_config.premultiplied_alpha {
        js_sys::Reflect::set(
            &context_options,
            &"premultipliedAlpha".into(),
            &value.into(),
        )
        .unwrap();
    }
    if let Some(value) = webgl_config.preserve_drawing_buffer {
        js_sys::Reflect::set(
            &context_options,
            &"preserveDrawingBuffer".into(),
            &value.into(),
        )
        .unwrap();
    }
    if let Some(value) = webgl_config.stencil {
        js_sys::Reflect::set(&context_options, &"stencil".into(), &value.into()).unwrap();
    }
    context_options
}
