

#[cfg(test)]
pub(crate) mod tests {

use sunrise_core::geometry::{Aabb, Vec3};

use sunrise_sim::{mobs::Who, projectiles::ProjectileWorld};

struct Air;
impl ProjectileWorld for Air {
        fn clip(&self, _: Vec3, _: Vec3) -> Option<(Vec3, [i32; 3], [i32; 3])> {
            None
        }
        fn in_water(&self, _: Aabb) -> bool {
            false
        }
        fn targets(&self) -> &[(Who, Aabb)] {
            &[]
        }
    }
}
