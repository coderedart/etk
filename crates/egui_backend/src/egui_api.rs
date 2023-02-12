use window_backend::WindowInput;


impl TakesInput for egui::Context {
    fn takes_input(&mut self, input: WindowInput) {}
}
pub trait TakesInput {
    fn takes_input(&mut self, input: WindowInput) {}
}
