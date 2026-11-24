#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Arg, Context, Dim, Game, R, Target, data::DataRef, err, wrap_degrees,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::execute::ExecState;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::execute::Source;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::execute::StoreEntry;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::execute::StoreTarget;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::execute::fork_limit_reached;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::execute::is_player_in_place;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::execute::java_int;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::execute::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::target_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    command::CommandError, nbt_path::NbtPath, text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::trig;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::nbt::Tag;

impl Game<'_> {
    pub(in crate::game) fn target_eye_height(&self, t: Target) -> f64 {
        match t {
            Target::Player => self
                .player
                .as_ref()
                .map_or(1.62, |p| f64::from(p.dimensions.eye_height)),
            Target::Guest(conn) => self.command_guest(conn).map_or(1.62, |g| g.eye_height()),
            Target::Mob(id) => self
                .mobs
                .iter()
                .find(|m| m.id == id)
                .map_or(0.0, |m| m.eye_y() - m.position.y),
            // ArmorStand.getDefaultDimensions: a marker's are empty, a small
            // stand's are BABY_DIMENSIONS, whose eye height is 0.9875F.
            Target::Stand(id) => match self.armor_stands.iter().find(|a| a.id == id) {
                Some(a) if a.marker => 0.0,
                Some(a) if a.small => f64::from(0.9875f32),
                _ => self.type_eye_height(&self.target_type(t)),
            },
            Target::Other(id) => self.client.interactions.get(&(id as i32)).map_or_else(
                || self.type_eye_height(&self.target_type(t)),
                |i| f64::from(i.eye_height()),
            ),
            // Entity.getEyeHeight: the type's dimensions.
            _ => self.type_eye_height(&self.target_type(t)),
        }
    }
}
