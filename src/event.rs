#[derive(Clone, Copy, Debug)]
pub enum AppEvent {
    Tick,
    PointerDown { x: f32, y: f32 },
    PointerUp { x: f32, y: f32 },
    PointerClick { x: f32, y: f32 },
    PointerMove { x: f32, y: f32 },
}
