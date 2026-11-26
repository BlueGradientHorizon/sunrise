

#[cfg(any(test, feature = "test-support"))]
pub mod tests {

use super::*;

use std::collections::HashMap;

/// A mob of the prepared resources' type `kind` (None without them).
    pub fn mob(kind: &str) -> Option<sunrise_sim::mobs::Mob> {
        let root = std::env::var_os("SUNRISE_RESOURCES")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../.local/sunrise-client")
                    .join(sunrise_core::VANILLA_VERSION)
            });
        let types = sunrise_sim::mobs::MobTypes::load(&root).ok()?;
        let t = std::sync::Arc::new(types.types.get(kind)?.clone());
        Some(sunrise_sim::mobs::Mob::new(
            7,
            t,
            sunrise_core::geometry::Vec3::new(1.5, 64.0, -2.5),
            0.0,
        ))
    }
#[cfg(test)]
/// A lava sea up to y 30 with full blocks placed in it; air above.
    pub(in crate::game) struct Sea {
        pub(in crate::game) blocks: HashMap<[i32; 3], ()>,
    }
#[cfg(test)]
impl Sea {
        pub(in crate::game::server_side) fn with(blocks: &[[i32; 3]]) -> Self {
            Self {
                blocks: blocks.iter().map(|&b| (b, ())).collect(),
            }
        }
        pub(in crate::game::server_side) fn solid(&self, p: [i32; 3]) -> bool {
            self.blocks.contains_key(&p)
        }
    }
#[cfg(test)]
impl DismountWorld for Sea {
        fn lava(&self, p: [i32; 3]) -> bool {
            p[1] <= 30 && !self.solid(p)
        }
        fn floor_height(&self, p: [i32; 3]) -> f64 {
            if self.solid(p) {
                1.0
            } else if self.solid([p[0], p[1] - 1, p[2]]) {
                0.0
            } else {
                f64::NEG_INFINITY
            }
        }
        fn can_dismount_to(&self, bounds: Aabb) -> bool {
            let (lo, hi) = (bounds.min_position(), bounds.max_position());
            let cells = |a: f64, b: f64| a.floor() as i32..=(b - 1.0e-7).floor() as i32;
            !cells(lo.x, hi.x).any(|x| {
                cells(lo.y, hi.y).any(|y| cells(lo.z, hi.z).any(|z| self.solid([x, y, z])))
            })
        }
    }
/// The top of a lava source at y 30 (`FlowingFluid.getOwnHeight`, 8/9).
    pub const SURFACE: f64 = 30.0 + 8.0 / 9.0;
pub fn rider(yaw: f32) -> Rider {
        Rider {
            yaw,
            width: 0.6,
            poses: DISMOUNT_POSES
                .iter()
                .map(|&pose| (pose, pose.dimensions(1.0)))
                .collect(),
            main_arm_left: false,
        }
    }
pub fn block_of(at: Vec3) -> [i32; 3] {
        [
            at.x.floor() as i32,
            at.y.floor() as i32,
            at.z.floor() as i32,
        ]
    }
#[cfg(test)]
/// What one feeding made the level show.
    #[derive(Debug)]
    pub(in crate::game) struct Shown {
        pub(in crate::game) sounds: Vec<(String, [f64; 3], f32, f32)>,
        pub(in crate::game) events: Vec<(&'static str, [f64; 3], Option<Who>)>,
    }
/// `AbstractHorse.handleEating`'s growth particle position draws:
    /// `getRandomX(1.0)`, `getRandomY()` and `getRandomZ(1.0)` take a double each.
    pub fn sparkle_draws(r: &mut sunrise_core::random::LegacyRandom) {
        for _ in 0..3 {
            r.next_double();
        }
    }
}
