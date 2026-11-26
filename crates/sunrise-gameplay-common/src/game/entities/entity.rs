

#[cfg(any(test, feature = "test-support"))]
pub mod join_order_tests {

use sunrise_core::geometry::Vec3;

pub fn at() -> Vec3 {
        Vec3::new(0.5, 100.0, 0.5)
    }
}
