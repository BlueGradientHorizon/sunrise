#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Arg, Context, Game, R, Target, err, wrap_degrees};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::target_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::world_cmds::command_operation;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::world_cmds::holder;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::world_cmds::java_double_text;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::world_cmds::look_at;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::world_cmds::num;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::world_cmds::operation_named;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::world_cmds::playsound_placement;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::world_cmds::take_new_reinforcements;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::world_cmds::text_error;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::world_cmds::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::living_attributes::AttributeWrite;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::minecarts::CartHit;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    mobs::{Damage, DamageKind, MobEvent, NamedDamage, Who, finalize},
    scoreboard_cmd::{Holder, Tracker},
    settings::SOUND_SOURCES,
    text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::random::RandomSource;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::packets::play::{self, Clientbound as C};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::attribute_map::ValueKind;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::damage_source::Catalog as DamageCatalog;

#[cfg(test)]
#[path = "particle_command_tests_client.rs"]
pub(in crate::game) mod particle_command_tests;
