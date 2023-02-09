//! This crate uses `glfw-passthrough` crate as a window backend for egui.

use egui::{Event, Key, PointerButton, Pos2, RawInput};
use egui_backend::*;
pub use glfw;
use glfw::Action;
use glfw::ClientApiHint;
use glfw::Context;
use glfw::Glfw;
use glfw::StandardCursor;
use glfw::WindowEvent;
use glfw::WindowHint;
use std::sync::mpsc::Receiver;
/// This is the window backend for egui using [`glfw`]
/// Most of the startup configuration is done inside [`default_glfw_callback()`] and [`default_window_callback()`]
/// These are passed to the `new` function using [`GlfwConfig`].
pub struct GlfwBackend {
    pub glfw: glfw::Glfw,
    pub events_receiver: Receiver<(f64, WindowEvent)>,
    pub window: glfw::Window,
    pub framebuffer_size_physical: [u32; 2],
    pub scale: [f32; 2],
    pub raw_input: RawInput,
    pub cursor_icon: glfw::StandardCursor,
    pub frame_events: Vec<WindowEvent>,
    pub resized_event_pending: bool,
    pub backend_config: BackendConfig,
    pub cursor_pos_physical_pixels: [f32; 2],
    pub cursor_inside_bounds: bool,
}
impl Drop for GlfwBackend {
    fn drop(&mut self) {
        tracing::warn!("dropping glfw backend");
    }
}
/// Signature of Glfw callback function inside [`GlfwConfig`]
/// we provide a default callback for common usecases -> [`default_glfw_callback()`]
pub type GlfwCallback = Box<dyn FnOnce(&mut Glfw)>;
/// This is usually provided by the default [`GlfwConfig`]
/// If you want to pass in a custom callback, then call this function within that custom callback.
/// If you want to override some setting, then call this callback inside your custom callback first and then override any hints/settings.
pub fn default_glfw_callback(glfw_context: &mut Glfw) {
    // emscripten doesn't have window hint string method. but window_hint fn actually uses it, so we will trigger link error.
    // on wasm32-unknown-unknown, prefer opengl (webgl)
    #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
    glfw_context.window_hint(WindowHint::ClientApi(ClientApiHint::OpenGl));
    // on desktop, don't use opengl by default
    #[cfg(not(target_arch = "wasm32"))]
    glfw_context.window_hint(WindowHint::ClientApi(ClientApiHint::NoApi));
}
/// This is the signature for window callback inside new function of [`GlfwBackend`]
pub type WindowCallback = Box<dyn FnOnce(&mut glfw::Window)>;
///
pub fn default_window_callback(window: &mut glfw::Window) {
    let should_poll = true;
    // set which events you care about
    window.set_pos_polling(should_poll);
    window.set_size_polling(should_poll);
    window.set_close_polling(should_poll);
    window.set_refresh_polling(should_poll);
    window.set_focus_polling(should_poll);
    window.set_iconify_polling(should_poll);
    window.set_framebuffer_size_polling(should_poll);
    window.set_key_polling(should_poll);
    window.set_char_polling(should_poll);
    window.set_mouse_button_polling(should_poll);
    window.set_cursor_pos_polling(should_poll);
    window.set_cursor_enter_polling(should_poll);
    window.set_scroll_polling(should_poll);
    window.set_drag_and_drop_polling(should_poll);
    #[cfg(not(target_os = "emscripten"))]
    {
        // emscripten doesn't have support for these
        window.set_char_mods_polling(should_poll);
        window.set_maximize_polling(should_poll);
        window.set_content_scale_polling(should_poll);
        window.set_store_lock_key_mods(should_poll);
    }
}
/// The configuration struct for Glfw Backend
/// passed in to [`WindowBackend::new()`] of [`GlfwBackend`]
pub struct GlfwConfig {
    /// This callback is called with `&mut Glfw` just before creating a window
    ///
    pub glfw_callback: GlfwCallback,
    /// This will be called right after window creation. you can use this to do things at startup like
    /// resizing, changing title, changing to fullscreen etc..
    pub window_callback: WindowCallback,
}
impl Default for GlfwConfig {
    fn default() -> Self {
        Self {
            glfw_callback: Box::new(default_glfw_callback),
            window_callback: Box::new(default_window_callback),
        }
    }
}
impl WindowBackend for GlfwBackend {
    type Configuration = GlfwConfig;
    type WindowType = glfw::Window;

    fn new(config: Self::Configuration, backend_config: BackendConfig) -> Self {
        let mut glfw_context =
            glfw::init(glfw::FAIL_ON_ERRORS).expect("failed to create glfw context");

        (config.glfw_callback)(&mut glfw_context);

        // create a window
        let (mut window, events_receiver) = glfw_context
            .create_window(800, 600, "Overlay Window", glfw::WindowMode::Windowed)
            .expect("failed to create glfw window");
        let api = window.get_client_api();
        if api == glfw::ffi::OPENGL_API || api == glfw::ffi::OPENGL_ES_API {
            window.make_current();
        }

        #[cfg(not(target_os = "emscripten"))]
        let scale = { window.get_content_scale() };
        #[cfg(target_os = "emscripten")]
        let scale = (1.0f32, 1.0f32); // maybe check framebuffer size vs window size to calculate scale?

        (config.window_callback)(&mut window);

        // collect details and keep them updated
        let (width, height) = window.get_framebuffer_size();
        let cursor_position = window.get_cursor_pos();
        let size_physical_pixels = [width as u32, height as u32];
        // set raw input screen rect details so that first frame
        // will have correct size even without any resize event
        let raw_input = RawInput {
            screen_rect: Some(egui::Rect::from_points(&[
                Default::default(),
                [width as f32 / scale.0, height as f32 / scale.0].into(),
            ])),
            pixels_per_point: Some(scale.0),
            ..Default::default()
        };
        Self {
            glfw: glfw_context,
            events_receiver,
            window,
            framebuffer_size_physical: size_physical_pixels,
            scale: [scale.0, scale.1],
            cursor_pos_physical_pixels: [cursor_position.0 as f32, cursor_position.1 as f32],
            raw_input,
            frame_events: vec![],
            resized_event_pending: true, // provide so that on first prepare frame, renderers can set their viewport sizes
            backend_config,
            cursor_icon: StandardCursor::Arrow,
            cursor_inside_bounds: false,
        }
    }

    fn take_raw_input(&mut self) -> RawInput {
        self.raw_input.take()
    }
    fn get_window(&mut self) -> Option<&mut Self::WindowType> {
        Some(&mut self.window)
    }

    fn get_live_physical_size_framebuffer(&mut self) -> Option<[u32; 2]> {
        let physical_fb_size = self.window.get_framebuffer_size();
        self.framebuffer_size_physical = [physical_fb_size.0 as u32, physical_fb_size.1 as u32];
        Some(self.framebuffer_size_physical)
    }

    fn get_raw_input(&mut self) -> RawInput {
        self.take_raw_input()
    }

    fn run_event_loop<U: EguiUserApp<Self> + 'static>(self, user_app: U) {
        // this is to make sure that user_app is dropped FIRST and then self.
        // we want the swapchain/surface to be destroyed before the window :) otherwise, we get a segfault.
        let mut tuple = (user_app, self);

        let mut wait_events_duration = std::time::Duration::ZERO;
        let callback = move || {
            let (user_app, window_backend) = &mut tuple;
            window_backend
                .glfw
                .wait_events_timeout(wait_events_duration.as_secs_f64());

            // gather events
            window_backend.tick();
            if window_backend.resized_event_pending {
                user_app.resize_framebuffer(window_backend);
                window_backend.resized_event_pending = false;
            }
            let logical_size = [
                window_backend.framebuffer_size_physical[0] as f32 / window_backend.scale[0],
                window_backend.framebuffer_size_physical[1] as f32 / window_backend.scale[1],
            ];
            // run userapp gui function. let user do anything he wants with window or gfx backends
            if let Some((platform_output, timeout)) = user_app.run(logical_size, window_backend) {
                wait_events_duration = timeout.min(std::time::Duration::from_secs(1));
                if !platform_output.copied_text.is_empty() {
                    window_backend
                        .window
                        .set_clipboard_string(&platform_output.copied_text);
                }
                window_backend.set_cursor(platform_output.cursor_icon);
            } else {
                wait_events_duration = std::time::Duration::ZERO;
            }
            #[cfg(not(target_os = "emscripten"))]
            window_backend.window.should_close()
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
                    tracing::warn!("event loop is exiting");
                    break;
                }
            }
        }
    }

    fn get_config(&self) -> &BackendConfig {
        &self.backend_config
    }

    fn swap_buffers(&mut self) {
        self.window.swap_buffers()
    }

    fn is_opengl(&self) -> bool {
        let api = self.window.get_client_api();
        match api {
            glfw::ffi::OPENGL_API | glfw::ffi::OPENGL_ES_API => true,
            glfw::ffi::NO_API => false,
            rest => panic!("invalid client api hint {rest}"),
        }
    }

    fn get_proc_address(&mut self, symbol: &str) -> *const core::ffi::c_void {
        self.window.get_proc_address(symbol)
    }
}

impl GlfwBackend {
    pub fn tick(&mut self) {
        self.frame_events.clear();
        // whether we got a cursor event in this frame.
        // if false, and the window is passthrough, we will manually get cursor pos and push it
        // otherwise, we do nothing.
        let mut cursor_event = false;
        for (_timestamp, event) in glfw::flush_messages(&self.events_receiver) {
            self.frame_events.push(event.clone());
            // if let &glfw::WindowEvent::CursorPos(..) = &event {
            //     continue;
            // }

            if let Some(ev) = match event {
                glfw::WindowEvent::FramebufferSize(w, h) => {
                    self.framebuffer_size_physical = [w as u32, h as u32];
                    self.resized_event_pending = true;
                    self.raw_input.screen_rect = Some(egui::Rect::from_two_pos(
                        Default::default(),
                        [w as f32 / self.scale[0], h as f32 / self.scale[1]].into(),
                    ));

                    None
                }
                glfw::WindowEvent::MouseButton(mb, a, m) => {
                    let emb = Event::PointerButton {
                        pos: Pos2 {
                            x: self.cursor_pos_physical_pixels[0] / self.scale[0],
                            y: self.cursor_pos_physical_pixels[1] / self.scale[1],
                        },
                        button: glfw_to_egui_pointer_button(mb),
                        pressed: glfw_to_egui_action(a),
                        modifiers: glfw_to_egui_modifers(m),
                    };
                    Some(emb)
                }
                // we scroll 25 pixels at a time
                glfw::WindowEvent::Scroll(x, y) => {
                    Some(Event::Scroll([x as f32 * 25.0, y as f32 * 25.0].into()))
                }
                glfw::WindowEvent::Key(k, _, a, m) => match k {
                    glfw::Key::C => {
                        if glfw_to_egui_action(a) && m.contains(glfw::Modifiers::Control) {
                            Some(Event::Copy)
                        } else {
                            None
                        }
                    }
                    glfw::Key::X => {
                        if glfw_to_egui_action(a) && m.contains(glfw::Modifiers::Control) {
                            Some(Event::Cut)
                        } else {
                            None
                        }
                    }
                    glfw::Key::V => {
                        if glfw_to_egui_action(a) && m.contains(glfw::Modifiers::Control) {
                            Some(Event::Text(
                                self.window.get_clipboard_string().unwrap_or_default(),
                            ))
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
                .or_else(|| {
                    glfw_to_egui_key(k).map(|key| Event::Key {
                        key,
                        pressed: glfw_to_egui_action(a),
                        modifiers: glfw_to_egui_modifers(m),
                    })
                }),
                glfw::WindowEvent::Char(c) => Some(Event::Text(c.to_string())),
                glfw::WindowEvent::ContentScale(x, y) => {
                    self.raw_input.pixels_per_point = Some(x);
                    self.scale = [x, y];
                    None
                }
                glfw::WindowEvent::Close => {
                    self.window.set_should_close(true);
                    None
                }

                glfw::WindowEvent::FileDrop(f) => {
                    self.raw_input
                        .dropped_files
                        .extend(f.into_iter().map(|p| egui::DroppedFile {
                            path: Some(p),
                            name: "".to_string(),
                            last_modified: None,
                            bytes: None,
                        }));
                    None
                }
                glfw::WindowEvent::CursorPos(x, y) => {
                    self.cursor_inside_bounds = true;
                    cursor_event = true;
                    self.cursor_pos_physical_pixels =
                        [x as f32 * self.scale[0], y as f32 * self.scale[1]];
                    Some(egui::Event::PointerMoved([x as f32, y as f32].into()))
                }
                WindowEvent::CursorEnter(c) => {
                    self.cursor_inside_bounds = c;
                    #[cfg(not(target_os = "emscripten"))]
                    if c {
                        None
                    } else if !self.window.is_mouse_passthrough() {
                        // if window is not passthrough, then we forward the event.
                        Some(Event::PointerGone)
                    } else {
                        // if it is passthrough, then we will let the simulated event take care of this
                        // because the pointer might still be within bounds even if we get cursor left event due to window losing focus due to passthrough
                        None
                    }
                    #[cfg(target_os = "emscripten")]
                    None
                }
                _rest => None,
            } {
                self.raw_input.events.push(ev);
            }
        }

        let cursor_position = self.window.get_cursor_pos();
        let cursor_position = [cursor_position.0 as f32, cursor_position.1 as f32];

        // when there's no cursor event and window is passthrough, then, simulate mouse events
        #[cfg(not(target_os = "emscripten"))]
        if !cursor_event && self.window.is_mouse_passthrough() {
            let window_bounds = egui_backend::egui::Rect::from_two_pos(
                Default::default(),
                egui::pos2(
                    self.framebuffer_size_physical[0] as f32,
                    self.framebuffer_size_physical[1] as f32,
                ),
            );
            // if cursor within window bounds
            if window_bounds.contains(cursor_position.into()) {
                // if cursor position has changed since last frame.
                if cursor_position != self.cursor_pos_physical_pixels {
                    // we will manually push the cursor moved event.
                    self.raw_input.events.push(Event::PointerMoved(
                        [
                            cursor_position[0] / self.scale[0],
                            cursor_position[1] / self.scale[1],
                        ]
                        .into(),
                    ));
                }
                self.cursor_inside_bounds = true;
            } else {
                // if present cursor is out of bounds for the first time, we need to simulate a pointer gone event.
                // we use the cursor inside bounds flag to keep track of whether the cursor was active
                if self.cursor_inside_bounds {
                    self.raw_input.events.push(Event::PointerGone);
                    // will only be true if we set a new pointermoved event using window event loop or cursor coming into bounds again.
                    self.cursor_inside_bounds = false;
                }
            }
        }
        self.cursor_pos_physical_pixels = cursor_position;
    }
    fn set_cursor(&mut self, cursor: egui::CursorIcon) {
        let cursor = egui_to_glfw_cursor(cursor);
        if cursor != self.cursor_icon {
            self.cursor_icon = cursor;
            self.window.set_cursor(Some(glfw::Cursor::standard(cursor)));
        }
    }
}

/// a function to get the matching egui key event for a given glfw key. egui does not support all the keys provided here.
fn glfw_to_egui_key(key: glfw::Key) -> Option<Key> {
    match key {
        glfw::Key::Space => Some(Key::Space),
        glfw::Key::Num0 => Some(Key::Num0),
        glfw::Key::Num1 => Some(Key::Num1),
        glfw::Key::Num2 => Some(Key::Num2),
        glfw::Key::Num3 => Some(Key::Num3),
        glfw::Key::Num4 => Some(Key::Num4),
        glfw::Key::Num5 => Some(Key::Num5),
        glfw::Key::Num6 => Some(Key::Num6),
        glfw::Key::Num7 => Some(Key::Num7),
        glfw::Key::Num8 => Some(Key::Num8),
        glfw::Key::Num9 => Some(Key::Num9),
        glfw::Key::A => Some(Key::A),
        glfw::Key::B => Some(Key::B),
        glfw::Key::C => Some(Key::C),
        glfw::Key::D => Some(Key::D),
        glfw::Key::E => Some(Key::E),
        glfw::Key::F => Some(Key::F),
        glfw::Key::G => Some(Key::G),
        glfw::Key::H => Some(Key::H),
        glfw::Key::I => Some(Key::I),
        glfw::Key::J => Some(Key::J),
        glfw::Key::K => Some(Key::K),
        glfw::Key::L => Some(Key::L),
        glfw::Key::M => Some(Key::M),
        glfw::Key::N => Some(Key::N),
        glfw::Key::O => Some(Key::O),
        glfw::Key::P => Some(Key::P),
        glfw::Key::Q => Some(Key::Q),
        glfw::Key::R => Some(Key::R),
        glfw::Key::S => Some(Key::S),
        glfw::Key::T => Some(Key::T),
        glfw::Key::U => Some(Key::U),
        glfw::Key::V => Some(Key::V),
        glfw::Key::W => Some(Key::W),
        glfw::Key::X => Some(Key::X),
        glfw::Key::Y => Some(Key::Y),
        glfw::Key::Z => Some(Key::Z),
        glfw::Key::Escape => Some(Key::Escape),
        glfw::Key::Enter => Some(Key::Enter),
        glfw::Key::Tab => Some(Key::Tab),
        glfw::Key::Backspace => Some(Key::Backspace),
        glfw::Key::Insert => Some(Key::Insert),
        glfw::Key::Delete => Some(Key::Delete),
        glfw::Key::Right => Some(Key::ArrowRight),
        glfw::Key::Left => Some(Key::ArrowLeft),
        glfw::Key::Down => Some(Key::ArrowDown),
        glfw::Key::Up => Some(Key::ArrowUp),
        glfw::Key::PageUp => Some(Key::PageUp),
        glfw::Key::PageDown => Some(Key::PageDown),
        glfw::Key::Home => Some(Key::Home),
        glfw::Key::End => Some(Key::End),
        _ => None,
    }
}

pub fn glfw_to_egui_modifers(modifiers: glfw::Modifiers) -> egui::Modifiers {
    egui::Modifiers {
        alt: modifiers.contains(glfw::Modifiers::Alt),
        ctrl: modifiers.contains(glfw::Modifiers::Control),
        shift: modifiers.contains(glfw::Modifiers::Shift),
        mac_cmd: false,
        command: modifiers.contains(glfw::Modifiers::Control),
    }
}

pub fn glfw_to_egui_pointer_button(mb: glfw::MouseButton) -> PointerButton {
    match mb {
        glfw::MouseButton::Button1 => PointerButton::Primary,
        glfw::MouseButton::Button2 => PointerButton::Secondary,
        glfw::MouseButton::Button3 => PointerButton::Middle,
        glfw::MouseButton::Button4 => PointerButton::Extra1,
        glfw::MouseButton::Button5 => PointerButton::Extra2,
        _ => PointerButton::Secondary,
    }
}

pub fn glfw_to_egui_action(a: glfw::Action) -> bool {
    match a {
        Action::Release => false,
        Action::Press => true,
        Action::Repeat => true,
    }
}
/// This converts egui's cursor  icon into glfw's cursor which can be set by glfw.
/// we can get some sample cursor images and use them in place of missing icons (like diagonal resizing cursor)
pub fn egui_to_glfw_cursor(cursor: egui::CursorIcon) -> glfw::StandardCursor {
    match cursor {
        egui::CursorIcon::Default => StandardCursor::Arrow,
        egui::CursorIcon::Crosshair => StandardCursor::Crosshair,
        egui::CursorIcon::VerticalText | egui::CursorIcon::Text => StandardCursor::IBeam,
        egui::CursorIcon::Grab | egui::CursorIcon::Grabbing => StandardCursor::Hand,
        egui::CursorIcon::ResizeColumn
        | egui::CursorIcon::ResizeWest
        | egui::CursorIcon::ResizeEast
        | egui::CursorIcon::ResizeHorizontal => StandardCursor::HResize,
        egui::CursorIcon::ResizeRow
        | egui::CursorIcon::ResizeNorth
        | egui::CursorIcon::ResizeSouth
        | egui::CursorIcon::ResizeVertical => StandardCursor::VResize,
        _ => StandardCursor::Arrow,
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
