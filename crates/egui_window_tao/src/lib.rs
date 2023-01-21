use egui::Event;
use egui_backend::{
    egui::{DroppedFile, Key, Modifiers, RawInput, Rect},
    raw_window_handle::HasRawWindowHandle,
    *,
};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::{event, keyboard::ModifiersState};
use tao::{event::MouseButton, window::WindowBuilder};

/// config that you provide to tao backend
#[derive(Debug)]
pub struct TaoConfig {
    /// window title
    pub title: String,
}
impl Default for TaoConfig {
    fn default() -> Self {
        Self {
            title: "egui tao window".to_string(),
        }
    }
}
/// This is the tao WindowBackend for egui
pub struct TaoBackend {
    /// we want to take out the event loop when we call the  `WindowBackend::run_event_loop` fn
    /// so, this will always be `None` once we start the event loop
    pub event_loop: Option<EventLoop<()>>,
    /// the tao window
    pub window: tao::window::Window,
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
    pub frame_events: Vec<tao::event::Event<'static, ()>>,
    /// should be true if there's been a resize event
    /// should be set to false once the renderer takes the latest size during `GfxBackend::prepare_frame`
    pub latest_resize_event: bool,
    /// ???
    pub should_close: bool,
    pub backend_config: BackendConfig,
}

impl WindowBackend for TaoBackend {
    type Configuration = TaoConfig;
    fn new(_config: Self::Configuration, backend_config: BackendConfig) -> Self

    {
        let el = EventLoop::new();
        #[allow(unused_mut)]
        let mut window_builder = WindowBuilder::new().with_resizable(true);

        let window = match backend_config.gfx_api_type.clone() {
            GfxApiType::Vulkan | GfxApiType::NoApi => window_builder
                .build(&el)
                .expect("failed ot create tao window"),
            _ => {
                // refer to https://github.com/tauri-apps/tao/issues/322
                // which redirects to use a github fork at https://github.com/wusyong/glutin
                // we can't publish this to crates.io with a git dependency.., so we will keep this unimplemented for now
                unimplemented!("tao doesn't work with glutin, so we have no way to create an opengl context with tao :( ");
                // tao doesn't have web backend either.. so no need for WebGL2 impl
            }
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
            backend_config,
        }
    }

    fn take_raw_input(&mut self) -> egui::RawInput {
        self.raw_input.take()
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

                        // prepare surface for drawing
                        gfx_backend.prepare_frame(self.latest_resize_event, &mut self);
                        self.latest_resize_event = false;
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

    fn get_config(&self) -> &BackendConfig {
        &self.backend_config
    }
}
unsafe impl HasRawWindowHandle for TaoBackend {
    fn raw_window_handle(&self) -> raw_window_handle::RawWindowHandle {
        self.window.raw_window_handle()
    }
}

impl TaoBackend {
    fn handle_event(&mut self, event: tao::event::Event<()>) {
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
                event::WindowEvent::KeyboardInput { event, .. } => {
                    let pressed = match event.state {
                        event::ElementState::Pressed => true,
                        event::ElementState::Released => false,
                        _ => todo!(),
                    };
                    if let Some(egui_key) = tao_key_to_egui(event.logical_key) {
                        Some(Event::Key {
                            key: egui_key,
                            pressed,
                            modifiers: self.modifiers,
                        })
                    } else {
                        None
                    }
                }
                event::WindowEvent::ModifiersChanged(modifiers) => {
                    self.modifiers = tao_modifiers_to_egui(modifiers);
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
                    _ => todo!(),
                },
                event::WindowEvent::MouseInput { state, button, .. } => {
                    let pressed = match state {
                        event::ElementState::Pressed => true,
                        event::ElementState::Released => false,
                        _ => todo!(),
                    };
                    Some(Event::PointerButton {
                        pos: self.cursor_pos_logical.into(),
                        button: tao_mouse_button_to_egui(button),
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
                event::WindowEvent::ReceivedImeText(text) => Some(Event::Text(text)),
                _ => None,
            },
            _ => None,
        } {
            self.raw_input.events.push(egui_event);
        }
    }
}

fn tao_modifiers_to_egui(modifiers: ModifiersState) -> Modifiers {
    Modifiers {
        alt: modifiers.alt_key(),
        ctrl: modifiers.control_key(),
        shift: modifiers.shift_key(),
        mac_cmd: false,
        command: modifiers.super_key(),
    }
}
fn tao_mouse_button_to_egui(mb: tao::event::MouseButton) -> egui::PointerButton {
    match mb {
        MouseButton::Left => egui::PointerButton::Primary,
        MouseButton::Right => egui::PointerButton::Secondary,
        MouseButton::Middle => egui::PointerButton::Middle,
        MouseButton::Other(_) => egui::PointerButton::Extra1,
        _ => todo!(),
    }
}
fn tao_key_to_egui(key_code: tao::keyboard::Key) -> Option<Key> {
    let key = match key_code {
        tao::keyboard::Key::ArrowDown => Key::ArrowDown,
        tao::keyboard::Key::ArrowLeft => Key::ArrowLeft,
        tao::keyboard::Key::ArrowRight => Key::ArrowRight,
        tao::keyboard::Key::ArrowUp => Key::ArrowUp,

        tao::keyboard::Key::Escape => Key::Escape,
        tao::keyboard::Key::Tab => Key::Tab,
        tao::keyboard::Key::Backspace => Key::Backspace,
        tao::keyboard::Key::Enter => Key::Enter,
        tao::keyboard::Key::Space => Key::Space,

        tao::keyboard::Key::Insert => Key::Insert,
        tao::keyboard::Key::Delete => Key::Delete,
        tao::keyboard::Key::Home => Key::Home,
        tao::keyboard::Key::End => Key::End,
        tao::keyboard::Key::PageUp => Key::PageUp,
        tao::keyboard::Key::PageDown => Key::PageDown,

        // tao::keyboard::Key::Key0 | tao::keyboard::Key::Numpad0 => Key::Num0,
        // tao::keyboard::Key::Key1 | tao::keyboard::Key::Numpad1 => Key::Num1,
        // tao::keyboard::Key::Key2 | tao::keyboard::Key::Numpad2 => Key::Num2,
        // tao::keyboard::Key::Key3 | tao::keyboard::Key::Numpad3 => Key::Num3,
        // tao::keyboard::Key::Key4 | tao::keyboard::Key::Numpad4 => Key::Num4,
        // tao::keyboard::Key::Key5 | tao::keyboard::Key::Numpad5 => Key::Num5,
        // tao::keyboard::Key::Key6 | tao::keyboard::Key::Numpad6 => Key::Num6,
        // tao::keyboard::Key::Key7 | tao::keyboard::Key::Numpad7 => Key::Num7,
        // tao::keyboard::Key::Key8 | tao::keyboard::Key::Numpad8 => Key::Num8,
        // tao::keyboard::Key::Key9 | tao::keyboard::Key::Numpad9 => Key::Num9,

        // tao::keyboard::Key::A => Key::A,
        // tao::keyboard::Key::B => Key::B,
        // tao::keyboard::Key::C => Key::C,
        // tao::keyboard::Key::D => Key::D,
        // tao::keyboard::Key::E => Key::E,
        // tao::keyboard::Key::F => Key::F,
        // tao::keyboard::Key::G => Key::G,
        // tao::keyboard::Key::H => Key::H,
        // tao::keyboard::Key::I => Key::I,
        // tao::keyboard::Key::J => Key::J,
        // tao::keyboard::Key::K => Key::K,
        // tao::keyboard::Key::L => Key::L,
        // tao::keyboard::Key::M => Key::M,
        // tao::keyboard::Key::N => Key::N,
        // tao::keyboard::Key::O => Key::O,
        // tao::keyboard::Key::P => Key::P,
        // tao::keyboard::Key::Q => Key::Q,
        // tao::keyboard::Key::R => Key::R,
        // tao::keyboard::Key::S => Key::S,
        // tao::keyboard::Key::T => Key::T,
        // tao::keyboard::Key::U => Key::U,
        // tao::keyboard::Key::V => Key::V,
        // tao::keyboard::Key::W => Key::W,
        // tao::keyboard::Key::X => Key::X,
        // tao::keyboard::Key::Y => Key::Y,
        // tao::keyboard::Key::Z => Key::Z,
        tao::keyboard::Key::F1 => Key::F1,
        tao::keyboard::Key::F2 => Key::F2,
        tao::keyboard::Key::F3 => Key::F3,
        tao::keyboard::Key::F4 => Key::F4,
        tao::keyboard::Key::F5 => Key::F5,
        tao::keyboard::Key::F6 => Key::F6,
        tao::keyboard::Key::F7 => Key::F7,
        tao::keyboard::Key::F8 => Key::F8,
        tao::keyboard::Key::F9 => Key::F9,
        tao::keyboard::Key::F10 => Key::F10,
        tao::keyboard::Key::F11 => Key::F11,
        tao::keyboard::Key::F12 => Key::F12,
        tao::keyboard::Key::F13 => Key::F13,
        tao::keyboard::Key::F14 => Key::F14,
        tao::keyboard::Key::F15 => Key::F15,
        tao::keyboard::Key::F16 => Key::F16,
        tao::keyboard::Key::F17 => Key::F17,
        tao::keyboard::Key::F18 => Key::F18,
        tao::keyboard::Key::F19 => Key::F19,
        tao::keyboard::Key::F20 => Key::F20,
        _ => return None,
    };
    Some(key)
}
