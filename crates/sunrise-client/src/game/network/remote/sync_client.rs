#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::sync::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::tab_list::{TabPlayer, default_skin};

impl Game<'_> {
    /// `Entity.push`'s share of the other entities' ticks: added to the local
    /// player's deltaMovement, unless it lies dead, sleeps or rides.
    pub(in crate::game::client_side) fn apply_remote_push(&mut self, pushed: Vec3) {
        let still =
            self.client.screen == Screen::Dead || self.sleeping.is_some() || self.riding.is_some();
        if let Some(player) = self.player.as_mut()
            && !still
        {
            player.velocity = player.velocity.add_vector(pushed);
        }
    }
    /// `LocalPlayer.tickDeath`: the level stops ticking the player at death tick 20,
    /// but the client keeps the player for death-screen rendering until respawn.
    pub(in crate::game) fn local_player_removed_by_death(&self) -> bool {
        self.client.screen == Screen::Dead && self.ui.death_time >= 20
    }
    /// The local player's share of every touching entity's
    /// `LivingEntity.pushEntities`, and of the boats' pushes: on a client
    /// `EntitySelector.pushableBy` lets entities push only the local player,
    /// so the server never sends these pushes. `doPush(player)` is
    /// `player.push(entity)`. `before_player`: the mobs and boats that tick
    /// before the player (`ticked_before_player`), else those that tick after
    /// it.
    pub(in crate::game) fn remote_pushes(&self, before_player: bool) -> Vec3 {
        let (Some(p), Some(r)) = (self.player.as_ref(), self.remote.as_ref()) else {
            return Vec3::ZERO;
        };
        // LivingEntity.isPushable: alive, no spectator, off ladders.
        let feet = [
            p.position.x.floor() as i32,
            p.position.y.floor() as i32,
            p.position.z.floor() as i32,
        ];
        if self.client.screen == Screen::Dead
            || p.spectator
            || self.named_block_in_tag("minecraft:climbable", &self.block_name(feet))
        {
            return Vec3::ZERO;
        }
        let bounds = p.bounds();
        let teams = self.mob_teams();
        // `Entity.getTeam` is `Scoreboard.getPlayersTeam(getScoreboardName())`:
        // a player's score holder is its profile name, every other entity's
        // its UUID text. `mob_teams` keys by UUID and names the local player
        // `PLAYER_NAME`, which misses a server's teams for players.
        let board = &self.score.board;
        let player_team = board.players_team(&r.profile_name);
        let mut push = Vec3::ZERO;
        let order = |id: u32| r.ticked_before_player.contains(&id) == before_player;
        for m in self.mobs.iter().filter(|m| order(m.id)) {
            // Bats never push, nor passengers of one vehicle, nor a spectator; the
            // box test comes before the lookups that far mobs never need (#14053).
            if !m.alive()
                || m.species() == sunrise_client::mobs::Species::Bat
                || self
                    .riding
                    .is_some_and(|v| v == m.id || m.extra.mount == Some(v))
                || !m.bounds().intersects(bounds)
            {
                continue;
            }
            let listed = r
                .level_players
                .get(&(m.id as i32))
                .and_then(|uuid| r.tab_players.get(uuid));
            let mob_team =
                listed.map_or_else(|| teams.of(Who::Mob(m.id)), |t| board.players_team(&t.name));
            if r.remote_player_info_game_mode(m.id as i32) == Some(SPECTATOR)
                || !sunrise_client::scoreboard::pushable_by(mob_team, player_team, true)
            {
                continue;
            }
            // Entity.push(Entity), `this` the player.
            let (mut xa, mut za) = (m.position.x - p.position.x, m.position.z - p.position.z);
            let mut dd = xa.abs().max(za.abs());
            if dd < f64::from(0.01_f32) {
                continue;
            }
            dd = dd.sqrt();
            xa /= dd;
            za /= dd;
            let pow = (1.0 / dd).min(1.0);
            xa *= pow;
            za *= pow;
            xa *= f64::from(0.05_f32);
            za *= f64::from(0.05_f32);
            push = Vec3::new(push.x - xa, push.y, push.z - za);
        }
        // AbstractBoat.push(player) from the boat's own tick (its box grown
        // by 0.2 sideways) and from the player's pushEntities (its box).
        let area = Vec3::new(f64::from(0.2_f32), f64::from(-0.01_f32), f64::from(0.2_f32));
        // A boat pushes from its own tick, in the same order as the mobs.
        for b in self
            .boats
            .iter()
            .filter(|b| order(b.id) && self.riding != Some(b.id))
        {
            let bb = b.bounds();
            let times = usize::from(
                bounds.intersects(bb.inflate(area))
                    && sunrise_client::scoreboard::pushable_by(None, player_team, true),
            ) + usize::from(
                bounds.intersects(bb)
                    && sunrise_client::scoreboard::pushable_by(player_team, None, true),
            );
            if times > 0
                && crate::game::boat_passengers::boat_pushes(bb, bounds, false)
                && let Some(v) = crate::game::boat_passengers::push_impulse(b.position, p.position)
            {
                push = push.add_vector(v.scale(times as f64));
            }
        }
        push
    }
    /// ClientPacketListener.handleMovePlayer.
    pub(in crate::game::client_side) fn remote_teleport(
        &mut self,
        p: &play::PlayerPosition,
    ) -> Result<()> {
        use play::relative as rel;
        let (had_player, current) = self
            .player
            .as_ref()
            .map(|pl| (true, pl.position))
            .unwrap_or((false, Vec3::ZERO));
        let respawn_velocity = self.remote.as_mut().and_then(|r| r.respawn_velocity.take());
        let velocity = self
            .player
            .as_ref()
            .map(|pl| pl.velocity)
            .or(respawn_velocity)
            .unwrap_or(Vec3::ZERO);
        let pick = |bit: i32, base: f64, v: f64| if p.relatives & bit != 0 { base + v } else { v };
        let position = Vec3::new(
            pick(rel::X, current.x, p.position[0]),
            pick(rel::Y, current.y, p.position[1]),
            pick(rel::Z, current.z, p.position[2]),
        );
        let yaw = if p.relatives & rel::Y_ROT != 0 {
            self.yaw + p.yaw
        } else {
            p.yaw
        };
        let relative_pitch = p.relatives & rel::X_ROT != 0;
        let pitch = position_packet_pitch(self.pitch, p.pitch, relative_pitch);
        let delta = sunrise_client::player_sync::teleport_velocity(
            velocity,
            (self.yaw, self.pitch),
            (yaw, pitch),
            p,
        );
        // BlockStatePredictionHandler.onTeleport: a prior prediction keeps its block, not its remembered player position.
        if let Some(r) = self.remote.as_mut() {
            r.last_teleport_sequence = r.sequence;
        }
        // The server's answer to a passage crossed (`cross_passage`), or any other.
        self.passages.crossing = None;
        // handleMovePlayer: a passenger keeps its own position and look (its vehicle
        // carries it), so a vehicle a datapack teleports every tick doesn't snap the
        // rider's view; the teleport is still accepted, with where the player is.
        if self.riding.is_some()
            && let Some(player) = self.player.as_ref()
        {
            let at = player.position;
            self.send(S::AcceptTeleportation(play::AcceptTeleportation {
                id: p.teleport,
                x: at.x,
                y: at.y,
                z: at.z,
                yaw: self.yaw,
                pitch: self.pitch,
            }));
            self.stop_remote_mining();
            self.mining = None;
            return Ok(());
        }
        match self.player.as_mut() {
            Some(player) => {
                player.position = position;
                player.velocity = delta;
            }
            None => {
                let mut player = PlayerMotion::new(self.motion, position)?;
                // The id the server gave, for the hooks the motion fires.
                player.entity_id = self.own_entity_id();
                // Entity.noPhysics starts false until Player.tick() resolves PlayerInfo;
                // LocalPlayer.aiStep reads MultiPlayerGameMode.isSpectator().
                player.local_spectator = self.spectator();
                player.velocity = delta;
                self.take_carried_attributes(&mut player)?;
                self.hook.discard();
                self.player_state.attachments.retire();
                self.player = Some(player);
                self.bind_local_player_residency();
                self.input_player_replaced();
                self.client.sway = Default::default();
                self.eye = 1.62;
                self.client.previous_eye = 1.62;
                self.clock.reset();
                self.apply_remote_health();
                self.apply_remote_abilities();
            }
        }
        entity_section_steps::player_teleport(self, had_player, current, position);
        if !self.replay.playing() {
            self.previous_position = position;
        }
        self.yaw = yaw;
        self.pitch = pitch;
        // setValuesFromPositionPacket: setOldPosAndRot gives xRotO the same
        // change, so a relative pitch keeps the model's ease and an absolute
        // one snaps it.
        if let Some(m) = self.client.player_model.as_mut() {
            m.previous_head_pitch =
                position_packet_pitch(m.previous_head_pitch, p.pitch, relative_pitch);
            m.head_pitch = pitch;
        }
        if let Some(id) = self.remote.as_mut().and_then(|r| r.ghast_dismount.take()) {
            self.place_ghast_under(id, position);
        }
        // handleMovePlayer does not change LocalPlayer.sendPosition's baseline:
        // the next tick must still report the corrected state.
        self.send(S::AcceptTeleportation(play::AcceptTeleportation {
            id: p.teleport,
            x: position.x,
            y: position.y,
            z: position.z,
            yaw,
            pitch,
        }));
        // Then MultiPlayerGameMode.stopDestroyBlock: a dig the teleport (an
        // anticheat's setback, a /tp) interrupts is aborted on the wire, and the
        // attack strength starts again.
        self.stop_remote_mining();
        self.mining = None;
        Ok(())
    }
    /// `MultiPlayerGameMode.handleCreativeModeItemAdd`, shared by each creative slot sender.
    pub(in crate::game) fn send_creative_item_add(&self, slot: i16, stack: Option<ItemStack>) {
        if !self.creative || self.remote.is_none() {
            return;
        }
        let stack = stack.filter(|stack| !stack.is_empty());
        // Vanilla checks the item's required features, and an empty stack's item is air.
        let air;
        let enabled = match &stack {
            Some(stack) => self
                .creative_screen
                .0
                .catalog()
                .item_enabled(stack, self.enabled_feature_ids()),
            None => {
                air = ItemStack::new("minecraft:air", 0);
                self.creative_screen
                    .0
                    .catalog()
                    .item_enabled(&air, self.enabled_feature_ids())
            }
        };
        if !enabled {
            return;
        }
        self.send_play(S::SetCreativeModeSlot(play::SetCreativeModeSlot {
            slot,
            item: self.to_network(&stack),
        }));
    }
}
