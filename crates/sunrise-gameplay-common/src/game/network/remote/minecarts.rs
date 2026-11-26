use super::*;

/// `NewMinecartBehavior.MinecartStep`, including the server's rail pitch.
#[derive(Clone, Copy)]
pub struct CartStep {
    pub(in crate::game) position: Vec3,
    pub(in crate::game) movement: Vec3,
    pub(in crate::game) yaw: f32,
    pub(in crate::game) pitch: f32,
    pub(in crate::game) weight: f32,
}

/// The client's state of a minecart on `NewMinecartBehavior`: the steps the
/// server sent (`lerpSteps`), the ones being played over 3 ticks
/// (`currentLerpSteps`) and where the cart was when they began (`oldLerp`).
pub struct CartLerp {
    pub pending: Vec<CartStep>,
    pub current: Vec<CartStep>,
    pub total_weight: f64,
    pub delay: i32,
    pub old: CartStep,
}
