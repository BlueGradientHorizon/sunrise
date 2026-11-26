use super::Dim;
use mod_api::__private::game::*;
use mod_api::__private::{ClientCommandExecutor, RetainedCallback, RetainedFailure};
use mod_api::LevelRef;
use mod_api::PlayerRef;
use std::{collections::HashMap, sync::Arc};
use sunrise_command::{Arg, ArgType, Context, Dispatcher};
use sunrise_core::position::BlockPos;
use sunrise_game_data::text::{RED, Text};
use sunrise_gameplay_common::game::api_values::TryIntoApi;
use sunrise_gameplay_common::game::commands_common::{
    CommandsCommonGameExt1 as _, format_syntax_failure_context,
};
use sunrise_gameplay_common::game::native_server::{
    ClientCommandOutcome, PlayerRead,
    argument_kinds::{custom_value, nbt_value, parsed_type, positional_value},
};
use sunrise_items::items::ItemStack;
use sunrise_sim::lan::abilities::PlayerAbilities;

pub(in crate::game) struct RegisteredClientExecutor {
    pub(in crate::game) callback: RetainedCallback,
    call: Option<Arc<ClientCommandExecutor>>,
}

impl RegisteredClientExecutor {
    pub(in crate::game) fn new(
        owner: &'static str,
        index: usize,
        usage: &str,
        call: Arc<ClientCommandExecutor>,
    ) -> Self {
        Self {
            callback: RetainedCallback::new(owner, index, "client_command.execute", usage),
            call: Some(call),
        }
    }

    pub(in crate::game) fn execute(
        &self,
        host: &mut dyn mod_api::ClientHost,
        invocation: &mod_api::CommandInvocation,
    ) -> Result<Option<i32>, RetainedFailure> {
        let call = self.call.as_ref().expect("live client executor");
        self.callback.call(|| call(host, invocation))
    }
}

impl Drop for RegisteredClientExecutor {
    fn drop(&mut self) {
        self.callback
            .dispose("client_command.drop", self.call.take());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum PendingClientCommandKind {
    Signed,
    Unattended,
}

/// An invocation path, shared by all submissions from the same executor. Use the retained
/// registration identity rather than a parsed node, which can change on a tree refresh.
pub(in crate::game) struct ClientCommandChain {
    pub(in crate::game) executor: RetainedCallback,
    pub(in crate::game) parent: Option<Arc<Self>>,
}

pub(in crate::game) struct PendingClientCommand {
    pub(in crate::game) command: String,
    pub(in crate::game) kind: PendingClientCommandKind,
    pub(in crate::game) chain: Arc<ClientCommandChain>,
}

/// The client's own native mod commands, bound to the current client dispatcher generation.
pub struct NativeClientCommandState {
    pub(in crate::game) dispatcher: Dispatcher,
    pub(in crate::game) executors: Vec<Option<Arc<RegisteredClientExecutor>>>,
    /// Present while an executor runs; nested slash submissions inherit this path and wait
    /// for the client event loop instead of re-entering synchronously.
    pub(in crate::game) active_chain: Option<Arc<ClientCommandChain>>,
    pub(in crate::game) pending_commands: Vec<PendingClientCommand>,
    /// Present while `client_chat` listeners run; a chat message a listener submits waits for
    /// the next event-loop wave instead of re-entering the hook synchronously.
    pub(in crate::game) chat_dispatching: bool,
    pub pending_chats: Vec<String>,
}

impl Default for NativeClientCommandState {
    fn default() -> Self {
        Self {
            dispatcher: Dispatcher::new(),
            executors: Vec::new(),
            active_chain: None,
            pending_commands: Vec::new(),
            chat_dispatching: false,
            pending_chats: Vec::new(),
        }
    }
}

impl NativeClientCommandState {
    pub(in crate::game) fn defer(&mut self, command: &str, kind: PendingClientCommandKind) -> bool {
        let Some(chain) = self.active_chain.clone() else {
            return false;
        };
        self.pending_commands.push(PendingClientCommand {
            command: command.to_owned(),
            kind,
            chain,
        });
        true
    }

    pub(in crate::game) fn install(
        &mut self,
        dispatcher: Dispatcher,
        executors: Vec<Option<RegisteredClientExecutor>>,
    ) {
        self.dispatcher = dispatcher;
        // A panicking retired callable may disable its replacement in the shared quarantine.
        self.executors = executors
            .into_iter()
            .map(|entry| entry.map(Arc::new))
            .collect();
    }
}

/// Helpers for server mods, kept as stable as the hook points.
// Native mods use these; nothing in Sunrise does.
#[allow(dead_code)]
impl crate::game::ClientGame<'_> {
    pub(in crate::game) fn player_discovered_recipes(
        &self,
        _player: PlayerRef,
    ) -> Option<Vec<String>> {
        None
    }

    /// Reads live `Remote::abilities` with predicted `flying` and `flying_speed` from the body.
    pub(in crate::game) fn client_abilities(&self) -> Option<PlayerAbilities> {
        let mut a = *self.remote_abilities()?;
        if let Some(body) = self.common.player.as_ref() {
            a.flying = body.flying;
            a.flying_speed = body.flying_speed;
        }
        Some(a)
    }

    pub(in crate::game) fn client_player(&self, uuid: u128) -> Option<PlayerRef> {
        let epoch = self.common.native_lifecycle.epoch()?;
        let remote = self
            .client
            .remote
            .as_ref()
            .filter(|r| r.logged_in && r.profile == uuid)?;
        Some(mod_api::__private::game::player_ref(
            mod_api::EntityRef::new(epoch, u32::try_from(remote.entity_id).ok()?),
            uuid,
        ))
    }

    pub(in crate::game) fn client_player_read_both(
        &self,
        player: PlayerRef,
    ) -> Option<PlayerRead<'_>> {
        (self.local_player()? == player).then(|| PlayerRead::Acting {
            mode: self
                .player_info_game_mode()
                .map(sunrise_command::args::GameMode::id),
            body: self.common.player.as_ref(),
            abilities: self.client_abilities(),
            last_death: self.common.level.last_death.as_ref(),
        })
    }

    pub(in crate::game) fn client_player_dimension_key(
        &self,
        player: PlayerRef,
    ) -> Option<mod_api::DimensionKey> {
        let epoch = self.common.native_lifecycle.epoch()?;
        if epoch != player.fields().entity.epoch() {
            return None;
        }
        (self.local_player()? == player).then(|| self.native_level_key())
    }

    pub(in crate::game) fn client_player_collision_flags(
        &self,
        player: PlayerRef,
    ) -> Result<Option<mod_api::CollisionFlags>, mod_api::LevelReadError> {
        Ok(self.client_player_read_both(player).and_then(|read| {
            let PlayerRead::Acting {
                body: Some(body), ..
            } = read
            else {
                return None;
            };
            let flags = body.collision;
            Some(mod_api::CollisionFlags::new(
                flags.horizontal,
                flags.minor,
                flags.vertical,
                flags.below,
            ))
        }))
    }

    pub(in crate::game) fn client_player_client_brand(&self, _player: PlayerRef) -> Option<&str> {
        None
    }

    /// `LocalPlayer.permissionLevel`, from the server's entity events 24 to 28.
    pub(in crate::game) fn player_permission_level(&self, player: PlayerRef) -> Option<u8> {
        (self.local_player()? == player).then_some(self.common.ui.registries.permission)
    }

    pub(in crate::game) fn client_tick_uses_server_sections(&self) -> bool {
        false
    }

    pub(in crate::game) fn client_crafting_remainder_hook(
        &self,
        _stack: &ItemStack,
        _remainder: &mut Option<ItemStack>,
    ) {
    }

    pub(in crate::game) fn client_world_table_items(
        &self,
    ) -> Option<&Arc<sunrise_items::items::Items>> {
        None
    }

    pub(in crate::game) fn client_lid_openers(&self) -> Option<&HashMap<(i32, i32, i32), i32>> {
        None
    }

    pub(in crate::game) fn client_signal_capability_site(
        &self,
        _at: mod_api::BlockSide,
    ) -> Option<mod_api::BlockCapabilitySite> {
        None
    }

    pub(in crate::game) fn client_signal_container_size(
        &self,
        _at: mod_api::BlockSide,
    ) -> Option<usize> {
        None
    }

    pub(in crate::game) fn client_signal_lend_container_stack(
        &self,
        _at: mod_api::BlockSide,
        _slot: usize,
        _read: &mut dyn for<'a> FnMut(mod_api::ItemStackView<'a>),
    ) {
    }

    pub(in crate::game) fn client_level_block_face_sturdy(
        &self,
        level: LevelRef,
        pos: [i32; 3],
        face: mod_api::BlockFace,
        support: mod_api::SupportType,
    ) -> Option<bool> {
        sunrise_gameplay_common::game::native_world_read_common::block_face_sturdy(
            self.client_level_view_world(level)?,
            self.common.res.behaviours(),
            self.common.res.collision(),
            pos,
            face,
            support,
        )
    }

    pub(in crate::game) fn client_signal_block_entity(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<mod_api::BlockEntityRef> {
        let world = self.client_level_view_world(level)?;
        sunrise_gameplay_common::game::native_block_entity_read::world_block_entity(
            world, level, pos,
        )
    }

    pub(in crate::game) fn client_signal_block_entity_data(
        &self,
        entity: mod_api::BlockEntityRef,
    ) -> Option<mod_api::Compound> {
        let world = self.client_level_view_world(entity.fields().level)?;
        sunrise_gameplay_common::game::native_block_entity_read::world_block_entity_data(
            world,
            self.common.pack.assets.registry,
            self.items(),
            entity,
        )
    }

    pub(in crate::game) fn client_level_is_raining_at(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<bool> {
        let world = self.client_level_view_world(level)?;
        if !world.is_loaded_at(pos[0], pos[1], pos[2]) {
            return None;
        }
        let (rain, _) = sunrise_gameplay_common::game::mod_api_host_common::weather_for_level(
            world.shape,
            level.dimension() == Dim::End.key(),
            world.weather.levels(1.0),
        );
        if rain <= 0.2 {
            return Some(false);
        }
        Some(sunrise_gameplay_common::game::native_rain_read::rain_at(
            world,
            self.common.res.facts(),
            self.common
                .world_biomes
                .as_deref()
                .unwrap_or_else(|| self.common.res.biomes()),
            self.common.res.behaviours(),
            Dim::from_id(level.dimension_id()),
            BlockPos::new(pos[0], pos[1], pos[2]),
        ))
    }

    fn client_command_item_stack(
        &self,
        sunrise_command::args::ItemInput { id, components, .. }: &sunrise_command::args::ItemInput,
        count: u32,
    ) -> Result<ItemStack, sunrise_command::CommandError> {
        let (damage, extra) = match components
            .as_deref()
            .map(sunrise_items::item_data::command_components)
        {
            Some(Ok(tag)) => sunrise_items::item_data::decode_components(Some(&tag)),
            Some(Err(e)) => return Err(sunrise_command::CommandError::new(Text::literal(e))),
            None => (0, None),
        };
        let stack = ItemStack {
            item: id.as_str().into(),
            count,
            damage,
            extra,
        };
        let max_stack = self.stack_max_size(&stack);
        if count > max_stack {
            return Err(sunrise_gameplay_common::game::commands_common::err(
                "arguments.item.overstacked",
                &[id, &max_stack],
            ));
        }
        Ok(stack)
    }

    /// Resolve argument kinds shared by server and client native commands.
    fn resolve_client_safe_argument(
        &self,
        ty: Option<&ArgType>,
        value: Arg,
        coords_position: impl FnOnce(&sunrise_command::args::Coordinates) -> [f64; 3],
        rotation: [f32; 2],
    ) -> Result<mod_api::CommandArgumentValue, sunrise_command::CommandError> {
        use mod_api::CommandArgumentValue as Value;
        use sunrise_gameplay_common::game::commands_common::err;
        Ok(match value {
            Arg::Str(text) if matches!(ty, Some(ArgType::Nbt)) => nbt_value(&text)?,
            Arg::Str(value) => Value::String(value),
            Arg::Int(value) => Value::Integer(value),
            Arg::Long(value) => Value::Long(value),
            Arg::Double(value) => Value::Number(value),
            Arg::Bool(value) => Value::Boolean(value),
            Arg::Coords(coordinates) => {
                positional_value(ty, &coordinates, coords_position(&coordinates), rotation)
            }
            Arg::Item(input) => Value::Item(
                self.client_command_item_stack(&input, 1)?
                    .try_into_api()
                    .expect("command item validation resolves registered items"),
            ),
            // `ResourceOrTagKeyArgument.Result`: `#` names a tag, else a resource.
            Arg::Resource(id) if matches!(ty, Some(ArgType::ResOrTagKey(_))) => {
                let (tag, name) = id
                    .strip_prefix('#')
                    .map_or((false, &*id), |name| (true, name));
                Value::ResourceOrTag {
                    tag,
                    id: mod_api::Identifier::parse(name).map_err(|_| err("command.failed", &[]))?,
                }
            }
            Arg::Resource(id) => {
                Value::Id(mod_api::Identifier::parse(&id).map_err(|_| err("command.failed", &[]))?)
            }
            Arg::Custom { kind, value } => custom_value(&kind, value),
            _ => unreachable!("the native tree contains only registered native argument kinds"),
        })
    }

    fn client_command_position(&self) -> [f64; 3] {
        match &self.common.player {
            Some(p) => [p.position.x, p.position.y, p.position.z],
            None => self.common.spawn.map_or([0.0, 64.0, 0.0], |s| {
                [s.x.floor(), s.y.floor(), s.z.floor()]
            }),
        }
    }

    fn resolve_client_native_command(
        &self,
        context: Context,
    ) -> Result<mod_api::CommandInvocation, sunrise_command::CommandError> {
        use mod_api::CommandArgument;
        let position = self.client_command_position();
        let rotation = [self.common.yaw, self.common.pitch];
        let mut arguments = Vec::with_capacity(context.args.len());
        for argument in context.args {
            let ty = parsed_type(
                &self.client.native_client_commands.dispatcher,
                &context.nodes,
                argument.range,
            );
            let value = self.resolve_client_safe_argument(
                ty,
                argument.value,
                |coordinates| coordinates.position(position, rotation),
                rotation,
            )?;
            arguments.push(CommandArgument::new(argument.name, value));
        }
        Ok(mod_api::CommandInvocation::new(
            context.literals,
            arguments,
            self.native_level_key(),
            position,
            rotation,
        ))
    }

    fn client_command_failure(&mut self, message: Text) {
        self.chat_line(Text::empty().append(message).with_color(RED));
    }

    fn client_command_syntax_failure(
        &mut self,
        error: &sunrise_command::CommandError,
        command: &str,
    ) {
        self.client_command_failure(error.message.clone());
        if let Some(context) = format_syntax_failure_context(error, command) {
            self.client_command_failure(context);
        }
    }

    /// `ClientCommandHandler.runCommand`: only Brigadier's unknown command and unknown argument
    /// outcomes leave the line to the server. All other failures stay on this client.
    pub(in crate::game) fn run_client_native_command(
        &mut self,
        line: &str,
    ) -> Option<ClientCommandOutcome> {
        self.run_client_native_command_in_chain(line, None)
    }

    pub(in crate::game) fn run_client_native_command_in_chain(
        &mut self,
        line: &str,
        ancestors: Option<Arc<ClientCommandChain>>,
    ) -> Option<ClientCommandOutcome> {
        if self.common.is_logical_server() {
            return None;
        }
        let parse = self
            .client
            .native_client_commands
            .dispatcher
            .parse(line, &self.common.ui.registries);
        let context = match parse.executable() {
            Ok((context, _)) => context.clone(),
            Err(error) => {
                if matches!(
                    error.key.as_str(),
                    "command.unknown.command" | "command.unknown.argument"
                ) {
                    return None;
                }
                self.client_command_syntax_failure(&error, line);
                return Some(ClientCommandOutcome::Failed);
            }
        };
        let Some(executor) = context
            .executor
            .filter(|&key| {
                self.client
                    .native_client_commands
                    .dispatcher
                    .owns_executor(key)
            })
            .and_then(|key| self.client.native_client_commands.executors.get(key.slot()))
            .and_then(Option::as_ref)
            .cloned()
        else {
            log::error!("client command has no executor in its dispatcher generation: {line}");
            self.client_command_failure(Text::translate("command.failed", &[]));
            return Some(ClientCommandOutcome::Failed);
        };
        if std::iter::successors(ancestors.as_deref(), |link| link.parent.as_deref())
            .any(|link| link.executor == executor.callback)
        {
            log::warn!("recursive client command refused: {line}");
            self.client_command_failure(Text::translate("command.failed", &[]));
            return Some(ClientCommandOutcome::Failed);
        }
        let invocation = match self.resolve_client_native_command(context) {
            Ok(invocation) => invocation,
            Err(error) => {
                self.client_command_syntax_failure(&error, line);
                return Some(ClientCommandOutcome::Failed);
            }
        };
        self.client.native_client_commands.active_chain = Some(Arc::new(ClientCommandChain {
            executor: executor.callback.clone(),
            parent: ancestors,
        }));
        let executed = executor.execute(self, &invocation);
        self.client.native_client_commands.active_chain = None;
        match executed {
            Ok(Some(result)) => Some(ClientCommandOutcome::Success(result)),
            failure => {
                match failure {
                    Err(why) => log::warn!("client command failed: {why:?}"),
                    _ => log::warn!(
                        "client command failed: mod {} returned no result",
                        executor.callback.owner()
                    ),
                }
                self.client_command_failure(Text::translate("command.failed", &[]));
                Some(ClientCommandOutcome::Failed)
            }
        }
    }

    /// Dispatch one event-loop wave. The next submissions retain this wave's executor
    /// ancestry but cannot run until the event loop returns again.
    pub fn drain_pending_client_commands(&mut self) {
        if self.client.native_client_commands.active_chain.is_some()
            || self.client.native_client_commands.chat_dispatching
        {
            return;
        }
        for message in std::mem::take(&mut self.client.native_client_commands.pending_chats) {
            if self.client.remote.is_none() {
                break;
            }
            let _ = self.send_chat(&message);
        }
        let pending = std::mem::take(&mut self.client.native_client_commands.pending_commands);
        for item in pending {
            if self.client.remote.is_none() {
                break;
            }
            let _ = self.submit_client_command(&item.command, item.kind, Some(item.chain));
        }
    }
}

use sunrise_gameplay_common::game::tables::GameItems as _;

use sunrise_gameplay_common::game::game_mode_common::GameModeCommonGameExt1 as _;

use sunrise_gameplay_common::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;

use sunrise_gameplay_common::game::player_identity::PlayerIdentityGameExt1 as _;
