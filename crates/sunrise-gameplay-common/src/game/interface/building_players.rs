

#[cfg(any(test, feature = "test-support"))]
pub mod tests {

use super::*;

use sunrise_core::geometry::Vec3;

pub fn player(conn: Option<u32>, x: f64, dimension: Dim, borrowed: bool) -> BuildingPlayer {
        BuildingPlayer {
            conn,
            dimension,
            borrowed,
            bounds: Some(Aabb::new(
                Vec3::new(x, 0.0, 0.0),
                Vec3::new(x + 0.6, 1.8, 0.6),
            )),
        }
    }
pub fn xs(players: &BuildingPlayers, dim: Dim) -> Vec<f64> {
        players.boxes(dim).iter().map(|(lo, _)| lo[0]).collect()
    }
}
