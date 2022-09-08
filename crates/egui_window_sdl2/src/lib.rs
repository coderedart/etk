use std::{path::PathBuf, str::FromStr};

use egui::{Event, Key, Modifiers, PointerButton, RawInput};
use egui_backend::{
    egui, raw_window_handle::HasRawWindowHandle, BackendSettings, EguiGfxOutput, GfxApiType,
    GfxBackend, NativeGlConfig, OpenGLWindowContext, UserApp, WindowBackend,
};
use sdl2::{
    keyboard::Scancode,
    video::{SwapInterval, Window},
    Sdl,
};

pub struct SDL2Backend {
    pub sdl_context: Sdl,
    pub event_pump: sdl2::EventPump,
    pub window: Window,
    pub size_physical_pixels: [u32; 2],
    pub scale: [f32; 2],
    pub cursor_pos_physical_pixels: [f32; 2],
    pub raw_input: RawInput,
    pub frame_events: Vec<sdl2::event::Event>,
    pub gl_context: Option<sdl2::video::GLContext>,
    pub latest_resize_event: bool,
    pub should_close: bool,
    pub backend_settings: BackendSettings,
}

#[derive(Debug)]
pub struct SDL2Settings {}
impl Default for SDL2Settings {
    fn default() -> Self {
        Self {}
    }
}
impl WindowBackend for SDL2Backend {
    type Configuration = SDL2Settings;

    fn new(_config: Self::Configuration, backend_settings: BackendSettings) -> Self
    where
        Self: Sized,
    {
        let sdl_context = sdl2::init().unwrap();
        let video_subsystem = sdl_context.video().unwrap();
        let attrs = video_subsystem.gl_attr();
        let mut swap_interval = None;
        let mut opengl = false;
        match backend_settings.gfx_api_type.clone() {
            GfxApiType::OpenGL { native_config } => {
                opengl = true;
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
                swap_interval = vsync;
                if let Some(value) = major {
                    attrs.set_context_major_version(value);
                }
                if let Some(value) = minor {
                    attrs.set_context_minor_version(value);
                }
                if let Some(value) = es {
                    if value {
                        attrs.set_context_profile(sdl2::video::GLProfile::GLES);
                    }
                }
                if let Some(value) = core {
                    if value {
                        attrs.set_context_profile(sdl2::video::GLProfile::Core);
                    } else {
                        attrs.set_context_profile(sdl2::video::GLProfile::Compatibility);
                    }
                }
                if let Some(value) = depth_bits {
                    attrs.set_depth_size(value);
                }
                if let Some(value) = stencil_bits {
                    attrs.set_stencil_size(value);
                }
                if let Some(value) = samples {
                    attrs.set_multisample_samples(value);
                }
                if let Some(value) = srgb {
                    attrs.set_framebuffer_srgb_compatible(value);
                }
                if let Some(value) = double_buffer {
                    attrs.set_double_buffer(value);
                }

                if let Some(debug) = debug {
                    if debug {
                        attrs.set_context_flags().debug().set();
                    }
                }
            }
            GfxApiType::NoApi => {}
            GfxApiType::Vulkan => {}
        }

        let mut window_builder = video_subsystem.window("rust-sdl2 demo", 800, 600);
        match backend_settings.gfx_api_type.clone() {
            GfxApiType::OpenGL { .. } => {
                window_builder.opengl();
            }
            GfxApiType::Vulkan => {
                window_builder.vulkan();
            }
            _ => {}
        }
        window_builder.allow_highdpi();
        window_builder.resizable();
        let window = window_builder.build().expect("failed to create a window");
        let event_pump = sdl_context.event_pump().expect("failed to get event pump");
        let mut gl_context = None;
        if opengl {
            gl_context = Some(
                window
                    .gl_create_context()
                    .expect("failed ot create opengl context"),
            );
            window
                .gl_make_current(&gl_context.as_ref().unwrap())
                .expect("failed to make gl context current");
            if let Some(value) = swap_interval {
                video_subsystem
                    .gl_set_swap_interval(if value {
                        SwapInterval::VSync
                    } else {
                        SwapInterval::Immediate
                    })
                    .expect("failed to set vsync option");
            }
        }
        let mouse_state = event_pump.relative_mouse_state();
        let cursor_pos_physical_pixels = [mouse_state.x() as f32, mouse_state.y() as f32];
        // display dpi shows 101.6 on my normal monitor.. and docs of sdl state that this is unreliable
        // so we will instead use logical and physical sizes and derive scale from that
        // let display_dpi = video_subsystem
        //     .display_dpi(window.display_index().expect("failed to get display index"))
        //     .expect("failed to get display dpi");
        // let scale = [display_dpi.1 / 96.0, display_dpi.2 / 96.0]; // 96 is the default dpi?
        let fb_size = window.drawable_size();
        let size_physical_pixels = [fb_size.0, fb_size.1];
        let (logical_width, logical_height) = window.size();
        let scale = [
            fb_size.0 as f32 / logical_width as f32,
            fb_size.1 as f32 / logical_height as f32,
        ];
        let raw_input = RawInput {
            screen_rect: Some(egui::Rect::from_points(&[
                [0.0, 0.0].into(),
                [logical_width as f32, logical_height as f32].into(),
            ])),
            pixels_per_point: Some(scale[0]),
            ..Default::default()
        };
        Self {
            sdl_context,
            window,
            size_physical_pixels,
            scale,
            cursor_pos_physical_pixels,
            raw_input,
            frame_events: Vec::new(),
            latest_resize_event: true,
            event_pump,
            should_close: false,
            gl_context,
            backend_settings,
        }
    }

    fn take_raw_input(&mut self) -> egui::RawInput {
        self.raw_input.take()
    }

    fn run_event_loop<G: GfxBackend<Self>, U: UserApp<Self, G>>(
        mut self,
        mut gfx_backend: G,
        mut user_app: U,
    ) {
        let egui_context = egui::Context::default();
        while !self.should_close {
            // gather events
            self.tick();
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
                    self.size_physical_pixels[0] as f32 / self.scale[0],
                    self.size_physical_pixels[1] as f32 / self.scale[0],
                ],
            };
            // render egui with gfx backend
            gfx_backend.prepare_render(gfx_output);
            gfx_backend.render();
            // present the frame and loop back
            gfx_backend.present(&mut self);
        }
    }

    fn get_live_physical_size_framebuffer(&mut self) -> [u32; 2] {
        let size = self.window.drawable_size();

        self.size_physical_pixels = [size.0, size.1];
        self.size_physical_pixels
    }

    fn get_settings(&self) -> &BackendSettings {
        &self.backend_settings
    }
}
impl OpenGLWindowContext for SDL2Backend {
    fn swap_buffers(&mut self) {
        self.window.gl_swap_window();
    }

    fn get_proc_address(&mut self, symbol: &str) -> *const core::ffi::c_void {
        self.window.subsystem().gl_get_proc_address(symbol) as *const core::ffi::c_void
    }
}
unsafe impl HasRawWindowHandle for SDL2Backend {
    fn raw_window_handle(&self) -> egui_backend::raw_window_handle::RawWindowHandle {
        self.window.raw_window_handle()
    }
}
impl SDL2Backend {
    pub fn tick(&mut self) {
        self.frame_events.clear();
        let mut modifiers = Modifiers::default();
        for pressed in self.event_pump.keyboard_state().pressed_scancodes() {
            match pressed {
                sdl2::keyboard::Scancode::LCtrl => {
                    modifiers.ctrl = true;
                }
                sdl2::keyboard::Scancode::LShift => {
                    modifiers.shift = true;
                }
                sdl2::keyboard::Scancode::LAlt => {
                    modifiers.alt = true;
                }
                sdl2::keyboard::Scancode::LGui => {
                    modifiers.command = true;
                }
                sdl2::keyboard::Scancode::RCtrl => {
                    modifiers.ctrl = true;
                }
                sdl2::keyboard::Scancode::RShift => {
                    modifiers.shift = true;
                }
                sdl2::keyboard::Scancode::RAlt => {
                    modifiers.alt = true;
                }
                sdl2::keyboard::Scancode::RGui => {
                    modifiers.command = true;
                }
                _ => {}
            }
        }
        for event in self.event_pump.poll_iter() {
            self.frame_events.push(event.clone());
            if let Some(egui_event) = match event {
                sdl2::event::Event::Quit { .. } => {
                    self.should_close = true;
                    None
                }
                sdl2::event::Event::Window { win_event, .. } => match win_event {
                    sdl2::event::WindowEvent::SizeChanged(w, h) => {
                        // assume w and h are in logical units because the docs are -_-
                        self.raw_input.screen_rect = Some(egui::Rect::from_two_pos(
                            Default::default(),
                            [w as f32, h as f32].into(),
                        ));
                        // physical width and height for framebuffer resize.
                        let (pw, ph) = self.window.drawable_size();
                        self.size_physical_pixels = [pw, ph];
                        self.latest_resize_event = true;

                        None
                    }
                    sdl2::event::WindowEvent::Close => {
                        self.should_close = true;
                        None
                    }
                    _ => None,
                },
                sdl2::event::Event::KeyDown {
                    scancode, keymod, ..
                } => {
                    let scan_code = scancode.expect("scan code empty");
                    let modifiers = sdl_to_egui_modifiers(keymod);
                    match scan_code {
                        Scancode::C => {
                            if modifiers.ctrl {
                                Some(Event::Copy)
                            } else {
                                None
                            }
                        }
                        Scancode::X => {
                            if modifiers.ctrl {
                                Some(Event::Cut)
                            } else {
                                None
                            }
                        }
                        Scancode::V => {
                            if modifiers.ctrl {
                                Some(Event::Text(
                                    self.window
                                        .subsystem()
                                        .clipboard()
                                        .clipboard_text()
                                        .unwrap_or_default(),
                                ))
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                    .or_else(|| {
                        sdl_to_egui_key(scan_code).map(|key| Event::Key {
                            key,
                            pressed: true,
                            modifiers,
                        })
                    })
                }

                sdl2::event::Event::KeyUp {
                    scancode, keymod, ..
                } => {
                    let scan_code = scancode.expect("scan code empty");
                    let modifiers = sdl_to_egui_modifiers(keymod);
                    match scan_code {
                        Scancode::C => {
                            if modifiers.ctrl {
                                Some(Event::Copy)
                            } else {
                                None
                            }
                        }
                        Scancode::X => {
                            if modifiers.ctrl {
                                Some(Event::Cut)
                            } else {
                                None
                            }
                        }
                        Scancode::V => {
                            if modifiers.ctrl {
                                Some(Event::Text(
                                    self.window
                                        .subsystem()
                                        .clipboard()
                                        .clipboard_text()
                                        .unwrap_or_default(),
                                ))
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                    .or_else(|| {
                        sdl_to_egui_key(scan_code).map(|key| Event::Key {
                            key,
                            pressed: false,
                            modifiers,
                        })
                    })
                }
                sdl2::event::Event::TextInput { text, .. } => Some(Event::Text(text)),
                sdl2::event::Event::MouseMotion { x, y, .. } => {
                    Some(Event::PointerMoved([x as f32, y as f32].into()))
                }
                sdl2::event::Event::MouseButtonDown {
                    mouse_btn, x, y, ..
                } => {
                    if let Some(pb) = sdl_to_egui_pointer_button(mouse_btn) {
                        Some(Event::PointerButton {
                            pos: [x as f32, y as f32].into(),
                            button: pb,
                            pressed: true,
                            modifiers,
                        })
                    } else {
                        None
                    }
                }
                sdl2::event::Event::MouseButtonUp {
                    mouse_btn, x, y, ..
                } => {
                    if let Some(pb) = sdl_to_egui_pointer_button(mouse_btn) {
                        Some(Event::PointerButton {
                            pos: [x as f32, y as f32].into(),
                            button: pb,
                            pressed: false,
                            modifiers,
                        })
                    } else {
                        None
                    }
                }
                sdl2::event::Event::MouseWheel { x, y, .. } => {
                    Some(Event::Scroll([x as f32 * 25.0, y as f32 * 25.0].into()))
                }

                sdl2::event::Event::DropFile { filename, .. } => {
                    self.raw_input.dropped_files.push(egui::DroppedFile {
                        path: Some(
                            PathBuf::from_str(&filename)
                                .expect("invalid path given for dropped file event"),
                        ),
                        name: "".to_string(),
                        last_modified: None,
                        bytes: None,
                    });
                    None
                }
                _ => todo!(),
            } {
                self.raw_input.events.push(egui_event);
            }
        }
    }
}

fn sdl_to_egui_pointer_button(mb: sdl2::mouse::MouseButton) -> Option<egui::PointerButton> {
    match mb {
        sdl2::mouse::MouseButton::Left => Some(PointerButton::Primary),
        sdl2::mouse::MouseButton::Middle => Some(PointerButton::Middle),
        sdl2::mouse::MouseButton::Right => Some(PointerButton::Secondary),
        sdl2::mouse::MouseButton::X1 => Some(PointerButton::Extra1),
        sdl2::mouse::MouseButton::X2 => Some(PointerButton::Extra2),
        _ => None,
    }
}

fn sdl_to_egui_modifiers(modifiers: sdl2::keyboard::Mod) -> Modifiers {
    use sdl2::keyboard::Mod;
    Modifiers {
        alt: modifiers.contains(Mod::LALTMOD) || modifiers.contains(Mod::RALTMOD),
        ctrl: modifiers.contains(Mod::LCTRLMOD) || modifiers.contains(Mod::RCTRLMOD),
        shift: modifiers.contains(Mod::LSHIFTMOD) || modifiers.contains(Mod::RSHIFTMOD),
        mac_cmd: false,
        command: modifiers.contains(Mod::LGUIMOD) || modifiers.contains(Mod::RGUIMOD),
    }
}
fn sdl_to_egui_key(key: Scancode) -> Option<egui::Key> {
    match key {
        Scancode::A => Some(Key::A),
        Scancode::B => Some(Key::B),
        Scancode::C => Some(Key::C),
        Scancode::D => Some(Key::D),
        Scancode::E => Some(Key::E),
        Scancode::F => Some(Key::F),
        Scancode::G => Some(Key::G),
        Scancode::H => Some(Key::H),
        Scancode::I => Some(Key::I),
        Scancode::J => Some(Key::J),
        Scancode::K => Some(Key::K),
        Scancode::L => Some(Key::L),
        Scancode::M => Some(Key::M),
        Scancode::N => Some(Key::N),
        Scancode::O => Some(Key::O),
        Scancode::P => Some(Key::P),
        Scancode::Q => Some(Key::Q),
        Scancode::R => Some(Key::R),
        Scancode::S => Some(Key::S),
        Scancode::T => Some(Key::T),
        Scancode::U => Some(Key::U),
        Scancode::V => Some(Key::V),
        Scancode::W => Some(Key::W),
        Scancode::X => Some(Key::X),
        Scancode::Y => Some(Key::Y),
        Scancode::Z => Some(Key::Z),
        Scancode::Num1 => Some(Key::Num1),
        Scancode::Num2 => Some(Key::Num2),
        Scancode::Num3 => Some(Key::Num3),
        Scancode::Num4 => Some(Key::Num4),
        Scancode::Num5 => Some(Key::Num5),
        Scancode::Num6 => Some(Key::Num6),
        Scancode::Num7 => Some(Key::Num7),
        Scancode::Num8 => Some(Key::Num8),
        Scancode::Num9 => Some(Key::Num9),
        Scancode::Num0 => Some(Key::Num0),
        Scancode::Return => Some(Key::Enter),
        Scancode::Escape => Some(Key::Escape),
        Scancode::Backspace => Some(Key::Backspace),
        Scancode::Tab => Some(Key::Tab),
        Scancode::Space => Some(Key::Space),
        Scancode::F1 => Some(Key::F1),
        Scancode::F2 => Some(Key::F2),
        Scancode::F3 => Some(Key::F3),
        Scancode::F4 => Some(Key::F4),
        Scancode::F5 => Some(Key::F5),
        Scancode::F6 => Some(Key::F6),
        Scancode::F7 => Some(Key::F7),
        Scancode::F8 => Some(Key::F8),
        Scancode::F9 => Some(Key::F9),
        Scancode::F10 => Some(Key::F10),
        Scancode::F11 => Some(Key::F11),
        Scancode::F12 => Some(Key::F12),
        Scancode::Insert => Some(Key::Insert),
        Scancode::Home => Some(Key::Home),
        Scancode::PageUp => Some(Key::PageUp),
        Scancode::Delete => Some(Key::Delete),
        Scancode::End => Some(Key::End),
        Scancode::PageDown => Some(Key::PageDown),
        Scancode::Right => Some(Key::ArrowRight),
        Scancode::Left => Some(Key::ArrowLeft),
        Scancode::Down => Some(Key::ArrowDown),
        Scancode::Up => Some(Key::ArrowUp),
        Scancode::F13 => Some(Key::F13),
        Scancode::F14 => Some(Key::F14),
        Scancode::F15 => Some(Key::F15),
        Scancode::F16 => Some(Key::F16),
        Scancode::F17 => Some(Key::F17),
        Scancode::F18 => Some(Key::F18),
        Scancode::F19 => Some(Key::F19),
        Scancode::F20 => Some(Key::F20),
        _ => None,
    }
}
