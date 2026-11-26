

#[cfg(any(test, feature = "test-support"))]
pub mod tests {

use super::*;

/// The eye sprite's atlas region; only its UVs are used here.
    pub fn eye_rect() -> sunrise_client::hud::Rect {
        sunrise_client::hud::Rect {
            u0: 0.0,
            v0: 0.0,
            u1: 1.0,
            v1: 1.0,
            w: 16,
            h: 16,
        }
    }
}
