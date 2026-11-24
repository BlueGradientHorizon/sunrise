#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Game, Screen, commands::execute::Source};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::command_blocks::Pos;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::command_blocks::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    chat::Commands,
    command_block::{self, CommandBlock, Mode},
    containers::BlockEntity,
    hud::{HudBatch, rgba},
    items::ItemStack,
    screen_draw::Measure,
    text::Text,
    ui,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client_gameplay::command_block_screen::{
    Button, CommandBlockScreen, previous_y,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::position::BlockPos;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_gameplay::command_blocks::{
    CommandBlockEdit, CommandBlockTransition, analog_signal, command_block_mut, conditional,
    facing_step, facing_yaw, mark_condition_met, offset, set_powered, state_of,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_protocol::packets::play::{
    self, Clientbound as C, Serverbound as S,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use winit::keyboard::KeyCode;
#[allow(unused_imports)]
pub(in crate::game::server_side) use {
    sunrise_core::nbt::Tag,
    sunrise_world::{chunk::BlockState, level::flags},
};

impl Game<'_> {
    /// `Commands.performPrefixedCommand` for a `BaseCommandBlock`: `command`
    /// from `source` at game master level, and how many of its results
    /// succeeded.
    pub(in crate::game) fn run_as_game_master(&mut self, command: &str, source: Source) -> i32 {
        // performPrefixedCommand: a leading slash is allowed.
        let line = command.strip_prefix('/').unwrap_or(command).to_owned();
        let outer = self.cmds.0.results.replace(Vec::new());
        let level = std::mem::replace(&mut self.ui.registries.permission, 2);
        self.with_source(source, |g| g.run_command(&line));
        self.ui.registries.permission = level;
        let results = std::mem::replace(&mut self.cmds.0.results, outer).unwrap_or_default();
        results.iter().filter(|(success, _)| *success).count() as i32
    }
}
