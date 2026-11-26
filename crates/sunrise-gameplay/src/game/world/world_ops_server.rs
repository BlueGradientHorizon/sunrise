//! Server-side implementations of `side_ops::world` and server-only world/save/control helpers.
use super::*;
use serde_json::{Value, json};

impl crate::game::ServerGame<'_> {
    #[inline(always)]
    pub(in crate::game) fn server_game_side_meta(
        &self,
    ) -> (Option<u128>, Option<mod_api::LevelRef>, bool) {
        let level = self.common.native_lifecycle.epoch().and_then(|epoch| {
            let key = self.common.dim.key();
            let id = mod_api::__private::dimension_id(key)?;
            Some(mod_api::LevelRef::with_generation(
                epoch,
                key,
                self.server.levels.0.runtime.generation_of(id),
            ))
        });
        (
            self.server.lan_acting_uuid,
            level,
            self.server.session_lock.0.is_some(),
        )
    }

    pub(in crate::game) fn server_with_level_access<R>(
        &mut self,
        placement_preview: bool,
        f: impl FnOnce(&Behaviours, &mut GameLevel<'_>, &mut Ticks) -> R,
    ) -> R {
        self.with_level_access(placement_preview, f)
    }

    pub(in crate::game) fn server_after_with_level(&mut self) {
        self.flush_spill();
        if self.server.lan.is_some() {
            self.flush_lan_particles();
            self.flush_native_sounds();
        }
    }

    pub(in crate::game) fn server_set_block_live(
        &mut self,
        pos: [i32; 3],
        state: BlockStateId,
        flags: i32,
        create_entity: bool,
    ) -> bool {
        let [x, y, z] = pos;
        let at = BlockPos::new(x, y, z);
        let previous = self.common.world.block(x, y, z);
        let b = self.common.res.behaviours();
        if flags & level_flags::UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS != 0
            || b.block(previous) == b.block(state)
            || b.keeps_block_entity(previous, state)
            || !(self.common.res.facts().get(previous).block_entity
                || self.common.world.block_entities.stateful_entity(previous.0)
                || self.common.world.block_entities.get(&(x, y, z)).is_some())
        {
            return self.common.with_level(|b, level, ticks| {
                let changed = b.set_block(level, ticks, at, state, flags);
                if create_entity {
                    level.finish_native_block(pos, previous, state, changed);
                }
                changed
            });
        }
        self.common.ensure_stateful_entity_table();
        let Some((write, removal, spawns, orbs)) =
            self.with_level_access(false, |b, level, ticks| {
                let write = b.begin_block_write(level, ticks, at, state, flags)?;
                let spawn_start = level.spill.spawns.len();
                let orb_start = level.spill.orbs.len();
                let removal = write
                    .effectful_removal()
                    .map(|old| level.pre_remove_entity(at, old, true));
                let spawns = level.spill.spawns.split_off(spawn_start);
                let orbs = level.spill.orbs.split_off(orb_start);
                Some((write, removal, spawns, orbs))
            })
        else {
            return false;
        };
        self.publish_removal_spawns(spawns, orbs);
        self.common.with_level(|b, level, ticks| {
            let removal_updates =
                removal.map(|removal| level.finish_remove_entity(at, previous, removal));
            let changed = b.finish_block_write(level, ticks, write, removal_updates);
            if create_entity {
                level.finish_native_block(pos, previous, state, changed);
            }
            changed
        })
    }

    pub(in crate::game) fn server_update_chunk_tickets(&mut self) {
        self.rebuild_chunk_tickets(true);
    }

    pub(in crate::game) fn server_section_mesh_pending_in(
        &self,
        _section: (i32, i32, i32),
    ) -> bool {
        false
    }

    pub(in crate::game) fn server_landing_change_cancelled(
        &mut self,
        player: Option<u128>,
        position: BlockPos,
        from: BlockStateId,
        to: BlockStateId,
    ) -> bool {
        if !mod_api::__private::listened(mod_api::HookId::EntityChangeBlockPre) {
            return false;
        }
        let mut cancel = false;
        self.native_entity_change_block_pre(native_hooks::EntityChangeBlockPreCall {
            player,
            entity: None,
            kind: "minecraft:player",
            pos: [position.x, position.y, position.z],
            from,
            to,
            cancel: &mut cancel,
        });
        cancel
    }

    pub(in crate::game) fn server_tick(&mut self) -> Result<()> {
        self.tick_server_first()
    }

    pub(in crate::game) fn server_drop_stack_from_eyes(
        &mut self,
        stack: ItemStack,
        thrown_from_hand: bool,
    ) -> Option<u32> {
        if stack.count == 0 {
            return None;
        }
        self.swing_hand(Hand::Main, SwingAnimation::DEFAULT);
        let eye = self.common.eye_position()?;
        self.throw_from_eyes(stack, eye, false, thrown_from_hand)
    }

    pub(in crate::game) fn server_give_experience_points(&mut self, points: i32) {
        let Some(points) = self.xp_change(points) else {
            return;
        };
        let old_level = self.common.experience.level;
        self.common.experience.add_points(points);
        while self.common.experience.progress < 0.0 {
            let xp = &self.common.experience;
            let lost = xp.progress * sunrise_sim::entities::Experience::needed(xp.level) as f32;
            let had_level = xp.level > 0;
            self.add_experience_levels(-1);
            let xp = &mut self.common.experience;
            xp.progress = if had_level {
                1.0 + lost / sunrise_sim::entities::Experience::needed(xp.level) as f32
            } else {
                0.0
            };
        }
        while self.common.experience.progress >= 1.0 {
            let xp = &mut self.common.experience;
            xp.progress =
                (xp.progress - 1.0) * sunrise_sim::entities::Experience::needed(xp.level) as f32;
            self.add_experience_levels(1);
            let xp = &mut self.common.experience;
            xp.progress /= sunrise_sim::entities::Experience::needed(xp.level) as f32;
        }
        self.native_player_level_changed(old_level);
    }

    pub(in crate::game) fn server_journal_entity(&mut self, id: u32) {
        self.common.transactions.writes = self.common.transactions.writes.wrapping_add(1);
        if self.common.transactions.is_empty() {
            return;
        }
        let dim = self.common.dim;
        let Some(slots) = self.edit_entity_slots(id, |slots, _| slots.to_vec()) else {
            return;
        };
        if let Some(level) = self.common.transactions.last_mut() {
            level.entities.entry((dim, id)).or_insert(slots);
        }
    }

    pub(in crate::game) fn server_active_level_ref(
        &self,
        epoch: mod_api::WorldEpoch,
    ) -> Option<mod_api::LevelRef> {
        Some(self.level_ref(epoch, self.common.dim.key()))
    }

    pub(in crate::game) fn server_recipes(&self) -> &Recipes {
        self.server
            .world_tables
            .0
            .as_ref()
            .map_or_else(|| self.common.res.recipes(), |t| &t.recipes)
    }

    #[allow(dead_code)]
    pub(in crate::game) fn server_reset_entity_residency(&mut self) {
        self.reset_entity_residency();
    }

    /// The block entity at `pos` saved with its id and position
    /// (`saveWithFullMetadata`), or the saved one Sunrise keeps as loaded:
    /// what a broken block's loot sees as `LootContextParams.BLOCK_ENTITY`.
    pub(in crate::game) fn loot_block_entity(
        &self,
        pos: [i32; 3],
        block: &str,
    ) -> Option<sunrise_core::nbt::Tag> {
        let key = (pos[0], pos[1], pos[2]);
        match self.common.world.block_entities.get(&key) {
            Some(entity) => {
                Some(self.common.encode_live_block_entity(key, entity, block, self.common.res.items()))
            }
            None => sunrise_gameplay_common::game::spill_common::saved_entity(&self.common.world, pos),
        }
    }

    /// `Block.getDrops` for a block a player breaks: the block's loot table
    /// with the player as `this`, drawing from its server random sequence
    /// (`LootContext.Builder.create`), else the level random. The roll's
    /// origin is the block's centre and `pending` retains its block entity.
    pub(in crate::game) fn block_loot_at(
        &mut self,
        pos: [i32; 3],
        state: BlockStateId,
        held: Option<&ItemStack>,
    ) -> Vec<ItemStack> {
        let dynamic = self.common.pending.dynamic(pos);
        let place = self.common.loot_place_at(pos.map(|v| f64::from(v) + 0.5), None);
        let (_, entity) = self.common.native_clicker();
        let mut drops = sunrise_world::loot::with_native_loot_place(place, || {
            let roll = sunrise_items::items::BlockLootRoll {
                held,
                dynamic: &dynamic,
                breaker: true,
                ..Default::default()
            };
            self.roll_block_loot(pos, state, Some(entity as u32), roll)
        });
        let player = self.common.native_acting_id();
        self.native_block_loot(pos, state, Some(player), held, None, &mut drops);
        drops
    }

    /// `Player.drop`: the statistics and a thrown item entity in front of the player.
    pub(in crate::game) fn player_drop(&mut self, thrown: ItemStack) -> Option<u32> {
        let eye = self.common.eye_position()?;
        // LivingEntity.drop: every drop swings the main hand, a menu's
        // THROW and a creative tab's drop included.
        self.swing_hand(Hand::Main, SwingAnimation::DEFAULT);
        // ServerPlayer.drop: the drop statistics, only when an item entity came of it.
        let (item, count) = (thrown.item.clone(), thrown.count as i32);
        let dropped = self.throw_from_eyes(thrown, eye, false, true)?;
        self.side_award_stat("dropped", &item, count);
        self.side_award_custom_stat("drop", 1);
        Some(dropped)
    }

    /// `Player.attack`'s `isSweepAttack`, decided before the hit as vanilla does
    /// (`crit`: the hit on a living target is critical): where the sweep plays,
    /// none without one. Reads the charge, so before the hit resets it.
    pub(in crate::game) fn sweep_before_hit(
        &mut self,
        target: &str,
        crit: bool,
    ) -> Option<[f64; 3]> {
        let scale = self.attack_strength_scale();
        self.sweep_attack_position(scale, crit, target)
    }

    /// `Player.attack`'s sounds when a vehicle takes the hit (it always does, and
    /// can't be hit critically): knockback when sprinting at full strength, then
    /// strong or weak, or the `sweep` decided before the hit around the vehicle's
    /// box `area`. Before the hit resets the charge (`Player.onAttack`).
    pub(in crate::game) fn vehicle_attack_sounds(
        &mut self,
        area: Aabb,
        sweep: Option<[f64; 3]>,
        magic: Option<i32>,
    ) {
        self.attack_effects(area, sweep, None, false, mace::Spared::Nothing, magic);
    }

    /// A mob explosion (`ServerLevel.explode` with `ExplosionInteraction.MOB`:
    /// blocks are kept without mobGriefing, else the mob_explosion_drop_decay
    /// rule picks decay). Mods are asked through `can_entity_grief` for
    /// `source`, the exploding entity (the rule alone without one).
    pub(in crate::game) fn explode_with(
        &mut self,
        source: Option<u32>,
        center: [f64; 3],
        radius: f32,
        fire: bool,
        blast: explosion::ExplosionSource,
    ) -> Result<()> {
        let interaction = if !self.can_entity_grief(source) {
            explosion::BlockInteraction::Keep
        } else {
            self.destroy_type(sunrise_game_data::gamerules::vanilla::MOB_EXPLOSION_DROP_DECAY)
        };
        self.server_explode_from(center, radius, interaction, fire, blast)
    }

    /// Copies of a save batch carrying its live block entities and save data.
    pub(in crate::game) fn with_entities<'a>(
        &self,
        chunks: impl IntoIterator<Item = &'a Arc<sunrise_level::chunk::Chunk>>,
    ) -> Vec<Arc<sunrise_level::chunk::Chunk>> {
        sunrise_sim::save::chunks_with_block_entities(
            chunks,
            &self.common.world.block_entities,
            &self.common.world.block_names,
        )
        .into_iter()
        .map(|mut c| {
            self.attach_save_data(&mut c);
            if mod_api::__private::has_part_hook(mod_api::HookId::BlockEntitySaveAdditional) {
                let edits: std::collections::BTreeMap<_, _> = c
                    .block_entities
                    .iter()
                    .filter_map(|(pos, entity)| {
                        let BlockEntity::Mod(entity) = entity else {
                            return None;
                        };
                        if !matches!(
                            self.world.block_entities.get(pos),
                            Some(BlockEntity::Mod(_))
                        ) {
                            return None;
                        }
                        let registry = self.common.pack.assets.registry;
                        let block = registry
                            .state(c.block(pos.0 & 15, pos.1, pos.2 & 15))
                            .map_or("", |state| state.name.as_str());
                        self.common.capture_block_entity_payload(*pos, block, entity, self.common.res.items())
                            .map(|edited| (*pos, edited))
                    })
                    .collect();
                if !edits.is_empty() {
                    Arc::make_mut(c.saved.get_or_insert_with(Default::default))
                        .block_entity_save_edits = Some(Arc::new(edits));
                }
            }
            Arc::new(c)
        })
        .collect()
    }

    /// `BlockTransformer.transformBlock`'s `COPPER_CHEST` extra: a double
    /// chest's other half hears a block change and gets the same particle.
    pub(in crate::game) fn transform_copper_chest_neighbour(
        &mut self,
        pos: [i32; 3],
        old: BlockStateId,
        particle: &str,
    ) {
        let old = self.common.pack.assets.registry.state(old).cloned();
        let Some(old) = old else { return };
        if !old.name.ends_with("copper_chest") {
            return;
        }
        let kind = old
            .properties
            .get("type")
            .map(String::as_str)
            .unwrap_or("single");
        let Some(facing) = old.properties.get("facing").map(String::as_str) else {
            return;
        };
        // `ChestBlock.getConnectedDirection`: LEFT is facing.getClockWise().
        let (dx, dz) = match (kind, facing) {
            ("left", "north") => (1, 0),
            ("left", "south") => (-1, 0),
            ("left", "west") => (0, -1),
            ("left", "east") => (0, 1),
            ("right", "north") => (-1, 0),
            ("right", "south") => (1, 0),
            ("right", "west") => (0, 1),
            ("right", "east") => (0, -1),
            _ => return,
        };
        let neighbour = [pos[0] + dx, pos[1], pos[2] + dz];
        let state = self.common.world.block(neighbour[0], neighbour[1], neighbour[2]);
        self.side_post_block_game_event(
            "minecraft:block_change",
            neighbour,
            self.side_sculk_actor(),
            Some(state),
        );
        self.transform_particle(particle, neighbour);
    }

    /// `ServerLevel.destroyBlockProgress`: breaker `breaker`'s crack on `pos`
    /// (0..=9, anything else takes it away), for every player within 64 blocks.
    pub(in crate::game) fn server_destroy_block_progress(
        &mut self,
        breaker: i32,
        pos: [i32; 3],
        progress: i32,
    ) {
        if let Some(host) = self.server.lan.as_ref() {
            host.hub.broadcast_near(
                [f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2])],
                64.0,
                &sunrise_protocol::packets::play::Clientbound::BlockDestruction(
                    sunrise_protocol::packets::play::BlockDestruction {
                        id: breaker,
                        pos,
                        progress: if (0..10).contains(&progress) {
                            progress as u8
                        } else {
                            255
                        },
                    },
                ),
                None,
            );
        }
    }

    /// `Player.getVehicle().getBoundingBox()` for the local player's vehicle (`riding`).
    pub(in crate::game) fn riding_box(&self) -> Option<Aabb> {
        let id = self.common.riding?;
        if let Some(m) = self.common.mobs.iter().find(|m| m.id == id) {
            return Some(m.bounds());
        }
        if let Some(b) = self.common.boats.iter().find(|b| b.id == id) {
            return Some(b.bounds());
        }
        if let Some(c) = self.common.carts.iter().find(|c| c.id == id) {
            return Some(c.bounds());
        }
        if let Some(a) = self.common.armor_stands.iter().find(|a| a.id == id) {
            return Some(a.bounds());
        }
        if let Some(c) = self.common.cushions.iter().find(|c| c.id == id) {
            return Some(Aabb::new(
                Vec3::new(c.position.x - 0.5, c.position.y, c.position.z - 0.5),
                Vec3::new(c.position.x + 0.5, c.position.y + 0.25, c.position.z + 0.5),
            ));
        }
        if let Some(f) = self.common.item_frames.iter().find(|f| f.id == id) {
            let (lo, hi) = f.bounds();
            return Some(Aabb::new(
                Vec3::new(lo[0], lo[1], lo[2]),
                Vec3::new(hi[0], hi[1], hi[2]),
            ));
        }
        if let Some(p) = self.common.paintings.iter().find(|p| p.id == id) {
            let (lo, hi) = p.bounds();
            return Some(Aabb::new(
                Vec3::new(lo[0], lo[1], lo[2]),
                Vec3::new(hi[0], hi[1], hi[2]),
            ));
        }
        if let Some(t) = self.common.tnts.iter().find(|t| t.id == id) {
            return Some(t.bounds());
        }
        if let Some(f) = self.common.falling.iter().find(|f| f.id == id) {
            return Some(f.bounds());
        }
        if let Some(row) = self.common.resident_entities.general.row(id) {
            return Some(entity::Entity::bounds(row));
        }
        self.common.tracked_special_box(id as i32)
    }

    pub(in crate::game) fn save_game(&mut self) -> Result<()> {
        self.save_game_with(true)
    }

    /// The autosave doesn't `wait` for the chunks to be on disk: every
    /// generated chunk is saved, hundreds at a time, encoded and written on
    /// the region writer meanwhile.
    pub(in crate::game) fn save_game_with(&mut self, wait: bool) -> Result<()> {
        assert!(
            self.is_logical_server(),
            "save_game_with called on Client role"
        );
        self.common.admit_level_save()?;
        // `DedicatedServer.saveAllChunks` tells the management protocol's clients when a save starts and when
        // it ends without failing, for every save: the command, the autosave, the pause.
        let dedicated = self.common.is_dedicated_server();
        if dedicated {
            dedicated::events::record_server("saving", serde_json::json!({}));
        }
        self.save_level(wait)?;
        self.save_parked_levels(wait)?;
        self.save_game_metadata()?;
        let _ = self.finish_player_autosaves(wait);
        self.native_world_saved(wait);
        if dedicated {
            dedicated::events::record_server("saved", serde_json::json!({}));
        }
        Ok(())
    }

    /// The hurts and melee attacks each player logged since the last call
    /// (`PlayerMotion::hurt_log`, `attack_log`), taken out.
    pub(in crate::game) fn control_hurt_log(&mut self) -> Value {
        fn drain(
            name: &str,
            hurt_log: &mut Vec<sunrise_sim::health::HurtRecord>,
            attack_log: &mut Vec<sunrise_sim::health::AttackRecord>,
        ) -> Value {
            let hurts: Vec<Value> = hurt_log
                .drain(..)
                .map(|r| {
                    json!({
                        "t": r.game_time, "source": r.source, "amount": r.amount,
                        "blocked": r.blocked, "taken": r.taken, "hurt": r.hurt,
                        "armor": r.armor, "toughness": r.toughness, "health": r.health,
                        "fire_ticks": r.fire_ticks, "knockback": r.knockback,
                    })
                })
                .collect();
            let attacks: Vec<Value> = attack_log
                .drain(..)
                .map(|r| {
                    json!({
                        "t": r.game_time, "target": r.target, "id": r.id, "killed": r.killed, "amount": r.amount,
                        "scale": r.scale, "crit": r.crit, "landed": r.landed, "dealt": r.dealt,
                    })
                })
                .collect();
            json!({ "name": name, "hurts": hurts, "attacks": attacks })
        }
        let mut players = Vec::new();
        if let Some(p) = self.common.player.as_mut() {
            players.push(drain("host", &mut p.hurt_log, &mut p.attack_log));
        }
        if let Some(host) = self.server.lan.as_mut() {
            host.hub.guests.update_all(|g| {
                let name = g.name.clone();
                if let Some(m) = g.motion.as_mut() {
                    players.push(drain(&name, &mut m.hurt_log, &mut m.attack_log));
                } else if let Some((mut hurts, mut attacks)) = g.logs_kept.take() {
                    // Dead, not yet respawned: what its last body logged.
                    players.push(drain(&name, &mut hurts, &mut attacks));
                }
            });
        }
        json!({ "ok": true, "players": players })
    }
}

use sunrise_gameplay_common::game::side_ops::events::EventsSideGameExt as _;

use sunrise_gameplay_common::game::side_ops::presentation::PresentationSideGameExt as _;

use sunrise_gameplay_common::game::tables::GameItems as _;
