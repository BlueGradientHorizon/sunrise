use super::*;
use crate::game::respawn_position::Respawn;

impl Game<'_> {
    pub(in crate::game) fn guest_respawn(&mut self, conn: u32) {
        let Some(guest) = self.lan.as_ref().and_then(|host| host.hub.guest(conn)) else {
            return;
        };
        if !guest.dead || self.death_respawn_pending(conn) {
            return;
        }
        let config = guest
            .respawn_elsewhere
            .as_ref()
            .and_then(RespawnConfig::from_saved);
        let forced = guest.spawn;
        if (forced.is_none() || config.is_some()) && self.respawn_trip(conn, config, None, true) {
            return;
        }
        self.server_trips.retain(|trip| !trip.is(conn));
        let found = forced.map_or_else(
            || self.world_respawn(),
            |[x, y, z]| {
                let [yaw, pitch] = self.level.world_spawn_angle;
                Respawn {
                    dim: Dim::Overworld,
                    at: Vec3::new(x, y, z),
                    yaw,
                    pitch,
                }
            },
        );
        self.finish_guest_respawn(conn, found, None);
    }

    /// TeleportTransition.createDefault: the world spawn's level and facing.
    pub(in crate::game) fn world_respawn(&self) -> Respawn {
        let [x, y, z] = self.world_spawn_position();
        let [yaw, pitch] = self.level.world_spawn_angle;
        Respawn {
            dim: self.world_spawn_level(),
            at: Vec3::new(x, y, z),
            yaw,
            pitch,
        }
    }

    /// PlayerList.respawn: decide with the old player, then install and send exactly
    /// one replacement in the final level before `PlayerRespawnEvent`.
    pub(in crate::game) fn finish_guest_respawn(
        &mut self,
        conn: u32,
        found: Respawn,
        missing: Option<RespawnConfig>,
    ) {
        let Some(guest) = self.lan.as_ref().and_then(|host| host.hub.guest(conn)) else {
            return;
        };
        let left = Dim::from_id(&guest.dimension);
        let back = self.dim;
        let result = self.replace_guest_after_death(conn, left, found, missing);
        if let Err(e) = result {
            log::error!("could not respawn player {conn}: {e}");
        }
        if self.dim != back
            && let Err(e) = self.enter_level(back)
        {
            log::error!("could not reopen {}: {e}", back.id());
        }
    }

    fn replace_guest_after_death(
        &mut self,
        conn: u32,
        left: Dim,
        found: Respawn,
        missing: Option<RespawnConfig>,
    ) -> sunrise_client::Result<()> {
        self.enter_level(left)?;
        // Run with the old player in place so their setters still reach that body.
        let decision = if mod_api::__private::listened(mod_api::HookId::PlayerRespawnPosition) {
            self.as_guest(conn, |game| game.respawn_position(conn, found, missing))
                .flatten()
        } else {
            None
        };
        let to = decision.map_or(found, |d| d.to);
        let keep_point = decision.map_or(missing.is_none(), |d| d.copy);

        // Open the destination before discarding any state of the old player.
        if to.dim != left {
            self.enter_level(to.dim)?;
            self.enter_level(left)?;
        }
        let Some(i) = self.guest_index(conn) else {
            return Ok(());
        };
        let guest = &self.lan.as_ref().unwrap().hub.guests[i];
        let (uuid, entity_id) = (guest.uuid, guest.entity_id);
        let original = self.capture_original_player(conn);
        // PlayerList.respawn removes the old player here. Menu retirement and chunk
        // unwatch stay in the source level before the replacement takes its new level.
        self.guest_drop_carried(i);
        self.guest_close_menus_and_tell(conn);
        self.entity_section_forget(entity_id as u32);
        if let Some(host) = self.lan.as_mut()
            && let Some(i) = host.hub.guests.connection_index(conn)
        {
            host.hub.unwatch_guest_chunks(i);
        }
        if self.dim != to.dim {
            self.enter_level(to.dim)?;
        }
        let Some(i) = self.guest_index(conn) else {
            return Ok(());
        };
        // restoreFrom: keep only attachments marked copy-on-death and PlayerPersisted NBT.
        if let Some(attachments) = self.lan.as_mut().unwrap().hub.guests.attachments_mut(conn)
            && let Ok(copy) = attachments.copy_on_death()
        {
            attachments.replace_contents(copy);
        }
        self.keep_only_player_persisted(conn);
        let Some(host) = self.lan.as_mut() else {
            return Ok(());
        };
        let (cloned, name, spawn, config) = host.hub.guests.update(i, |g| {
            // PlayerList.respawn constructs the replacement in respawnInfo.newLevel()
            // before restoreFrom, PlayerEvent.Clone and copyRespawnPosition run.
            g.dimension = to.dim.id().to_owned();
            // restartClientLoadTimerAfterRespawn.
            g.dead = false;
            g.landing = false;
            g.tp_held = false;
            g.load_timeout = sunrise_client::lan::guest::CLIENT_LOADED_TIMEOUT;
            g.loading = true;
            g.death_time = 0;
            g.food = 20;
            g.saturation = 5.0;
            g.sent_health = None;
            // ServerPlayer.restoreFrom(old, false): the base values come back,
            // and the new player has the max health they give.
            if let Some(m) = g.new_body(false) {
                let m = *m;
                g.logs_kept = Some((m.hurt_log, m.attack_log));
            }
            g.health = g.respawn_health(self.motion);
            let mut xp = Experience {
                level: g.xp_level,
                progress: g.xp_progress,
                total: g.xp_total,
            };
            let keep_inventory = self
                .ui
                .rules
                .bool_at(sunrise_client::gamerules::vanilla::KEEP_INVENTORY)
                || g.game_mode == SPECTATOR;
            // Death normally emptied these slots already. Any later writes to the old
            // body are copied only when ServerPlayer.restoreFrom would copy the inventory.
            if !keep_inventory {
                g.inventory.fill(None);
                g.armor.fill(None);
                g.offhand = None;
                g.equipment = Default::default();
                g.selected = 0;
            }
            xp.respawn(keep_inventory);
            g.effects = Default::default();
            (g.xp_level, g.xp_progress, g.xp_total) = (xp.level, xp.progress, xp.total);
            // PlayerEvent.Clone, at the end of `restoreFrom`: the data is copied,
            // the new player not yet placed.
            (
                g.uuid,
                g.name.clone(),
                g.spawn.take(),
                g.respawn_elsewhere.take(),
            )
        });
        if let Some(play) = host.play.get_mut(&conn) {
            // The fresh ServerPlayer has no broadcast baselines, ItemCooldowns or orb pickup delay.
            play.equipment_heard = None;
            play.inventory_heard = None;
            play.level_up_sound = Default::default();
            play.cooldowns = Default::default();
            play.xp_delay = 0;
        }
        self.reinitialize_server_player(uuid, conn);
        if let Some(original) = original {
            self.with_original_player(original, |game, original| {
                game.native_player_clone(cloned, &name, true, original);
            });
        }
        // PlayerList.respawn calls copyRespawnPosition after restoreFrom (and PlayerEvent.Clone).
        if keep_point {
            let (dim, point, forced) = match config.as_ref() {
                Some((dim, pos, _, _, forced)) => (dim.as_str(), Some(*pos), *forced),
                None => {
                    let point = spawn.map(|at| at.map(|coord| coord.floor() as i32));
                    let dim = if point.is_some() {
                        Dim::Overworld
                    } else {
                        found.dim
                    };
                    (dim.id(), point, point.is_some())
                }
            };
            let mut cancel = false;
            self.native_player_set_spawn(mod_api::__private::PlayerSetSpawnCall {
                player: cloned,
                player_entity_id: entity_id,
                name: &name,
                dimension: dim,
                pos: point,
                forced,
                cancel: &mut cancel,
            });
            if !cancel && let Some(host) = self.lan.as_mut() {
                let _ = host.hub.update_guest(conn, |g| {
                    g.spawn = spawn;
                    g.respawn_elsewhere = config;
                });
            }
        }
        let Some(i) = self.guest_index(conn) else {
            return Ok(());
        };
        let world = self.world_spawn_position();
        let clock_rate = self.network_clock_rate();
        let ctx = TickCtx {
            world: &mut self.world,
            entities: &[],
            next_entity: &mut self.next_entity,
            // PlayerList.respawn / sendLevelInfo send level respawn data,
            // independently of the personal destination passed to Hub::respawn.
            spawn: world,
            spawn_angle: self.level.world_spawn_angle,
            spawn_dimension: crate::game::Dim::of_world_spawn(&self.level).id(),
            sea_level: self.end.locator().map_or(63, |terrain| terrain.sea_level()),
            difficulty: i32::from(self.ui.difficulty),
            difficulty_locked: self.level.difficulty_locked,
            clock_rate,
            tick_rate: self.clock.rate(),
            tick_frozen: self.clock.frozen(),
            tick_steps: self.clock.step_state(),
            collision: Some(self.collision),
            border: &self.border,
            streaming: Some(&self.streaming),
        };
        if let Some(host) = self.lan.as_mut() {
            if missing.is_some() {
                host.hub.missing_respawn_block(conn);
            }
            host.hub.respawn(
                i,
                [to.at.x, to.at.y, to.at.z],
                Some([to.yaw, to.pitch]),
                &ctx,
            );
        }
        self.bind_player_attachments(conn, false);
        // ServerGamePacketListenerImpl.handleClientCommand: a hardcore world's
        // respawned player only watches.
        if self.level.hardcore
            && let Some(guest) = self.lan.as_ref().and_then(|host| host.hub.guest(conn))
        {
            let from = crate::game::game_mode::mode_from_id(guest.game_mode);
            let (mut cancel, mut to) = (false, GameMode::Spectator);
            if from != GameMode::Spectator {
                self.native_player_game_mode_changed(cloned, &name, from, &mut to, &mut cancel);
                // A redirect to the mode the player has is no change: nothing is sent.
                cancel |= to == from;
            }
            if !cancel && let Some(host) = self.lan.as_mut() {
                host.hub
                    .set_game_mode(conn, to.id(), &self.world, Some(self.collision));
            }
        }
        self.as_guest(conn, |game| game.native_player_respawned(cloned, &name));
        self.tell_mods_changed_dimension(conn, Some(left), to.dim);
        Ok(())
    }

    /// `ServerPlayer.die` for a guest whose health has run out: the death
    /// message, the loot and experience, the death screen.
    pub(in crate::game) fn guest_die(&mut self, gi: usize, conn: u32) {
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        let dying = host.hub.guests[gi].health <= 0.0 && !host.hub.guests[gi].dead;
        // The killing blow, for the death hooks.
        let blow = (dying && !mod_api::__private::mods().is_empty())
            .then(|| self.guest_damage_source(conn));
        if dying && self.guest_death_canceled(gi, conn) {
            return;
        }
        // A dead player carries no mob a mod seated on it.
        if dying && let Some(carrier) = self.lan.as_ref().map(|h| h.hub.guests[gi].entity_id as u32)
        {
            self.eject_player_riders(Some(carrier));
        }
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        if host.hub.guests[gi].health <= 0.0 && !host.hub.guests[gi].dead {
            self.guest_forget_digging(conn);
            // FishingHook.shouldStopFishing: its owner is dead.
            if let Some(play) = self.lan.as_mut().and_then(|host| host.play.get_mut(&conn)) {
                play.hook.discard();
            }
        }
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        let here = host.hub.here(&host.hub.guests[gi]);
        let g = &host.hub.guests[gi];
        if g.health <= 0.0 && !g.dead {
            host.hub.guests.update(gi, |g| {
                g.dead = true;
                g.last_death = Some((g.dimension.clone(), g.position.map(|v| v.floor() as i32)));
            });
            let g = &host.hub.guests[gi];
            let (id, at) = (g.entity_id, g.position);
            let reward = Experience {
                level: g.xp_level,
                progress: g.xp_progress,
                total: g.xp_total,
            }
            .death_reward(
                self.ui
                    .rules
                    .bool_at(sunrise_client::gamerules::vanilla::KEEP_INVENTORY)
                    || g.game_mode == SPECTATOR,
            );
            let (name, uuid) = (g.name.clone(), g.uuid);
            // ServerPlayer.die: dropAllDeathLoot unless a spectator, from the eyes less 0.3.
            let drops = (g.game_mode != SPECTATOR)
                .then(|| [at[0], at[1] + g.eye_height() - f64::from(0.3f32), at[2]]);
            let cause = host.play.get_mut(&conn).and_then(|p| p.death_cause.take());
            // ServerPlayer.die opens with `gameEvent(ENTITY_DIE)` at the player:
            // sculk sensors and wardens hear the death.
            if here {
                self.as_guest(conn, |game| {
                    let ctx = game.player_context(None);
                    game.post_game_event("minecraft:entity_die", at, ctx);
                });
            }
            // `getKillCredit`, as the death finds it.
            let killer = self.guest_kill_credit_type(conn);
            self.native_player_died(uuid, cause.as_deref());
            // ServerPlayer.die: the neutral mobs near a guest in this level
            // forget their anger at them.
            if here {
                self.tell_neutral_mobs_player_died(at, Some(conn));
            }
            // The death's last blow, read before the death message empties the
            // combat record: ServerPlayer.die's triggers read it from a record
            // that CombatTracker.recheckStatus clears only at the end of die.
            let killing = self.guest_combat.get(&conn).and_then(|c| c.killing_blow());
            // ServerPlayer.die: with showDeathMessages, the message goes to
            // the dying player's death screen and to everyone the team lets
            // see it; without it, the death screen gets an empty one.
            let message = if self
                .ui
                .rules
                .bool_at(sunrise_client::gamerules::vanilla::SHOW_DEATH_MESSAGES)
            {
                let me = self.player_text(&name, uuid);
                let text = match cause {
                    Some(key) => sunrise_client::text::Text::translate(&key, &[me]),
                    None if at[1] < self.world.below_world_y() => {
                        sunrise_client::text::Text::translate("death.attack.outOfWorld", &[me])
                    }
                    None => self.guest_death_message(conn, me),
                };
                let message = sunrise_client::lan::data::component_nbt(&text.component());
                let board = &self.score.board;
                let Some(host) = self.lan.as_ref() else {
                    return;
                };
                host.hub.system_message_to(message.clone(), |other| {
                    sunrise_client::scoreboard::death_message_reaches(board, &name, other)
                });
                if self.is_dedicated_server() {
                    crate::game::dedicated::events::record(
                        "death",
                        &name,
                        uuid,
                        serde_json::json!({ "message": text.plain() }),
                    );
                    // Only a message for everyone is logged (`broadcastSystemMessage`).
                    if board.players_team(&name).is_none_or(|t| {
                        t.death_message_visibility == sunrise_client::scoreboard::Visibility::Always
                    }) {
                        crate::game::dedicated::log(
                            "INFO",
                            &format!("System chat: {}", text.plain()),
                        );
                    }
                } else if self.player.is_some()
                    && sunrise_client::scoreboard::death_message_reaches(
                        board,
                        &name,
                        crate::game::chat::PLAYER_NAME,
                    )
                {
                    self.system_message(text);
                }
                message
            } else {
                literal("")
            };
            let Some(host) = self.lan.as_ref() else {
                return;
            };
            host.hub.send_to(
                conn,
                &C::PlayerCombatKill(play::PlayerCombatKill { id, message }),
            );
            // LivingEntity.die: the death animation, the dying player included.
            host.hub.entity_event(id, 3);
            let recently_hit = killing.is_some()
                && self
                    .guest_combat
                    .get(&conn)
                    .is_some_and(|c| c.recently_hit_by_player(self.world.game_time));
            let dropping = crate::game::creatures::DyingDrops {
                entity: id as u32,
                kind: "minecraft:player",
                player: Some(uuid),
                source: killing.as_ref().map(|(damage_type, attacker)| {
                    crate::game::creatures::death_hurt_source(damage_type, attacker.unwrap_or(0))
                }),
                recently_hit,
            };
            // LivingEntity.die: the death is heard, and a sculk catalyst
            // nearby takes the experience (before the loot is dropped), as it
            // does for a mob.
            let mut catalyst_xp = 0;
            let consumed = self.catalyst_takes_experience(at, |_| {
                catalyst_xp = reward;
                catalyst_xp
            });
            // CatalystListener: with experience to take, the player who last
            // hurt them gets KILL_MOB_NEAR_SCULK_CATALYST.
            if consumed && catalyst_xp > 0 {
                self.guest_catalyst_trigger(gi, conn, at);
            }
            // ServerPlayer.die: the drops, the deathCount objectives, the
            // experience (`dropAllDeathLoot`), then a kill credit's `killed_by`,
            // awardKillScore and wither rose, then `deaths` and the two timers.
            self.as_guest(conn, |game| {
                if let Some(eyes) = drops {
                    game.drop_inventory(eyes, &dropping);
                }
                game.score_player_died();
                if let Some(kind) = &killer {
                    game.award_stat("killed_by", kind, 1);
                }
            });
            // ServerPlayer.die: a spectator never reaches dropAllDeathLoot. The
            // experience the catalyst took does not drop (`Player.skipDropExperience`),
            // and `dropAllDeathLoot` drops it before the rose.
            if drops.is_some() {
                // LivingEntity.dropExperience: Player.isAlwaysExperienceDropper.
                let now = self.world.game_time;
                let attacker = if mod_api::__private::mods().is_empty() {
                    None
                } else {
                    self.guest_combat
                        .get_mut(&conn)
                        .and_then(|c| c.take_hurt_by_player(now))
                        .and_then(|c| self.resolve_player_credit(c).map(|_| c.0))
                };
                let reward = self.living_experience_drop(
                    id as u32,
                    "minecraft:player",
                    attacker,
                    reward as i32,
                );
                if !consumed {
                    self.spawn_orbs(at, reward as u32);
                }
            }
            self.guest_killed_player_trigger(conn, at, killing);
            self.guest_award_kill_score(conn, at);
            self.as_guest(conn, |game| {
                game.award_custom_stat("deaths", 1);
                game.stats.reset_death_timers();
            });
            // `ServerPlayer.die`'s `broadcastEntityEvent`, after the message,
            // the loot and the experience: Fabric's AFTER_DEATH. The blow is
            // the one `living_death` heard, so a death it cancelled returns
            // above and is never told here.
            if let Some((damage_type, attacker)) = &blow
                && mod_api::__private::listened(mod_api::HookId::LivingDeathPost)
            {
                self.native_living_death_post(id as u32, damage_type, *attacker);
            }
        }
    }

    /// `SculkCatalystBlockEntity.CatalystListener`'s trigger on a player's
    /// death: for the player the dying guest's combat record credits
    /// (`LivingEntity.getKillCredit`), with the death's `killing_blow`.
    pub(super) fn guest_catalyst_trigger(&mut self, gi: usize, conn: u32, at: [f64; 3]) {
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        let now = self.world.game_time;
        let combat = self.guest_combat.get(&conn);
        let player_credit = combat.and_then(|c| c.player_credit(now));
        let who = if let Some(credit) = player_credit {
            self.resolve_player_credit(credit)
        } else {
            combat.and_then(|c| c.kill_credit(now)).and_then(|id| {
                if self.lan_acting.is_none()
                    && self
                        .player
                        .as_ref()
                        .is_some_and(|p| p.entity_id as u32 == id)
                {
                    Some(None)
                } else {
                    host.hub
                        .guests
                        .iter()
                        .find(|g| g.entity_id as u32 == id)
                        .map(|g| Some(g.conn))
                }
            })
        };
        let Some(who) = who else { return };
        let (damage, attacker) = combat
            .and_then(|c| c.killing_blow())
            .unwrap_or_else(|| ("minecraft:player_attack".into(), None));
        let g = &host.hub.guests[gi];
        let view = sunrise_client::advancements::EntityView {
            kind: "minecraft:player".into(),
            position: at,
            sneaking: g.sneaking,
            input_sneak: g.input & 32 != 0,
            gamemode: Some(
                match g.game_mode {
                    1 => "creative",
                    2 => "adventure",
                    3 => "spectator",
                    _ => "survival",
                }
                .into(),
            ),
            ..Default::default()
        };
        // The damage's causing entity: the player in place, another guest, or
        // a mob (`DamageSource.getEntity`).
        let source = attacker.and_then(|id| {
            if self.lan_acting.is_none()
                && self
                    .player
                    .as_ref()
                    .is_some_and(|p| p.entity_id as u32 == id)
            {
                Some(self.player_entity_for_any(&["minecraft:kill_mob_near_sculk_catalyst"]))
            } else {
                host.hub
                    .guests
                    .iter()
                    .find(|g| g.entity_id as u32 == id)
                    .map(|g| sunrise_client::advancements::EntityView {
                        kind: "minecraft:player".into(),
                        position: g.position,
                        sneaking: g.sneaking,
                        input_sneak: g.input & 32 != 0,
                        ..Default::default()
                    })
                    .or_else(|| {
                        self.mobs
                            .iter()
                            .find(|m| m.id == id)
                            .map(|m| self.mob_entity(m))
                    })
            }
        });
        let projectile = match damage.as_str() {
            "minecraft:arrow" => Some("minecraft:arrow"),
            "minecraft:fireball" | "minecraft:small_fireball" => Some("minecraft:fireball"),
            "minecraft:wither_skull" => Some("minecraft:wither_skull"),
            "minecraft:fireworks" => Some("minecraft:firework_rocket"),
            _ => None,
        };
        let blow = sunrise_client::advancements::DamageView {
            kind: damage,
            // DamageSource.isDirect: the causing entity is the direct one.
            is_direct: projectile.is_none(),
            direct: match projectile {
                Some(kind) => Some(sunrise_client::advancements::EntityView {
                    kind: kind.into(),
                    position: at,
                    ..Default::default()
                }),
                None => source.clone(),
            },
            source,
            ..Default::default()
        };
        // CatalystListener: trigger(player, killedEntity, damageSource).
        match who {
            None => {
                self.trigger_killed_as("minecraft:kill_mob_near_sculk_catalyst", &view, Some(&blow))
            }
            Some(kconn) => {
                self.as_guest(kconn, |g| {
                    g.trigger_killed_as(
                        "minecraft:kill_mob_near_sculk_catalyst",
                        &view,
                        Some(&blow),
                    );
                });
            }
        }
    }

    /// `ServerPlayer.die` → `Entity.awardKillScore` on the entity the dying
    /// guest's combat record credits (`LivingEntity.getKillCredit`): the
    /// guest's `entity_killed_player`, with that entity and the death's
    /// `killing_blow`. Nothing credited, nothing fires.
    pub(super) fn guest_killed_player_trigger(
        &mut self,
        conn: u32,
        at: [f64; 3],
        blow: Option<(String, Option<u32>)>,
    ) {
        const TRIGGER: &str = "minecraft:entity_killed_player";
        let now = self.world.game_time;
        let Some(id) = self
            .guest_combat
            .get(&conn)
            .and_then(|c| c.kill_credit(now))
        else {
            return;
        };
        let Some(killer) = self.guest_entity_view(id, TRIGGER) else {
            return;
        };
        let blow = self.guest_killing_blow(blow, at, TRIGGER);
        self.as_guest(conn, |g| {
            g.trigger_killed_as(TRIGGER, &killer, blow.as_ref());
        });
    }

    /// The type of the entity a dying guest's combat record credits with the
    /// kill (`LivingEntity.getKillCredit`): a player (the one in place or a
    /// guest), a mob (one gone already credits nobody, as vanilla's reference
    /// no longer resolves), a mob whose own blast this tick killed the guest,
    /// or the ender dragon.
    fn guest_kill_credit_type(&self, conn: u32) -> Option<String> {
        let id = self
            .guest_combat
            .get(&conn)?
            .kill_credit(self.world.game_time)?;
        let in_place = self.lan_acting.is_none()
            && self
                .player
                .as_ref()
                .is_some_and(|p| p.entity_id as u32 == id);
        if in_place
            || self
                .lan
                .as_ref()
                .is_some_and(|host| host.hub.guests.iter().any(|g| g.entity_id as u32 == id))
        {
            return Some("minecraft:player".into());
        }
        if let Some(m) = self.mobs.iter().find(|m| m.id == id) {
            return Some(m.kind.id.clone());
        }
        // A mob that blew up this tick has left the list, but vanilla's player
        // dies inside that blast, before the source is discarded.
        if let Some((_, kind)) = self.combat.exploded_kinds.iter().rfind(|(m, _)| *m == id) {
            return Some(kind.clone());
        }
        self.dragon
            .as_ref()
            .filter(|d| d.id == id)
            .map(|_| "minecraft:ender_dragon".into())
    }

    /// The dying guest's last blow (`LivingEntity.getLastDamageSource`), its
    /// damage type and causing entity as the combat record held them, as a
    /// damage source: the projectile as the direct entity.
    pub(super) fn guest_killing_blow(
        &self,
        blow: Option<(String, Option<u32>)>,
        at: [f64; 3],
        trigger: &str,
    ) -> Option<sunrise_client::advancements::DamageView> {
        let (damage, attacker) = blow?;
        // The damage's causing entity: the player in place, another guest, or
        // a mob (`DamageSource.getEntity`).
        let source = attacker.and_then(|id| self.guest_entity_view(id, trigger));
        Some(sunrise_client::advancements::recorded_blow(
            damage, source, at,
        ))
    }

    /// The entity behind a combat record's id, as `trigger`'s predicates see
    /// it: the player in place, a guest, or a mob.
    pub(super) fn guest_entity_view(
        &self,
        id: u32,
        trigger: &str,
    ) -> Option<sunrise_client::advancements::EntityView> {
        if self.lan_acting.is_none()
            && self
                .player
                .as_ref()
                .is_some_and(|p| p.entity_id as u32 == id)
        {
            // `player_entity_for` takes `&mut self` (the compound for `nbt`);
            // this view of the killer has the player's records without it.
            return Some(self.player_entity_for_any(&[trigger]));
        }
        self.lan
            .as_ref()?
            .hub
            .guests
            .iter()
            .find(|g| g.entity_id as u32 == id)
            .map(|g| sunrise_client::advancements::EntityView {
                kind: "minecraft:player".into(),
                position: g.position,
                sneaking: g.sneaking,
                input_sneak: g.input & 32 != 0,
                ..Default::default()
            })
            .or_else(|| {
                self.mobs
                    .iter()
                    .find(|m| m.id == id)
                    .map(|m| self.mob_entity(m))
            })
    }

    /// The scalar death-tail/death-message source. `living_death` uses the
    /// full accepted hit retained on the body instead.
    pub(super) fn guest_damage_source(&self, conn: u32) -> (String, u32) {
        let (damage_type, attacker) = match self
            .lan
            .as_ref()
            .and_then(|host| host.play.get(&conn))
            .and_then(|p| p.death_cause.as_deref())
        {
            Some("death.attack.genericKill") => ("minecraft:generic_kill", 0),
            _ => self
                .guest_combat
                .get(&conn)
                .and_then(|tracker| tracker.last_blow())
                .unwrap_or(("minecraft:generic", 0)),
        };
        (damage_type.to_owned(), attacker)
    }

    /// `ServerPlayer.die`'s message from a guest's own `CombatTracker`, which
    /// sits in `guest_combat` between turns.
    pub(super) fn guest_death_message(
        &mut self,
        conn: u32,
        me: sunrise_client::text::Text,
    ) -> sunrise_client::text::Text {
        let mut tracker = self.guest_combat.remove(&conn).unwrap_or_default();
        std::mem::swap(&mut self.combat, &mut tracker);
        let text = self.combat_death_message_for(me);
        std::mem::swap(&mut self.combat, &mut tracker);
        self.guest_combat.insert(conn, tracker);
        text
    }
}
