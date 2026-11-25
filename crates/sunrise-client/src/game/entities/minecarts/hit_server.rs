#[allow(unused_imports)]
pub(in crate::game::server_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::Target;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::hit::CartHit;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::minecarts::hit::*;

impl CartHit {
    /// The acting player's own hit (`Player.attack`'s `playerAttack`): its
    /// direct entity is the player in place (the host, or the guest whose turn
    /// it is), whose state `anger_piglins` reads.
    pub(in crate::game) fn player() -> Self {
        Self {
            by: Some(Target::Player),
            by_player: true,
        }
    }
    /// A projectile's hit: `getEntity()` is its owner, `getDirectEntity()` the
    /// projectile itself, so it angers no piglin.
    pub(in crate::game) fn projectile(owner: Option<Who>, guest: Option<u32>) -> Self {
        let by = match owner {
            Some(Who::Player) => Some(guest.map_or(Target::Player, Target::Guest)),
            Some(Who::Mob(id)) => Some(Target::Mob(id)),
            None => None,
        };
        Self {
            by,
            by_player: false,
        }
    }
}
