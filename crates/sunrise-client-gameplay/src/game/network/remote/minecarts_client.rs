use super::*;
use sunrise_gameplay_common::game::remote::minecarts::CartLerp;
use sunrise_gameplay_common::game::remote::minecarts::CartStep;

/// `NewMinecartBehavior.getCartLerpPosition`, `getCartLerpMovements` and
/// `getCartLerpYRot` / `getCartLerpXRot` at `partial`: the step the weights
/// put this moment in, and how far along it from the one before.
pub(in crate::game) fn cart_lerp_at(lerp: &CartLerp, partial: f32) -> (Vec3, Vec3, f32, f32) {
    let alpha = ((3 - lerp.delay) as f32 + partial) / 3.0;
    let mut count_up = 0.0f32;
    let mut in_step = 1.0f32;
    let mut found = None;
    for (i, s) in lerp.current.iter().enumerate() {
        if s.weight <= 0.0 {
            continue;
        }
        count_up += s.weight;
        if f64::from(count_up) >= lerp.total_weight * f64::from(alpha) {
            let before = count_up - s.weight;
            in_step = ((f64::from(alpha) * lerp.total_weight - f64::from(before))
                / f64::from(s.weight)) as f32;
            found = Some(i);
            break;
        }
    }
    let index = found.unwrap_or(lerp.current.len() - 1);
    let current = lerp.current[index];
    let previous = if index > 0 {
        lerp.current[index - 1]
    } else {
        lerp.old
    };
    let t = f64::from(in_step);
    let lerp = |a: Vec3, b: Vec3| {
        Vec3::new(
            a.x + t * (b.x - a.x),
            a.y + t * (b.y - a.y),
            a.z + t * (b.z - a.z),
        )
    };
    (
        lerp(previous.position, current.position),
        lerp(previous.movement, current.movement),
        previous.yaw + in_step * wrap_degrees(current.yaw - previous.yaw),
        previous.pitch + in_step * wrap_degrees(current.pitch - previous.pitch),
    )
}
pub(in crate::game) fn new_cart_lerp() -> CartLerp {
    CartLerp {
        pending: Vec::new(),
        current: Vec::new(),
        total_weight: 0.0,
        delay: 0,
        old: CartStep {
            position: Vec3::ZERO,
            movement: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            weight: 0.0,
        },
    }
}
/// `NewMinecartBehavior.lerpClientPositionAndRotation`: once the steps
/// being played are used up, the queued ones become the current ones;
/// the cart then sits where the steps put it at the end of this tick.
/// False when it has no steps to play.
pub(in crate::game) fn tick_cart_lerp(lerp: &mut CartLerp, c: &mut super::super::minecarts::Minecart) -> bool {
    lerp.delay -= 1;
    if lerp.delay <= 0 {
        lerp.old = CartStep {
            position: c.position,
            movement: c.velocity,
            yaw: c.yaw,
            pitch: c.pitch,
            weight: 0.0,
        };
        lerp.current.clear();
        if !lerp.pending.is_empty() {
            lerp.current.append(&mut lerp.pending);
            lerp.total_weight = lerp.current.iter().map(|s| f64::from(s.weight)).sum();
            lerp.delay = if lerp.total_weight == 0.0 { 0 } else { 3 };
        }
    }
    if lerp.current.is_empty() {
        return false;
    }
    let (position, movement, yaw, pitch) = cart_lerp_at(lerp, 1.0);
    c.position = position;
    c.velocity = movement;
    c.yaw = yaw;
    c.pitch = pitch;
    true
}

#[cfg(feature = "client")]
impl crate::game::ClientGame<'_> {
    /// `ClientPacketListener.handleMinecartAlongTrack`: the steps join the ones
    /// the minecart has queued, if it runs `NewMinecartBehavior`.
    pub(in crate::game) fn remote_move_minecart(&mut self, p: play::MoveMinecartAlongTrack) {
        if !self.common.minecart_improvements() || !self.common.carts.iter().any(|c| c.id == p.id as u32) {
            return;
        }
        let Some(r) = self.client.remote.as_mut() else {
            return;
        };
        let at = |v: [f64; 3]| Vec3::new(v[0], v[1], v[2]);
        r.cart_lerps
            .entry(p.id)
            .or_insert_with(new_cart_lerp)
            .pending
            .extend(p.steps.iter().map(|s| CartStep {
                position: at(s.position),
                movement: at(s.movement),
                yaw: sunrise_protocol::buf::unpack_degrees(s.yaw),
                pitch: sunrise_protocol::buf::unpack_degrees(s.pitch),
                weight: s.weight,
            }));
    }
}
