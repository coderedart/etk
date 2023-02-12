
/*
AuxiliaryClick

Pointer clicks on an element with something other than the primary button
DoubleClick

Pointer clicks twice on an element with the primary button
KeyDown

A key is pressed down
KeyTyped

A key is pressed down, and would normally produce a character value
KeyUp


Last held pointer button is released
PrimaryClick

Pointer clicks on an element with the primary button

*/
pub enum InputEvent {
    ScaleChanged(f32),
    WindowResized([f32; 2]),
    ViewportChanged([f32; 4]),
    // PointerEnter(PointerEnter),
    // PointerMove(PointerMove),
    // PointerLeave(PointerLeave),
    // PointerDown(PointerDown),
    // PointerUp(PointerUp),
    // PointerCancel(PointerCancel),
    // WheelMove(WheelMove),
    // KeyUp(KeyUp),
    // KeyDown(KeyDown),
    // KeyTyped(KeyTyped),
    // TextInput(String),
    // PrimaryClick(PrimaryClick),
    // DoubleClick(DoubleClick),
    // AuxiliaryClick(AuxiliaryClick),

    // PointerDown(PointerDow),
    // MouseScroll {
    //     delta: Vec2,
    // },
    // KeyChanged {
    //     key: Code,
    //     down: bool,
    // },
    // ModifiersChanged(Modifiers),
    // TextInput(char),
}

