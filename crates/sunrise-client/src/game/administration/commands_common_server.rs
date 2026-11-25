#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands_common::OtherEntity;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands_common::R;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands_common::Target;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands_common::entity_name_key;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands_common::entity_pitch;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands_common::err;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands_common::format_double;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands_common::format_syntax_failure_context;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands_common::formatted_command_failure;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands_common::num;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands_common::percent_f;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands_common::rotation_in;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands_common::teleport_velocity;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands_common::texts;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands_common::without_knots;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands_common::wrap_degrees;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands_common::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    command::CommandError,
    items::ItemStack,
    text::{self, Style, Text},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::{
    geometry::Vec3,
    nbt::Tag,
    version::{BUILD_TIME, DATA_VERSION, STABLE, VANILLA_VERSION, VERSION_ID},
};

impl Game<'_> {
    /// `BlockEntity.saveWithoutMetadata` of the block entity at `pos`, without id or position.
    pub(in crate::game) fn block_entity_tag(&self, pos: [i32; 3]) -> Option<Tag> {
        let e = self.world.block_entities.get(&(pos[0], pos[1], pos[2]))?;
        let name = self.data_block_name(pos);
        let mut tag =
            self.encode_live_block_entity((pos[0], pos[1], pos[2]), e, &name, self.res.items());
        if let Tag::Compound(c) = &mut tag {
            for key in ["id", "x", "y", "z", "keepPacked"] {
                c.remove(&key.into());
            }
        }
        Some(tag)
    }
}
