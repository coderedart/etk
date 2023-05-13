use std::{path::PathBuf, str::FromStr};

use egui::{Event, Key, Modifiers, PointerButton, RawInput};
use egui_backend::WindowBackend;
use egui_backend::*;
use sdl2::{keyboard::Scancode, video::Window, Sdl};

pub use sdl2;
pub struct Sdl2Backend {
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
    pub backend_config: BackendConfig,
}
pub type WindowCreatorCallback = Box<dyn FnOnce(&sdl2::VideoSubsystem) -> sdl2::video::Window>;
pub fn default_window_creator_callback(
    video_subsystem: &sdl2::VideoSubsystem,
) -> sdl2::video::Window {
    let mut window_builder = video_subsystem.window("default title", 800, 600);
    // use opengl on wasm
    #[cfg(target_arch = "wasm32")]
    window_builder.opengl();
    #[cfg(not(target_arch = "wasm32"))]
    window_builder.vulkan();
    window_builder.allow_highdpi();
    window_builder.resizable();
    window_builder.build().expect("failed to create a window")
}
pub struct SDL2Config {
    pub window_creator_callback: WindowCreatorCallback,
}
impl Default for SDL2Config {
    fn default() -> Self {
        Self {
            window_creator_callback: Box::new(default_window_creator_callback),
        }
    }
}

impl WindowBackend for Sdl2Backend {
    type Configuration = SDL2Config;

    type WindowType = sdl2::video::Window;

    fn new(config: Self::Configuration, backend_config: BackendConfig) -> Self {
        let sdl_context = sdl2::init().unwrap();
        let video_subsystem = sdl_context.video().unwrap();
        let event_pump = sdl_context.event_pump().expect("failed to get event pump");
        let window = (config.window_creator_callback)(&video_subsystem);
        let window_flags = window.window_flags();
        let opengl_window_flag: u32 = sdl2::sys::SDL_WindowFlags::SDL_WINDOW_OPENGL as u32;
        let gl_context = if (window_flags & opengl_window_flag) != 0 {
            tracing::warn!("sdl2 window is created with opengl context. making it current");
            // if window flags has opengl flag, create and make the context current.
            let gl_context = window
                .gl_create_context()
                .expect("failed to create opengl context");
            window
                .gl_make_current(&gl_context)
                .expect("failed to make gl context current");
            Some(gl_context)
        } else {
            None
        };
        let mouse_state = event_pump.relative_mouse_state();
        let cursor_pos_physical_pixels = [mouse_state.x() as f32, mouse_state.y() as f32];
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
            backend_config,
        }
    }

    fn take_raw_input(&mut self) -> egui::RawInput {
        self.raw_input.take()
    }

    fn get_window(&mut self) -> Option<&mut Self::WindowType> {
        Some(&mut self.window)
    }

    fn get_live_physical_size_framebuffer(&mut self) -> Option<[u32; 2]> {
        let size = self.window.drawable_size();
        self.size_physical_pixels = [size.0, size.1];
        Some(self.size_physical_pixels)
    }

    fn run_event_loop<U: UserApp<UserWindowBackend = Self> + 'static>(mut user_app: U) {
        let mut events_wait_duration = std::time::Duration::ZERO;
        let callback = move || {
            // gather events
            user_app.get_all().0.tick(events_wait_duration);
            // prepare surface for drawing
            if user_app.get_all().0.latest_resize_event {
                user_app.resize_framebuffer();
                user_app.get_all().0.latest_resize_event = false;
            }
            // run userapp gui function. let user do anything he wants with window or gfx backends
            let logical_size = [
                user_app.get_all().0.size_physical_pixels[0] as f32 / user_app.get_all().0.scale[0],
                user_app.get_all().0.size_physical_pixels[1] as f32 / user_app.get_all().0.scale[1],
            ];
            if let Some((platform_output, timeout)) = user_app.run(logical_size) {
                events_wait_duration = timeout;
                if !platform_output.copied_text.is_empty() {
                    if let Err(err) = user_app
                        .get_all()
                        .0
                        .window
                        .subsystem()
                        .clipboard()
                        .set_clipboard_text(&platform_output.copied_text)
                    {
                        tracing::error!("failed to set clipboard text due to error: {err}");
                    }
                }
            } else {
                events_wait_duration = std::time::Duration::ZERO
            }
            // on non emscripten targets (desktop), return a boolean indicating if event loop should close.
            #[cfg(not(target_os = "emscripten"))]
            user_app.get_all().0.should_close
        };
        // on emscripten, just keep calling forever i guess.
        #[cfg(target_os = "emscripten")]
        set_main_loop_callback(callback);

        #[cfg(not(target_os = "emscripten"))]
        {
            let mut callback = callback;
            loop {
                // returns if loop should close.
                if callback() {
                    break;
                }
            }
        }
    }

    fn get_config(&self) -> &BackendConfig {
        &self.backend_config
    }
    fn swap_buffers(&mut self) {
        self.window.gl_swap_window();
    }

    fn get_proc_address(&mut self, symbol: &str) -> *const core::ffi::c_void {
        self.window.subsystem().gl_get_proc_address(symbol) as *const core::ffi::c_void
    }

    fn is_opengl(&self) -> bool {
        (self.window.window_flags() & sdl2::sys::SDL_WindowFlags::SDL_WINDOW_OPENGL as u32) != 0
    }
    fn set_window_title(&mut self, title: &str) {
        self.window
            .set_title(title)
            .expect("failed to set sdl window title");
    }

    fn get_window_position(&mut self) -> Option<[f32; 2]> {
        let pos = self.window.position();
        [pos.0 as f32, pos.1 as f32].into()
    }

    fn set_window_position(&mut self, pos: [f32; 2]) {
        self.window.set_position(
            sdl2::video::WindowPos::Positioned(pos[0] as i32),
            sdl2::video::WindowPos::Positioned(pos[1] as i32),
        );
    }

    fn get_window_size(&mut self) -> Option<[f32; 2]> {
        let size = self.window.size();
        [size.0 as f32, size.1 as f32].into()
    }

    fn set_window_size(&mut self, size: [f32; 2]) {
        self.window
            .set_size(size[0] as u32, size[1] as u32)
            .expect("failed to set sdl window size");
    }

    fn get_window_minimized(&mut self) -> Option<bool> {
        self.window.is_minimized().into()
    }

    fn set_minimize_window(&mut self, min: bool) {
        if min {
            self.window.minimize();
        } else {
            self.window.restore();
        }
    }

    fn get_window_maximized(&mut self) -> Option<bool> {
        self.window.is_maximized().into()
    }

    fn set_maximize_window(&mut self, max: bool) {
        if max {
            self.window.maximize();
        } else {
            self.window.restore();
        }
    }

    fn get_window_visibility(&mut self) -> Option<bool> {
        // self.window.sho().into()
        unimplemented!()
    }

    fn set_window_visibility(&mut self, vis: bool) {
        if vis {
            self.window.show()
        } else {
            self.window.hide();
        }
    }

    fn get_always_on_top(&mut self) -> Option<bool> {
        unimplemented!()
    }

    fn set_always_on_top(&mut self, _always_on_top: bool) {
        unimplemented!()
    }

    fn get_passthrough(&mut self) -> Option<bool> {
        todo!()
    }

    fn set_passthrough(&mut self, _passthrough: bool) {
        todo!()
    }
}

impl Sdl2Backend {
    pub fn tick(&mut self, events_wait_duration: std::time::Duration) {
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
        // first wait for the event or until time out.
        if let Some(event) = self
            .event_pump
            .wait_event_timeout(events_wait_duration.as_millis() as u32)
        {
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
            self.on_event(modifiers, event);
        }
        // after the timeout or an event before timeout, drain the rest of the events from pump
        let mut events = vec![]; // use vec to avoid borrow checker error
        for event in self.event_pump.poll_iter() {
            events.push(event);
        }
        for event in events {
            self.on_event(modifiers, event);
        }
    }

    fn on_event(&mut self, modifiers: Modifiers, event: sdl2::event::Event) {
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
                sdl2::event::WindowEvent::Leave => Some(Event::PointerGone),
                _ => None,
            },
            sdl2::event::Event::KeyDown {
                scancode,
                keymod,
                repeat,
                ..
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
                            match self.window.subsystem().clipboard().clipboard_text() {
                                Ok(text) => Some(Event::Text(text)),
                                Err(err) => {
                                    tracing::error!(
                                        "failed to get clipboard text due to error: {err}"
                                    );
                                    None
                                }
                            }
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
                        repeat,
                    })
                })
            }

            sdl2::event::Event::KeyUp {
                scancode,
                keymod,
                repeat,
                ..
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
                        repeat,
                    })
                })
            }
            sdl2::event::Event::TextInput { text, .. } => Some(Event::Text(text)),
            sdl2::event::Event::MouseMotion { x, y, .. } => {
                Some(Event::PointerMoved([x as f32, y as f32].into()))
            }
            sdl2::event::Event::MouseButtonDown {
                mouse_btn, x, y, ..
            } => sdl_to_egui_pointer_button(mouse_btn).map(|pb| Event::PointerButton {
                pos: [x as f32, y as f32].into(),
                button: pb,
                pressed: true,
                modifiers,
            }),
            sdl2::event::Event::MouseButtonUp {
                mouse_btn, x, y, ..
            } => sdl_to_egui_pointer_button(mouse_btn).map(|pb| Event::PointerButton {
                pos: [x as f32, y as f32].into(),
                button: pb,
                pressed: false,
                modifiers,
            }),
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
            sdl2::event::Event::AppTerminating { .. } => {
                tracing::info!("app terminating event");
                None
            }
            rest => {
                unimplemented!("sdl2 egui backend doesn't support this kinda event yet: {rest:#?}")
            }
        } {
            self.raw_input.events.push(egui_event);
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

#[allow(non_camel_case_types)]
type em_callback_func = unsafe extern "C" fn();

extern "C" {
    // This extern is built in by Emscripten.
    pub fn emscripten_run_script_int(x: *const std::ffi::c_uchar) -> std::ffi::c_int;
    pub fn emscripten_cancel_main_loop();
    pub fn emscripten_set_main_loop(
        func: em_callback_func,
        fps: std::ffi::c_int,
        simulate_infinite_loop: std::ffi::c_int,
    );
}

thread_local!(static MAIN_LOOP_CALLBACK: std::cell::RefCell<Option<Box<dyn FnMut()>>>  = std::cell::RefCell::new(None));

pub fn set_main_loop_callback<F: 'static>(callback: F)
where
    F: FnMut(),
{
    MAIN_LOOP_CALLBACK.with(|log| {
        *log.borrow_mut() = Some(Box::new(callback));
    });

    unsafe {
        emscripten_set_main_loop(wrapper::<F>, 0, 1);
    }

    #[allow(clippy::extra_unused_type_parameters)]
    extern "C" fn wrapper<F>()
    where
        F: FnMut(),
    {
        MAIN_LOOP_CALLBACK.with(|z| {
            if let Some(ref mut callback) = *z.borrow_mut() {
                callback();
            }
        });
    }
}
