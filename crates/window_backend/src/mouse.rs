//! https://w3c.github.io/uievents/#events-mouseevents
//!
// PointerEnter(PointerEnter),
// PointerMove(PointerMove),
// PointerLeave(PointerLeave),
// PointerDown(PointerDown),
// PointerUp(PointerUp),
// PointerCancel(PointerCancel),

use keyboard_types::Modifiers;

pub struct MouseEvent {
    /// The horizontal coordinate at which the event occurred relative to the viewport associated with the event. 
    /// dev note: x and y are in points inside window space
    pub client_x: f32,
    /// The vertical coordinate at which the event occurred relative to the viewport associated with the event. 
    pub client_y: f32,
    
    /// modifiers pressed during this event
    pub modifiers: Modifiers,
    pub button: MouseButton,
}
impl MouseEvent {
    
}
#[repr(u8)]
pub enum MouseButton {
    /// the primary button of the device (in general, the left button or the only button on single-button devices,
    /// used to activate a user interface control or select text) or the un-initialized value.
    Primary = 0,
    /// the auxiliary button (in general, the middle button, often combined with a mouse wheel).
    Auxilliary = 1,
    /// the secondary button (in general, the right button, often used to display a context menu).
    Secondary = 2,
    /// Extra1 button. Usually used for browser back .
    X1 = 3,
    /// Extra2 button. Usually used for browser forward.
    X2 = 4,
    /// Any other extra mouse buttons. the value can be negative or greater than 2. 
    /// dev note: I recommend only having values above 4, so that you can use X1/X2 above instead.
    Custom(i16),
}

bitflags::bitflags! {
    /// During any mouse events, buttons MUST be used to indicate which combination of mouse buttons are currently being pressed, expressed as a bitmask.
    pub struct MouseButtons: u16 {
        /// 0: No button or un-initialized
        const NONE = 0;
        /// 1: Primary button (usually the left button)
        const PRIMARY = 1;
        /// 2: Secondary button (usually the right button)
        const SECONDARY = 1 << 1;
        /// 4: Auxiliary button (usually the mouse wheel button or middle button)
        const AUXILIARY = 1 << 2;
        /// 8: 4th button (typically the "Browser Back" button)
        const X1 = 1 << 3;
        /// 16 : 5th button (typically the "Browser Forward" button) and so on..
        const X2 = 1 << 4;
        const X3 = 1 << 5;
        const X4 = 1 << 6;
        const X5 = 1 << 7;
        const X6 = 1 << 8;
        const X7 = 1 << 9;
        const X8 = 1 << 10;
        const X9 = 1 << 11;
        const X10 = 1 << 12;
        const X11 = 1 << 13;
        const X12 = 1 << 14;
        const X13 = 1 << 15;
    }
}
