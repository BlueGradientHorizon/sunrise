

#[cfg(any(test, feature = "test-support"))]
pub mod tests {

use super::*;

pub fn boat() -> Boat {
        Boat::new(1, Arc::from("oak"), Vec3::ZERO, 0.0)
    }
}
