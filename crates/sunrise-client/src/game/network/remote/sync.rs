#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use sunrise_client::tab_list::{TabPlayer, default_skin};

impl Remote {
    /// `ClientPacketListener.getPlayerInfo(String)`: exact profile name, whether listed or not.
    pub(in crate::game) fn player_info_by_name(&self, name: &str) -> Option<&TabPlayer> {
        self.tab_players.values().find(|p| p.name == name)
    }

    /// `PlayerInfo.getSkin`: resolved skin or the profile's default, independent of listing.
    pub(in crate::game) fn player_info_skin(&self, uuid: u128) -> &str {
        self.skins
            .get(&uuid)
            .map(|skin| skin.body(uuid).0)
            .unwrap_or_else(|| default_skin(uuid))
    }
}

#[cfg(test)]
#[path = "team_packet_tests.rs"]
pub(in crate::game) mod team_packet_tests;

impl Game<'_> {
    /// A `Holder<SoundEvent>` off the wire: an inline event's location, or
    /// the name of a registered one.
    pub(in crate::game) fn remote_sound_name(
        &self,
        sound: &(i32, Option<(String, Option<f32>)>),
    ) -> Option<String> {
        match &sound.1 {
            Some((location, _)) => Some(location.clone()),
            None => self.remote.as_ref().and_then(|r| {
                r.builtin
                    .get("minecraft:sound_event")?
                    .name(sound.0)
                    .map(str::to_owned)
            }),
        }
    }
    pub(in crate::game) fn remote_sound_entity_state(
        &mut self,
        id: i32,
    ) -> Option<sunrise_client::sound_engine::SoundEntityState> {
        let r = self.remote.as_ref()?;
        let player = self.player.as_ref().map(|p| {
            (
                r.entity_id,
                [p.position.x, p.position.y, p.position.z],
                r.local_entity_silent,
            )
        });
        // The local player is not in mobs, so avoid a guaranteed index miss.
        if player.is_some_and(|p| p.0 == id) {
            return entity_sound_state(id, player, None, &r.entities, None);
        }
        let mob = self.mob_index.get(&self.mobs, id as u32).map(|m| {
            sunrise_client::sound_engine::SoundEntityState {
                position: [m.position.x, m.position.y, m.position.z],
                silent: m.silent,
                removed: m.removed,
            }
        });
        let dragon = self
            .dragon
            .as_ref()
            .map(|d| (d.id as i32, [d.position.x, d.position.y, d.position.z]));
        let mut state = entity_sound_state(id, player, mob, &r.entities, dragon)?;
        // Mob states already use the live position. In particular,
        // level_entity would find the dragon's stale Mob before its Dragon.
        if mob.is_some() || dragon.is_some_and(|d| d.0 == id) {
            return Some(state);
        }
        // Vehicles, projectiles and other simulated objects may have moved
        // since the last tracking packet.
        if let Some(position) = self
            .level_entity(id)
            .and_then(|target| self.target_position(target))
        {
            state.position = position;
        }
        Some(state)
    }
    /// The server's network id of a mob effect (`brewing::EFFECTS` id).
    pub(in crate::game) fn remote_effect_network_id(&self, effect: &str) -> Option<i32> {
        let r = self.remote.as_ref()?;
        r.builtin
            .get("minecraft:mob_effect")?
            .id(&format!("minecraft:{effect}"))
    }

    /// A mob effect's network id as the effect (`brewing::EFFECTS` id).
    pub(in crate::game) fn remote_effect_id(&self, effect: i32) -> Option<&'static str> {
        self.remote_effect_def(effect).map(|def| def.id)
    }
    pub(in crate::game) fn remote_effect_def(
        &self,
        effect: i32,
    ) -> Option<&'static sunrise_client::brewing::EffectDef> {
        let r = self.remote.as_ref()?;
        let name = r.builtin.get("minecraft:mob_effect")?.name(effect)?;
        sunrise_client::brewing::effect(name)
    }

    // remote_effect: remote/effect.rs

    /// `handleAddObjective`: add, remove or change an objective.
    pub(in crate::game) fn remote_objective(&mut self, p: play::SetObjective) {
        use sunrise_client::scoreboard::{RenderType, tag_to_json};
        let board = &mut self.score.board;
        match (p.method, p.display) {
            (1, _) => board.remove_objective(&p.name),
            (method, Some((name, render, format))) => {
                let render = if render == 1 {
                    RenderType::Hearts
                } else {
                    RenderType::Integer
                };
                let format = format.as_ref().map(number_format);
                if method == 0 {
                    board.add_objective(
                        &p.name,
                        "dummy",
                        tag_to_json(&name),
                        render,
                        false,
                        format,
                    );
                } else if let Some(o) = board.objective_mut(&p.name) {
                    o.display_name = tag_to_json(&name);
                    o.render_type = render;
                    o.number_format = format;
                }
            }
            _ => {}
        }
    }

    /// `handleSetPlayerTeamPacket`.
    pub(in crate::game) fn remote_team(&mut self, p: play::SetPlayerTeam) {
        use sunrise_client::scoreboard::{CollisionRule, Visibility, tag_to_json};
        let board = &mut self.score.board;
        let team = if p.method == 0 {
            board.add_team(&p.name)
        } else if let Some(team) = board.team_mut(&p.name) {
            team
        } else {
            log::warn!(
                "Received team packet with method {} for unknown team {:?}",
                p.method,
                p.name
            );
            return;
        };
        if p.method == 1 {
            board.remove_team(&p.name);
            return;
        }
        if let Some(params) = &p.parameters {
            team.display_name = tag_to_json(&params.display_name);
            team.prefix = tag_to_json(&params.prefix);
            team.suffix = tag_to_json(&params.suffix);
            team.color = params
                .color
                .and_then(|c| sunrise_client::text::COLORS.get(c as usize))
                .map(|c| c.1.to_owned());
            team.allow_friendly_fire = params.options & 1 != 0;
            team.see_friendly_invisibles = params.options & 2 != 0;
            team.nametag_visibility = Visibility::ALL
                .get(params.name_tag_visibility as usize)
                .copied()
                .unwrap_or(Visibility::Always);
            team.collision_rule = CollisionRule::ALL
                .get(params.collision_rule as usize)
                .copied()
                .unwrap_or(CollisionRule::Always);
        }
        match p.method {
            0 | 3 => {
                for player in &p.players {
                    board.add_to_team(player, &p.name);
                }
            }
            // Scoreboard.removePlayerFromTeam: only out of this team (vanilla
            // throws for a player in another one).
            4 => {
                for player in &p.players {
                    if board.players_team(player).is_some_and(|t| t.name == p.name) {
                        board.remove_from_team(player);
                    }
                }
            }
            _ => {}
        }
    }
    pub(in crate::game) fn remote_tab_players(
        &mut self,
    ) -> Option<(Vec<TabPlayer>, bool, Vec<u128>)> {
        self.update_remote_skins();
        self.publish_tab_object_skins();
        let r = self.remote.as_ref()?;
        let players: Vec<_> = r
            .listed
            .iter()
            .filter_map(|u| {
                let mut player = r.tab_players.get(u)?.clone();
                player.skin = r.player_info_skin(*u).to_owned();
                Some(player)
            })
            .collect();
        let upside_down = players
            .iter()
            .filter(|p| {
                (p.name == "Dinnerbone" || p.name == "Grumm")
                    && (p.uuid == r.profile || r.level_players.values().any(|u| *u == p.uuid))
            })
            .map(|p| p.uuid)
            .collect();
        Some((players, r.online_mode, upside_down))
    }

    /// The end of the player's tick: `AbstractBoat.tick`'s paddles (the vehicle ticks before its
    /// passenger's `sendChanges`), then `LocalPlayer.sendChanges`: the input, and a passenger's look,
    /// vehicle and sprint, or `sendPosition`'s sprint and move. The held slot is not sent here:
    /// `MultiPlayerGameMode.tick` sends it at the start of the next tick (`remote_handle_input`).
    pub(in crate::game) fn remote_send_state(&mut self, input: u8) {
        // Minecraft.tick's sendChanges also skips the tick that removed the player.
        if self.local_player_removed_by_death() {
            return;
        }
        let Some(p) = self.player.as_ref() else {
            return;
        };
        let (pos, on_ground, collision, sprinting) = (
            [p.position.x, p.position.y, p.position.z],
            p.ground.on_ground,
            p.collision.horizontal,
            p.sprinting,
        );
        let (yaw, pitch) = (self.yaw, self.pitch);
        let riding = self.riding.is_some();
        let controls_camera = self.controls_camera();
        let (paddles, vehicle): (Vec<S>, Vec<S>) = self
            .remote_vehicle_packets()
            .into_iter()
            .partition(|p| matches!(p, S::PaddleBoat(_)));
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        let mut out = paddles;
        if input != r.last_input {
            r.last_input = input;
            out.push(S::PlayerInput(play::PlayerInput { flags: input }));
        }
        // LocalPlayer.sendIsSprintingIfNeeded.
        let sprint = |r: &mut Remote, out: &mut Vec<S>| {
            if sprinting != r.was_sprinting {
                r.was_sprinting = sprinting;
                out.push(S::PlayerCommand(play::PlayerCommand {
                    id: r.entity_id,
                    action: if sprinting { 1 } else { 2 },
                    data: 0,
                }));
            }
        };
        let mut moved_or_rotated = false;
        if riding {
            // LocalPlayer.sendChanges: a passenger sends its look, then the
            // vehicle it controls and, only then, its sprint.
            out.push(S::MovePlayerRot(play::MovePlayerRot {
                yaw,
                pitch,
                flags: u8::from(on_ground) | u8::from(collision) << 1,
            }));
            if !vehicle.is_empty() {
                out.extend(vehicle);
                sprint(r, &mut out);
            }
            moved_or_rotated = true;
        } else {
            sprint(r, &mut out);
            // LocalPlayer.sendPosition: no move while the view is another
            // entity's (the server keeps the player on it).
            if controls_camera
                && let Some(packet) =
                    r.position_sender
                        .update(pos, (yaw, pitch), on_ground, collision)
            {
                // A server that sent colliders reads how old the copy carrying the player is.
                if !r.colliders.is_empty()
                    && let Some(hint) = r.carry_age.hint()
                {
                    out.push(S::CustomPayload(
                        sunrise_protocol::packets::configuration::CustomPayload {
                            channel: sunrise_protocol::carry_age::CHANNEL.into(),
                            data: sunrise_protocol::carry_age::encode(hint),
                        },
                    ));
                }
                out.push(packet);
                moved_or_rotated = true;
            }
        }
        if moved_or_rotated && r.session.is_recording() {
            let pkt = sunrise_protocol::packets::play::Clientbound::PlayerPosition(
                sunrise_protocol::packets::play::PlayerPosition {
                    teleport: 0,
                    position: pos,
                    delta: [0.0, 0.0, 0.0],
                    yaw,
                    pitch,
                    relatives: 0,
                },
            );
            r.session.record_clientbound(&pkt);
        }
        if r.session.is_recording() {
            let is_using = self.using.is_some();
            let living_flags: u8 = if is_using {
                1 | if self.hand == Hand::Off { 2 } else { 0 }
            } else {
                0
            };
            let crouching = p.local_crouching || matches!(p.pose, crate::pose::Pose::Crouching);
            let mut shared: u8 = 0;
            if crouching {
                shared |= 0x02;
            }
            if sprinting {
                shared |= 0x08;
            }
            if p.swimming {
                shared |= 0x10;
            }
            if p.fall_flying {
                shared |= 0x80;
            }
            if shared != r.was_shared_flags
                || living_flags != r.was_living_flags
                || r.last_recorded_pose != Some(p.pose)
            {
                r.was_shared_flags = shared;
                r.was_living_flags = living_flags;
                r.last_recorded_pose = Some(p.pose);
                use sunrise_protocol::entity_data::{Value, serializer};
                let pkt = sunrise_protocol::packets::play::Clientbound::SetEntityData(
                    sunrise_protocol::packets::play::SetEntityData {
                        id: r.entity_id,
                        values: vec![
                            (0, serializer::BYTE, Value::Byte(shared as i8)),
                            (6, serializer::POSE, Value::Id(p.pose.id())),
                            (8, serializer::BYTE, Value::Byte(living_flags as i8)),
                        ],
                    },
                );
                r.session.record_clientbound(&pkt);
            }
        }
        // Creative inventory edits are sent slot by slot (SetCreativeModeSlot).
        let mut creative_slots = Vec::new();
        if self.creative {
            // A server's open menu takes clicks, which it applies itself: what
            // this client predicts for them isn't an edit to send (it would
            // overwrite the server's stacks whenever the prediction is wrong).
            let in_menu = r.menu.as_ref().is_some();
            let stacks = creative_sync_stacks(&self.inventory, &self.offhand, &self.armor);
            for (i, stack) in stacks.into_iter().enumerate() {
                if stack != r.synced[i] {
                    r.synced[i] = stack.clone();
                    if !in_menu {
                        creative_slots.push((creative_sync_slot(i), stack));
                    }
                }
            }
        }
        for packet in out {
            self.send(packet);
        }
        for (slot, stack) in creative_slots {
            self.send_creative_item_add(slot, stack);
        }
    }
    pub(in crate::game) fn creative_stacks(&self) -> Vec<Option<ItemStack>> {
        creative_sync_stacks(&self.inventory, &self.offhand, &self.armor)
    }

    /// `CreativeInventoryListener.slotChanged`: `handleCreativeModeItemAdd` for every slot the
    /// player's inventory menu changed. `CreativeModeInventoryScreen.slotClicked` ends in
    /// `broadcastChanges`, so a click's slots go out with the click, right behind a thrown
    /// stack, and not at the end of the tick: a slot update the server sends meanwhile (the
    /// pickup of what a held Q threw) can then never land on a change still unsent and undo it.
    pub(in crate::game) fn send_creative_slots(&mut self) {
        if !self.creative {
            return;
        }
        let stacks = self.creative_stacks();
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        let mut changed = Vec::new();
        for (i, stack) in stacks.into_iter().enumerate() {
            if stack != r.synced[i] {
                r.synced[i] = stack.clone();
                changed.push((creative_sync_slot(i), stack));
            }
        }
        for (slot, stack) in changed {
            self.send_creative_item_add(slot, stack);
        }
    }

    /// `MultiPlayerGameMode.ensureHasSentCarriedItem`: the server hears of a
    /// new selected slot before anything the slot acts with.
    pub(in crate::game) fn remote_send_carried(&self) {
        if let Some(r) = &self.remote
            && r.carried.get() != Some(self.selected)
        {
            r.carried.set(Some(self.selected));
            self.send_play(S::SetCarriedItem(play::SetCarriedItem {
                slot: self.selected as i16,
            }));
        }
    }
}

impl Game<'_> {
    pub(in crate::game) fn apply_remote_health(&mut self) {
        let Some((health, food, saturation)) = self.remote.as_ref().map(|r| r.health) else {
            return;
        };
        if let Some(p) = self.player.as_mut() {
            if (p.health.health() - health).abs() > f32::EPSILON {
                let _ = p.set_health(health.max(0.0), false);
            }
            p.health.food.food = food;
            p.health.food.saturation = saturation;
        }
    }
}
