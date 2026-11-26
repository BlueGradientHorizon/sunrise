//! Server-side command-block execution, ticks, chaining, and edit packet handling.
use super::command_blocks_common::Pos;
use super::{Game, commands::execute::Source};
use sunrise_client_core::text::Text;
use sunrise_core::position::BlockPos;
use sunrise_gameplay::command_blocks::{
    CommandBlockEdit, CommandBlockTransition, command_block_mut, conditional, facing_step,
    facing_yaw, mark_condition_met, offset, set_powered,
};
use sunrise_items::items::ItemStack;
use sunrise_level::command_block::{self, CommandBlock, Mode};
use sunrise_protocol::packets::play::{self, Clientbound as C};
use {
    sunrise_core::nbt::Tag,
    sunrise_world::{chunk::BlockState, level::flags},
};

impl Game<'_, crate::game::server_side::ServerState> {
    /// `Commands.performPrefixedCommand` for a `BaseCommandBlock` at game master level.
    pub(in crate::game::server_side) fn run_as_game_master(
        &mut self,
        command: &str,
        source: Source,
    ) -> i32 {
        // performPrefixedCommand: a leading slash is allowed.
        let line = command.strip_prefix('/').unwrap_or(command).to_owned();
        let outer = self.side.cmds.0.results.replace(Vec::new());
        let level = std::mem::replace(&mut self.ui.registries.permission, 2);
        self.with_source(source, |g| g.run_command(&line));
        self.ui.registries.permission = level;
        let results = std::mem::replace(&mut self.side.cmds.0.results, outer).unwrap_or_default();
        results.iter().filter(|(success, _)| *success).count() as i32
    }

    pub(in crate::game::server_side) fn schedule_command_block(&mut self, pos: Pos) {
        let at = BlockPos::new(pos.0, pos.1, pos.2);
        self.with_level(|b, level, ticks| b.schedule_tick(level, ticks, at, 1));
    }

    /// `isAutomatic` of the command block at `pos`, before new data loads.
    pub(in crate::game::server_side) fn command_block_auto(&self, pos: Pos) -> bool {
        self.command_block(pos).is_some_and(|c| c.auto)
    }

    /// `CommandBlockEntity.loadAdditional`: schedule first tick when turning automatic.
    pub(in crate::game::server_side) fn command_block_loaded(&mut self, pos: Pos, was_auto: bool) {
        let Some(c) = self.command_block(pos) else {
            return;
        };
        let (auto, powered) = (c.auto, c.powered);
        let mode = self
            .block_state_at(pos)
            .and_then(|s| Mode::of_block(&s.name));
        if !was_auto && auto && !powered && mode.is_some_and(|m| m != Mode::Sequence) {
            self.schedule_command_block_tick(pos);
        }
    }

    /// `CommandBlockEntity.scheduleTick`.
    pub(in crate::game::server_side) fn schedule_command_block_tick(&mut self, pos: Pos) {
        mark_condition_met(&mut self.world, self.pack.assets.registry, pos);
        self.schedule_command_block(pos);
    }

    pub(in crate::game::server_side) fn update_output_signal(&mut self, pos: Pos) {
        let at = BlockPos::new(pos.0, pos.1, pos.2);
        self.with_level(|b, level, ticks| b.update_neighbour_for_output_signal(level, ticks, at));
    }

    /// `CommandBlock.setPlacedBy`: apply default flags unless item carried block entity data.
    pub(in crate::game) fn command_block_placed(&mut self, pos: Pos, stack: Option<&ItemStack>) {
        let defaults = !stack.is_some_and(|stack| {
            stack.carries_component(
                self.items().get(&stack.item),
                sunrise_sim::block_entity_item::BLOCK_ENTITY_DATA,
            )
        });
        let feedback = self
            .ui
            .rules
            .bool_at(sunrise_client_core::gamerules::vanilla::SEND_COMMAND_FEEDBACK);
        let Some(name) = self.block_state_at(pos).map(|s| s.name.as_str()) else {
            return;
        };
        let saved = |c: &CommandBlock| (c.track_output, c.auto, c.powered, c.condition_met);
        let before = self.command_block(pos).map(saved);
        let auto = command_block::automatic_block(name);
        if defaults && let Some(c) = command_block_mut(&mut self.world, pos) {
            c.track_output = feedback;
            c.auto = auto;
        }
        let at = BlockPos::new(pos.0, pos.1, pos.2);
        let signal = self.with_level(|b, level, ticks| b.has_neighbor_signal(level, ticks, at));
        let registry = self.pack.assets.registry;
        if let Some((was, auto)) = set_powered(&mut self.world, registry, pos, signal)
            && signal
            && !was
            && !auto
            && !command_block::automatic_block(name)
        {
            self.schedule_command_block(pos);
        }
        if self.command_block(pos).map(saved) != before {
            self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
        }
    }

    /// `CommandBlock.tick`.
    pub(in crate::game) fn command_block_tick(&mut self, pos: BlockPos) {
        let key = (pos.x, pos.y, pos.z);
        let Some(state) = self.block_state_at(key) else {
            return;
        };
        let Some(mode) = Mode::of_block(&state.name) else {
            return;
        };
        let Some(was_met) = self.command_block(key).map(|c| c.condition_met) else {
            return;
        };
        if mode == Mode::Auto {
            mark_condition_met(&mut self.world, self.pack.assets.registry, key);
        }
        match command_block::tick_action(mode, was_met, conditional(state)) {
            Some(true) => self.command_block_execute(key, state),
            Some(false) => {
                if let Some(c) = command_block_mut(&mut self.world, key) {
                    c.success_count = 0;
                }
            }
            None => {}
        }
        if let Some(c) = self.command_block(key)
            && command_block::repeats(mode, c.powered, c.auto)
        {
            self.schedule_command_block(key);
        }
        self.update_output_signal(key);
    }

    /// `CommandBlock.execute`: run the block's command, then the chain in front.
    pub(in crate::game::server_side) fn command_block_execute(
        &mut self,
        pos: Pos,
        state: &BlockState,
    ) {
        let has_command = self
            .command_block(pos)
            .is_some_and(|c| !c.command.is_empty());
        if has_command {
            self.perform_command_block(pos);
        } else if let Some(c) = command_block_mut(&mut self.world, pos) {
            c.success_count = 0;
        }
        self.execute_chain(pos, facing_step(state));
    }

    /// `CommandBlock.executeChain`: run chain command blocks in front up to the sequence limit.
    pub(in crate::game::server_side) fn execute_chain(&mut self, start: Pos, mut step: [i32; 3]) {
        let registry = self.pack.assets.registry;
        let limit = self
            .ui
            .rules
            .int_at(sunrise_client_core::gamerules::vanilla::MAX_COMMAND_SEQUENCE_LENGTH);
        let mut remaining = limit;
        let mut pos = start;
        while remaining > 0 {
            remaining -= 1;
            pos = offset(pos, step);
            let Some(state) = self.block_state_at(pos) else {
                break;
            };
            if Mode::of_block(&state.name) != Some(Mode::Sequence) {
                break;
            }
            let Some(c) = self.command_block(pos) else {
                break;
            };
            if c.powered || c.auto {
                if mark_condition_met(&mut self.world, registry, pos) {
                    if !self.perform_command_block(pos) {
                        break;
                    }
                    self.update_output_signal(pos);
                } else if conditional(state)
                    && let Some(c) = command_block_mut(&mut self.world, pos)
                {
                    c.success_count = 0;
                }
            }
            step = facing_step(state);
        }
        if remaining <= 0 {
            log::warn!(
                "Command Block chain tried to execute more than {} steps!",
                limit.max(0)
            );
        }
    }

    /// `BaseCommandBlock.performCommand`: runs the block's command at game master level.
    pub(in crate::game::server_side) fn perform_command_block(&mut self, pos: Pos) -> bool {
        let game_time = self.world.game_time;
        let Some(c) = command_block_mut(&mut self.world, pos) else {
            return false;
        };
        if c.last_execution == game_time {
            return false;
        }
        if c.command.eq_ignore_ascii_case("Searge") {
            c.last_output = Some(Text::literal("#itzlipofutzli"));
            c.success_count = 1;
            return true;
        }
        c.success_count = 0;
        let command = c.command.clone();
        if self
            .ui
            .rules
            .bool_at(sunrise_client_core::gamerules::vanilla::COMMAND_BLOCKS_WORK)
            && !command.is_empty()
        {
            if let Some(c) = command_block_mut(&mut self.world, pos) {
                c.last_output = None;
            }
            let Some(state) = self.block_state_at(pos) else {
                return false;
            };
            let source = Source {
                entity: None,
                pos: [
                    f64::from(pos.0) + 0.5,
                    f64::from(pos.1) + 0.5,
                    f64::from(pos.2) + 0.5,
                ],
                rot: [facing_yaw(state), 0.0],
                eyes: false,
                silent: false,
                store: None,
                dimension: self.dim,
                origin_dimension: self.dim,
                origin_player: false,
                level: None,
                block: Some(pos),
                cart: None,
            };
            let successes = self.run_as_game_master(&command, source);
            if let Some(c) = command_block_mut(&mut self.world, pos) {
                c.success_count = successes;
            }
        }
        if let Some(c) = command_block_mut(&mut self.world, pos) {
            c.last_execution = if c.update_last_execution {
                game_time
            } else {
                -1
            };
        }
        true
    }

    /// `CloseableCommandBlockSource.sendSystemMessage`: stamp and store the block's last output.
    pub(in crate::game::server_side) fn command_block_output(
        &mut self,
        pos: Pos,
        format: impl FnOnce(Text) -> Text,
    ) {
        if let Some(c) = command_block_mut(&mut self.world, pos)
            && c.track_output
        {
            let time = chrono::Local::now().format("[%H:%M:%S] ").to_string();
            c.last_output = Some(format(Text::literal(time)));
        }
    }

    /// `CommandSourceStack.sendSuccess` for a command block.
    pub(in crate::game::server_side) fn command_block_success(&mut self, pos: Pos, message: Text) {
        if self
            .ui
            .rules
            .bool_at(sunrise_client_core::gamerules::vanilla::SEND_COMMAND_FEEDBACK)
        {
            self.command_block_output(pos, |timestamp| timestamp.append(message));
        }
    }

    /// `ServerGamePacketListenerImpl.handleSetCommandBlock` for a guest.
    pub(in crate::game::server_side) fn lan_set_command_block(
        &mut self,
        conn: u32,
        p: play::SetCommandBlock,
    ) {
        let Some(g) = self.lan.as_ref().and_then(|host| host.hub.guest(conn)) else {
            return;
        };
        let allowed = g.creative() && g.permission >= 2;
        let edit = CommandBlockEdit {
            pos: (p.pos[0], p.pos[1], p.pos[2]),
            command: p.command,
            track_output: p.flags & 1 != 0,
            mode: Mode::from_ordinal(p.mode),
            conditional: p.flags & 2 != 0,
            auto: p.flags & 4 != 0,
        };
        // The confirmation (`advMode.setCommand.success`) is the editing player's alone.
        self.as_guest_messaged(conn, |game| {
            if allowed {
                game.apply_command_block(&edit);
            } else {
                game.server_system_message(Text::translate("advMode.notAllowed", &[]));
            }
        });
    }

    /// `ServerPlayer.openCommandBlock`: send BlockEntityData and OPEN_COMMAND_BLOCK_CHANNEL.
    pub(in crate::game::server_side) fn lan_open_command_block(&mut self, conn: u32, pos: Pos) {
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        if !host
            .hub
            .guest(conn)
            .is_some_and(|g| g.creative() && g.permission >= 2)
        {
            return;
        }
        let Some(block) = self.command_block(pos) else {
            return;
        };
        let tag = Tag::compound(block.save());
        let state = self.world.block(pos.0, pos.1, pos.2).0;
        let Some(kind) = host.hub.encoder.entity_type(state) else {
            return;
        };
        host.hub.send_to(
            conn,
            &C::BlockEntityData(play::BlockEntityData {
                pos: [pos.0, pos.1, pos.2],
                kind,
                tag,
            }),
        );
        host.hub.send_to(
            conn,
            &C::CustomPayload(sunrise_protocol::packets::configuration::CustomPayload {
                channel: sunrise_sim::lan::data::OPEN_COMMAND_BLOCK_CHANNEL.into(),
                data: sunrise_sim::lan::data::encode_open_command_block([pos.0, pos.1, pos.2]),
            }),
        );
    }

    /// What `handleSetCommandBlock` does once the player may edit the block.
    pub(in crate::game) fn apply_command_block(&mut self, s: &CommandBlockEdit) {
        let pos = s.pos;
        let registry = self.pack.assets.registry;
        let Some(current) = self.block_state_at(pos) else {
            return;
        };
        let Some(old_mode) = Mode::of_block(&current.name) else {
            return;
        };
        if self.command_block(pos).is_none() {
            return;
        }
        // The mode is the block, the conditional flag its state; the block entity stays.
        let mut wanted = current.clone();
        wanted.name = s.mode.block().to_owned();
        wanted
            .properties
            .insert("conditional".into(), s.conditional.to_string());
        if let Ok(id) = registry.resolve(&wanted)
            && wanted != *current
        {
            let old = self.world.block(pos.0, pos.1, pos.2);
            let (lease, entity) = self.world.block_entities.take(&pos).unzip();
            let at = BlockPos::new(pos.0, pos.1, pos.2);
            let lease = self.with_level(|b, level, ticks| {
                level.command_block_transition = lease.map(|lease| CommandBlockTransition {
                    lease,
                    old,
                    new: id,
                    available: old_mode != s.mode,
                });
                b.set_block(level, ticks, at, id, flags::UPDATE_CLIENTS);
                level
                    .command_block_transition
                    .take()
                    .map(|change| change.lease)
            });
            if let Some((lease, entity)) = lease.zip(entity) {
                self.world.block_entities.put(lease, entity);
            }
            let _ = self.server_process_level_events();
        }
        let command = s.command.clone();
        let Some(c) = command_block_mut(&mut self.world, pos) else {
            return;
        };
        c.set_command(&command);
        c.track_output = s.track_output;
        if !s.track_output {
            c.last_output = None;
        }
        // CommandBlockEntity.setAutomatic, then onModeSwitch.
        let (was_auto, powered) = (c.auto, c.powered);
        c.auto = s.auto;
        if !was_auto && s.auto && !powered && s.mode != Mode::Sequence {
            self.schedule_command_block_tick(pos);
        }
        if old_mode != s.mode && s.mode == Mode::Auto && (powered || s.auto) {
            self.schedule_command_block_tick(pos);
        }
        if !command.is_empty() {
            let key = if self
                .ui
                .rules
                .bool_at(sunrise_client_core::gamerules::vanilla::COMMAND_BLOCKS_WORK)
            {
                "advMode.setCommand.success"
            } else {
                "advMode.setCommand.disabled"
            };
            self.server_system_message(Text::translate(key, &[Text::literal(command)]));
        }
        // `BaseCommandBlock.onUpdated`: save and notify clients and comparators.
        self.mark_block_entity_updated(pos);
    }
}
