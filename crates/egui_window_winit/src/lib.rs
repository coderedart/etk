use std::convert::TryInto;

use egui::Event;
use egui_backend::{
    egui::{DroppedFile, Key, Modifiers, RawInput, Rect},
    raw_window_handle::HasRawWindowHandle,
    *,
};
use winit::{event::MouseButton, window::WindowBuilder, *};
use winit::{
    event::{ModifiersState, VirtualKeyCode},
    event_loop::{ControlFlow, EventLoop},
};

/// settings that you provide to winit backend
#[derive(Debug)]
pub struct WinitSettings {
    /// window title
    pub title: String,
    /// on web: winit will try to get the canvas element with this id attribute and use it as the window's context
    /// for now, it must not be empty. we can later provide options like creating a canvas ourselves and adding it to dom
    /// defualt value is : `egui_winit_canvas`
    /// so, make sure there's a canvas element in html body with this id
    pub dom_element_id: String,
}
impl Default for WinitSettings {
    fn default() -> Self {
        Self {
            title: "egui winit window".to_string(),
            dom_element_id: "egui_winit_canvas".to_string(),
        }
    }
}
/// This is the winit WindowBackend for egui
pub struct WinitBackend {
    /// we want to take out the event loop when we call the  `WindowBackend::run_event_loop` fn
    /// so, this will always be `None` once we start the event loop
    pub event_loop: Option<EventLoop<()>>,
    /// the winit window
    pub window: winit::window::Window,
    /// current modifiers state
    pub modifiers: egui::Modifiers,
    /// frame buffer size in physical pixels
    pub framebuffer_size: [u32; 2],
    /// scale
    pub scale: f32,
    /// cusor position in logical pixels
    pub cursor_pos_logical: [f32; 2],
    /// input for egui's begin_frame
    pub raw_input: RawInput,
    /// all current frame's events will be stored in this vec
    pub frame_events: Vec<winit::event::Event<'static, ()>>,
    /// should be true if there's been a resize event
    /// should be set to false once the renderer takes the latest size during `GfxBackend::prepare_frame`
    pub latest_resize_event: bool,
    /// ???
    pub should_close: bool,
    pub backend_settings: BackendSettings,
    #[cfg(not(target_arch = "wasm32"))]
    pub gl_context: Option<glutin::ContextWrapper<glutin::PossiblyCurrent, ()>>,
}

impl WindowBackend for WinitBackend {
    type Configuration = WinitSettings;
    fn new(_config: Self::Configuration, mut backend_settings: BackendSettings) -> Self
    where
        Self: Sized,
    {
        let el = EventLoop::new();
        #[allow(unused_mut)]
        let mut window_builder = WindowBuilder::new().with_resizable(true);
        #[cfg(not(target_arch = "wasm32"))]
        let mut gl_context = None;
        let window = match backend_settings.gfx_api_type.clone() {
            #[cfg(not(target_arch = "wasm32"))]
            GfxApiType::OpenGL { native_config } => {
                let NativeGlConfig {
                    major,
                    minor,
                    es,
                    core,
                    depth_bits,
                    stencil_bits,
                    samples,
                    srgb,
                    double_buffer,
                    vsync,
                    debug,
                } = native_config;
                let mut context_builder = glutin::ContextBuilder::new();

                if let Some(es) = es {
                    if let Some(major) = major {
                        context_builder = context_builder.with_gl(glutin::GlRequest::Specific(
                            if es {
                                glutin::Api::OpenGlEs
                            } else {
                                glutin::Api::OpenGl
                            },
                            (major, minor.unwrap_or_default()),
                        ));
                    } else {
                        context_builder = context_builder.with_gl(glutin::GlRequest::Specific(
                            if es {
                                glutin::Api::OpenGlEs
                            } else {
                                glutin::Api::OpenGl
                            },
                            (major.unwrap_or(3), minor.unwrap_or_default()),
                        ));
                    }
                } else {
                    if let Some(major) = major {
                        context_builder = context_builder.with_gl(glutin::GlRequest::Specific(
                            glutin::Api::OpenGl,
                            (major, minor.unwrap_or_default()),
                        ));
                    }
                }
                if let Some(value) = core {
                    context_builder = context_builder.with_gl_profile(if value {
                        glutin::GlProfile::Core
                    } else {
                        glutin::GlProfile::Compatibility
                    });
                }
                if let Some(value) = depth_bits {
                    context_builder = context_builder.with_depth_buffer(value);
                }
                if let Some(value) = stencil_bits {
                    context_builder = context_builder.with_stencil_buffer(value);
                }
                if let Some(samples) = samples {
                    context_builder = context_builder.with_multisampling(samples.into());
                }
                if let Some(srgb) = srgb {
                    context_builder = context_builder.with_srgb(srgb);
                }
                context_builder = context_builder.with_double_buffer(double_buffer);

                if let Some(value) = vsync {
                    context_builder = context_builder.with_vsync(value);
                }
                if let Some(debug) = debug {
                    context_builder = context_builder.with_gl_debug_flag(debug);
                }
                dbg!(&context_builder.pf_reqs, &context_builder.gl_attr);
                let windowed_context = context_builder
                    .build_windowed(window_builder, &el)
                    .expect("failed to build glutin window");
                unsafe {
                    let windowed_context = windowed_context
                        .make_current()
                        .expect("failed to make glutin window current");
                    let (opengl_context, window) = windowed_context.split();
                    // start setting the options in backend settings
                    let pixel_format = opengl_context.get_pixel_format();
                    let api = opengl_context.get_api();
                    backend_settings.gfx_api_type = GfxApiType::OpenGL {
                        native_config: NativeGlConfig {
                            major,
                            minor,
                            es: match api {
                                glutin::Api::OpenGl => Some(false),
                                glutin::Api::OpenGlEs => Some(true),
                                glutin::Api::WebGl => {
                                    unreachable!(" why would we get webgl on native opengl ???")
                                }
                            },
                            core,
                            depth_bits: Some(pixel_format.depth_bits),
                            stencil_bits: Some(pixel_format.stencil_bits),
                            samples: pixel_format.multisampling.map(|ms| {
                                ms.try_into()
                                    .expect("failed ot fit number of samples in u8")
                            }),
                            srgb: Some(pixel_format.srgb),
                            double_buffer: Some(pixel_format.double_buffer),
                            vsync,
                            debug,
                        },
                    };

                    gl_context = Some(opengl_context);

                    window
                }
            }
            #[cfg(target_arch = "wasm32")]
            GfxApiType::WebGL2 {
                canvas_id,
                webgl_config,
            } => {
                {
                    use wasm_bindgen::JsCast;
                    use winit::platform::web::{WindowBuilderExtWebSys, WindowExtWebSys};
                    let document = web_sys::window()
                        .expect("failed ot get websys window")
                        .document()
                        .expect("failed to get websys doc");

                    {
                        let canvas = if let Some(canvas_id) = canvas_id {
                            document
                                .get_element_by_id(&canvas_id)
                                .expect("settings doesn't contain canvas and DOM doesn't have a canvas element either")
                                .dyn_into::<web_sys::HtmlCanvasElement>().expect("failed to get canvas converted into html canvas element")
                        } else {
                            document.get_elements_by_tag_name("canvas").item(0)
                                .expect("canvas_id doesn't contain an id and DOM doesn't have a canvas element either")
                                .dyn_into::<web_sys::HtmlCanvasElement>()
                                .expect("egui winit canvas element conversion failed")
                        };
                        window_builder = window_builder.with_canvas(Some(canvas));
                    }
                    // create winit window
                    let window = window_builder
                        .with_prevent_default(true)
                        .build(&el)
                        .expect("failed to create winit window");

                    window
                }
            }

            _ => window_builder
                .build(&el)
                .expect("failed ot create winit window"),
        };

        let framebuffer_size_physical = window.inner_size();

        let framebuffer_size = [
            framebuffer_size_physical.width,
            framebuffer_size_physical.height,
        ];
        let scale = window.scale_factor() as f32;
        let window_size = framebuffer_size_physical.to_logical::<f32>(scale as f64);
        let raw_input = RawInput {
            screen_rect: Some(Rect::from_two_pos(
                [0.0, 0.0].into(),
                [window_size.width, window_size.height].into(),
            )),
            pixels_per_point: Some(scale),
            ..Default::default()
        };
        Self {
            event_loop: Some(el),
            window,
            modifiers: Modifiers::new(),
            framebuffer_size,
            scale,
            cursor_pos_logical: [0.0, 0.0],
            raw_input,
            frame_events: Vec::new(),
            latest_resize_event: true,
            should_close: false,
            #[cfg(not(target_arch = "wasm32"))]
            gl_context,
            backend_settings,
        }
    }

    fn take_raw_input(&mut self) -> egui::RawInput {
        self.raw_input.take()
    }

    fn take_latest_size_update(&mut self) -> Option<[u32; 2]> {
        if self.latest_resize_event {
            Some(self.get_live_physical_size_framebuffer())
        } else {
            None
        }
    }

    fn run_event_loop<G: GfxBackend<Self> + 'static, U: UserApp<Self, G> + 'static>(
        mut self,
        mut gfx_backend: G,
        mut user_app: U,
    ) {
        let egui_context = egui::Context::default();
        self.event_loop
            .take()
            .expect("event loop missing")
            .run(move |event, _, control_flow| {
                *control_flow = ControlFlow::Poll;

                match event {
                    event::Event::MainEventsCleared => {
                        self.window.request_redraw();
                    }
                    event::Event::RedrawRequested(_) => {
                        // take egui input
                        let input = self.take_raw_input();
                        // take any frambuffer resize events
                        let fb_size_update = self.take_latest_size_update();
                        // prepare surface for drawing
                        gfx_backend.prepare_frame(fb_size_update, &mut self);
                        // begin egui with input
                        egui_context.begin_frame(input);
                        // run userapp gui function. let user do anything he wants with window or gfx backends
                        user_app.run(&egui_context, &mut self, &mut gfx_backend);
                        // end frame
                        let output = egui_context.end_frame();
                        // prepare egui render data for gfx backend
                        let gfx_output = EguiGfxOutput {
                            meshes: egui_context.tessellate(output.shapes),
                            textures_delta: output.textures_delta,
                            screen_size_logical: [
                                self.framebuffer_size[0] as f32 / self.scale,
                                self.framebuffer_size[1] as f32 / self.scale,
                            ],
                        };
                        // render egui with gfx backend
                        gfx_backend.prepare_render(gfx_output);
                        gfx_backend.render();
                        // present the frame and loop back
                        gfx_backend.present(&mut self);
                    }
                    rest => self.handle_event(rest),
                }
                if self.should_close {
                    *control_flow = ControlFlow::Exit;
                }
            })
    }

    fn get_live_physical_size_framebuffer(&mut self) -> [u32; 2] {
        let size = self.window.inner_size();
        [size.width, size.height]
    }

    fn get_settings(&self) -> &BackendSettings {
        &self.backend_settings
    }
}
unsafe impl HasRawWindowHandle for WinitBackend {
    fn raw_window_handle(&self) -> raw_window_handle::RawWindowHandle {
        self.window.raw_window_handle()
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl OpenGLWindowContext for WinitBackend {
    fn swap_buffers(&mut self) {
        self.gl_context
            .as_ref()
            .expect("opengl context is none")
            .swap_buffers()
            .expect("failed to swap buffers");
    }

    fn get_proc_address(&mut self, symbol: &str) -> *const core::ffi::c_void {
        self.gl_context.as_ref().unwrap().get_proc_address(symbol)
    }
}
impl WinitBackend {
    fn handle_event(&mut self, event: winit::event::Event<()>) {
        if let Some(egui_event) = match event {
            event::Event::WindowEvent { event, .. } => match event {
                event::WindowEvent::Resized(size) => {
                    let logical_size = size.to_logical::<f32>(self.scale as f64);
                    self.raw_input.screen_rect = Some(Rect::from_two_pos(
                        Default::default(),
                        [logical_size.width, logical_size.height].into(),
                    ));
                    self.latest_resize_event = true;
                    self.framebuffer_size = size.into();
                    None
                }
                event::WindowEvent::CloseRequested => {
                    self.should_close = true;
                    None
                }
                event::WindowEvent::DroppedFile(df) => {
                    self.raw_input.dropped_files.push(DroppedFile {
                        path: Some(df.clone()),
                        name: df
                            .file_name()
                            .unwrap_or_default()
                            .to_str()
                            .unwrap_or_default()
                            .to_string(),
                        last_modified: None,
                        bytes: None,
                    });
                    None
                }

                event::WindowEvent::ReceivedCharacter(c) => Some(Event::Text(c.to_string())),

                event::WindowEvent::KeyboardInput { input, .. } => {
                    let pressed = match input.state {
                        event::ElementState::Pressed => true,
                        event::ElementState::Released => false,
                    };
                    if let Some(key_code) = input.virtual_keycode {
                        if let Some(egui_key) = winit_key_to_egui(key_code) {
                            Some(Event::Key {
                                key: egui_key,
                                pressed,
                                modifiers: self.modifiers,
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                event::WindowEvent::ModifiersChanged(modifiers) => {
                    self.modifiers = winit_modifiers_to_egui(modifiers);
                    None
                }
                event::WindowEvent::CursorMoved { position, .. } => {
                    let logical = position.to_logical::<f32>(self.scale as f64);
                    self.cursor_pos_logical = [logical.x, logical.y];
                    Some(Event::PointerMoved([logical.x, logical.y].into()))
                }
                event::WindowEvent::CursorLeft { .. } => Some(Event::PointerGone),
                event::WindowEvent::MouseWheel { delta, .. } => match delta {
                    event::MouseScrollDelta::LineDelta(x, y) => Some(Event::Scroll([x, y].into())),
                    event::MouseScrollDelta::PixelDelta(pos) => {
                        let lpos = pos.to_logical::<f32>(self.scale as f64);
                        Some(Event::Scroll([lpos.x, lpos.y].into()))
                    }
                },
                event::WindowEvent::MouseInput { state, button, .. } => {
                    let pressed = match state {
                        event::ElementState::Pressed => true,
                        event::ElementState::Released => false,
                    };
                    Some(Event::PointerButton {
                        pos: self.cursor_pos_logical.into(),
                        button: winit_mouse_button_to_egui(button),
                        pressed,
                        modifiers: self.modifiers,
                    })
                }
                event::WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                    self.scale = scale_factor as f32;
                    self.raw_input.pixels_per_point = Some(scale_factor as f32);
                    self.latest_resize_event = true;
                    None
                }
                _ => None,
            },
            _ => None,
        } {
            self.raw_input.events.push(egui_event);
        }
    }
}

fn winit_modifiers_to_egui(modifiers: ModifiersState) -> Modifiers {
    Modifiers {
        alt: modifiers.alt(),
        ctrl: modifiers.ctrl(),
        shift: modifiers.shift(),
        mac_cmd: false,
        command: modifiers.logo(),
    }
}
fn winit_mouse_button_to_egui(mb: winit::event::MouseButton) -> egui::PointerButton {
    match mb {
        MouseButton::Left => egui::PointerButton::Primary,
        MouseButton::Right => egui::PointerButton::Secondary,
        MouseButton::Middle => egui::PointerButton::Middle,
        MouseButton::Other(_) => egui::PointerButton::Extra1,
    }
}
fn winit_key_to_egui(key_code: VirtualKeyCode) -> Option<Key> {
    let key = match key_code {
        VirtualKeyCode::Down => Key::ArrowDown,
        VirtualKeyCode::Left => Key::ArrowLeft,
        VirtualKeyCode::Right => Key::ArrowRight,
        VirtualKeyCode::Up => Key::ArrowUp,

        VirtualKeyCode::Escape => Key::Escape,
        VirtualKeyCode::Tab => Key::Tab,
        VirtualKeyCode::Back => Key::Backspace,
        VirtualKeyCode::Return => Key::Enter,
        VirtualKeyCode::Space => Key::Space,

        VirtualKeyCode::Insert => Key::Insert,
        VirtualKeyCode::Delete => Key::Delete,
        VirtualKeyCode::Home => Key::Home,
        VirtualKeyCode::End => Key::End,
        VirtualKeyCode::PageUp => Key::PageUp,
        VirtualKeyCode::PageDown => Key::PageDown,

        VirtualKeyCode::Key0 | VirtualKeyCode::Numpad0 => Key::Num0,
        VirtualKeyCode::Key1 | VirtualKeyCode::Numpad1 => Key::Num1,
        VirtualKeyCode::Key2 | VirtualKeyCode::Numpad2 => Key::Num2,
        VirtualKeyCode::Key3 | VirtualKeyCode::Numpad3 => Key::Num3,
        VirtualKeyCode::Key4 | VirtualKeyCode::Numpad4 => Key::Num4,
        VirtualKeyCode::Key5 | VirtualKeyCode::Numpad5 => Key::Num5,
        VirtualKeyCode::Key6 | VirtualKeyCode::Numpad6 => Key::Num6,
        VirtualKeyCode::Key7 | VirtualKeyCode::Numpad7 => Key::Num7,
        VirtualKeyCode::Key8 | VirtualKeyCode::Numpad8 => Key::Num8,
        VirtualKeyCode::Key9 | VirtualKeyCode::Numpad9 => Key::Num9,

        VirtualKeyCode::A => Key::A,
        VirtualKeyCode::B => Key::B,
        VirtualKeyCode::C => Key::C,
        VirtualKeyCode::D => Key::D,
        VirtualKeyCode::E => Key::E,
        VirtualKeyCode::F => Key::F,
        VirtualKeyCode::G => Key::G,
        VirtualKeyCode::H => Key::H,
        VirtualKeyCode::I => Key::I,
        VirtualKeyCode::J => Key::J,
        VirtualKeyCode::K => Key::K,
        VirtualKeyCode::L => Key::L,
        VirtualKeyCode::M => Key::M,
        VirtualKeyCode::N => Key::N,
        VirtualKeyCode::O => Key::O,
        VirtualKeyCode::P => Key::P,
        VirtualKeyCode::Q => Key::Q,
        VirtualKeyCode::R => Key::R,
        VirtualKeyCode::S => Key::S,
        VirtualKeyCode::T => Key::T,
        VirtualKeyCode::U => Key::U,
        VirtualKeyCode::V => Key::V,
        VirtualKeyCode::W => Key::W,
        VirtualKeyCode::X => Key::X,
        VirtualKeyCode::Y => Key::Y,
        VirtualKeyCode::Z => Key::Z,

        VirtualKeyCode::F1 => Key::F1,
        VirtualKeyCode::F2 => Key::F2,
        VirtualKeyCode::F3 => Key::F3,
        VirtualKeyCode::F4 => Key::F4,
        VirtualKeyCode::F5 => Key::F5,
        VirtualKeyCode::F6 => Key::F6,
        VirtualKeyCode::F7 => Key::F7,
        VirtualKeyCode::F8 => Key::F8,
        VirtualKeyCode::F9 => Key::F9,
        VirtualKeyCode::F10 => Key::F10,
        VirtualKeyCode::F11 => Key::F11,
        VirtualKeyCode::F12 => Key::F12,
        VirtualKeyCode::F13 => Key::F13,
        VirtualKeyCode::F14 => Key::F14,
        VirtualKeyCode::F15 => Key::F15,
        VirtualKeyCode::F16 => Key::F16,
        VirtualKeyCode::F17 => Key::F17,
        VirtualKeyCode::F18 => Key::F18,
        VirtualKeyCode::F19 => Key::F19,
        VirtualKeyCode::F20 => Key::F20,
        _ => return None,
    };
    Some(key)
}
