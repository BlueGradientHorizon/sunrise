#[allow(unused_imports)]
pub(in crate::game::server_side) use super::api_values::{TryIntoApi, TryIntoEngine};
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{
    Dim, Game, commands::execute, respawn_anchor::RespawnConfig,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::level_control_hooks::LevelControl;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::Acting;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::BaseTree;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::CommandExecutorSlots;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::FakeAction;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::LEVELS;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::PlayerRead;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::ScopedPlayerState;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::argument_kinds::{
    CommandArguments, command_arguments,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::argument_kinds::{
    UNCENTERED, argument_type, bounded_parser, custom_parser, custom_value, dimension_value,
    nbt_value, parsed_type, parser, positional_value, resource_parser,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::base_root;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::client_command_tree;
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::command_tree;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::command_tree_for;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::declares_provider;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::finish;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::online_mode_of;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::orb_meets;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::ordered_box;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::register_command_nodes;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::register_dispatcher;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::registered_commands::NativeCommandState;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::system_chat;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::usage_nodes;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::native_server::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::__private::game::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::LevelRef;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::{
    ActionOutcome, ActionRefused, FakeEffect, GameMode, GlobalPos, Identifier, PlayerAction,
    PlayerActionSetup, PlayerFood, PlayerRef, PlayerRespawnConfig, ScopedActionResult, Sidebar,
    TextFormError, VirtualHost,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use serde_json::{Map, Value, json};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::{
    LazyLock, Mutex, PoisonError,
    atomic::{AtomicBool, Ordering},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::{collections::HashMap, path::PathBuf, sync::Arc};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    command::{Arg, ArgType, Context, Dispatcher},
    items::ItemStack,
    lan::{
        abilities::PlayerAbilities, data::component_nbt, guest::Guest, scoreboard::SidebarContents,
    },
    motion::PlayerMotion,
    text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::position::BlockPos;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_protocol::packets::play::{self, Clientbound as C};

#[path = "native_server/registered_commands_server.rs"]
pub(in crate::game) mod registered_commands;

/// Helpers for server mods, kept as stable as the hook points.
// Native mods use these; nothing in Sunrise does.
#[allow(dead_code)]
impl Game<'_, crate::game::server_side::ServerState> {
    /// Whether the acting player is in the level in place: a guest's own level is their hub
    /// record's, also when a hook of another level borrows them into its turn. A raw throw,
    /// equip sound or block break requires that level in place; `/give`'s effects replay in
    /// that level separately.
    pub(in crate::game) fn acting_in_level_in_place(&self) -> bool {
        let Some(conn) = self.side.lan_acting else {
            return true;
        };
        self.lan
            .as_ref()
            .and_then(|host| host.hub.guest(conn))
            .is_none_or(|g| g.dimension == self.dim.id())
    }
    /// Discard state belonging to an already removed connectionless guest.
    pub(in crate::game) fn cleanup_fake_player(
        &mut self,
        host: &mut super::lan::LanHost,
        guest: &Guest,
    ) {
        host.play.remove(&guest.conn);
        host.forget_menus(guest.conn);
        host.poses.remove(&guest.conn);
        host.mob_players.retain(|_, conn| *conn != guest.conn);
        self.side.guest_combat.0.remove(&guest.conn);
    }
    pub(in crate::game) fn fake_action_actor(
        &self,
        player: PlayerRef,
        action: PlayerAction,
    ) -> Result<(usize, u32), ActionRefused> {
        if self.native_lifecycle.epoch() != Some(player.fields().entity.epoch()) {
            return Err(ActionRefused::Stale);
        }
        let found = self.lan.as_ref().and_then(|host| {
            let index = host
                .hub
                .guests
                .identity_index(player.fields().uuid, player.fields().entity.id())?;
            let guest = host.hub.guests.get(index)?;
            guest.placed.then_some((index, guest))
        });
        let (gi, conn) = match found {
            Some((gi, g)) if g.is_fake() => (gi, g.conn),
            Some(_) => return Err(ActionRefused::NotFake),
            // Not in the hub's list as itself: the acting player, or gone.
            None => {
                return Err(match self.player_is_fake(player) {
                    Some(false) => ActionRefused::NotFake,
                    Some(true) => ActionRefused::Busy,
                    None => ActionRefused::Stale,
                });
            }
        };
        if !self.side.lan_guest_swaps.is_empty()
            || !self.transactions.is_empty()
            || self.side.fake_action.0.acting()
        {
            return Err(ActionRefused::Busy);
        }
        if let PlayerAction::InteractOn { location, .. } = action
            && !location.iter().all(|v| v.is_finite())
        {
            return Err(ActionRefused::InvalidSetup);
        }
        if let PlayerAction::Attack { target } | PlayerAction::InteractOn { target, .. } = action
            && (self.native_lifecycle.epoch() != Some(target.epoch())
                || target.id() == player.fields().entity.id())
        {
            return Err(ActionRefused::InvalidTarget);
        }
        Ok((gi, conn))
    }
    /// Whether `effect` must not start because the player acting is a fake one: true stops it,
    /// and, inside `perform_action`, records it for the action's answer. The one check every
    /// effect a fake can't take part in makes where it starts.
    pub(in crate::game) fn fake_blocks(&self, effect: FakeEffect) -> bool {
        if !self.side.lan_acting_fake {
            return false;
        }
        self.side.fake_action.0.block(effect);
        true
    }
    /// Create or retrieve a connectionless player for the level currently in place.
    pub(in crate::game) fn fake_player(
        &mut self,
        level: LevelRef,
        uuid: u128,
        name: &str,
    ) -> Option<PlayerRef> {
        if !self.level_in_place(level)
            || !self.side.lan_guest_swaps.is_empty()
            || !sunrise_client::lan::conn::valid_name(name)
        {
            return None;
        }
        let epoch = self.native_lifecycle.epoch()?;
        if self
            .level
            .player_uuid
            .map(sunrise_client::lan::playerdata::ints_uuid)
            == Some(uuid)
        {
            return None;
        }
        let host = self.lan.as_ref()?;
        if host
            .hub
            .guests
            .iter()
            .any(|g| g.uuid == uuid && g.participates_online())
        {
            return None;
        }
        if let Some(g) = host
            .hub
            .guests
            .iter()
            .find(|g| g.uuid == uuid && g.dimension == level.dimension_id())
        {
            return (g.name == name).then_some(mod_api::__private::game::player_ref(
                mod_api::EntityRef::new(epoch, g.entity_id as u32),
                uuid,
            ));
        }
        let id = self.side.next_entity.checked_add(1)?;
        let entity_id = i32::try_from(id).ok()?;
        let motion = PlayerMotion::new(
            self.motion,
            sunrise_core::geometry::Vec3::new(0.0, 0.0, 0.0),
        )
        .ok()?;
        let host = self.lan.as_mut()?;
        let conn = host.hub.lan.shared.next_id()?;
        let mut guest = Guest::connectionless(conn, uuid, name.into(), entity_id, [0.0; 3], motion);
        guest.dimension = level.dimension_id().to_owned();
        host.hub.guests.push(guest);
        host.play
            .insert(conn, super::lan_actions::GuestPlay::default());
        self.side.next_entity = id;
        Some(mod_api::__private::game::player_ref(
            mod_api::EntityRef::new(epoch, id),
            uuid,
        ))
    }
    /// The player whose action or command a hook hears, as their state is
    /// swapped in (command executors, `native_right_click_block`,
    /// `native_use_on`, `native_break_block`...): None outside such a
    /// hook.
    pub(in crate::game) fn native_acting_player(&self) -> Option<PlayerRef> {
        self.player.as_ref()?;
        self.side.lan_acting_name.as_ref()?;
        self.side.lan_acting_permission?;
        Some(mod_api::__private::game::player_ref(
            mod_api::EntityRef::new(self.native_lifecycle.epoch()?, self.native_acting_entity()),
            self.side.lan_acting_uuid?,
        ))
    }
    /// Runs `f` with the player `uuid`'s state in place, against the level in place: theirs
    /// already when they are acting (a guest in their turn, or the `Game`'s own player where no
    /// guest holds them), else a guest's through `as_guest`, including within another guest's
    /// turn. What `f` writes to the player's own state holds wherever they are; what would land
    /// in the world checks [`Self::acting_in_level_in_place`]. False when it didn't run: no such
    /// player here, or dead.
    pub(in crate::game) fn native_as_player(
        &mut self,
        uuid: u128,
        f: impl FnOnce(&mut Self),
    ) -> bool {
        if self.side.lan_acting_uuid == Some(uuid) {
            return self.native_as_acting(f);
        }
        // With no guest's turn running, `native_acting_id` is the world's owner: on a server
        // thread that owner is a guest too, whose state is theirs, not the `Game`'s own.
        let conn = self
            .native_player_holder(uuid)
            .filter(|g| !g.dead)
            .map(|g| g.conn);
        match conn {
            Some(conn) => self.as_guest(conn, f).is_some(),
            None if uuid == self.native_acting_id() => self.native_as_acting(f),
            None => false,
        }
    }
    /// Runs `f` in the turn of the player a handle names, against the level in place. False
    /// when it didn't run: stale or dead.
    pub(in crate::game) fn native_as_player_handle(
        &mut self,
        player: PlayerRef,
        f: impl FnOnce(&mut Self),
    ) -> bool {
        match self.player_handle_read(player) {
            None => false,
            Some(PlayerRead::Acting { .. }) => self.native_as_acting(f),
            Some(PlayerRead::Guest(g)) if !g.dead => {
                let conn = g.conn;
                self.as_guest(conn, f).is_some()
            }
            Some(PlayerRead::Guest(_)) => false,
        }
    }
    /// Resolve the source once at the command boundary, retaining its output origin.
    pub(in crate::game) fn native_command_origin(
        &mut self,
        permission: u8,
    ) -> super::commands::R<mod_api::CommandSource> {
        use super::commands::err;
        use mod_api::CommandSource;
        let permission = mod_api::PermissionLevel::from_level(i32::from(permission));
        let source = self
            .side
            .command_source
            .unwrap_or_else(|| self.player_source());
        let origin = if let Some(id) = self.side.exec.0.functions.last() {
            CommandSource::Function {
                id: mod_api::Identifier::parse(id).map_err(|_| err("command.failed", &[]))?,
                permission,
            }
        } else if let Some((x, y, z)) = source.block {
            CommandSource::CommandBlock {
                position: [x, y, z],
                level: source.origin_dimension.key(),
                permission,
            }
        } else if let Some(id) = source.cart {
            let entity = self
                .with_source(
                    super::commands::execute::Source {
                        dimension: source.origin_dimension,
                        ..source
                    },
                    |game| game.entity(id),
                )
                .ok_or_else(|| err("argument.entity.notfound.entity", &[]))?;
            CommandSource::CommandBlockMinecart { entity, permission }
        } else if source.origin_player {
            let player = self
                .native_acting_player()
                .ok_or_else(|| err("argument.entity.notfound.player", &[]))?;
            CommandSource::Player { player, permission }
        } else {
            CommandSource::Console { permission }
        };
        Ok(origin)
    }
    /// `CommandSourceStack.sendFailure` for a mod's executor: the red line to the originating
    /// source, using vanilla's silence and block/cart track-output rules. The command's result
    /// is whatever the executor returns.
    pub(in crate::game) fn native_failure(&mut self, line: Text) {
        let source = self
            .side
            .command_source
            .unwrap_or_else(|| self.player_source());
        self.with_source(
            super::commands::execute::Source {
                dimension: source.origin_dimension,
                ..source
            },
            |game| game.command_failure(line),
        );
    }
    /// A command's success feedback: `CommandSourceStack.sendSuccess(line,
    /// broadcast_to_admins)` from the executor's source, the line to its
    /// originating source and, with `broadcast_to_admins`, the other operators
    /// and the log (`send_success`). The executing level is in place: the
    /// entity's name is read there, the origin block or minecart in its own.
    pub(in crate::game) fn native_feedback(&mut self, line: Text, broadcast_to_admins: bool) {
        let source = self
            .side
            .command_source
            .unwrap_or_else(|| self.player_source());
        self.with_source(source, |game| game.send_success(line, broadcast_to_admins));
    }
    /// Asks the mods what root commands each player's tree keeps
    /// (`native_player_commands_sent`) and stores the answer in the hub,
    /// which sends the trees without what the mods took out: for `only`, or
    /// for every placed player.
    pub(in crate::game) fn native_filter_commands(&mut self, only: Option<u32>) {
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        let players: Vec<(u32, u128, String)> = host
            .hub
            .guests
            .iter()
            .filter(|g| only.map_or(g.placed, |conn| g.conn == conn))
            .map(|g| (g.conn, g.uuid, g.name.clone()))
            .collect();
        for (conn, uuid, name) in players {
            let Some(host) = self.lan.as_ref() else {
                return;
            };
            let names = host.hub.command_names(conn);
            let mut kept = names.clone();
            let placed = host.hub.guest(conn).is_some_and(|g| g.placed);
            if !placed {
                // A joining player has no world yet: nothing can be told them.
                self.native_player_commands_sent(uuid, &name, &mut kept);
            } else if self.side.lan_acting == Some(conn) {
                self.native_player_commands_sent(uuid, &name, &mut kept);
            } else {
                self.as_guest(conn, |g| {
                    g.native_player_commands_sent(uuid, &name, &mut kept);
                });
            }
            if let Some(host) = self.lan.as_mut() {
                let _ = host.hub.update_guest(conn, |g| {
                    g.hidden_commands = names.into_iter().filter(|n| !kept.contains(n)).collect();
                });
            }
        }
    }
    /// Resolve a UUID-based item place in the acting state or a parked guest's own record.
    /// Unlike a write, a read never initializes a body or the level owner's UUID.
    pub(in crate::game::server_side) fn native_item_place_player(
        &self,
        uuid: u128,
    ) -> Option<PlayerRead<'_>> {
        if self.side.lan_acting_uuid != Some(uuid) {
            if let Some(guest) = self.native_player_holder(uuid) {
                return (!guest.dead && !self.side.lan_guest_swaps.contains(&guest.conn))
                    .then_some(PlayerRead::Guest(guest));
            }
            if self.side.lan_acting_uuid.is_some()
                || self
                    .level
                    .player_uuid
                    .map(sunrise_client::lan::playerdata::ints_uuid)
                    != Some(uuid)
            {
                return None;
            }
        }
        Some(PlayerRead::Acting {
            mode: Some(self.mode_id()),
            body: Some(self.native_live_acting_body()?),
            abilities: self.side.acting_abilities,
            last_death: self.level.last_death.as_ref(),
        })
    }
    /// A mod changed the commands it registers (`native_register_commands`):
    /// at the start of the next server tick every mod is asked again, the
    /// commands players may run are rebuilt, and every player on the server is
    /// sent their tree again, after `native_player_commands_sent`. Nothing
    /// happens where no world is open to players, and nothing for a server
    /// whose mods never call it. API @API:refresh_commands@.
    pub(in crate::game) fn native_refresh_commands(&mut self) {
        self.side.native_commands.0.refresh = true;
    }
    pub(in crate::game) fn native_send_all(&mut self, uuid: u128, packets: Vec<C>) -> bool {
        if self.side.lan_acting_uuid == Some(uuid) && !self.side.lan_acting_fake {
            self.side.guest_packets.extend(packets);
            return true;
        }
        let Some(host) = &self.lan else {
            return false;
        };
        let Some(g) = host.hub.online_players().find(|g| g.uuid == uuid) else {
            return false;
        };
        for packet in &packets {
            host.hub.send_to(g.conn, packet);
        }
        true
    }
    /// The real players on this server, in join order, including an integrated
    /// server's own player even when the world is not open to LAN. Empty on a client.
    pub(in crate::game) fn native_server_players(&self) -> Vec<PlayerRef> {
        let Some(epoch) = self.native_lifecycle.epoch() else {
            return Vec::new();
        };
        let Some(host) = &self.lan else {
            return Vec::new();
        };
        // Construct from the online records, not the live-body resolver: outer
        // players remain connected while their bodies are displaced by a turn.
        host.hub
            .online_players()
            .map(|g| {
                mod_api::__private::game::player_ref(
                    mod_api::EntityRef::new(epoch, g.entity_id as u32),
                    g.uuid,
                )
            })
            .collect()
    }
    pub(in crate::game) fn perform_fake_action(
        &mut self,
        gi: usize,
        conn: u32,
        action: PlayerAction,
    ) -> Result<ActionOutcome, ActionRefused> {
        let acting = Acting::start(&self.side.fake_action.0);
        let failed = std::mem::replace(&mut self.side.use_failed, false);
        let consumed = (|| match action {
            PlayerAction::UseOn { hand, pos, hit } => {
                let hit = super::Hit {
                    pos,
                    face: hit.face,
                    point: hit.point,
                    inside: hit.inside,
                };
                self.guest_use_item_on(conn, gi, hit, hand, None)
            }
            PlayerAction::UseItem { hand } => self.guest_use_item(conn, gi, hand, None),
            PlayerAction::InteractOn {
                hand,
                target,
                location,
            } => {
                let id = i32::try_from(target.id()).map_err(|_| ActionRefused::InvalidTarget)?;
                self.guest_interact(conn, gi, id, hand, location)
            }
            PlayerAction::Attack { target } => {
                let id = i32::try_from(target.id()).map_err(|_| ActionRefused::InvalidTarget)?;
                self.guest_attack(conn, gi, id)
            }
            PlayerAction::BreakBlock { pos } => self.guest_break_block(conn, gi, pos),
        })();
        // FAIL ends the click, but is not a consuming InteractionResult.
        let consumed = consumed.map(|used| used && !self.side.use_failed);
        self.side.use_failed = failed;
        let consumed = consumed?;
        Ok(match acting.blocked() {
            Some(effect) => ActionOutcome::EffectBlocked(effect),
            None if consumed => ActionOutcome::Consumed,
            None => ActionOutcome::Passed,
        })
    }
    /// A guest's command for the mods, inside their swap (`run_guest_command`).
    /// `None` leaves dispatch to the caller; `Some(false)` is a handled failure.
    pub(in crate::game) fn perform_native_command(
        &mut self,
        conn: u32,
        permission: u8,
        line: &str,
        quiet: bool,
    ) -> Option<bool> {
        let host = self.lan.as_ref()?;
        if self.side.native_commands.0.level(line)? > permission {
            return None;
        }
        host.hub.guest(conn)?;
        let previous = std::mem::replace(&mut self.ui.registries.permission, permission);
        let ok = if quiet {
            self.run_command_quietly(line)
        } else {
            self.run_command(line)
        };
        self.ui.registries.permission = previous;
        Some(ok)
    }
    /// A console line for the mods: true when a mod registered its command
    /// (`native_register_commands`). The console is level 4, so a registered
    /// usage is always allowed; a command no mod takes answers as vanilla's
    /// unknown command.
    pub(in crate::game::server_side) fn perform_native_console_command(
        &mut self,
        line: &str,
    ) -> bool {
        if self.lan.is_none() || self.side.native_commands.0.level(line).is_none() {
            return false;
        }
        self.as_console_in_spawn_level(|game| {
            let previous = game
                .side
                .command_source
                .replace(super::commands::execute::Source {
                    silent: false,
                    ..game.server_source()
                });
            game.run_command(line);
            game.side.command_source = previous;
        });
        true
    }
    pub(in crate::game) fn server_player(&self, uuid: u128) -> Option<PlayerRef> {
        let epoch = self.native_lifecycle.epoch()?;
        let (_, id) = self.player_lookup(uuid)?;
        Some(mod_api::__private::game::player_ref(
            mod_api::EntityRef::new(epoch, id),
            uuid,
        ))
    }
    pub(in crate::game) fn server_player_read_both(
        &self,
        player: PlayerRef,
    ) -> Option<PlayerRead<'_>> {
        self.player_handle_read(player)
    }
    pub(in crate::game) fn server_player_client_brand(&self, player: PlayerRef) -> Option<&str> {
        if let Some(guest) = self.online_player_record(player) {
            return Some(&guest.brand);
        }
        (self.native_acting_player() == Some(player)).then_some("")
    }
    pub(in crate::game) fn server_player_collision_flags(
        &self,
        player: PlayerRef,
    ) -> Result<Option<mod_api::CollisionFlags>, mod_api::LevelReadError> {
        Ok(self.player_handle_read(player).and_then(|read| {
            // A connected player's position reports do not retain collision
            // flags, even when its body is swapped in for a server guest turn.
            let PlayerRead::Acting {
                body: Some(body), ..
            } = read
            else {
                return None;
            };
            if self.side.lan_acting.is_some() {
                return None;
            }
            let flags = body.collision;
            Some(mod_api::CollisionFlags::new(
                flags.horizontal,
                flags.minor,
                flags.vertical,
                flags.below,
            ))
        }))
    }
    /// Whether `SleepStatus.update` counts the player: not a spectator, not a
    /// fake, and not an ignored player who is out of bed (`sleep_counts`'s
    /// own rule). None for a stale or unavailable player.
    pub(in crate::game) fn player_counts_for_sleep(&self, player: PlayerRef) -> Option<bool> {
        let ignored = |game: &Self| {
            game.lan
                .as_ref()
                .is_some_and(|host| host.sleep_ignored.contains(&player.uuid()))
        };
        Some(match self.player_handle_read(player)? {
            // The player in place: the host's own, or a guest in their turn.
            PlayerRead::Acting { .. } => {
                !self.side.lan_acting_fake
                    && !self.spectator()
                    && (self.sleeping.is_some() || !ignored(self))
            }
            PlayerRead::Guest(guest) => self.guest_counts_for_sleep(guest),
        })
    }
    /// Server arm for the dimension key of an online player (`ServerPlayer.level().dimension()`).
    pub(in crate::game) fn server_player_dimension_key(
        &self,
        player: PlayerRef,
    ) -> Option<mod_api::DimensionKey> {
        let epoch = self.native_lifecycle.epoch()?;
        if epoch != player.fields().entity.epoch() {
            return None;
        }
        if self.native_acting_player() == Some(player) {
            let dim = match self.side.lan_acting {
                Some(conn) => {
                    let g = self.command_guest(conn)?;
                    Dim::from_id(&g.dimension).key()
                }
                None => self.dim.key(),
            };
            return Some(dim);
        }
        if let Some(guest) = self.online_player_record(player) {
            return Some(Dim::from_id(&guest.dimension).key());
        }
        if self.serving.is_some()
            && self.server_player(player.uuid()) == Some(player)
            && self
                .level
                .player_uuid
                .map(sunrise_client::lan::playerdata::ints_uuid)
                == Some(player.uuid())
        {
            return Some(self.dim.key());
        }
        None
    }
    /// Whether a live player handle names a connectionless actor.
    pub(in crate::game) fn player_is_fake(&self, player: PlayerRef) -> Option<bool> {
        match self.player_handle_read(player)? {
            PlayerRead::Acting { .. } => Some(self.side.lan_acting_fake),
            PlayerRead::Guest(guest) => Some(guest.is_fake()),
        }
    }
    /// The stable name of a connected or acting player. None for a stale or
    /// unavailable player; displaced online players retain their identity.
    pub(in crate::game) fn player_name(&self, player: PlayerRef) -> Option<&str> {
        if self.native_acting_player() == Some(player) {
            return self.side.lan_acting_name.as_deref();
        }
        Some(&self.online_player_record(player)?.name)
    }
    /// A machine's one use on the persistent actor, with its private state put back even
    /// when the action refuses. Only the existing guest turn runs gameplay and its hooks.
    pub(in crate::game) fn player_perform_action_scoped(
        &mut self,
        player: PlayerRef,
        action: PlayerAction,
        setup: PlayerActionSetup,
    ) -> Result<ScopedActionResult, ActionRefused> {
        let (gi, conn) = self.fake_action_actor(player, action)?;
        if !setup.eye.iter().all(|v| v.is_finite())
            || !setup.yaw.is_finite()
            || !setup.pitch.is_finite()
        {
            return Err(ActionRefused::InvalidSetup);
        }
        let full_attack_charge = setup.full_attack_charge;
        let mut state = ScopedPlayerState::new(self.motion, self.items(), setup)?;
        let host = self.lan.as_mut().expect("validated fake's host");
        host.hub.guests.update(gi, |guest| state.swap(guest));
        let play = host.play.insert(conn, Default::default());
        let menus = host.menus.insert(conn, Default::default());
        // This body is created with the setup's equipment and never reaches a body tick.
        // Install its modifiers before the first action or calculation of attack charge.
        let initialized = self.as_guest_at(gi, conn, |game| {
            game.put_equipment_modifiers();
            if full_attack_charge {
                game.charge_attack_fully();
            }
        });
        let outcome = if initialized.is_ok() {
            self.perform_fake_action(gi, conn, action)
        } else {
            Err(ActionRefused::Busy)
        };
        let host = self.lan.as_mut().expect("guest turns retain the host");
        let (held, other, position) = host.hub.guests.update(gi, |guest| {
            let held = guest
                .inventory
                .get_mut(guest.selected)
                .and_then(Option::take)
                .filter(|s| s.count > 0);
            let other: Vec<sunrise_items::items::ItemStack> = guest
                .inventory
                .iter_mut()
                .chain([&mut guest.offhand, &mut guest.carried])
                .chain(guest.armor.iter_mut())
                .filter_map(Option::take)
                .filter(|s| s.count > 0)
                .collect();
            let position = guest.position;
            state.swap(guest);
            (held, other, position)
        });
        host.menus.remove(&conn);
        if let Some(menus) = menus {
            host.menus.insert(conn, menus);
        }
        host.play.remove(&conn);
        if let Some(play) = play {
            host.play.insert(conn, play);
        }
        // A stack the API can't represent (a saved item this build doesn't have, #1364) is never
        // destroyed or replaced: it goes back into the world where the actor stood, with
        // vanilla's default pickup delay so a repeating action doesn't take it again next tick.
        // It joins as any drop does, so a join listener may keep it out; only the stacks that
        // joined are counted.
        let mut unrepresentable = 0;
        let mut represent = |stack: sunrise_items::items::ItemStack| match stack.try_into_api() {
            Ok(stack) => Some(stack),
            Err(_) => {
                // ItemEntity.setDefaultPickUpDelay.
                self.push_item_entity(stack, position, None, 10, None);
                if self.join_item_entity(self.side.next_entity, position) {
                    unrepresentable += 1;
                }
                None
            }
        };
        let held = held.and_then(&mut represent);
        let other = other.into_iter().filter_map(&mut represent).collect();
        outcome.map(|outcome| ScopedActionResult::new(outcome, held, other, unrepresentable))
    }
    /// Remove a fake without treating it as a disconnect or saving it.
    pub(in crate::game) fn player_remove_fake(&mut self, player: PlayerRef) -> bool {
        if self.native_lifecycle.epoch() != Some(player.fields().entity.epoch())
            || !self.side.lan_guest_swaps.is_empty()
        {
            return false;
        }
        let Some(mut host) = self.lan.take() else {
            return false;
        };
        let index = host.hub.guests.iter().position(|g| {
            g.is_fake()
                && g.uuid == player.fields().uuid
                && g.entity_id as u32 == player.fields().entity.id()
                && host
                    .hub
                    .holder_guest(g.uuid, &g.dimension)
                    .is_some_and(|owner| owner.conn == g.conn)
        });
        let removed = if let Some(index) = index {
            let guest = host.hub.guests.remove(index);
            self.cleanup_fake_player(&mut host, &guest);
            true
        } else {
            false
        };
        self.lan = Some(host);
        removed
    }
    /// `ServerPlayer.getRespawnConfig`: the player's respawn point as the
    /// game keeps it (the guest's record, or the acting player's in place),
    /// `Some(None)` when they have none. None for a stale or unavailable
    /// player, or a dimension key the API can't name.
    pub(in crate::game) fn player_respawn_config(
        &self,
        player: PlayerRef,
    ) -> Option<Option<PlayerRespawnConfig>> {
        let saved = match self.player_handle_read(player)? {
            PlayerRead::Acting { .. } => self.side.respawn_config.map(|c| c.saved()),
            PlayerRead::Guest(guest) => guest.respawn_elsewhere.clone(),
        };
        let Some((dimension, [x, y, z], yaw, _pitch, forced)) = saved else {
            return Some(None);
        };
        Some(Some(PlayerRespawnConfig {
            position: GlobalPos::new(Identifier::parse(&dimension).ok()?, BlockPos::new(x, y, z)),
            yaw,
            forced,
        }))
    }
    /// Stores a server player's `Abilities` and tells their client
    /// (`ServerPlayer.onUpdateAbilities`). They stay as written until the
    /// player's game mode changes (`GameType.updatePlayerAbilities`), and are
    /// saved with the player. A speed that is not finite refuses the write
    /// (vanilla's setters take any float). Returns false for a
    /// stale or unavailable player, a displaced outer player, and a speed
    /// that is not finite.
    pub(in crate::game) fn player_set_abilities(
        &mut self,
        player: PlayerRef,
        abilities: mod_api::PlayerAbilities,
    ) -> bool {
        if !abilities.fly_speed.is_finite() || !abilities.walk_speed.is_finite() {
            return false;
        }
        let stored = PlayerAbilities {
            invulnerable: abilities.invulnerable,
            flying: abilities.flying,
            may_fly: abilities.may_fly,
            instabuild: abilities.instabuild,
            may_build: abilities.may_build,
            flying_speed: abilities.fly_speed,
            walking_speed: abilities.walk_speed,
        };
        if self.player_handle_read(player).is_none() {
            return false;
        }
        let mut stored_here = false;
        self.native_as_player_handle(player, |game| {
            let Some(slot) = game.side.acting_abilities.as_mut() else {
                return;
            };
            *slot = stored;
            if let Some(body) = game.player.as_mut() {
                body.invulnerable = stored.invulnerable;
                body.may_fly = stored.may_fly;
                body.flying = stored.flying;
                body.flying_speed = stored.flying_speed;
            }
            stored_here = true;
        }) && stored_here
    }
    /// `Inventory.setSelectedSlot` for a server write: `slot` (0 to 8) becomes the hotbar slot the
    /// player holds and a guest's client is told (`SetHeldSlot`). A new slot ends a main-hand use,
    /// as `handleSetCarriedItem` does, without `player_switch_hotbar_slot_pre`, which is for the
    /// player's own input. False for a slot past 8, a stale or dead player.
    pub(in crate::game) fn player_set_selected_slot(
        &mut self,
        player: PlayerRef,
        slot: usize,
    ) -> bool {
        if slot >= 9 {
            return false;
        }
        // The connection to tell: a parked guest's, or the acting guest's, whose state is in
        // place for their turn (the put-back at the end of `guest_turn` stores the slot back but sends nothing). The host
        // has none.
        let conn = match self.player_handle_read(player) {
            Some(PlayerRead::Guest(guest)) => Some(guest.conn),
            Some(PlayerRead::Acting { .. }) => self.side.lan_acting_uuid.and_then(|uuid| {
                self.lan
                    .as_ref()?
                    .hub
                    .guests
                    .iter()
                    .find(|guest| guest.uuid == uuid)
                    .map(|guest| guest.conn)
            }),
            None => None,
        };
        let changed = self.player_selected_slot(player) != Some(slot);
        if !self.native_as_player_handle(player, |game| {
            if changed
                && game
                    .using
                    .is_some_and(|(held, _)| held != crate::game::shield::OFFHAND_SLOT)
            {
                game.stop_active_item_use();
            }
            game.selected = slot;
        }) {
            return false;
        }
        if let (Some(conn), Some(host)) = (conn, self.lan.as_ref()) {
            host.hub.send_to(
                conn,
                &C::SetHeldSlot(play::SetHeldSlot { slot: slot as i32 }),
            );
        }
        true
    }
    /// Drop the fake players belonging to a level as it closes.
    pub(in crate::game::server_side) fn remove_level_fake_players(&mut self, dimension: &str) {
        let Some(mut host) = self.lan.take() else {
            return;
        };
        let mut i = 0;
        while i < host.hub.guests.len() {
            let g = &host.hub.guests[i];
            if g.is_fake() && g.dimension == dimension {
                let guest = host.hub.guests.remove(i);
                self.cleanup_fake_player(&mut host, &guest);
            } else {
                i += 1;
            }
        }
        self.lan = Some(host);
    }
    /// The level changes the level in place's tickets made since the last
    /// call (`ChunkTickets::take_changes`), told to the mods in order as
    /// `chunk_ticket_level_updated`, with no player's state swapped in. The
    /// tickets keep them only while a mod listens (`update_chunk_tickets`).
    pub(in crate::game::server_side) fn run_chunk_ticket_level_updates(&mut self) {
        use sunrise_client::ticket_levels::UNLOADED;
        let changes = self.side.tickets.0.take_changes();
        if changes.is_empty() {
            return;
        }
        let Some(level) = self.level(self.dim.id()) else {
            return;
        };
        let told = |level| match level {
            UNLOADED => mod_api::ChunkTicketLevel::Unloaded,
            level => mod_api::ChunkTicketLevel::Loaded(level),
        };
        for change in changes {
            let chunk = mod_api::ChunkRef::new(level, [change.chunk.0, change.chunk.1]);
            self.native_chunk_ticket_level_updated(chunk, told(change.old), told(change.new));
        }
    }
    /// The chunks players started and stopped watching that the hub
    /// recorded (`lan::chunk_watch`), told to the mods in the hub's order,
    /// with no player's state swapped in, and a sent chunk's passages sent
    /// after it. The player handle is built from the record, so an unwatch
    /// after a disconnect still names them. Nothing while the log is empty.
    pub(in crate::game) fn run_chunk_watches(&mut self) {
        use sunrise_client::lan::chunk_watch::ChunkWatchKind;
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        if host.hub.chunk_watches.is_empty() {
            return;
        }
        let runs = host.hub.chunk_watches.take();
        let Some(epoch) = self.native_lifecycle.epoch() else {
            return;
        };
        for run in runs {
            // A level the server no longer holds: nothing to name.
            let Some(level) = self.level(&run.dimension) else {
                continue;
            };
            for watch in run.watches {
                let Ok(id) = u32::try_from(watch.entity_id) else {
                    continue;
                };
                let player = mod_api::__private::game::player_ref(
                    mod_api::EntityRef::new(epoch, id),
                    watch.uuid,
                );
                let chunk = mod_api::ChunkRef::new(level, watch.chunk);
                match watch.kind {
                    ChunkWatchKind::Watch => self.native_chunk_watch(player, chunk),
                    ChunkWatchKind::Sent => {
                        self.native_chunk_sent(player, chunk);
                        self.send_chunk_passages(level, watch.uuid, watch.chunk);
                    }
                    ChunkWatchKind::Unwatch => self.native_chunk_unwatch(player, chunk),
                }
            }
        }
    }
    /// The one path `native_run_command` and `native_run_command_with_output` run a line
    /// by: the console's source from the world spawn in its level, the native command filter on,
    /// and the running command's own chat capture, so a call inside another capture leaves
    /// the outer one as it was. With `capture`, the source is not silent and the feedback
    /// comes back as the second value.
    pub(in crate::game) fn run_console_command(
        &mut self,
        line: &str,
        capture: bool,
    ) -> (bool, Vec<sunrise_client::text::Text>) {
        if self.lan.is_none() {
            return (false, Vec::new());
        }
        let base = self.server_source();
        let server = execute::Source {
            silent: base.silent && !capture,
            ..base
        };
        let outer = self.side.command_source.replace(server);
        let filter = std::mem::replace(&mut self.side.native_command_filter, true);
        let outer_exec = std::mem::take(&mut self.side.exec.0);
        let outer_results = self.side.cmds.0.results.take();
        let outer_capture = capture.then(|| self.side.chat_capture.replace(Vec::new()));
        let ok = self.as_console(|g| g.run_command(line));
        let lines = match outer_capture {
            Some(outer_capture) => {
                std::mem::replace(&mut self.side.chat_capture, outer_capture).unwrap_or_default()
            }
            None => Vec::new(),
        };
        self.side.exec.0 = outer_exec;
        self.side.cmds.0.results = outer_results;
        self.side.native_command_filter = filter;
        self.side.command_source = outer;
        (ok, lines)
    }
    /// The shared command-node executor, also reached by functions and `execute`.
    pub(in crate::game::server_side) fn run_native_command(
        &mut self,
        context: Context,
    ) -> super::commands::R<i32> {
        use super::commands::err;
        use mod_api::__private::RetainedFailure;
        // Argument resolution uses node IDs from the parse. Never use them against a
        // replacement tree, even if the new tree has the same command at the same ID.
        let executor = context
            .executor
            .filter(|&key| self.ui.dispatcher.0.owns_executor(key))
            .and_then(|key| {
                self.side
                    .native_commands
                    .0
                    .registered
                    .as_ref()?
                    .executors
                    .get(key.slot())
            })
            .filter(|executor| executor.callback.enabled())
            .ok_or_else(|| err("command.unknown.command", &[]))?;
        let callback = executor.callback.clone();
        let call = Arc::clone(&executor.call);
        let command = self.resolve_native_command(context)?;
        let origin = self.native_command_origin(self.ui.registries.permission)?;
        // Isolate journaled writes when the caller already has a transaction. An ordinary
        // command is not implicitly transactional, and writes outside the journal stay put.
        let result = if self.has_open_transaction() {
            self.storage_transaction(|game, _| callback.call(|| call(game, &origin, &command)))
        } else {
            callback.call(|| call(self, &origin, &command))
        };
        match result {
            Ok(Ok(value)) => Ok(value),
            // Failure text can reach a packet or a saved command block. An
            // invalid component still reports an ordinary failed command.
            Ok(Err(message)) => Err(sunrise_client_lib::api_text::to_serialized_engine(&message)
                .map_or_else(
                    |_| err("command.failed", &[]),
                    sunrise_client::command::CommandError::new,
                )),
            Err(RetainedFailure::Disabled) => Err(err("command.unknown.command", &[])),
            Err(RetainedFailure::Panicked) => {
                Err(sunrise_client::command::CommandError::unexpected())
            }
        }
    }
    /// The open world's folder (`world/`, beside its `level.dat`): a server
    /// mod keeps its data in a folder of its own there, so the world's
    /// backups carry it. None when no world is open.
    pub(in crate::game) fn world_dir(&self) -> Option<PathBuf> {
        (!self.side.world_root.as_os_str().is_empty()).then(|| self.side.world_root.clone())
    }

    pub(in crate::game) fn player_perform_action(
        &mut self,
        player: PlayerRef,
        action: PlayerAction,
    ) -> Result<ActionOutcome, ActionRefused> {
        let (gi, conn) = self.fake_action_actor(player, action)?;
        self.perform_fake_action(gi, conn, action)
    }

    pub(in crate::game) fn player_set_game_mode(
        &mut self,
        player: PlayerRef,
        mode: GameMode,
    ) -> bool {
        if self.player_handle_read(player).is_none() {
            return false;
        }
        let mut changed = false;
        self.native_as_player_handle(player, |game| {
            changed = game.set_game_mode(mode);
        }) && changed
    }

    pub(in crate::game) fn player_drop_stack(
        &mut self,
        player: PlayerRef,
        stack: ItemStack,
        thrown_by_player: bool,
    ) -> Option<ItemStack> {
        if self.entity_ids_exhausted() {
            return Some(stack);
        }
        if stack.count == 0 {
            return Some(stack);
        }
        let copy = stack.clone();
        let mut dropped = false;
        let ran = self.native_as_player_handle(player, |game| {
            if game.acting_in_level_in_place() {
                dropped = game.drop_stack_from_eyes(stack, thrown_by_player).is_some();
            }
        });
        (!(ran && dropped)).then_some(copy)
    }

    pub(in crate::game) fn native_player_by_name(&self, name: &str) -> Option<PlayerRef> {
        let epoch = self.native_lifecycle.epoch()?;
        let player = self
            .lan
            .as_ref()?
            .hub
            .online_players()
            .find(|player| player.name.eq_ignore_ascii_case(name))?;
        Some(mod_api::__private::game::player_ref(
            mod_api::EntityRef::new(epoch, player.entity_id as u32),
            player.uuid,
        ))
    }

    pub(in crate::game) fn online_player_record(&self, player: PlayerRef) -> Option<&Guest> {
        (self.native_lifecycle.epoch()? == player.entity().epoch()).then_some(())?;
        let guests = &self.lan.as_ref()?.hub.guests;
        let guest = guests.get(guests.identity_index(player.uuid(), player.entity().id())?)?;
        (guest.joined() && guest.participates_online()).then_some(guest)
    }

    pub(in crate::game) fn player_address(&self, player: PlayerRef) -> Option<&str> {
        if let Some(guest) = self.online_player_record(player) {
            return Some(guest.ip_address());
        }
        (self.native_acting_player() == Some(player)).then_some("")
    }

    pub(in crate::game) fn player_virtual_host(&self, player: PlayerRef) -> Option<&VirtualHost> {
        if !self.is_logical_server() {
            return None;
        }
        self.online_player_record(player)?.sender.virtual_host()
    }

    pub(in crate::game) fn player_announced_channels(
        &self,
        player: PlayerRef,
    ) -> Option<Arc<[String]>> {
        static EMPTY: LazyLock<Arc<[String]>> = LazyLock::new(Arc::default);
        if let Some(guest) = self.online_player_record(player) {
            return Some(guest.sender.announced.list());
        }
        (self.native_acting_player() == Some(player)).then(|| Arc::clone(&EMPTY))
    }

    pub(in crate::game) fn message_player_uuid(&self, player: PlayerRef) -> Option<u128> {
        (self.native_acting_player() == Some(player) || self.online_player_record(player).is_some())
            .then_some(player.uuid())
    }

    pub(in crate::game) fn player_tell(&mut self, player: PlayerRef, line: Text) -> bool {
        let Some(uuid) = self.message_player_uuid(player) else {
            return false;
        };
        self.native_send(uuid, system_chat(&line))
    }

    pub(in crate::game) fn player_action_bar(&mut self, player: PlayerRef, line: Text) -> bool {
        let Some(uuid) = self.message_player_uuid(player) else {
            return false;
        };
        let packet = C::SystemChat(play::SystemChat {
            content: component_nbt(&line.component()),
            overlay: true,
        });
        self.native_send(uuid, packet)
    }

    pub(in crate::game) fn native_send(&mut self, uuid: u128, packet: C) -> bool {
        self.native_send_all(uuid, vec![packet])
    }

    pub(in crate::game) fn player_set_sidebar(
        &mut self,
        player: PlayerRef,
        sidebar: Option<Sidebar>,
    ) -> Result<bool, TextFormError> {
        let Some(conn) = self.online_player_record(player).map(|guest| guest.conn) else {
            return Ok(false);
        };
        let contents = sidebar
            .map(|sidebar| {
                let items = self.res.item_defaults().entries();
                SidebarContents::prepare(sidebar, &|item| items.contains_key(item)).map(Box::new)
            })
            .transpose()?;
        self.after_commit(super::transactions::After::PersonalSidebar(conn, contents));
        Ok(true)
    }

    pub(in crate::game) fn player_tab_list(
        &mut self,
        player: PlayerRef,
        header: &Text,
        footer: &Text,
    ) -> bool {
        let Some(uuid) = self.message_player_uuid(player) else {
            return false;
        };
        let packet = C::TabList(play::TabList {
            header: component_nbt(&header.component()),
            footer: component_nbt(&footer.component()),
        });
        self.native_send(uuid, packet)
    }

    pub(in crate::game) fn set_tick_rate(&mut self, rate: f32) -> bool {
        !LevelControl::TickRate.is_heard() && self.apply_tick_rate(rate)
    }

    pub(in crate::game) fn apply_tick_rate(&mut self, rate: f32) -> bool {
        if !self.is_logical_server() || !(1.0..=10_000.0).contains(&rate) {
            return false;
        }
        self.clock.set_rate(rate);
        self.on_tick_rate_changed();
        true
    }

    pub(in crate::game) fn player_set_sleep_ignored(
        &mut self,
        player: PlayerRef,
        ignored: bool,
    ) -> bool {
        let Some(uuid) = self.sleep_list_uuid(player) else {
            return false;
        };
        let Some(host) = &mut self.lan else {
            return false;
        };
        if ignored {
            host.sleep_ignored.insert(uuid);
        } else {
            host.sleep_ignored.remove(&uuid);
        }
        true
    }

    pub(in crate::game) fn sleep_list_uuid(&self, player: PlayerRef) -> Option<u128> {
        let uuid = self.message_player_uuid(player)?;
        self.lan
            .as_ref()?
            .hub
            .online_players()
            .any(|g| g.uuid == uuid)
            .then_some(uuid)
    }

    pub(in crate::game) fn player_sleep_ignored(&self, player: PlayerRef) -> Option<bool> {
        let uuid = self.sleep_list_uuid(player)?;
        Some(self.lan.as_ref()?.sleep_ignored.contains(&uuid))
    }

    pub(in crate::game) fn player_sleep_timer(&self, player: PlayerRef) -> Option<u32> {
        Some(match self.player_handle_read(player)? {
            PlayerRead::Acting { .. } => self.sleep_timer,
            PlayerRead::Guest(guest) => guest.sleep_timer,
        })
    }

    pub(in crate::game) fn player_set_sleep_timer(
        &mut self,
        player: PlayerRef,
        ticks: u32,
    ) -> bool {
        if ticks > super::MAX_SLEEP_TIMER || self.player_handle_read(player).is_none() {
            return false;
        }
        let Some(target) = self.native_entity_target(player.entity()) else {
            return false;
        };
        self.native_player_sleep_write(target, |game| {
            game.sleep_timer = ticks;
            true
        })
    }

    pub(in crate::game) fn player_text_filtering(&self, player: PlayerRef) -> Option<bool> {
        if let Some(guest) = self.online_player_record(player) {
            return Some(guest.info.text_filtering);
        }
        (self.native_acting_player() == Some(player)).then_some(false)
    }

    pub(in crate::game) fn player_set_respawn_config(
        &mut self,
        player: PlayerRef,
        config: Option<PlayerRespawnConfig>,
    ) -> bool {
        let stored = match config {
            Some(config) => {
                let dimension = config.position.dimension().to_string();
                if !config.yaw.is_finite() || !Dim::exists(&dimension) {
                    return false;
                }
                let at = config.position.pos();
                Some(RespawnConfig::wrapped(
                    Dim::from_id(&dimension),
                    [at.x, at.y, at.z],
                    config.yaw,
                    config.forced,
                ))
            }
            None => None,
        };
        match self.player_handle_read(player) {
            Some(PlayerRead::Acting { .. }) => {
                let mut set = false;
                self.native_as_acting(|game| set = game.set_respawn_point(stored)) && set
            }
            Some(PlayerRead::Guest(guest))
                if !guest.dead
                    || self
                        .cloning_guest(guest.uuid)
                        .is_some_and(|original| original.conn == guest.conn) =>
            {
                let conn = guest.conn;
                self.as_guest(conn, |game| game.set_respawn_point(stored)) == Some(true)
            }
            _ => false,
        }
    }

    pub(in crate::game) fn player_latency(
        &self,
        player: PlayerRef,
    ) -> Result<i32, mod_api::PlayerWorldRefusal> {
        use mod_api::PlayerWorldRefusal as Refusal;
        if !self.is_logical_server() {
            return Err(Refusal::Client);
        }
        if let Some(guest) = self.online_player_record(player) {
            return Ok(guest.latency.load(Ordering::Relaxed));
        }
        (self.native_acting_player() == Some(player) || self.player_handle_read(player).is_some())
            .then_some(0)
            .ok_or(Refusal::Stale)
    }

    pub(in crate::game) fn player_send_ping(
        &mut self,
        player: PlayerRef,
        id: i32,
    ) -> Result<(), mod_api::PlayerPingRefusal> {
        use mod_api::PlayerPingRefusal as Refusal;
        let Some(guest) = self.online_player_record(player) else {
            return Err(
                if self.native_acting_player() == Some(player)
                    || self.player_handle_read(player).is_some()
                {
                    Refusal::NoConnection
                } else {
                    Refusal::Stale
                },
            );
        };
        if !guest.play_connection_open() {
            return Err(Refusal::NoConnection);
        }
        guest
            .sender
            .try_send_play_ping(id)
            .then_some(())
            .ok_or(Refusal::SendFailed)
    }

    pub(in crate::game) fn player_hidden(
        &self,
        player: PlayerRef,
    ) -> Result<Option<u8>, mod_api::PlayerWorldRefusal> {
        use mod_api::PlayerWorldRefusal as Refusal;
        if !self.is_logical_server() {
            return Err(Refusal::Client);
        }
        let hub = &self.lan.as_ref().ok_or(Refusal::Stale)?.hub;
        if self.native_lifecycle.epoch() != Some(player.entity().epoch()) {
            return Err(Refusal::Stale);
        }
        Ok(hub.hidden.get(&player.uuid()).copied())
    }

    pub(in crate::game) fn player_nickname(
        &self,
        player: PlayerRef,
    ) -> Result<Option<&str>, mod_api::PlayerWorldRefusal> {
        use mod_api::PlayerWorldRefusal as Refusal;
        if !self.is_logical_server() {
            return Err(Refusal::Client);
        }
        let uuid = self.message_player_uuid(player).ok_or(Refusal::Stale)?;
        let hub = &self.lan.as_ref().ok_or(Refusal::Stale)?.hub;
        if !hub.online_players().any(|g| g.uuid == uuid) {
            return Err(Refusal::Stale);
        }
        Ok(hub.nicknames.get(&uuid).map(String::as_str))
    }

    pub(in crate::game) fn player_hide(&mut self, player: PlayerRef, level: Option<u8>) -> bool {
        if self.native_lifecycle.epoch() != Some(player.entity().epoch()) {
            return false;
        }
        let uuid = player.uuid();
        let Some(host) = &mut self.lan else {
            return false;
        };
        match level {
            Some(level) => host.hub.hidden.insert(uuid, level),
            None => host.hub.hidden.remove(&uuid),
        };
        true
    }

    pub(in crate::game) fn player_set_nickname(
        &mut self,
        player: PlayerRef,
        nickname: Option<&str>,
    ) -> bool {
        let Some(uuid) = self.message_player_uuid(player) else {
            return false;
        };
        let Some(host) = &mut self.lan else {
            return false;
        };
        if !host.hub.online_players().any(|g| g.uuid == uuid) {
            return false;
        }
        host.hub.set_nickname(uuid, nickname.map(str::to_owned));
        true
    }

    pub(in crate::game) fn native_broadcast(&self, mut line: Text) {
        let mut cancel = false;
        self.native_broadcast_message(&mut line, &mut cancel);
        if cancel {
            return;
        }
        let plain = line.plain();
        if self.is_dedicated_server() {
            super::dedicated::log("INFO", &plain);
        } else {
            log::info!("{plain}");
        }
        let Some(host) = &self.lan else {
            return;
        };
        let packet = system_chat(&line);
        for g in host.hub.guests.iter().filter(|g| g.placed) {
            host.hub.send_to(g.conn, &packet);
        }
    }

    pub(in crate::game) fn native_server_event(
        &self,
        name: &str,
        uuid: u128,
        message: &str,
    ) -> bool {
        if !self.is_dedicated_server() || message.trim().is_empty() {
            return false;
        }
        super::dedicated::events::record("announce", name, uuid, json!({ "message": message }));
        true
    }

    pub(in crate::game) fn native_run_command(&mut self, line: &str) -> bool {
        self.run_console_command(line, false).0
    }

    pub(in crate::game) fn experience_orbs(
        &self,
        dimension: &str,
        min: [f64; 3],
        max: [f64; 3],
    ) -> Vec<mod_api::ExperienceOrb> {
        let Some((min, max)) = ordered_box(min, max) else {
            return Vec::new();
        };
        if (self.serving.is_none() && self.options.dedicated.is_none())
            || dimension != self.dim.id()
        {
            return Vec::new();
        }
        self.orbs
            .iter()
            .filter(|o| o.count > 0)
            .map(|o| mod_api::ExperienceOrb {
                id: o.id,
                position: [o.position.x, o.position.y, o.position.z],
                value: o.value,
                count: o.count,
            })
            .filter(|o| orb_meets(o.position, min, max))
            .collect()
    }

    pub(in crate::game) fn set_experience_orb(
        &mut self,
        dimension: &str,
        id: u32,
        value: Option<i32>,
        count: Option<u32>,
        age: Option<u32>,
    ) -> bool {
        if (self.serving.is_none() && self.options.dedicated.is_none())
            || dimension != self.dim.id()
            || count == Some(0)
            || count.is_some_and(|c| c > crate::orb_limits::MAX_COUNT)
            || value.is_some_and(|v| !crate::orb_limits::value_fits(v))
        {
            return false;
        }
        let Ok(age) = age.map(i32::try_from).transpose() else {
            return false;
        };
        let Some(orb) = self.orbs.iter_mut().find(|o| o.id == id) else {
            return false;
        };
        orb.value = value.unwrap_or(orb.value);
        orb.count = count.unwrap_or(orb.count);
        orb.age = age.unwrap_or(orb.age);
        true
    }

    pub(in crate::game) fn native_player_holder(&self, uuid: u128) -> Option<&Guest> {
        self.cloning_guest(uuid)
            .or_else(|| self.lan.as_ref()?.hub.holder_guest(uuid, self.dim.id()))
    }

    pub(in crate::game) fn native_live_acting_body(&self) -> Option<&PlayerMotion> {
        let body = self.player.as_ref()?;
        if body.health.health() <= 0.0 {
            return None;
        }
        Some(body)
    }

    pub(in crate::game) fn native_as_acting(&mut self, f: impl FnOnce(&mut Self)) -> bool {
        if self.native_live_acting_body().is_none() {
            return false;
        }
        f(self);
        true
    }

    pub(in crate::game) fn resend_commands_filtered(&mut self) {
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        host.hub.refresh_permissions();
        self.native_filter_commands(None);
        if let Some(host) = self.lan.as_mut() {
            host.hub.resend_commands();
        }
    }

    pub(in crate::game) fn native_command_player_target(
        &self,
        target: super::commands::Target,
    ) -> Option<PlayerRef> {
        use super::commands::Target;
        match target {
            Target::Player => self.native_acting_player(),
            Target::Guest(conn) => self
                .command_guest(conn)
                .and_then(|guest| self.player(guest.uuid)),
            _ => None,
        }
    }

    pub(in crate::game) fn native_command_entity_target(
        &mut self,
        target: super::commands::Target,
    ) -> Option<mod_api::EntityRef> {
        if let Some(player) = self.native_command_player_target(target) {
            return Some(player.entity());
        }
        let id = super::native_entities::target_id(target)?;
        self.in_target_level(target, |game| game.entity(id))
            .flatten()
    }

    pub(in crate::game) fn resolve_native_command(
        &mut self,
        context: Context,
    ) -> Result<mod_api::CommandInvocation, sunrise_client::command::CommandError> {
        use super::commands::err;
        use mod_api::{CommandArgument, CommandArgumentValue as Value};
        let mut arguments = Vec::with_capacity(context.args.len());
        for argument in context.args {
            let ty = parsed_type(&self.ui.dispatcher.0, &context.nodes, argument.range);
            let value = match argument.value {
                Arg::Str(text) if matches!(ty, Some(ArgType::Nbt)) => nbt_value(&text)?,
                Arg::Str(value) => Value::String(value),
                Arg::Int(value) => Value::Integer(value),
                Arg::Long(value) => Value::Long(value),
                Arg::Double(value) => Value::Number(value),
                Arg::Bool(value) => Value::Boolean(value),
                Arg::Entity(selector) => {
                    let (single, players) = context
                        .nodes
                        .iter()
                        .find_map(|(node, range)| {
                            if *range != argument.range {
                                return None;
                            }
                            match &self.ui.dispatcher.0.nodes[*node].kind {
                                sunrise_client::command::NodeKind::Argument {
                                    ty: ArgType::Entity { single, players },
                                    ..
                                } => Some((*single, *players)),
                                _ => None,
                            }
                        })
                        .expect("parsed entity argument has its dispatcher node");
                    let targets = self.resolve_entity_argument(&selector, players)?;
                    if single && targets.len() > 1 {
                        return Err(err(
                            if players {
                                "argument.player.toomany"
                            } else {
                                "argument.entity.toomany"
                            },
                            &[],
                        ));
                    }
                    if players {
                        if single {
                            Value::Player(
                                self.native_command_player_target(targets[0])
                                    .ok_or_else(|| err("argument.entity.notfound.player", &[]))?,
                            )
                        } else {
                            let selected = targets
                                .into_iter()
                                .map(|target| {
                                    self.native_command_player_target(target)
                                        .ok_or_else(|| err("argument.entity.notfound.player", &[]))
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                            Value::Players(selected)
                        }
                    } else if single {
                        Value::Entity(
                            self.native_command_entity_target(targets[0])
                                .ok_or_else(|| err("argument.entity.notfound.entity", &[]))?,
                        )
                    } else {
                        let selected: Vec<_> = targets
                            .into_iter()
                            .filter_map(|target| self.native_command_entity_target(target))
                            .collect();
                        if selected.is_empty() {
                            return Err(err("argument.entity.notfound.entity", &[]));
                        }
                        Value::Entities(selected)
                    }
                }
                Arg::Coords(coordinates) => positional_value(
                    ty.as_ref(),
                    &coordinates,
                    self.resolve_coords(&coordinates),
                    self.source_rotation(),
                ),
                Arg::Item(input) => Value::Item(
                    self.command_item_stack(&input, 1)?
                        .try_into_api()
                        .expect("command item validation resolves registered items"),
                ),
                Arg::Resource(id) if matches!(ty, Some(ArgType::Dimension)) => {
                    dimension_value(&id, |dimension| self.level(dimension))?
                }
                Arg::Resource(id) => Value::Id(
                    mod_api::Identifier::parse(&id).map_err(|_| err("command.failed", &[]))?,
                ),
                Arg::Custom { kind, value } => custom_value(&kind, value),
                _ => unreachable!("the native tree contains only registered native argument kinds"),
            };
            arguments.push(CommandArgument::new(argument.name, value));
        }
        Ok(self.native_invocation(context.literals, arguments))
    }

    pub(in crate::game) fn native_online_mode(&self) -> Option<bool> {
        Some(online_mode_of(&self.dedicated_lan()?.shared.config))
    }

    pub(in crate::game) fn native_server_port(&self) -> Option<u16> {
        Some(self.dedicated_lan()?.port)
    }

    pub(in crate::game) fn dedicated_lan(&self) -> Option<&sunrise_client::lan::Lan> {
        self.options.dedicated.as_ref()?;
        Some(&self.lan.as_ref()?.hub.lan)
    }

    pub(in crate::game) fn server_crafting_remainder_hook(
        &self,
        stack: &ItemStack,
        remainder: &mut Option<ItemStack>,
    ) {
        self.native_crafting_remainder(stack, remainder);
    }

    pub(in crate::game) fn server_world_table_items(
        &self,
    ) -> Option<&Arc<sunrise_client::items::Items>> {
        self.side
            .world_tables
            .0
            .as_ref()
            .map(|tables| &tables.items)
    }

    pub(in crate::game) fn server_lid_openers(&self) -> Option<&HashMap<(i32, i32, i32), i32>> {
        Some(&self.side.lid_openers)
    }

    pub(in crate::game) fn server_signal_capability_site(
        &self,
        at: mod_api::BlockSide,
    ) -> Option<mod_api::BlockCapabilitySite> {
        let [x, y, z] = at.pos;
        self.containers()
            .capability_site(at, self.world.block(x, y, z), false)
    }

    pub(in crate::game) fn server_signal_container_size(
        &self,
        at: mod_api::BlockSide,
    ) -> Option<usize> {
        self.container_size(at)
    }

    pub(in crate::game) fn server_signal_container_stack(
        &self,
        at: mod_api::BlockSide,
        slot: usize,
    ) -> Option<mod_api::ItemStack> {
        self.container_stack(at, slot)?.try_into_api().ok()
    }

    pub(in crate::game) fn server_level_block_face_sturdy(
        &self,
        level: LevelRef,
        pos: [i32; 3],
        face: mod_api::BlockFace,
        support: mod_api::SupportType,
    ) -> Option<bool> {
        self.level_block_face_sturdy(level, pos, face, support)
    }

    pub(in crate::game) fn server_signal_block_entity(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<mod_api::BlockEntityRef> {
        self.block_entity(level, pos)
    }

    pub(in crate::game) fn server_signal_block_entity_data(
        &self,
        entity: mod_api::BlockEntityRef,
    ) -> Option<mod_api::Compound> {
        self.block_entity_data(entity)
    }

    pub(in crate::game) fn server_level_is_raining_at(
        &self,
        level: LevelRef,
        pos: [i32; 3],
    ) -> Option<bool> {
        self.level_is_raining_at(level, pos)
    }
}

impl<__Side: crate::game::Side> crate::game::Game<'_, __Side> {
    pub(in crate::game) fn cloning_guest(&self, uuid: u128) -> Option<&Guest> {
        let &(_, conn, _) = self
            .player_state
            .cloning_players
            .iter()
            .rev()
            .find(|&&(player, _, _)| player == uuid)?;
        self.command_guest(conn)
            .filter(|_| !crate::game::Side::lan_guest_swaps(&self.side).contains(&conn))
    }

    pub(in crate::game) fn player_handle_read(&self, player: PlayerRef) -> Option<PlayerRead<'_>> {
        (self.native_lifecycle.epoch()? == player.fields().entity.epoch()).then_some(())?;
        if crate::game::Side::lan_acting_fake(&self.side)
            && crate::game::Side::lan_acting_uuid(&self.side) == Some(player.fields().uuid)
            && crate::game::Side::lan_acting_entity(&self.side) == Some(player.fields().entity.id())
        {
            return Some(PlayerRead::Acting {
                mode: Some(self.mode_id()),
                body: self.player.as_ref(),
                abilities: crate::game::Side::acting_abilities(&self.side),
                last_death: self.level.last_death.as_ref(),
            });
        }
        if let Some((read, id)) = self.player_lookup(player.fields().uuid)
            && id == player.fields().entity.id()
        {
            return Some(read);
        }
        let guests = &self.lan.as_ref()?.hub.guests;
        let guest = guests
            .get(guests.identity_index(player.fields().uuid, player.fields().entity.id())?)?;
        if !guest.placed
            || !guest.is_fake()
            || crate::game::Side::lan_guest_swaps(&self.side).contains(&guest.conn)
        {
            return None;
        }
        Some(PlayerRead::Guest(guest))
    }

    pub(in crate::game) fn player_lookup(&self, uuid: u128) -> Option<(PlayerRead<'_>, u32)> {
        if crate::game::Side::lan_acting_uuid(&self.side) == Some(uuid)
            && !crate::game::Side::lan_acting_fake(&self.side)
        {
            return Some((
                PlayerRead::Acting {
                    mode: Some(self.mode_id()),
                    body: self.player.as_ref(),
                    abilities: crate::game::Side::acting_abilities(&self.side),
                    last_death: self.level.last_death.as_ref(),
                },
                crate::game::Side::lan_acting_entity(&self.side)?,
            ));
        }
        if let Some(guest) = self.cloning_guest(uuid).or_else(|| {
            self.lan.as_ref().and_then(|host| {
                host.hub.guests.simulation_iter().find(|guest| {
                    host.hub.online(guest)
                        && guest.uuid == uuid
                        && !crate::game::Side::lan_guest_swaps(&self.side).contains(&guest.conn)
                })
            })
        }) {
            return Some((PlayerRead::Guest(guest), guest.entity_id as u32));
        }
        if crate::game::Side::lan_acting_uuid(&self.side).is_none()
            && self.serving.is_some()
            && self
                .level
                .player_uuid
                .map(sunrise_client::lan::playerdata::ints_uuid)
                == Some(uuid)
        {
            let body = self.player.as_ref()?;
            return Some((
                PlayerRead::Acting {
                    mode: Some(self.mode_id()),
                    body: Some(body),
                    abilities: crate::game::Side::acting_abilities(&self.side),
                    last_death: self.level.last_death.as_ref(),
                },
                body.entity_id as u32,
            ));
        }
        None
    }
}

#[cfg(test)]
#[path = "native_server/command_failure_tests_server.rs"]
pub(in crate::game) mod command_failure_tests;

#[cfg(test)]
#[path = "native_server/selector_tests_server.rs"]
pub(in crate::game) mod selector_tests;

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_server::tests::Heard;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_server::tests::HostileArgument;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_server::tests::MachineArgument;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_server::tests::Machines;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_server::tests::MixedOffsets;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_server::tests::POSITIONED;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_server::tests::PatternArgument;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_server::tests::Watcher;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_server::tests::player_guest;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_server::tests::stack;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_server::tests::tree;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_server::tests::with_command_sources;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_server::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_client::items::ItemStack;

    #[test]
    #[ignore = "daily"]
    fn command_refresh_rebuilds_the_typed_dispatcher_only_on_request() {
        crate::game::in_own_process(
            "game::server_side::native_server_server::tests::command_refresh_rebuilds_the_typed_dispatcher_only_on_request",
            || {
                use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
                struct ChangingCommands {
                    changed: Arc<AtomicBool>,
                    registrations: Arc<AtomicUsize>,
                    calls: Arc<Mutex<Vec<Option<&'static str>>>>,
                    replacement: bool,
                }
                #[mod_api::__subscriptions]
                impl mod_api::Hooks for ChangingCommands {
                    fn register_commands(
                        &self,
                        _: &mut dyn mod_api::ServerHost,
                        event: &mut mod_api::RegisterCommandsEvent<'_>,
                    ) {
                        let changed = self.changed.load(Ordering::Relaxed);
                        if !self.replacement {
                            self.registrations.fetch_add(1, Ordering::Relaxed);
                            let usage = if changed {
                                "after <n:integer>"
                            } else {
                                "before"
                            };
                            event
                                .commands
                                .push(mod_api::Command::new(usage).executes(|_, _, _| Ok(1)));
                        }
                        if changed == self.replacement {
                            let calls = self.calls.clone();
                            let result = if self.replacement { 37 } else { 0 };
                            event
                                .commands
                                .push(mod_api::Command::new("retained").executes(
                                    move |_, _, _| {
                                        calls
                                            .lock()
                                            .unwrap()
                                            .push(mod_api::content::crash::current_native_mod());
                                        Ok(result)
                                    },
                                ));
                        }
                    }
                    fn player_commands_sent(
                        &self,
                        _: &mut dyn mod_api::ServerHost,
                        event: &mut mod_api::PlayerCommandsSentEvent<'_>,
                    ) {
                        event.names.retain(|name| name != "before");
                    }
                }
                let changed = Arc::new(AtomicBool::new(false));
                let registrations = Arc::new(AtomicUsize::new(0));
                let calls = Arc::new(Mutex::new(Vec::new()));
                mod_platform::testing::with_mods(
                    [("refresh", false), ("replacement", true)]
                        .map(|(id, replacement)| {
                            mod_api::ModEntry::new(
                                id,
                                Box::new(ChangingCommands {
                                    changed: changed.clone(),
                                    registrations: registrations.clone(),
                                    calls: calls.clone(),
                                    replacement,
                                }),
                            )
                        })
                        .into(),
                    || {
                        super::super::native_guest_turn_tests::with_guests(|game| {
                            game.build_registries();
                            game.register_native_commands();
                            assert_eq!(registrations.load(Ordering::Relaxed), 1);
                            let old_parse =
                                game.ui.dispatcher.0.parse("retained", &game.ui.registries);
                            let old_context = old_parse.executable().unwrap().0;
                            let (result, outcome) = game.recording_command_outcome(|game| {
                                game.execute_line("retained", game.server_source())
                            });
                            assert_eq!(result, Ok(0));
                            assert_eq!(outcome, Some((true, 0)));
                            assert_eq!(*calls.lock().unwrap(), [Some("refresh")]);
                            changed.store(true, Ordering::Relaxed);
                            game.register_native_commands();
                            assert_eq!(registrations.load(Ordering::Relaxed), 1);
                            let registered =
                                game.side.native_commands.0.registered.as_ref().unwrap();
                            assert!(registered.levels.contains_key("before"));
                            mod_api::Commands::refresh_commands(game);
                            mod_api::Commands::refresh_commands(game);
                            assert_eq!(
                                registrations.load(Ordering::Relaxed),
                                1,
                                "requests are deferred"
                            );
                            game.register_native_commands();
                            assert!(!game.side.native_commands.0.refresh);
                            assert_eq!(
                                registrations.load(Ordering::Relaxed),
                                2,
                                "requests coalesce"
                            );
                            assert_eq!(
                                game.run_native_command(old_context.clone())
                                    .unwrap_err()
                                    .key,
                                "command.unknown.command"
                            );
                            assert_eq!(*calls.lock().unwrap(), [Some("refresh")]);
                            assert_eq!(game.execute_line("retained", game.server_source()), Ok(37));
                            assert_eq!(
                                *calls.lock().unwrap(),
                                [Some("refresh"), Some("replacement")]
                            );
                            assert!(mod_api::__private::set_mod_enabled("replacement", false));
                            assert_eq!(
                                game.execute_line("retained", game.server_source())
                                    .unwrap_err()
                                    .key,
                                "command.unknown.command"
                            );
                            assert_eq!(
                                *calls.lock().unwrap(),
                                [Some("refresh"), Some("replacement")]
                            );
                            assert!(mod_api::__private::set_mod_enabled("replacement", true));
                            let commands = game.side.native_commands.0.registered.as_ref().unwrap();
                            assert!(!commands.levels.contains_key("before"));
                            assert!(commands.levels.contains_key("after"));
                            let dispatcher = &game.ui.dispatcher.0;
                            assert!(
                                dispatcher
                                    .parse("after 7", &game.ui.registries)
                                    .executable()
                                    .is_ok()
                            );
                            assert!(
                                dispatcher
                                    .parse("after nope", &game.ui.registries)
                                    .executable()
                                    .is_err()
                            );
                            assert!(
                                dispatcher
                                    .parse("before", &game.ui.registries)
                                    .executable()
                                    .is_err()
                            );
                            game.register_native_commands();
                            assert_eq!(registrations.load(Ordering::Relaxed), 2);
                            mod_api::Commands::refresh_commands(game);
                            game.register_native_commands();
                            assert_eq!(registrations.load(Ordering::Relaxed), 3);
                            assert!(game.side.native_commands.0.level("after").is_some());
                            changed.store(false, Ordering::Relaxed);
                            game.reload_datapacks(false, &game.level.enabled_features.clone())
                                .unwrap();
                            let (sent, received) = std::sync::mpsc::channel();
                            game.lan
                                .as_mut()
                                .unwrap()
                                .hub
                                .update_guest(12, |guest| {
                                    guest.sender = sunrise_client::lan::PlayerSender::new(
                                        sent,
                                        Default::default(),
                                    );
                                })
                                .unwrap();
                            game.register_native_commands();
                            let packet = received
                                .try_iter()
                                .flat_map(|out| match out {
                                    sunrise_client::lan::Out::Packet(body) => vec![body],
                                    sunrise_client::lan::Out::Packets(bodies) => bodies,
                                    sunrise_client::lan::Out::Disconnect(_) => {
                                        panic!("reload disconnected guest")
                                    }
                                })
                                .find_map(|body| {
                                    let mut reader = sunrise_protocol::buf::Reader::new(&body);
                                    let id = reader.varint().unwrap();
                                    match C::decode(id, &body[reader.position()..]).unwrap() {
                                        C::Commands(packet) => Some(packet),
                                        _ => None,
                                    }
                                })
                                .expect("reload must deliver a command tree");
                            let names: Vec<_> = packet.nodes[packet.root as usize]
                                .children
                                .iter()
                                .map(|&i| packet.nodes[i as usize].name.as_str())
                                .collect();
                            assert!(!names.is_empty());
                            assert!(
                                !names.contains(&"before"),
                                "new reload root must be filtered"
                            );
                            assert!(!names.contains(&"after"), "old root must be retired");
                        })
                    },
                );
            },
        );
    }

    #[test]
    #[ignore = "daily"]
    fn native_command_feedback_respects_track_output_and_silence() {
        use super::super::commands::execute::Source;
        use sunrise_client::chunk::block_entity::BlockEntity;
        with_command_sources(|game| {
            let source = Source {
                block: Some((1, 64, 1)),
                silent: false,
                ..game.server_source()
            };
            for track in [true, false] {
                if let Some(BlockEntity::CommandBlock(block)) =
                    game.world.block_entities.get_mut(&(1, 64, 1))
                {
                    block.track_output = track;
                    block.last_output = None;
                }
                game.with_source(source, |game| {
                    game.native_feedback(Text::literal("answer"), false)
                });
                let Some(BlockEntity::CommandBlock(block)) =
                    game.world.block_entities.get(&(1, 64, 1))
                else {
                    panic!("block missing")
                };
                assert_eq!(block.last_output.is_some(), track);
            }
            game.side.chat_capture = Some(Vec::new());
            game.with_source(
                Source {
                    silent: true,
                    block: None,
                    ..source
                },
                |game| game.native_feedback(Text::literal("silent"), false),
            );
            assert!(game.side.chat_capture.take().unwrap().is_empty());
            assert!(game.side.command_source.is_none());
            game.as_guest(12, |game| {
                let source = game.server_source();
                game.with_source(source, |game| {
                    assert!(matches!(
                        game.native_command_origin(4),
                        Ok(mod_api::CommandSource::Console { .. })
                    ));
                });
            })
            .unwrap();
            game.side.exec.0.functions.push("test:outer".into());
            assert!(game.native_run_command("list"));
            assert_eq!(game.side.exec.0.functions, ["test:outer"]);
        });
    }

    #[test]
    #[ignore = "daily"]
    fn native_commands_keep_function_block_and_cart_sources() {
        crate::game::in_own_process(
            "game::server_side::native_server_server::tests::native_commands_keep_function_block_and_cart_sources",
            || {
                use mod_api::{CommandSource, PermissionLevel as Permission};
                use sunrise_client::{
                    chunk::block_entity::BlockEntity,
                    datapack::{Function, FunctionLine},
                };
                use sunrise_core::position::BlockPos;
                struct Commands(Arc<Mutex<Vec<CommandSource>>>);
                #[mod_api::__subscriptions]
                impl mod_api::Hooks for Commands {
                    fn register_commands(
                        &self,
                        _: &mut dyn mod_api::ServerHost,
                        event: &mut mod_api::RegisterCommandsEvent<'_>,
                    ) {
                        let heard = self.0.clone();
                        let execute =
                            move |host: &mut dyn mod_api::ServerHost,
                                  source: &CommandSource,
                                  _: &mod_api::CommandInvocation| {
                                heard.lock().unwrap().push(source.clone());
                                host.commands()
                                    .feedback(mod_api::Text::literal("source answer"), false)
                                    .unwrap();
                                Ok(1)
                            };
                        event.commands.extend([
                            mod_api::Command::new("origin").executes(execute.clone()),
                            mod_api::Command::op("denied", 4).executes(execute),
                            // Replaced by the later accepted registration; failure must not fall back here.
                            mod_api::Command::new("unhandled").executes(|_, _, _| Ok(99)),
                            mod_api::Command::new("unhandled")
                                .executes(|_, _, _| Err(mod_api::Text::literal("refused").into())),
                        ]);
                    }
                }
                let heard = Arc::new(Mutex::new(Vec::new()));
                mod_platform::testing::with_mods(
                    vec![mod_api::ModEntry::new(
                        "origins",
                        Box::new(Commands(heard.clone())),
                    )]
                    .into(),
                    || {
                        with_command_sources(|game| {
                            game.command_block_tick(BlockPos::new(1, 64, 1));
                            let Some(BlockEntity::CommandBlock(block)) =
                                game.world.block_entities.get(&(1, 64, 1))
                            else {
                                panic!("block missing")
                            };
                            assert_eq!(block.success_count, 1);
                            let source = heard.lock().unwrap()[0].clone();
                            assert!(
                                matches!(source, CommandSource::CommandBlock { position: [1, 64, 1], level, .. } if level == game.dim.key())
                            );
                            assert_eq!(source.permission(), Permission::Gamemasters);
                            assert!(game.perform_cart_command(99));
                            assert_eq!(game.carts[0].command.success_count, 1);
                            assert!(game.carts[0].command.last_output.is_some());
                            assert!(
                                matches!(heard.lock().unwrap().last().unwrap(), CommandSource::CommandBlockMinecart { entity, .. } if entity.id() == 99)
                            );
                            game.side.chat_capture = Some(Vec::new());
                            let mut function = Function {
                                id: "test:inner".into(),
                                lines: vec![
                                    FunctionLine::Command(
                                        "execute positioned 2 64 3 run origin".into(),
                                    ),
                                    FunctionLine::Command("denied".into()),
                                ],
                                parameters: vec![],
                            };
                            game.ensure_datapacks();
                            game.side
                                .fns
                                .0
                                .store
                                .functions
                                .insert(function.id.clone(), function.clone());
                            function.id = "test:outer".into();
                            function.lines =
                                vec![FunctionLine::Command("function test:inner".into())];
                            let before = heard.lock().unwrap().len();
                            game.run_function(&function, None, game.server_source())
                                .unwrap();
                            assert_eq!(heard.lock().unwrap().len(), before + 1);
                            assert!(
                                matches!(heard.lock().unwrap().last().unwrap(), CommandSource::Function { id, permission } if id.to_string() == "test:inner" && *permission == Permission::Gamemasters)
                            );
                            assert!(
                                game.side.chat_capture.take().unwrap().is_empty(),
                                "functions suppress both success and failure"
                            );
                            assert!(game.side.exec.0.functions.is_empty());
                            game.ui.registries.permission = 4;
                            assert!(!game.run_command("unhandled"));
                            assert!(game.run_command(
                                "execute store result storage test:result value int 1 run origin"
                            ));
                            assert!(game.run_command(
                                "execute if data storage test:result {value:1} run origin"
                            ));
                        });
                    },
                );
            },
        );
    }

    #[test]
    #[ignore = "daily"]
    fn native_typed_commands_reach_listeners_after_parse_and_source_resolution() {
        crate::game::in_own_process(
            "game::server_side::native_server_server::tests::native_typed_commands_reach_listeners_after_parse_and_source_resolution",
            || {
                use mod_api::{CommandArgumentValue as Value, CommandInvocation};
                struct Commands(Arc<Mutex<Vec<(bool, CommandInvocation)>>>);
                #[mod_api::__subscriptions]
                impl mod_api::Hooks for Commands {
                    fn register_commands(
                        &self,
                        _: &mut dyn mod_api::ServerHost,
                        event: &mut mod_api::RegisterCommandsEvent<'_>,
                    ) {
                        let heard = self.0.clone();
                        let record =
                            move |_: &mut dyn mod_api::ServerHost,
                                  source: &mod_api::CommandSource,
                                  command: &CommandInvocation| {
                                if command.literals == ["typed", "give"] {
                                    let Some(Value::Integer(amount)) = command.argument("amount")
                                    else {
                                        panic!("executor must receive an integer")
                                    };
                                    let Some(Value::Player(player)) = command.argument("who")
                                    else {
                                        panic!("executor must receive a resolved player")
                                    };
                                    assert_ne!(*amount, 0);
                                    assert_ne!(player.uuid(), 0);
                                }
                                let console =
                                    matches!(source, mod_api::CommandSource::Console { .. });
                                heard.lock().unwrap().push((console, command.clone()));
                                Ok(1)
                            };
                        event.commands.extend([
                            mod_api::Command::new("typed give <who:player> <amount:integer> [message:text]"),
                            mod_api::Command::new("typed data <word:word> <number:number> <enabled:boolean> <item:item> <pos:block_pos>"),
                            mod_api::Command::new("typed opt [value:integer]"),
                            mod_api::Command::new("/set <x:word>"),
                            mod_api::Command::new("typed repeat <value:player> <value:integer>"),
                            mod_api::Command::new("typed at <pos:vec3>"),
                            mod_api::Command::op("protected <who:player>", 4),
                        ].map(|command| command.executes(record.clone())));
                    }
                }
                let heard = Arc::new(Mutex::new(Vec::new()));
                mod_platform::testing::with_mods(
                    vec![mod_api::ModEntry::new(
                        "typed_commands",
                        Box::new(Commands(heard.clone())),
                    )]
                    .into(),
                    || {
                        super::super::native_guest_turn_tests::with_guests(|game| {
                            game.build_registries();
                            game.register_native_commands();
                            let other = game.player(13).unwrap();
                            let acting = game.player(12).unwrap();
                            game.as_guest(12, |game| {
                                for line in [
                                    "typed give guest13 nope",
                                    "typed give guest13",
                                    "typed give missing 7",
                                    "typed give @a 7",
                                    "typed data word 1 maybe stone ~ ~ ~",
                                    "typed data word NaN true stone ~ ~ ~",
                                    "typed data word 1 true does_not_exist ~ ~ ~",
                                    "typed opt 1 extra",
                                ] {
                                    assert_eq!(game.perform_native_command(12, 4, line, false), Some(false), "{line}");
                                }
                                let random = game.world.random.state();
                                assert_eq!(game.perform_native_command(12, 0, "protected @r", false), None);
                                assert_eq!(game.world.random.state(), random, "denied selectors must not resolve");
                                assert!(heard.lock().unwrap().is_empty(), "failed input must not reach listeners");
                                for line in [
                                    "typed give guest13 37 hello typed world",
                                    "typed give @s -4",
                                    "typed opt",
                                    "typed opt 9",
                                    "typed data word -1.25 true stone[minecraft:custom_name='\"Rock\"'] ~1 ~2 ~-3",
                                    "typed data word 2 false dirt ^0 ^0 ^0",
                                ] {
                                    assert_eq!(game.perform_native_command(12, 4, line, false), Some(true), "{line}");
                                }
                            }).unwrap();
                            game.spawn = Some(sunrise_core::geometry::Vec3::new(10.5, 70.0, -2.5));
                            game.yaw = 45.0;
                            game.pitch = 30.0;
                            for (line, key) in [
                                ("typed give guest13 nope", "parsing.int.expected"),
                                ("typed give missing 1", "argument.entity.notfound.player"),
                            ] {
                                game.side.chat_capture = Some(Vec::new());
                                assert!(game.perform_native_console_command(line));
                                let output = game.side.chat_capture.take().unwrap();
                                assert_eq!(
                                    output[0].plain(),
                                    Text::translate(key, &[]).plain(),
                                    "{line}"
                                );
                            }
                            assert!(game.perform_native_console_command("typed give @s 1"));
                            assert_eq!(
                                heard.lock().unwrap().len(),
                                6,
                                "console has no source player"
                            );
                            assert!(game.perform_native_console_command("typed give guest13 11"));
                            assert!(game.perform_native_console_command(
                                "typed data console 3 true dirt ~1 ~2 ~3"
                            ));
                            assert!(game.perform_native_console_command(
                                "typed data console 3 true dirt ^0.5 ^0.5 ^0.5"
                            ));
                            assert!(game.perform_native_console_command("typed repeat guest13 3"));
                            assert!(game.perform_native_console_command("typed at 1 64.25 ~1"));
                            // A point is three numbers: two, or text, is a parse error and is not heard
                            // (the count of 12 heard lines below holds neither).
                            game.perform_native_console_command("typed at 1 64.25");
                            game.perform_native_console_command("typed at 1 up 3");
                            assert!(
                                game.side.command_source.is_none(),
                                "console source restored"
                            );
                            game.as_guest(12, |game| {
                                assert!(
                                    game.native_run_command("execute as guest13 run typed opt 13")
                                )
                            })
                            .unwrap();
                            let observations = heard.clone();
                            let heard = heard.lock().unwrap();
                            assert_eq!(heard.len(), 12);
                            let first = &heard[0].1;
                            assert_eq!(first.literals, ["typed", "give"]);
                            assert_eq!(first.argument("who"), Some(&Value::Player(other)));
                            assert_eq!(first.argument("amount"), Some(&Value::Integer(37)));
                            assert_eq!(
                                first.argument("message"),
                                Some(&Value::String("hello typed world".into()))
                            );
                            assert_eq!(heard[1].1.argument("who"), Some(&Value::Player(acting)));
                            assert_eq!(heard[1].1.argument("amount"), Some(&Value::Integer(-4)));
                            assert!(heard[1].1.argument("message").is_none());
                            assert!(heard[2].1.arguments.is_empty());
                            assert_eq!(heard[3].1.argument("value"), Some(&Value::Integer(9)));
                            let data = &heard[4].1;
                            assert_eq!(data.argument("word"), Some(&Value::String("word".into())));
                            assert_eq!(data.argument("number"), Some(&Value::Number(-1.25)));
                            assert_eq!(data.argument("enabled"), Some(&Value::Boolean(true)));
                            assert_eq!(data.argument("pos"), Some(&Value::BlockPos([4, 66, 2])));
                            let Some(Value::Item(item)) = data.argument("item") else {
                                panic!("typed item")
                            };
                            assert_eq!(
                                mod_api::__private::item_key_id(item.item()),
                                Some("minecraft:stone")
                            );
                            assert_eq!(item.count(), 1);
                            assert!(item.components().is_some(), "command components retained");
                            assert_eq!(
                                heard[5].1.argument("enabled"),
                                Some(&Value::Boolean(false))
                            );
                            assert_eq!(
                                heard[5].1.argument("pos"),
                                Some(&Value::BlockPos([3, 64, 5]))
                            );
                            assert!(heard[..6].iter().all(|(console, _)| !console));
                            assert!(heard[6..].iter().all(|(console, _)| *console));
                            assert_eq!(heard[6].1.argument("who"), Some(&Value::Player(other)));
                            assert_eq!(heard[6].1.argument("amount"), Some(&Value::Integer(11)));
                            assert_eq!(
                                heard[7].1.argument("pos"),
                                Some(&Value::BlockPos([11, 72, 0]))
                            );
                            assert_eq!(
                                heard[8].1.argument("pos"),
                                Some(&Value::BlockPos([10, 70, -3]))
                            );
                            assert_eq!(heard[9].1.arguments[0].value, Value::Player(other));
                            assert_eq!(heard[9].1.argument("value"), Some(&Value::Integer(3)));
                            // `1 64.25 ~1`: a whole x is a block centre (1.5), y is as typed, and `~1`
                            // is one block from the console's z.
                            assert_eq!(
                                heard[10].1.argument("pos"),
                                Some(&Value::Vec3([1.5, 64.25, -2.0]))
                            );
                            drop(heard);
                            game.lan
                                .as_mut()
                                .unwrap()
                                .hub
                                .update_guest(12, |g| g.permission = 4)
                                .unwrap();
                            let inventory = game
                                .lan
                                .as_ref()
                                .unwrap()
                                .hub
                                .guest(12)
                                .unwrap()
                                .inventory
                                .clone();
                            game.guest_command(12, "/set stone", None);
                            game.guest_command(12, "typed opt 17", None);
                            game.guest_command(12, "/give guest12 minecraft:stone 1", None);
                            game.run_guest_commands();
                            let heard = observations.lock().unwrap();
                            assert_eq!(heard.len(), 14);
                            assert_eq!(heard[12].1.literals, ["/set"]);
                            assert_eq!(
                                heard[12].1.argument("x"),
                                Some(&Value::String("stone".into()))
                            );
                            assert_eq!(heard[13].1.literals, ["typed", "opt"]);
                            assert_eq!(heard[13].1.argument("value"), Some(&Value::Integer(17)));
                            assert_eq!(
                                game.lan.as_ref().unwrap().hub.guest(12).unwrap().inventory,
                                inventory
                            );
                        });
                    },
                );
            },
        );
    }
}

#[cfg(test)]
pub(in crate::game) mod fake_action_tests {
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::super::Game;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::super::{
        boats::Boat, minecarts::CART_KEY_Y, minecarts::CartKind, minecarts::Minecart,
        respawn_anchor::RespawnConfig, stations::Kind, stations::Station,
    };
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::{
        Acting, ActionOutcome, ActionRefused, FakeEffect, PlayerAction,
    };
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_server::fake_action_tests::AttackWatcher;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_server::fake_action_tests::ChargeWatcher;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_server::fake_action_tests::fake;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_server::fake_action_tests::health;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_server::fake_action_tests::held;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_server::fake_action_tests::on_top;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_server::fake_action_tests::place;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_server::fake_action_tests::trader;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_server::fake_action_tests::zombie;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_server::fake_action_tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use mod_api::{BlockHit, Hand, PlayerRef};
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_client::items::ItemStack;

    #[test]
    #[ignore = "daily"]
    fn a_fakes_action_is_refused_before_anything_runs() {
        super::super::native_entity_refs::tests::with_game(|game| {
            let (player, ..) = fake(game, Some("minecraft:water_bucket"), 0);
            // Out of reach: nothing used, the bucket still full.
            let far = PlayerAction::UseOn {
                hand: Hand::Main,
                pos: [40, 63, 0],
                hit: BlockHit::new([40.5, 64.0, 0.5], [0, 1, 0], false),
            };
            assert_eq!(
                game.player_perform_action(player, far),
                Err(ActionRefused::OutOfReach)
            );
            assert_eq!(
                held(game, 2).map(|s| s.item.to_string()).as_deref(),
                Some("minecraft:water_bucket")
            );
            // A real player's handle.
            let real = game.player(1).unwrap();
            assert_eq!(
                game.player_perform_action(real, on_top([1, 63, 0])),
                Err(ActionRefused::NotFake)
            );
            // Inside another action, as a hook running during it would ask.
            let acting = Acting::start(&game.side.fake_action.0);
            assert_eq!(
                game.player_perform_action(player, on_top([1, 63, 0])),
                Err(ActionRefused::Busy)
            );
            drop(acting);
            assert!(!game.side.fake_action.0.acting());
        });
    }

    #[test]
    #[ignore = "daily"]
    fn a_fakes_action_leaves_the_per_action_state_as_it_was() {
        super::super::native_entity_refs::tests::with_game(|game| {
            let (player, ..) = fake(game, Some("minecraft:oak_log"), 0);
            let before = super::super::Hit {
                pos: [7, 7, 7],
                face: [1, 0, 0],
                point: [7.0, 7.5, 7.5],
                inside: true,
            };
            game.side.forced_hit = Some(before);
            assert_eq!(
                game.player_perform_action(player, on_top([1, 63, 0])),
                Ok(ActionOutcome::Consumed)
            );
            let after = game.side.forced_hit.expect("the forced hit is put back");
            assert_eq!(
                (after.pos, after.face, after.point, after.inside),
                (before.pos, before.face, before.point, before.inside)
            );
            assert!(game.side.guest_use.is_none());
        });
    }

    /// A fake player's click on a shulker box or an ender chest is refused
    /// at the open: no open count climbs, no stand-in chest enters the level
    /// and no lid opens.
    #[test]
    #[ignore = "daily"]
    fn a_shulker_box_and_an_ender_chest_a_fake_player_used_are_left_shut() {
        super::super::native_entity_refs::tests::with_game(|game| {
            let (player, ..) = fake(game, None, 0);
            let shulker = [1, 64, 0];
            place(game, shulker, "minecraft:shulker_box");
            let count = |game: &crate::game::ServerGame<'_>| {
                game.be_anims.shulkers.get(&shulker).map(|s| s.open_count)
            };
            assert_eq!(
                game.player_perform_action(player, on_top(shulker)),
                Ok(ActionOutcome::EffectBlocked(FakeEffect::Menu))
            );
            assert!(
                count(game).is_none_or(|c| c == 0),
                "the use is refused at the open, nothing opened"
            );
            assert!(game.container.is_none());
            assert_eq!(
                game.player_perform_action(player, on_top(shulker)),
                Ok(ActionOutcome::EffectBlocked(FakeEffect::Menu)),
                "the next use is refused the same way",
            );
            assert!(
                count(game).is_none_or(|c| c == 0),
                "the count doesn't climb"
            );

            let diamonds = Some(ItemStack::new("minecraft:diamond", 5));
            game.side.ender_items[0] = diamonds.clone();
            let ender = [2, 64, 0];
            place(game, ender, "minecraft:ender_chest");
            let key = (ender[0], ender[1], ender[2]);
            // A raw block state has no stand-in. Establish the closed state
            // whose preservation this refused action is meant to check.
            game.open_ender_chest(key);
            game.close_ender_chest();
            game.container = None;
            assert_eq!(
                game.player_perform_action(player, on_top(ender)),
                Ok(ActionOutcome::EffectBlocked(FakeEffect::Menu))
            );
            assert!(game.side.ender_open.is_none());
            match game.world.block_entities.get(&key) {
                Some(sunrise_client::chunk::block_entity::BlockEntity::Chest(items)) => assert!(
                    items.iter().all(Option::is_none),
                    "the player's items stayed"
                ),
                other => panic!("the ender chest lost its block entity: {other:?}"),
            }
            assert_eq!(
                game.side.ender_items[0], diamonds,
                "the server's own ender items changed"
            );
            assert!(
                !game.side.lid_openers.contains_key(&key),
                "its lid stayed open"
            );
        });
    }

    /// `attack_entity` runs at `Player.attack`'s start, before the blow and the
    /// swing's attack-strength reset (vanilla's `getAttackStrengthScale(0.5)`
    /// is read there too, and `onAttack` resets the ticker after it). The hook
    /// sees the target untouched, the blow uses the charge that was there when
    /// the hook ran, and a veto leaves both the target and the charge alone.
    /// The charge itself is read through the damage it deals: the read a mod
    /// gets in the hook is `PlayerRef::attack_strength_scale` (part of the same
    /// issue, landed separately).
    #[test]
    #[ignore = "daily"]
    fn attack_entity_runs_before_the_blow_resets_the_attack_charge() {
        use super::super::native_entity_refs::tests::with_game;
        use std::sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        };

        let seen = Arc::new(Mutex::new(Vec::new()));
        let cancel = Arc::new(AtomicBool::new(false));
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "attack-watcher",
            Box::new(AttackWatcher {
                seen: seen.clone(),
                cancel: cancel.clone(),
            }),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            // Half a fist's cooldown (delay 5 ticks): the charge is (2 + 0.5) / 5 = 0.5.
            with_game(|game| {
                let (player, fake_conn, _) = fake(game, None, 0);
                // A placed fake participates in actions, not online-player tracking.
                // The packet attack gate must still reach the hook and damage below.
                let host = game.lan.as_ref().unwrap();
                let guest = host.hub.guest(fake_conn).unwrap();
                assert!(guest.is_fake());
                assert!(guest.placed);
                assert_eq!(guest.dimension, host.hub.config.dimension);
                assert!(!guest.participates_online());
                game.lan
                    .as_mut()
                    .unwrap()
                    .hub
                    .guests
                    .update_all(|g| g.yaw = -90.0);
                let near = zombie(game, 5000, [2.0, 64.0, 0.5]);
                let full = health(game, 5000);
                game.as_guest(fake_conn, |g| g.side.attack_ticks = 2)
                    .unwrap();
                assert_eq!(
                    game.player_perform_action(player, PlayerAction::Attack { target: near }),
                    Ok(ActionOutcome::Consumed)
                );
                assert_eq!(
                    *seen.lock().unwrap(),
                    vec![full],
                    "the hook runs before the blow lands"
                );
                // The hit then uses the charge that was there for the hook: 0.5 scales a fist's
                // 1 to 0.4, and a zombie's 2 armour takes `getDamageAfterAbsorb`'s share.
                let dealt = 0.4f32 * (1.0 - (2.0 - 0.4 / 2.0) / 25.0);
                assert!(
                    (full - health(game, 5000) - dealt).abs() < 1e-4,
                    "the blow uses the pre-reset charge"
                );
                assert_eq!(
                    game.as_guest(fake_conn, |g| g.side.attack_ticks).unwrap(),
                    0,
                    "the swing resets the charge after the hit"
                );
            });
            // A veto at the hook leaves the target unhurt and the charge where it was: neither
            // the hit nor the reset has run yet.
            with_game(|game| {
                let (player, fake_conn, _) = fake(game, None, 0);
                game.lan
                    .as_mut()
                    .unwrap()
                    .hub
                    .guests
                    .update_all(|g| g.yaw = -90.0);
                let near = zombie(game, 5001, [2.0, 64.0, 0.5]);
                let full = health(game, 5001);
                seen.lock().unwrap().clear();
                cancel.store(true, Ordering::Relaxed);
                game.as_guest(fake_conn, |g| g.side.attack_ticks = 2)
                    .unwrap();
                assert_eq!(
                    game.player_perform_action(player, PlayerAction::Attack { target: near }),
                    Ok(ActionOutcome::Passed)
                );
                assert_eq!(*seen.lock().unwrap(), vec![full]);
                assert_eq!(health(game, 5001), full, "a veto stops the blow");
                assert_eq!(
                    game.as_guest(fake_conn, |g| g.side.attack_ticks).unwrap(),
                    2,
                    "and the swing that would reset the charge never starts"
                );
            });
        });
    }

    #[test]
    #[ignore = "daily"]
    fn attack_strength_rejects_non_finite_partial_without_restricting_finite_ticks() {
        use super::super::native_entity_refs::tests::with_game;
        with_game(|game| {
            let (player, conn, _) = fake(game, None, 0);
            game.as_guest(conn, |g| g.side.attack_ticks = 2).unwrap();
            for partial in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                assert_eq!(game.player_attack_strength_scale(player, partial), None);
                game.as_guest(conn, |g| {
                    assert_eq!(g.attack_strength_scale_at(partial), None);
                    assert_eq!(g.player_attack_strength_scale(player, partial), None);
                })
                .unwrap();
            }
            for (partial, expected) in [(-1.0, 0.2), (0.5, 0.5), (2.0, 0.8)] {
                assert_eq!(
                    game.player_attack_strength_scale(player, partial),
                    Some(expected)
                );
                game.as_guest(conn, |g| {
                    assert_eq!(g.attack_strength_scale_at(partial), Some(expected));
                    assert_eq!(
                        g.player_attack_strength_scale(player, partial),
                        Some(expected)
                    );
                })
                .unwrap();
            }
        });
    }

    /// `PlayerRef::attack_strength_scale` inside `attack_entity` reads the
    /// charge the blow uses: `Player.attack` reads `getAttackStrengthScale`
    /// itself at its start, and the hook runs before that read and before the
    /// swing resets the ticker. After the blow the same read sees 0. A
    /// resting player reads its saved ticker and body too.
    #[test]
    #[ignore = "daily"]
    fn attack_strength_scale_reads_the_charge_at_the_hook_and_not_after_the_swing() {
        use super::super::native_entity_refs::tests::with_game;
        use std::sync::{Arc, Mutex};

        let seen = Arc::new(Mutex::new(Vec::new()));
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "charge-watcher",
            Box::new(ChargeWatcher(seen.clone())),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            with_game(|game| {
                let (player, fake_conn, _) = fake(game, None, 0);
                game.lan
                    .as_mut()
                    .unwrap()
                    .hub
                    .guests
                    .update_all(|g| g.yaw = -90.0);
                let near = zombie(game, 5000, [2.0, 64.0, 0.5]);
                let full = health(game, 5000);
                // A fist's cooldown is 5 ticks: a full ticker reads 1.0, and half of one
                // ((2 + 0.5) / 5) reads 0.5.
                game.as_guest(fake_conn, |g| g.side.attack_ticks = 5)
                    .unwrap();
                assert_eq!(
                    player.attack_strength_scale(game, 0.5),
                    Some(1.0),
                    "a full ticker is a fully charged blow"
                );
                game.as_guest(fake_conn, |g| g.side.attack_ticks = 2)
                    .unwrap();
                assert_eq!(
                    player.attack_strength_scale(game, 0.5),
                    Some(0.5),
                    "a resting fake reads its saved ticker and body"
                );
                assert_eq!(
                    game.player_perform_action(player, PlayerAction::Attack { target: near }),
                    Ok(ActionOutcome::Consumed)
                );
                assert_eq!(*seen.lock().unwrap(), vec![Some(0.5)]);
                // The blow uses the charge the hook read: 0.5 scales a fist's 1 to 0.4, and a
                // zombie's 2 armour takes `getDamageAfterAbsorb`'s share.
                let dealt = 0.4f32 * (1.0 - (2.0 - 0.4 / 2.0) / 25.0);
                assert!((full - health(game, 5000) - dealt).abs() < 1e-4);
                assert_eq!(
                    player.attack_strength_scale(game, 0.5),
                    Some(0.1),
                    "the swing reset the ticker to 0 after the blow: (0 + 0.5) / 5"
                );
            });
        });
    }

    /// A scoped attack with `full_attack_charge` deals the sword's full damage, one without it
    /// only the empty charge's share, and the fake's own ticker comes back after each.
    #[test]
    #[ignore = "daily"]
    fn scoped_fake_attack_with_full_charge_deals_the_weapons_full_damage() {
        use super::super::native_entity_refs::tests::{run_fresh_cases, with_game};
        use mod_api::PlayerActionSetup;
        if run_fresh_cases(
            "game::server_side::native_server_server::fake_action_tests::scoped_fake_attack_with_full_charge_deals_the_weapons_full_damage",
            "SUNRISE_SCOPED_ATTACK_CASE",
            &["attack"],
        ) {
            return;
        }
        with_game(|game| {
            let (player, conn, _) = fake(game, None, 0);
            game.as_guest(conn, |g| g.side.attack_ticks = 3).unwrap();
            let charged = zombie(game, 5000, [2.0, 64.0, 0.5]);
            // Out of a sweep's reach of the first.
            let uncharged = zombie(game, 5001, [-1.5, 64.0, 0.5]);
            let mut attack = |target, full_attack_charge| {
                let mut setup = PlayerActionSetup::new(
                    Some(crate::game::api_values::test_snapshot(&ItemStack::new(
                        "minecraft:diamond_sword",
                        1,
                    ))),
                    [0.5, 64.0 + f64::from(1.62_f32), 0.5],
                    0.0,
                    0.0,
                    false,
                );
                setup.full_attack_charge = full_attack_charge;
                let result = player
                    .perform_action_scoped(game, PlayerAction::Attack { target }, setup)
                    .unwrap();
                assert_eq!(result.outcome, ActionOutcome::Consumed);
                assert_eq!(
                    game.as_guest(conn, |g| g.side.attack_ticks).unwrap(),
                    3,
                    "the fake's own ticker comes back"
                );
            };
            attack(charged, true);
            attack(uncharged, false);
            // Diamond sword: 7 damage and 4 + (-2.4f32) attack speed. Player.attack samples
            // the empty ticker at 0.5, then CombatRules applies the zombie's 2 armour.
            let delay = (1.0 / (4.0 + f64::from(-2.4f32)) * 20.0) as f32;
            let charge = 0.5f32 / delay;
            let after_hit = |damage: f32| {
                let armor = (2.0 - damage / 2.0).max(2.0 * 0.2);
                20.0 - damage * (1.0 - armor / 25.0)
            };
            assert_eq!(health(game, 5000), after_hit(7.0));
            assert_eq!(
                health(game, 5001),
                after_hit(7.0 * (0.2 + charge * charge * 0.8))
            );
        });
    }

    #[test]
    #[ignore = "daily"]
    fn scoped_fake_use_hooks_see_the_setup_and_fail_is_not_consumed() {
        use mod_api::{Flow, Hooks, HostSide, PlayerActionSetup, RightClickBlockEvent};
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        #[derive(Default)]
        struct Deny(Arc<AtomicBool>);
        #[mod_api::sunrise_mod]
        impl Hooks for Deny {
            fn right_click_block(&self, host: HostSide, event: &mut RightClickBlockEvent) -> Flow {
                let HostSide::Server(host) = host else {
                    panic!("server action");
                };
                assert_eq!(event.click.player, 2);
                let entity = host.entity(event.click.player_entity_id as u32).unwrap();
                assert_eq!(entity.rotation(host), Some([123.0, 17.0]));
                assert_eq!(
                    entity.eye_position(host),
                    Some([0.5, 64.0 + f64::from(1.62_f32), 0.5])
                );
                assert_eq!(entity.is_shift_key_down(host), Some(true));
                self.0.store(true, Ordering::Relaxed);
                *event.cancellation_result = mod_api::InteractionResult::Fail;
                Flow::Cancel
            }
        }
        let heard = Arc::new(AtomicBool::new(false));
        mod_platform::testing::with_mods(
            vec![mod_api::ModEntry::new(
                "deny",
                Box::new(Deny(heard.clone())),
            )]
            .into(),
            || {
                super::super::native_entity_refs::tests::with_game(|game| {
                    let (player, ..) = fake(game, Some("minecraft:diamond"), 0);
                    game.side.use_failed = true;
                    let result = player
                        .perform_action_scoped(
                            game,
                            on_top([1, 63, 0]),
                            PlayerActionSetup::new(
                                Some(crate::game::api_values::test_snapshot(&ItemStack::new(
                                    "minecraft:oak_log",
                                    1,
                                ))),
                                [0.5, 64.0 + f64::from(1.62_f32), 0.5],
                                123.0,
                                17.0,
                                true,
                            ),
                        )
                        .unwrap();
                    assert!(
                        heard.load(Ordering::Relaxed),
                        "normal Host-bearing hook ran"
                    );
                    assert_eq!(result.outcome, ActionOutcome::Passed);
                    assert_eq!(
                        result.held,
                        Some(crate::game::api_values::test_snapshot(&ItemStack::new(
                            "minecraft:oak_log",
                            1
                        )))
                    );
                    assert!(game.side.use_failed, "outer per-action flag restored");
                    let level = game.level(game.dim.id()).unwrap();
                    assert_eq!(
                        game.level_block(level, [1, 64, 0])
                            .and_then(mod_api::__private::block_key_id),
                        Some("minecraft:air")
                    );
                    assert_eq!(held(game, 2), Some(ItemStack::new("minecraft:diamond", 1)));
                });
            },
        );
    }

    #[test]
    #[ignore = "daily"]
    fn scoped_fake_use_uses_viewpoint_secondary_use_and_restores_on_refusal() {
        use mod_api::PlayerActionSetup;
        super::super::native_entity_refs::tests::with_game(|game| {
            let (player, conn, _) = fake(game, Some("minecraft:diamond"), 1);
            let setup = |item, sneaking| {
                PlayerActionSetup::new(
                    Some(crate::game::api_values::test_snapshot(&ItemStack::new(
                        item, 1,
                    ))),
                    [0.5, 64.0 + f64::from(1.62_f32), 0.5],
                    0.0,
                    90.0,
                    sneaking,
                )
            };
            // Air use rays from the supplied eyes/look, not the persistent fake's view.
            let mut viewpoint = setup("minecraft:water_bucket", false);
            viewpoint.eye = [2.5, 64.0 + f64::from(1.62_f32), 2.5];
            viewpoint.yaw = 117.0;
            let result = game
                .player_perform_action_scoped(
                    player,
                    PlayerAction::UseItem { hand: Hand::Main },
                    viewpoint,
                )
                .unwrap();
            assert_eq!(result.outcome, ActionOutcome::Consumed);
            assert_eq!(
                result.held,
                Some(crate::game::api_values::test_snapshot(&ItemStack::new(
                    "minecraft:bucket",
                    1
                )))
            );
            let level = game.level(game.dim.id()).unwrap();
            assert_eq!(
                game.level_block(level, [2, 64, 2])
                    .and_then(mod_api::__private::block_key_id),
                Some("minecraft:water")
            );
            place(game, [1, 64, 0], "minecraft:chest");
            let result = game
                .player_perform_action_scoped(
                    player,
                    on_top([1, 64, 0]),
                    setup("minecraft:oak_log", false),
                )
                .unwrap();
            assert_eq!(
                result.outcome,
                ActionOutcome::EffectBlocked(FakeEffect::Menu)
            );
            let result = game
                .player_perform_action_scoped(
                    player,
                    on_top([1, 64, 0]),
                    setup("minecraft:oak_log", true),
                )
                .unwrap();
            assert_eq!(result.outcome, ActionOutcome::Consumed);
            assert_eq!(
                result.held, None,
                "survival placement consumes the temporary stack"
            );
            assert_eq!(
                game.level_block(level, [1, 65, 0])
                    .and_then(mod_api::__private::block_key_id),
                Some("minecraft:oak_log")
            );
            assert_eq!(
                game.player_perform_action_scoped(
                    player,
                    on_top([40, 63, 0]),
                    setup("minecraft:oak_log", false)
                ),
                Err(ActionRefused::OutOfReach)
            );
            let mut invalid = setup("minecraft:oak_log", false);
            invalid.eye[0] = f64::NAN;
            assert_eq!(
                game.player_perform_action_scoped(player, on_top([1, 63, 0]), invalid),
                Err(ActionRefused::InvalidSetup)
            );
            let real = game.player(1).unwrap();
            assert_eq!(
                game.player_perform_action_scoped(
                    real,
                    on_top([1, 63, 0]),
                    setup("minecraft:oak_log", false)
                ),
                Err(ActionRefused::NotFake)
            );
            let acting = Acting::start(&game.side.fake_action.0);
            assert_eq!(
                game.player_perform_action_scoped(
                    player,
                    on_top([1, 63, 0]),
                    setup("minecraft:oak_log", false)
                ),
                Err(ActionRefused::Busy)
            );
            drop(acting);
            assert_eq!(held(game, 2), Some(ItemStack::new("minecraft:diamond", 1)));
            let guest = game.lan.as_ref().unwrap().hub.guest(conn).unwrap();
            assert_eq!(guest.game_mode, 1);
            assert_eq!(guest.position, [0.5, 64.0, 0.5]);
        });
    }

    /// Each field holds a value the use can't have written, so a swap that
    /// forgets one fails whether the use changed it or not.
    #[test]
    #[ignore = "daily"]
    fn the_servers_own_player_state_comes_back_after_a_fake_use() {
        use mod_api::PlayerActionSetup;
        super::super::native_entity_refs::tests::with_game(|game| {
            let (player, ..) = fake(game, None, 0);
            let dim = game.dim;
            let config = RespawnConfig {
                dim,
                pos: [1, 2, 3],
                yaw: 0.0,
                pitch: 0.0,
                forced: false,
            };
            let spawn = sunrise_core::geometry::Vec3::new(1.0, 2.0, 3.0);
            game.station = Some(Station::new_kind(Kind::Anvil, [4, 5, 6]));
            game.side.craft_table = Some([7, 8, 9]);
            game.experience.level = 7;
            game.side.respawn_config = Some(config);
            game.spawn = Some(spawn);
            // An ender inventory left open goes back while the fake player
            // acts, and its stand-in returns afterwards, as a guest's turn does.
            let stand_in = (12, 64, 8);
            game.open_ender_chest(stand_in);
            game.container = None;

            place(game, [1, 64, 0], "minecraft:crafting_table");
            assert_eq!(
                game.player_perform_action(player, on_top([1, 64, 0])),
                Ok(ActionOutcome::EffectBlocked(FakeEffect::Menu))
            );
            assert_eq!(
                game.station.as_ref().map(|s| (s.kind, s.pos)),
                Some((Kind::Anvil, [4, 5, 6])),
            );
            assert_eq!(game.side.craft_table, Some([7, 8, 9]));
            assert_eq!(game.experience.level, 7);
            assert_eq!(game.side.respawn_config, Some(config));
            assert_eq!(game.spawn, Some(spawn));
            assert_eq!(game.side.ender_open, Some(stand_in));
            assert!(game.world.block_entities.get(&stand_in).is_some());

            let setup = PlayerActionSetup::new(
                Some(crate::game::api_values::test_snapshot(&ItemStack::new(
                    "minecraft:fishing_rod",
                    1,
                ))),
                [0.5, 64.0 + f64::from(1.62_f32), 0.5],
                0.0,
                0.0,
                false,
            );
            let rod = player
                .perform_action_scoped(game, PlayerAction::UseItem { hand: Hand::Main }, setup)
                .unwrap();
            assert_eq!(rod.outcome, ActionOutcome::Consumed, "the rod casts");
            assert!(game.hook.is_none(), "no hook is left for nobody to reel in");
        });
    }
}
