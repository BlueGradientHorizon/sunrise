#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Dim, Game, chat::PLAYER_NAME, vehicle_teleport::TeleportMotion,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::OtherEntity;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::R;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::Target;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::block_input_matches;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::data::{
    place_summoned, strip_finalize_tree,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::difficulty_names;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::entity_name_key;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::entity_pitch;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::err;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::format_double;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::formatted_command_failure;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::percent_f;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::rotation_in;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::split_top_level;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::target_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::teleport_velocity;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::texts;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::without_knots;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::wrap_degrees;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    command::{
        Arg, CommandError, Context, Reader,
        args::{BlockInput, Coordinates, GameMode, ItemInput, Selector},
        vanilla::DIFFICULTIES,
    },
    gamerules::{self, Value},
    items::ItemStack,
    text::{self, Style, Text},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::Vec3, java_number::float_to_string, random::RandomSource,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::packets::play::{self, Clientbound as C};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::registry::BlockStateId;

#[path = "effects_client.rs"]
pub(in crate::game) mod effects;

#[path = "execute_client.rs"]
pub(in crate::game) mod execute;

#[path = "extra_client.rs"]
pub(in crate::game) mod extra;

#[path = "raid_client.rs"]
pub(in crate::game) mod raid;

#[path = "world_cmds_client.rs"]
pub(in crate::game) mod world_cmds;

#[cfg(test)]
#[path = "enchant_tests_client.rs"]
pub(in crate::game) mod enchant_tests;

#[cfg(test)]
#[path = "summon_bolt_tests_client.rs"]
pub(in crate::game) mod summon_bolt_tests;
