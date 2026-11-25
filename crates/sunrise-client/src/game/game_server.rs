#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::api_values::TryIntoApi;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_state::ClientState;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::condition_reload_state::PreparedWorldReload;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::event_loop::EventLoopCtl;
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::server_side) use crate::game::hud_render::block_outline_drawn;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::hud_render::{
    debug_crosshair_drawn, is_game_master_block,
};
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::server_side) use crate::game::inventory_input::drop_whole_stack;
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::server_side) use crate::game::key_codes::drop_held_behind_dialog;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::key_codes::{
    follow_named_keymap, gameplay_has_keyboard, glfw_button, glfw_modifiers, keyboard_button,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::loading_screen::{
    App, Loading, leave_fullscreen_after, preflight_resources, run_event_loop,
    unclean_start_recovery,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::loading_window::{
    build_renderer, loading_frame, loading_renderer, prepare_renderer_textures, start_client_game,
    start_dedicated_game,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::motion_ids::{motion_at, motion_block_entity};
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::server_side) use crate::game::resource_load::load_resources;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::resource_load::{
    Progress, StartupLoadContext, launch_world, load_resources_for_mods, load_resources_sync,
    named_scope, synchronous_startup,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::resource_loader::{
    EntityModels, GameCpuResources, Loaded, LoaderMsg, PendingWorldEntry, WorldRestLoader,
    run_loading, write_report,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::server_state::ServerState;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::subtitles::{Subtitles, in_game_ui, subtitles};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
        mpsc::{Receiver, TryRecvError},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::actions::Action as Mapping;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::held::Source;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::random_source;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::spans::{startup, time as span};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    Result,
    assets::{Assets, RenderAssets},
    audio::Audio,
    biome::Biomes,
    block_entity_render::{
        self, BlockEntityRenderer, CacheOptions, Scene as BlockEntityScene, StaticCacheFrameStats,
        StaticSectionSnapshot,
    },
    blocks::{BlockFacts, Fluid},
    clock::FixedStep,
    collisions,
    containers::{BlockEntity, Providers},
    crafting::Recipes,
    creative::{Creative, Names},
    entities::{Experience, ExperienceOrb, ItemEntity},
    environment::Environment,
    first_person::{Hand, SwingAnimation},
    furnace::FurnaceCooking,
    gfx::{Frame, Renderer},
    graphics::EntityCullingMode,
    hud::{HudAtlas, HudBatch, rgba},
    hud_layout, hud_overlays,
    items::{ItemStack, Items},
    keymapping::{KeyAction, SNEAK, SPRINT},
    light::LightTables,
    mesher::{MeshTables, Vertex},
    mobs::{Mob, MobType, MobTypes},
    models,
    motion::{MotionResources, PlayerMotion},
    mouse::{LookOptions, MouseLook},
    particles::Particle,
    save::{Level, Persistent, SaveDir},
    settings::Settings,
    spans,
    sprint::Keys,
    streaming::{Streaming, WorkerContext},
    testgen::TestTerrain,
    title::{Action, Title},
    vanillagen::VanillaTerrain,
    world::World,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client_gameplay::frame_autotune;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client_gameplay::occlusion_culling;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client_render::block_entity_reach::Reach;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::{
    biome::{BiomeZoom, obfuscate_seed},
    geometry::{Aabb, Direction},
    position::BlockPos,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::{geometry::Vec3, random::RandomSource};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_gameplay::lan::idle as lan_idle;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_gameplay::{
    interaction_shapes, item_areas, mob_index, tick_profile,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_mod_package::selection::{Argument, LaunchArguments};
#[allow(unused_imports)]
pub(in crate::game::server_side) use winit::{
    application::ApplicationHandler,
    error::EventLoopError,
    event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowId},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use {
    sunrise_core::dimension::Dimension as LevelDimension,
    sunrise_world::{
        block_collision::BlockCollisionRegistry,
        collision_context::{CollisionContext, CollisionEntity, CollisionItem},
        entity_move::MoverType,
        entity_obstruction::{EntityIdentity, EntityObstructionWorld, ObstructionEntity},
        fluid::FluidState,
        level::{
            Behaviours, BucketContent, EntityQuery, Landing, LevelAccess, LevelEvent, PlaceContext,
            PlacementContext, PlayerContext, PlayerEffect, RandomTickMaskUpdate, Ticks,
            flags as level_flags,
        },
        registry::BlockStateId,
    },
};

impl Game<'_> {
    /// The UUID of the player acting: the guest whose turn it is, else the host.
    pub(crate) fn acting_player_uuid(&mut self) -> [i32; 4] {
        match self.server.lan_acting_uuid {
            Some(guest) => sunrise_client::lan::playerdata::uuid_ints(guest),
            None => self.level.ensure_player_uuid(),
        }
    }
    /// `ItemCooldowns.addCooldown` (a guest also gets the packet).
    pub(crate) fn add_cooldown(&mut self, group: Arc<str>, ticks: i32) {
        self.cooldowns.add(Arc::clone(&group), ticks);
        if self.server.lan_acting.is_some() {
            self.server.guest_cooldowns.push((group, ticks));
        }
    }
    /// Joins an item only after drop hooks have seen its constructor's motion.
    pub(in crate::game) fn add_item_entity_prepared(
        &mut self,
        stack: ItemStack,
        position: [f64; 3],
        velocity: Option<[f64; 3]>,
        pickup_delay: u32,
        random: Option<spill::ItemRandom>,
    ) -> [f64; 3] {
        let velocity = self.push_item_entity(stack, position, velocity, pickup_delay, random);
        let _ = self.join_item_entity(self.server.next_entity, position);
        velocity
    }
    /// `ItemStack.applyAfterUseComponentSideEffects`: a used stack with a
    /// `use_cooldown` component starts its entry (`UseCooldown.apply`). A
    /// server's guest also gets `ClientboundCooldownPacket`
    /// (`ServerItemCooldowns.onCooldownStarted`), queued for their turn's end.
    pub(crate) fn apply_use_cooldown(&mut self, stack: &ItemStack) {
        let default = self
            .res
            .items()
            .get(&stack.item)
            .and_then(|i| i.use_cooldown.clone());
        let Some(component) = sunrise_client::cooldowns::component(stack, default.as_ref()) else {
            return;
        };
        let group = sunrise_client::cooldowns::group(stack, default.as_ref());
        let mut cancel = false;
        if let (Some(player), Some(name)) = (
            self.server.lan_acting_uuid,
            self.server.lan_acting_name.clone(),
        ) {
            self.native_player_item_cooldown(
                player,
                &name,
                &stack.item,
                component.ticks(),
                &mut cancel,
            );
        }
        if !cancel {
            self.add_cooldown(group, component.ticks());
        } else if self.server.lan_acting.is_some() {
            // The guest's client started the cooldown itself; 0 ends it there.
            self.server.guest_cooldowns.push((group, 0));
        }
    }
    pub(in crate::game::server_side) fn break_block_as_player(
        &mut self,
        pos: [i32; 3],
    ) -> Result<bool> {
        // DebugStickItem.canDestroyBlock: a left click selects a property and
        // never destroys the block.
        if !self.debug_stick_can_destroy(pos)? {
            return Ok(false);
        }
        // ServerPlayerGameMode.destroyBlock.
        if self.block_action_restricted(pos) {
            return Ok(false);
        }
        if !self
            .res
            .items()
            .can_destroy_block(self.held(), self.creative)
        {
            return Ok(false);
        }
        // ServerPlayerGameMode.destroyBlock (and MultiPlayerGameMode.destroyBlock
        // on the client): a GameMasterBlock stays when the player cannot use
        // game master blocks (Player.canUseGameMasterBlocks).
        let state = self.world.block(pos[0], pos[1], pos[2]);
        if !self.can_use_game_master_blocks()
            && is_game_master_block(&self.res.facts().get(state).name)
        {
            return Ok(false);
        }
        // Native mods, once the game allowed the break: a canceled one keeps
        // the block, and a guest's acknowledgement puts it back in its world.
        let mut canceled = false;
        let mut exp_to_drop = None;
        self.native_break_block(pos, state, &mut canceled, &mut exp_to_drop);
        if canceled {
            return Ok(false);
        }
        let broken = state;
        let facts = self.res.facts().get(state).clone();
        let held = self.held().cloned();
        let key = (pos[0], pos[1], pos[2]);
        self.unpack_loot(container_loot::LootTarget::Block(key), None);
        let live = (!self.creative)
            .then(|| self.live_block_entity(key, &facts.name))
            .flatten();
        let mut saved_data = None;
        // Mod data is encoded only if the removed block goes on to roll loot.
        let mod_source = match &live {
            Some((BlockEntity::Mod(_), _)) => Some((
                self.world.block_entities.generation(&key),
                self.world.block_names.get(&key).cloned(),
                self.world.block_lock(key).cloned(),
            )),
            Some((entity, _)) => {
                let saved = sunrise_client::save::block_entity_encode::encode(
                    key,
                    entity,
                    &facts.name,
                    self.res.items(),
                );
                self.pending.record_from(&self.world, pos, saved);
                None
            }
            None => {
                if !self.creative
                    && let Some(tag) = spill::saved_entity(&self.world, pos)
                {
                    self.pending.record_saved(pos, &tag);
                    // Pots and other saved-only entities have no resident object, but
                    // this is still the original entity the break is about to remove.
                    if self.native_authoritative()
                        && mod_api::__private::listened(mod_api::HookId::BlockDrops)
                    {
                        saved_data = spill::packed_block_entity_payload(tag);
                    }
                }
                None
            }
        };
        let hive = match self.world.block_entities.get(&key) {
            Some(BlockEntity::Beehive(h)) => self.hive_state(pos).map(|s| ((**h).clone(), s)),
            _ => None,
        };
        let filled_hive = hive
            .as_ref()
            .is_some_and(|(h, (honey, _))| !h.bees.is_empty() || honey.is_some_and(|l| l > 0));
        let creative = self.creative;
        // The block's parts first, then playerWillDestroy and Level.removeBlock.
        let (removed, state, harvest) =
            self.player_destroy_block_removal(pos, state, &facts, held.as_ref(), filled_hive);
        if !creative {
            // The stack as the removal left it: a listener may have swapped it.
            let held = self.held().cloned();
            let broken_tool = if removed {
                self.mine_block_with_held(&facts.name, facts.hardness)
            } else {
                None
            };
            if removed && harvest {
                // Block.playerDestroy: the mined statistic, the exhaustion and the drops.
                self.award_stat("mined", &facts.name, 1);
                if let Some(p) = self.player.as_mut() {
                    p.cause_food_exhaustion(0.005);
                }
                if let (Some((entity, _)), Some((generation, name, lock))) = (&live, mod_source) {
                    let saved = self.encode_removed_block_entity(
                        key,
                        entity,
                        generation,
                        &facts.name,
                        self.res.items(),
                    );
                    self.pending
                        .record_source(pos, spill::source_snapshot(saved, name, lock.as_ref()));
                }
                let stacks = self.block_loot_at(pos, state, held.as_ref());
                let silk = held
                    .as_ref()
                    .is_some_and(|h| h.enchantment_level("minecraft:silk_touch") > 0);
                // Block.dropResources: the item entities, then spawnAfterBreak's
                // experience, with the mods' say between the two.
                let breaker = Some(self.native_acting_id());
                let block_entity_data = live
                    .as_ref()
                    .and_then(|(_, data)| data.as_ref())
                    .or(saved_data.as_ref());
                let dropped = self.player_block_dropped(
                    (pos, state),
                    (breaker, held.as_ref(), block_entity_data),
                    (stacks, exp_to_drop),
                );
                let canceled = dropped.canceled;
                if let Some(kept) = self.server.break_collect.as_mut() {
                    kept.extend(dropped.items.into_iter().map(|item| item.stack));
                    if dropped.experience > 0 {
                        self.spawn_orbs(pos.map(|v| f64::from(v) + 0.5), dropped.experience);
                    }
                } else {
                    self.spawn_block_drops(pos, dropped);
                }
                // BeehiveBlock.playerDestroy: silk touch keeps the bees in, so
                // the trigger counts them; otherwise they are out already.
                if let Some((hive, (honey, facing))) = hive {
                    let bees = if silk { hive.bees.len() as i64 } else { 0 };
                    if !silk {
                        self.hive_broken_by_player(pos, hive, honey, facing);
                    }
                    self.trigger_bee_nest_destroyed(broken, held.as_ref(), bees);
                }
                // InfestedBlock.spawnAfterBreak: its silverfish comes out unless the
                // tool has silk touch (#prevents_infested_spawns) or blocks don't drop.
                if facts.name.starts_with("minecraft:infested_")
                    && !silk
                    && !canceled
                    && self
                        .ui
                        .rules
                        .bool_at(sunrise_client::gamerules::vanilla::BLOCK_DROPS)
                {
                    let at = [
                        f64::from(pos[0]) + 0.5,
                        f64::from(pos[1]),
                        f64::from(pos[2]) + 0.5,
                    ];
                    if let Some(id) = self.spawn_mob(
                        "minecraft:silverfish",
                        at,
                        false,
                        creatures::SpawnSite::Plain,
                    ) {
                        self.mob_poof(id);
                    }
                }
                // TurtleEggBlock.playerDestroy: after the drops, one egg of a
                // mined stack is taken and the rest stay.
                sunrise_threads::work::clear_work();
                self.with_level(|b, level, ticks| {
                    b.turtle_egg_destroyed_by_player(
                        level,
                        ticks,
                        BlockPos::new(pos[0], pos[1], pos[2]),
                        broken,
                    );
                });
            }
            // IceBlock.playerDestroy: the ice melts back into water (the
            // #prevents_ice_melting tool tag holds only silk touch).
            let prevents_melting = held
                .as_ref()
                .is_some_and(|h| h.enchantment_level("minecraft:silk_touch") > 0);
            if removed {
                sunrise_threads::work::clear_work();
                self.with_level(|b, level, ticks| {
                    b.ice_destroyed_by_player(
                        level,
                        ticks,
                        BlockPos::new(pos[0], pos[1], pos[2]),
                        broken,
                        prevents_melting,
                    );
                });
            }
            if let Some(original) = broken_tool {
                self.player_item_destroyed(&original, Some(Hand::Main));
            }
        }
        self.pending.clear(pos);
        self.server_process_level_events()?;
        Ok(removed)
    }
    /// Let go of every entity of the level (saved first): the next world
    /// opened must not inherit them, nor save them as its own.
    pub(crate) fn clear_entities(&mut self) {
        self.clear_pending_mob_actions();
        self.mobs.clear();
        self.dropped.clear();
        self.orbs.clear();
        self.creatures.projectiles.clear();
        self.falling.clear();
        self.tnts.clear();
        self.boats.clear();
        self.carts.clear();
        self.armor_stands.clear();
        self.cushions.clear();
        self.item_frames.clear();
        self.paintings.clear();
        self.others.0.clear();
        self.dragon.discard();
        self.crystals.clear();
        self.breath_clouds.clear();
        self.tridents.clear();
        self.rockets.clear();
        self.bolts.clear();
        if !self.is_logical_server() {
            self.clear_client_particles();
        }
        self.riding = None;
        self.persist = Default::default();
        self.clear_entities_server_missing_professions();
        self.entity_attachments.clear();
        self.clear_remote_objects();
        self.clear_remote_projectiles();
        self.levels.0.clear();
        self.side_clear_render_caches();
    }
    #[inline(always)]
    pub(in crate::game) fn server_clear_render_caches(&mut self) {}
    #[inline(always)]
    pub(in crate::game) fn server_expire_cracks(&mut self) {}
    #[inline(always)]
    pub(in crate::game) fn server_pick_camera_id(&self) -> Option<u32> {
        self.player
            .as_ref()
            .map(|p| p.entity_id as u32)
            .filter(|&id| id != 0)
    }
    #[inline(always)]
    pub(in crate::game) fn server_is_pick_camera(&self, id: u32) -> bool {
        self.server_pick_camera_id() == Some(id)
    }
    #[inline(always)]
    pub(in crate::game) fn server_live_remote_camera_id(&self) -> Option<u32> {
        None
    }
    #[inline(always)]
    pub(in crate::game) fn server_outline_camera_kind(
        &self,
    ) -> sunrise_world::collision_context::EntityContextKind {
        sunrise_world::collision_context::EntityContextKind::Player
    }
    #[inline(always)]
    pub(in crate::game::server_side) fn clear_entities_server_missing_professions(&mut self) {
        self.server.missing_professions.clear();
    }
    pub(crate) fn close_container_server(&mut self) {
        assert!(
            self.is_logical_server(),
            "close_container_server called on Client role"
        );
        self.close_station_server();
        let mut returned: Vec<ItemStack> = self.carried.take().into_iter().collect();
        returned.extend(self.craft_grid.iter_mut().filter_map(Option::take));
        for stack in returned {
            if let Some(rest) = self.add_stack_to_inventory(stack)
                && let Some(eye) = self.eye_position()
            {
                // AbstractContainerMenu.dropOrPlaceInInventory: `drop(stack, false)`.
                self.throw_from_eyes(rest, eye, false, false);
            }
        }
        self.craft_size = 2;
        self.server.craft_table = None;
        self.server.open_container_slots = None;
        if let Some(key) = self.container.take() {
            if self.server.lan_acting.is_none() {
                self.mod_container_player_opener(key, false);
            }
            // A cart's or chest boat's items go back to it.
            self.close_cart_container(key);
            let last = self.container_opener_event(key, false);
            if last {
                self.post_block_game_event(
                    "minecraft:container_close",
                    [key.0, key.1, key.2],
                    Some(sunrise_client::mobs::Who::Player),
                    None,
                );
                self.container_edge_sound(key, false);
            }
            self.world.modified.insert((key.0 >> 4, key.2 >> 4));
        }
        self.close_ender_chest();
    }
    #[inline(always)]
    pub(in crate::game) fn finish_close_world_server(&mut self) {
        self.server_loot.0.set(None);
        self.server_items = None;
        self.session_lock.0 = None;
        self.clear_world_parts();
        self.server.world_adopted = false;
        self.server.startup_ticket_restore_pending = false;
    }
    #[inline(always)]
    pub(in crate::game) fn finish_close_world_server_clear_bosses(&mut self) {
        self.server.bosses = Default::default();
    }
    #[inline(always)]
    pub(in crate::game) fn finish_close_world_server_clear_player_beds(&mut self) {
        self.server.player_beds.clear();
    }
    pub(in crate::game) fn finish_open_world(
        &mut self,
        prepared: world_loading::PreparedWorld,
    ) -> Result<functions::PackStack> {
        let world_loading::PreparedWorld {
            save_root,
            shapes,
            level,
            world,
            biomes,
            streaming,
            save,
            session_lock,
            rand_sequences,
            locator,
            loot,
            owner_migration,
        } = prepared;
        self.server_loot.0 = loot;
        let dim = Dim::from_id(&level.dimension);
        self.server.level_shapes = shapes;
        self.server.world_root = save_root.clone();
        // The root moves before the prepared level is installed. Until then a
        // final save must not run: the lock below only marks tag tables.
        self.server.world_adopted = false;
        self.server.owner_migration = owner_migration;
        // Saves and chunk readers share the same placeholder IDs.
        self.world_biomes = Some(biomes);
        self.world_biomes_base = self.world_biomes.clone();
        sunrise_client::crash::set_detail(
            "World",
            format!("{} ({})", level.name, save_root.display()),
        );
        self.end.set_locator(locator);
        self.share_spawn_conditions();
        self.score = Default::default();
        reset_world_progress(&mut self.adv, &mut self.stats);
        let stack = self
            .open_world_datapacks(&level.enabled_features)
            .map_err(|error| error.diagnostic())?;
        let root = self.options.resources.clone();
        self.damage_tag_generation = self.damage_tag_generation.wrapping_add(1);
        // A code-mod switch can have rebuilt the root since `Game::new`.
        self.server.structure_spawns =
            sunrise_client::mobs::spawn_selection::StructureSpawns::load(&root);
        // Native tag reads use the lock to tell that the tag tables are ready.
        self.session_lock.0 = Some(session_lock);
        self.resolve_parts()?;
        self.refresh_recipe_property_context();
        // Warm this world's pack-aware loot data before gameplay can unpack
        // a container's pending table on its first inventory access.
        self.load_server_loot();
        // MinecraftServer's constructor reads CustomBossEvents before any
        // server event.
        self.server.bosses = Default::default();
        self.load_bossbars();
        // ServerLevel's constructor reads its Raids saved data before any
        // server event; their randoms are seeded on the first raid tick.
        if self.is_logical_server() {
            self.read_raids();
        }
        self.announce_data_bound(mod_api::TagsUpdateCause::ServerDataLoad { reload: false });
        self.announce_data_maps_updated(mod_api::DataMapsUpdateCause::ServerReload);
        self.sync_recipe_content();
        self.server.restore_player_state = true;
        self.server.rand_sequences = rand_sequences;
        // ServerLevel.getWorldBorder: the level in place reads its own saved
        // border when it is made, before any command (the parked levels read
        // theirs in `open_level`).
        self.border = self.load_border(dim);
        self.set_mode_id(level.mode());
        self.previous_mode = level.previous_game_mode.map(game_mode::mode_from_id);
        let mut inventory = self.load_saved_inventory(&level);
        if self.creative && level.inventory.is_empty() && level.position.is_none() {
            for (i, name) in [
                "minecraft:grass_block",
                "minecraft:stone",
                "minecraft:oak_planks",
                "minecraft:glass",
                "minecraft:torch",
                "minecraft:oak_log",
                "minecraft:chest",
                "minecraft:furnace",
                "minecraft:diamond_pickaxe",
            ]
            .iter()
            .enumerate()
            {
                inventory[i] = Some(ItemStack {
                    item: Arc::from(*name),
                    count: 1,
                    damage: 0,
                    extra: None,
                });
            }
        }
        let (yaw, pitch) = self
            .options
            .look
            .unwrap_or((level.rotation[0], level.rotation[1]));
        self.yaw = yaw;
        self.pitch = pitch;
        self.selected = level.selected_slot.clamp(0, 8) as usize;
        self.streaming = streaming;
        self.world = world;
        self.inventory = inventory;
        self.bind_chunk_disk_worker(&save, dim);
        self.save.0 = self.with_profession_save_snapshot(save);
        // MinecraftServer.prepareLevels restores the saved tickets after the
        // built-in levels have been published.
        self.tickets.0 = Default::default();
        self.server.startup_ticket_restore_pending =
            self.options.execution_role.is_logical_server();
        // The ride the player's file names, once the vehicle is in the world.
        self.server.pending_vehicle = level.root_vehicle;
        self.level = level;
        self.check_mod_mismatch();
        self.server.world_adopted = true;
        if self.options.execution_role.is_logical_server() {
            self.options
                .save_failure_diagnostics
                .register(self.save.0.save_failure_reporter());
        }
        // ServerRecipeBook.loadUntrusted: recipes the manager does not have are dropped.
        let index = &world_stations!(self).recipe_index;
        let book = &mut self.level.recipe_book;
        book.recipes.retain(|r| index.contains(r));
        book.to_be_displayed.retain(|r| index.contains(r));
        self.enter_opened_dim(dim);
        self.world_flat = sunrise_client::save::WorldGen::read(&save_root)
            .is_some_and(|w| w.preset == sunrise_client::save::WorldPreset::Flat);
        self.server.world_debug = sunrise_client::save::WorldGen::read(&save_root)
            .is_some_and(|w| w.preset == sunrise_client::save::WorldPreset::Debug);
        self.block_ticks = Ticks::new(self.level.seed as i32);
        self.server.tick_chunks.clear();
        self.clear_entities();
        self.player = None;
        self.sleeping = None;
        self.sleep_timer = 0;
        self.server.player_beds.clear();
        self.spawn = None;
        self.server.respawn_config = self
            .level
            .respawn_elsewhere
            .as_ref()
            .and_then(respawn_anchor::RespawnConfig::from_saved);
        self.server.spawn_origin = None;
        self.server.spawn_hook_fired = false;
        self.server.spawn_hook_cancelled = false;
        self.server.spawn_spiral = Default::default();
        self.server.player_spawn = None;
        self.creatures.reset_trader();
        self.creatures.cat = Default::default();
        self.creatures.custom_spawners.clear();
        self.portals = Default::default();
        self.experience = Experience {
            level: self.level.xp_level,
            progress: self.level.xp_progress,
            total: self.level.xp_total,
        };
        self.restore_effects();
        self.mining = None;
        self.container = None;
        let kinds = std::mem::take(&mut self.be_anims.kinds);
        self.be_anims = sunrise_client::block_entity_anim::BlockEntityAnims {
            kinds,
            ..Default::default()
        };
        self.carried = None;
        self.using = None;
        self.apply_settings();
        let facts = self.dimension_type_of(dim);
        self.streaming.sky_light = facts.height.has_skylight;
        self.streaming.nether_shading =
            facts.cardinal_light == sunrise_world::dimension_type::CardinalLight::Nether;
        self.load_world_state()?;
        self.open_builtin_levels()?;
        Ok(stack)
    }
    /// `Block.fallOn` for item `id`, which fell `distance` onto the block at `on`
    /// (`Entity.checkFallDamage`): a mod block's `fall_on` Part hears it, through the same
    /// `block_contact::fall` as a player's and a mob's landing. An item takes no fall damage.
    /// Nothing is lent or built unless a Part of that block listens.
    pub(in crate::game::server_side) fn item_fall_on(
        &mut self,
        id: u32,
        on: sunrise_core::position::BlockPos,
        distance: f64,
    ) {
        let state = self.world.block(on.x, on.y, on.z);
        if !sunrise_client::block_contact::fall_listens(state) {
            return;
        }
        let mut lent = sunrise_client::health::LentLevel::take(&mut self.world);
        // A native fall_on Part can unwind. Keep the persistent level carrier
        // outside the callback's temporary views, and return its progressed state
        // before propagating the original panic. This does not restore the item batch.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut level = sunrise_client::health::DamageLevel::lent(
                &self.world,
                self.pack.assets.registry,
                &mut lent,
                Some(world_items!(self)),
                Some(&self.pack.block_registries),
                &self.damage_hooks,
            )
            .with_effects(crate::game::entity::hit_effects_of!(self, None));
            let contact = sunrise_client::block_contact::Contact {
                entity: id as i32,
                kind: "minecraft:item",
                state,
                position: on,
            };
            let _ = sunrise_client::block_contact::fall(&mut level, contact, distance, 0.0);
        }));
        lent.give_back(&mut self.world);
        if let Err(payload) = result {
            std::panic::resume_unwind(payload);
        }
    }
    /// An item-specific POV outline ray at the current rotation and block reach.
    pub(in crate::game::server_side) fn item_pick(&self) -> Option<Hit> {
        if let Some(hit) = self.server.forced_hit {
            return Some(hit);
        }
        let (origin, direction) = self.aim()?;
        self.clip_ray(origin, direction, self.block_reach)
    }
    /// `LivingEntity.startSleeping`: the player takes the bed whose head
    /// `head` names, moved onto it and asleep from now on. False when the
    /// bed gives no shape to lie on.
    pub(in crate::game::server_side) fn lie_in_bed(&mut self, head: [i32; 3]) -> bool {
        // The shape the bed gives the player to lie on (`getSleepHeight`).
        // A straw bed's head answers with its foot's shape, and with no foot
        // of its own there is none, so the player stays up as
        // `startSleeping` returning false leaves them.
        let Some(at) = self.sleeping_position(head) else {
            return false;
        };
        // LivingEntity.startSleeping: stopRiding once the bed is valid.
        self.stop_riding();
        self.stop_mining();
        self.server_set_acting_attack(false);
        self.server_set_acting_use(false);
        self.using = None;
        self.sleeping = Some(head);
        self.sleep_timer = 0;
        // LivingEntity.startSleeping and ServerPlayer.startSleeping: setPosToBed
        // and connection.teleport.
        if let Some(p) = self.player.as_mut() {
            p.position = at;

            p.velocity = Vec3::ZERO;
            let _ = p.set_sleeping(true);
            p.pitch = 0.0;
            p.teleported = true;
            p.teleported_delta = [false; 3];
        }
        self.pitch = 0.0;
        if self.server.lan_acting.is_none() {
            self.previous_position = at;
            self.snap_player_model_pitch();
        }
        self.occupy_bed(head);
        true
    }
    /// The saved player's hands, armor, body, saddle and ender chest go in
    /// their fields; the 36 inventory slots come back.
    pub(in crate::game) fn load_saved_inventory(
        &mut self,
        level: &Level,
    ) -> Vec<Option<ItemStack>> {
        let mut inventory = vec![None; 36];
        self.armor = [None, None, None, None];
        self.offhand = None;
        self.equipment = Default::default();
        self.server.ender_items = if level.ender_items.len() == 27 {
            level.ender_items.clone()
        } else {
            vec![None; 27]
        };
        for (slot, id, count, damage) in &level.inventory {
            let stack = ItemStack {
                item: Arc::from(id.as_str()),
                count: *count,
                damage: *damage,
                extra: level.inventory_components.get(slot).cloned(),
            };
            if (100..104).contains(slot) {
                self.armor[usize::from(103 - slot)] = Some(stack);
            } else if *slot == 150 {
                self.offhand = Some(stack);
            } else if let Some(equipment_slot) = sunrise_client::save::equipment_slot(*slot) {
                self.equipment.set(equipment_slot, Some(stack));
            } else if let Some(cell) = inventory.get_mut(usize::from(*slot)) {
                *cell = Some(stack);
            }
        }
        inventory
    }
    /// Menus opened by PlayerEffect::OpenMenu.
    pub(in crate::game) fn open_block_effect_menu(&mut self, pos: [i32; 3]) {
        if self.fake_blocks(mod_api::FakeEffect::Menu) {
            return;
        }
        let name = self
            .res
            .facts()
            .get(self.world.block(pos[0], pos[1], pos[2]))
            .name
            .clone();
        let key = (pos[0], pos[1], pos[2]);
        if self.chest_menu_blocked(key, &name) {
            // The use still succeeds, but no menu, stat or piglin anger.
            return;
        }
        // BaseContainerBlockEntity.createMenu: a lock the opener cannot
        // unlock refuses the menu with vanilla's action bar and sound.
        if self.refuse_locked_container(key, &name) {
            return;
        }
        if &*name == "minecraft:lectern" {
            self.server_request_lectern(pos);
            return;
        }
        if sunrise_client::command_block::Mode::of_block(&name).is_some() {
            self.server_request_command_block(key);
            return;
        }
        // The game master blocks' screens open on the player's own client
        // (`BlockEntity.usedBy`); the server has nothing to open.
        if matches!(
            &*name,
            "minecraft:structure_block"
                | "minecraft:jigsaw"
                | "minecraft:test_block"
                | "minecraft:test_instance_block"
        ) {
            return;
        }
        // SignBlock.useWithoutItem: the text editor, not a container.
        let block = self.world.block(pos[0], pos[1], pos[2]);
        if self.res.facts().get(block).sign_form().is_some() {
            self.use_sign_empty_hand(pos);
            return;
        }
        if &*name == "minecraft:crafting_table" {
            self.craft_size = 3;
            self.server.craft_table = Some(pos);
        } else if &*name == "minecraft:ender_chest" {
            self.open_ender_chest(key);
            // EnderChestBlock.useWithoutItem
            self.award_custom_stat("open_enderchest", 1);
            self.anger_piglins(true);
            // Guests start the opener only when the handoff installs their menu.
            // EnderChestBlockEntity.startOpen: the first opener's CONTAINER_OPEN.
            if self.server.lan_acting.is_none() && self.container_opener_event(key, true) {
                self.post_block_game_event(
                    "minecraft:container_open",
                    [key.0, key.1, key.2],
                    Some(sunrise_client::mobs::Who::Player),
                    None,
                );
                self.container_edge_sound(key, true);
            }
        } else if matches!(&*name, "minecraft:beacon" | "minecraft:enchanting_table")
            && self.open_station(pos, &name)
        {
            // Station menus whose block has a block entity, not a container.
            self.world.block_entities.get_or_insert_with(key, || {
                BlockEntity::for_block(&name).unwrap_or(BlockEntity::Chest(Vec::new()))
            });
        } else if storage::is_shulker_box(&name) && !self.shulker_can_open(pos) {
            // ShulkerBoxBlock.canOpen: the lid has no room; nothing opens.
        } else if let Some(entity) = BlockEntity::for_block(&name) {
            self.world.block_entities.get_or_insert_with(key, || entity);
            sunrise_client::spans::time("open_loot", || self.unpack_opened_loot(key, &name));
            let centre = [
                f64::from(key.0) + 0.5,
                f64::from(key.1) + 0.5,
                f64::from(key.2) + 0.5,
            ];
            sunrise_client::spans::time("open_maps", || {
                self.resolve_container_maps(key, centre);
            });
            self.container = Some(key);
            self.server.open_container_slots = Some((key, self.open_container_size()));
            // ChestMenu and ShulkerBoxMenu start the opener in their constructors.
            // For a guest, open_block_menu does that after the turn's handoff.
            let first = self.server.lan_acting.is_none() && self.container_opener_event(key, true);
            if first {
                self.post_block_game_event(
                    "minecraft:container_open",
                    [key.0, key.1, key.2],
                    Some(sunrise_client::mobs::Who::Player),
                    None,
                );
            }
            let stat = match &*name {
                "minecraft:chest" => "open_chest",
                "minecraft:trapped_chest" => "trigger_trapped_chest",
                "minecraft:barrel" => "open_barrel",
                "minecraft:hopper" => "inspect_hopper",
                "minecraft:dropper" => "inspect_dropper",
                "minecraft:dispenser" => "inspect_dispenser",
                n if storage::is_shulker_box(n) => "open_shulker_box",
                _ => "",
            };
            if !stat.is_empty() {
                self.award_custom_stat(stat, 1);
            }
            // Opening a piglin-guarded container angers the piglins who see it.
            if self.guarded_by_piglins(&name) {
                self.anger_piglins(true);
            }
            if first {
                self.container_edge_sound(key, true);
            }
        } else if self.open_mod_menu(key) {
            // A mod block's container (`native_block_entity_container`).
            self.container = Some(key);
            if self.server.lan_acting.is_none() {
                self.mod_container_player_opener(key, true);
            }
        } else if self.open_station(pos, &name) {
            // Workstation menus: game/blocks/stations.rs.
        } else {
            self.server_send_overlay_message(sunrise_client::text::Text::literal(format!(
                "{} is not available yet",
                name.trim_start_matches("minecraft:").replace('_', " ")
            )));
            return;
        }
        self.server_set_acting_use(false);
    }
    /// `ExperienceOrb.followNearbyPlayer`'s candidates: each orb goes for the
    /// nearest living player within 8 blocks, the host's own or a guest. Each
    /// is the player's feet, the point the orb homes on and the player's UUID.
    pub(crate) fn orb_followed(&self) -> Vec<xp_orb_targeting::OrbCandidate> {
        let spectator = self.spectator();
        let player = self
            .player
            .as_ref()
            .filter(|p| !p.health.dead_or_dying() && !spectator && !self.server.lan_acting_fake);
        // ExperienceOrb.followNearbyPlayer: each orb goes for the nearest
        // living player within 8 blocks, the host's own or a guest.
        let uuid = self
            .level
            .player_uuid
            .map(sunrise_client::lan::playerdata::ints_uuid);
        let mut followed: Vec<xp_orb_targeting::OrbCandidate> = player
            .map(|p| {
                let feet = [p.position.x, p.position.y, p.position.z];
                let eye = f64::from(p.dimensions.eye_height);
                (
                    feet,
                    [feet[0], feet[1] + eye / 2.0, feet[2]],
                    uuid,
                    p.entity_id as u32,
                )
            })
            .into_iter()
            .collect();
        if let Some(host) = self.lan.as_ref() {
            followed.extend(
                host.hub
                    .guests
                    .iter()
                    .filter(|g| {
                        g.placed
                            && g.participates_in_player_simulation()
                            && g.loaded
                            && !g.dead
                            && g.health > 0.0
                    })
                    .filter(|g| !g.spectator())
                    .map(|g| {
                        let p = g.position;
                        (
                            p,
                            [p[0], p[1] + g.eye_height() / 2.0, p[2]],
                            Some(g.uuid),
                            g.entity_id as u32,
                        )
                    }),
            );
        }
        followed
    }
    pub(in crate::game) fn server_process_level_events(&mut self) -> Result<()> {
        let events = std::mem::take(&mut self.level_events);
        self.lan_record_events(&events);
        dispense_fixture::record_events(&events);
        for event in events {
            match event {
                LevelEvent::LevelEvent { .. } => {}
                LevelEvent::ContainerLootTable { pos, tag } => {
                    let key = (pos.x, pos.y, pos.z);
                    if let Some(mut entity) = self.world.block_entities.remove(&key) {
                        self.take_loot_table(key, &tag, &mut entity);
                        self.world.block_entities.insert(key, entity);
                        self.world.modified.insert((pos.x >> 4, pos.z >> 4));
                    }
                }
                // A prime the mods refused is silent: no fuse sound, no game event.
                LevelEvent::Sound { pos, ref sound, .. }
                    if self.server.tnt_refused == Some(pos)
                        && &**sound == "minecraft:entity.tnt.primed" => {}
                LevelEvent::GameEvent {
                    pos,
                    event: "minecraft:prime_fuse",
                    ..
                } if self.server.tnt_refused == Some(pos) => self.server.tnt_refused = None,
                LevelEvent::Sound {
                    pos,
                    sound,
                    source,
                    volume,
                    pitch,
                } => {
                    let c = [
                        f64::from(pos.x) + 0.5,
                        f64::from(pos.y) + 0.5,
                        f64::from(pos.z) + 0.5,
                    ];
                    let name = if sound.contains(':') {
                        sound.to_string()
                    } else {
                        format!("minecraft:{sound}")
                    };
                    match source {
                        Some(sunrise_world::level::LevelSoundSource::Blocks) => self
                            .server_play_sound_on(
                                mod_api::SoundSource::Blocks.index(),
                                &name,
                                c,
                                volume,
                                pitch,
                            ),
                        None => self.server_play_sound(&name, c, volume, pitch),
                    }
                }
                // Handled as the behaviour emits it (GameLevel::emit).
                LevelEvent::DropResources { pos, state } => {
                    self.with_level(|_, level, _| level.drop_resources_now(pos, state));
                }
                LevelEvent::FallingBlock {
                    pos,
                    state,
                    hurt_per_distance,
                } => {
                    let mut damage = sunrise_client::falling_block::Damage::falling(
                        &self.res.facts().get(state).name,
                    );
                    if let Some(per_distance) = hurt_per_distance {
                        damage.hurt_entities = true;
                        damage.per_distance = per_distance;
                    }
                    let p = Vec3::new(
                        f64::from(pos.x) + 0.5,
                        f64::from(pos.y),
                        f64::from(pos.z) + 0.5,
                    );
                    self.server.next_entity += 1;
                    self.falling.push(FallingBlock {
                        size_answer: None,
                        attachment_lifetime: Default::default(),
                        rotation: [0.0, 0.0],
                        join: Default::default(),
                        id: self.server.next_entity,
                        state,
                        blocks_building: true,
                        start_pos: pos,
                        position: p,
                        previous: p,
                        velocity: Vec3::ZERO,
                        age: 0,
                        no_gravity: false,
                        portal: Default::default(),
                        drop_item: true,
                        fall_distance: 0.0,
                        ground: Default::default(),
                        fire_ticks: 0,
                        sync_velocity: false,
                        stuck: Vec3::ZERO,
                        damage,
                        saved: None,
                    });
                }
                LevelEvent::Player { pos, effect, .. } => match effect {
                    PlayerEffect::OpenMenu => self.open_block_effect_menu([pos.x, pos.y, pos.z]),
                    PlayerEffect::Sleep if self.beds_destroy_on_use() => {
                        self.destroy_bed_on_use([pos.x, pos.y, pos.z])?
                    }
                    PlayerEffect::Sleep => self.try_sleep([pos.x, pos.y, pos.z]),
                    PlayerEffect::GiveItem(item) => {
                        if let Some(rest) = self.add_stack_to_inventory(ItemStack {
                            item,
                            count: 1,
                            damage: 0,
                            extra: None,
                        }) {
                            self.pop_resource([pos.x, pos.y, pos.z], rest);
                        }
                    }
                    // Popped as the behaviour emits it (GameLevel::emit).
                    PlayerEffect::DropItem(_) => {}
                    PlayerEffect::EatCake => {
                        // CakeBlock.eat
                        self.award_custom_stat("eat_cake_slice", 1);
                        if let Some(p) = self.player.as_mut() {
                            let food = &mut p.health.food;
                            food.food = (food.food + 2).min(20);
                            food.saturation = (food.saturation + 0.4).min(food.food as f32);
                        }
                    }
                    PlayerEffect::RespawnAnchor => {
                        self.respawn_anchor_used([pos.x, pos.y, pos.z])?
                    }
                    PlayerEffect::InteractLoot(_) => {}
                },
                LevelEvent::PrimeTnt { pos, rot } => self.prime_tnt_event(pos, rot),
                LevelEvent::BlockEvent {
                    pos,
                    state,
                    kind: "dispense",
                } => self.dispense_from(pos, state)?,
                LevelEvent::BlockEvent {
                    pos, kind: "golem", ..
                } => self.check_golem_spawn([pos.x, pos.y, pos.z])?,
                LevelEvent::BlockEvent {
                    pos,
                    kind: "command_block",
                    ..
                } => self.command_block_tick(pos),
                LevelEvent::BlockEvent {
                    pos,
                    kind: "structure_block",
                    ..
                } => self.structure_block_neighbor_changed((pos.x, pos.y, pos.z)),
                LevelEvent::BlockEvent {
                    pos,
                    kind: "test_block",
                    ..
                } => self.test_block_neighbor_changed((pos.x, pos.y, pos.z)),
                LevelEvent::BlockEvent {
                    pos,
                    kind: "test_block_tick",
                    ..
                } => self.reset_test_block((pos.x, pos.y, pos.z)),
                LevelEvent::BlockEvent {
                    pos,
                    kind: "brushable",
                    ..
                } => self.brushable_tick([pos.x, pos.y, pos.z]),
                LevelEvent::BlockEvent {
                    pos, kind: "hatch", ..
                } => {
                    // FrogspawnBlock.hatchFrogspawn: 2-5 tadpoles.
                    let at = [
                        f64::from(pos.x) + 0.5,
                        f64::from(pos.y) + 0.5,
                        f64::from(pos.z) + 0.5,
                    ];
                    self.server_play_sound("minecraft:block.frogspawn.hatch", at, 1.0, 1.0);
                    let count = 2 + self.world.random.next_int_bound(4).unwrap_or(0);
                    for _ in 0..count {
                        // getRandomTadpolePositionOffset: 0.2..0.8 inside the block.
                        let r = &mut self.world.random;
                        let off = |r: &mut sunrise_core::random::LegacyRandom| {
                            f64::from(r.next_float()) * 0.6 + 0.2
                        };
                        let (dx, dz) = (off(r), off(r));
                        let spawn = [
                            f64::from(pos.x) + dx,
                            f64::from(pos.y) - 0.5,
                            f64::from(pos.z) + dz,
                        ];
                        let _ = self.spawn_mob(
                            "minecraft:tadpole",
                            spawn,
                            true,
                            creatures::SpawnSite::Plain,
                        );
                    }
                }
                LevelEvent::BlockEvent {
                    pos,
                    kind: "hatch_sniffer",
                    ..
                } => self.hatch_sniffer([pos.x, pos.y, pos.z]),
                LevelEvent::BlockEvent {
                    pos,
                    state,
                    kind: "hatch_turtles",
                } => self.hatch_turtles([pos.x, pos.y, pos.z], state),
                LevelEvent::BlockEvent {
                    pos,
                    kind: "portal_spawn",
                    ..
                } => self.portal_spawn([pos.x, pos.y, pos.z]),
                LevelEvent::BlockEvent {
                    pos,
                    state,
                    kind: "spawn_ghastling",
                } => self.spawn_ghastling([pos.x, pos.y, pos.z], state),
                LevelEvent::GameEvent { pos, event, state } => {
                    self.sculk_level_event(pos, event, state)
                }
                LevelEvent::BlockEvent {
                    pos,
                    state,
                    kind: "shrieker_respond",
                } => self.shrieker_block_event(pos, state),
                LevelEvent::BlockEntityEvent { pos, a, b, .. } => {
                    self.block_entity_event([pos.x, pos.y, pos.z], a, b)
                }
                // NoteBlock.getCustomSoundId: the skull above's note block sound,
                // played at volume 3 (vanilla's pitch for a head is 1).
                LevelEvent::BlockEvent {
                    pos,
                    kind: "note_custom_sound",
                    ..
                } => {
                    if let Some(BlockEntity::Skull(skull)) =
                        self.world.block_entities.get(&(pos.x, pos.y + 1, pos.z))
                        && let Some(sound) = skull.note_block_sound.clone()
                    {
                        let at = [
                            f64::from(pos.x) + 0.5,
                            f64::from(pos.y) + 0.5,
                            f64::from(pos.z) + 0.5,
                        ];
                        self.server_play_sound(&sound, at, 3.0, 1.0);
                    }
                }
                // The block at `pos` has just replaced `state`: Block.pushEntitiesUp.
                LevelEvent::BlockEvent {
                    pos,
                    state,
                    kind: "push_entities_up",
                } => {
                    self.push_entities_up(
                        pos,
                        state,
                        self.world.block(pos.x, pos.y, pos.z),
                        |_| true,
                    );
                }
                LevelEvent::GrowTree { .. }
                | LevelEvent::PlaceFeature { .. }
                | LevelEvent::BlockEvent { .. } => {}
            }
        }
        Ok(())
    }
    pub(in crate::game) fn rebuild_chunk_tickets(&mut self, notify: bool) {
        let rule = self
            .ui
            .rules
            .bool_at(sunrise_client::gamerules::vanilla::SPECTATORS_GENERATE_CHUNKS);
        let mut players: Vec<((i32, i32), bool)> = self.lan.as_ref().map_or_else(Vec::new, |h| {
            h.hub
                .guests
                .iter()
                .filter(|g| h.hub.here(g))
                .filter(|g| !g.is_fake()) // fake players hold small forced tickets below
                .map(|g| (g.ticket_chunk(), g.spectator()))
                .collect()
        });
        if let Some(p) = &self.player {
            players.push((
                (
                    (p.position.x.floor() as i32) >> 4,
                    (p.position.z.floor() as i32) >> 4,
                ),
                self.spectator(),
            ));
        }
        // A spectator the rule skips is still a player: a game that has one
        // must not take the headless fixtures' tick-everywhere case below.
        let any_player = !players.is_empty();
        let sources = (!self.options.execution_role.is_client() || any_player).then(|| {
            sunrise_client::ticket_levels::Sources {
                players: players
                    .into_iter()
                    .filter(|&(_, spectator)| game_mode::loads_chunks(rule, spectator))
                    .map(|(chunk, _)| chunk)
                    .collect(),
                forced: self.streaming.forced.iter().copied().collect(),
                // Filled from the `PORTAL` tickets by `update`.
                portal: Vec::new(),
                spawn: Vec::new(),
                // EnderDragonFight.tick: while a player watches the fight, the arena holds a
                // `DRAGON` ticket of radius 9 at (0, 0), so its island ticks out to 7 chunks
                // whatever the watcher's simulation distance. The fight is where the type has one.
                dragon: (self.dimension_type().has_ender_dragon_fight
                    && self.server.fight.arena_watched())
                .then_some((0, 0)),
                viewed: self.viewed_tickets(),
                view_distance: self.settings.render_distance.clamp(2, 32),
                simulation_distance: self.settings.simulation_distance.max(2),
            }
        });
        // The levels' changes are kept only for a listener, so that a tick
        // with none looks at no chunk.
        self.tickets.0.record_changes(
            notify
                && self.is_logical_server()
                && mod_api::__private::listened(mod_api::HookId::ChunkTicketLevelUpdated),
        );
        self.tickets.0.update(sources);
        if notify {
            self.run_chunk_ticket_level_updates();
        }
    }
    /// `ServerLevel.startTickingChunk`: a loaded chunk's saved ticks are
    /// scheduled from the game time at which it becomes a ticking chunk
    /// (block ticking, with its neighbours loaded). Chunks become ticking
    /// between ticks in vanilla: before this tick's clock advanced.
    pub(in crate::game::server_side) fn start_ticking_chunks(&mut self) {
        let levels = self.tickets.0.levels();
        let world = &self.world;
        let started = self
            .server
            .tick_chunks
            .start_ticking(|(x, z)| levels.loaded_ticking(x, z) && ticking_chunk(world, x, z));
        let time = self.world.game_time - 1;
        for (x, z) in started {
            let pos = sunrise_core::position::ChunkPos::new(x, z);
            self.block_ticks.blocks.unpack_chunk(pos, time);
            self.block_ticks.fluids.unpack_chunk(pos, time);
        }
    }
    /// Copy the live player and world state into the `Level` that gets saved.
    pub(crate) fn sync_level(&mut self) {
        self.level.player_impulse_grace = self.player.as_ref().map(|p| p.impulse.reset_grace_time);
        self.level.player_fall_particles = self
            .player
            .as_ref()
            .map(|p| p.spawn_extra_particles_on_fall);
        // Level.write saves the game rules (game_rules.dat) with the rest.
        self.level.game_rules = self.ui.rules.by_name();
        // PrimaryLevelData.setDifficulty: /difficulty and the options button change it.
        self.level.difficulty =
            sunrise_client::title::DIFFICULTIES[usize::from(self.ui.difficulty) & 3].into();
        self.level.time = self.world.game_time;
        self.level.weather = self.world.weather;
        // level.dat holds the Overworld clock's total, whichever level is in place.
        self.level.day_time = self.command_clock_time("minecraft:overworld");
        self.level.end_clock = Some(self.world.end_clock);
        // A server has no player of its own; the entry of `guest_turn` puts one in
        // `self.player` only while a command reads it.
        if self.player.is_none() && self.options.dedicated.is_some() {
            return;
        }
        self.level.selected_slot = self.selected as i32;
        let mode = self.mode_id();
        // The vehicle the player rides (`ServerPlayer.saveParentVehicle`).
        let others = self
            .lan
            .as_ref()
            .map(|h| self.ridden_by_others(&h.hub.guests, self.dim.id(), None, false))
            .unwrap_or_default();
        let vehicle = self.parent_vehicle_tag(self.riding, &others);
        // In-frame singleplayer is gone; server players save through guest records.
        let level = &mut self.level;
        level.creative = self.creative;
        level.game_mode = mode;
        level.previous_game_mode = self.previous_mode.map(mod_api::GameMode::id);
        level.spawn = self.spawn.map(|s| [s.x, s.y, s.z]);
        level.respawn_elsewhere = respawn_anchor::RespawnConfig::kept(
            level.respawn_elsewhere.take(),
            self.server.respawn_config,
        );
        level.rotation = [self.yaw, self.pitch];
        level.xp_level = self.experience.level;
        level.xp_progress = self.experience.progress;
        level.xp_total = self.experience.total;
        level.root_vehicle = vehicle
            .as_ref()
            .map(|v| v.2)
            .or(self.server.pending_vehicle);
        // The ride's tree, or none once the file's own came back.
        if let Some((tag, ..)) = vehicle {
            let extra = level
                .player_extra
                .get_or_insert_with(|| sunrise_core::nbt::Tag::Compound(Default::default()));
            if let sunrise_core::nbt::Tag::Compound(map) = extra {
                map.insert("RootVehicle".into(), tag);
            }
        }
        if let Some(p) = &self.player {
            level.position = Some([p.position.x, p.position.y, p.position.z]);
            level.health = p.health.health();
            level.absorption = p.health.absorption();
            let extra = level
                .player_extra
                .get_or_insert_with(|| sunrise_core::nbt::Tag::Compound(Default::default()));
            if let sunrise_core::nbt::Tag::Compound(map) = extra {
                map.insert(
                    "Fire".into(),
                    sunrise_core::nbt::Tag::Short(
                        p.fire_ticks.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16,
                    ),
                );
                map.insert("Air".into(), sunrise_core::nbt::Tag::Short(p.air as i16));
                // Entity.saveWithoutId: the `Invulnerable` flag always, its
                // `invulnerable_time` window only while open, like a mob's
                // save (#916).
                map.insert(
                    "Invulnerable".into(),
                    sunrise_core::nbt::Tag::Byte(i8::from(p.health.permanently_invulnerable)),
                );
                if p.health.invulnerable_time > 0 {
                    map.insert(
                        "invulnerable_time".into(),
                        sunrise_core::nbt::Tag::Int(p.health.invulnerable_time),
                    );
                } else {
                    map.remove(&"invulnerable_time".into());
                }
                // LivingEntity.addAdditionalSaveData: AttributeMap.pack.
                map.insert("attributes".into(), p.saved_attributes());
            }
            level.fall_distance = p.fall_distance;

            level.food = p.health.food.food;
            level.saturation = p.health.food.saturation;
            level.flying = p.flying;
        }
        level.inventory = self
            .inventory
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                s.as_ref()
                    .map(|s| (i as u8, s.item.to_string(), s.count, s.damage))
            })
            .collect();
        level.inventory_components = self
            .inventory
            .iter()
            .enumerate()
            .filter_map(|(i, s)| Some((i as u8, s.as_ref()?.extra.clone()?)))
            .collect();
        if let Some(s) = &self.offhand {
            level
                .inventory
                .push((150, s.item.to_string(), s.count, s.damage));
            if let Some(extra) = s.extra.clone() {
                level.inventory_components.insert(150, extra);
            }
        }
        for (equipment_slot, s) in self.equipment.iter().enumerate() {
            if let (Some(s), Some(slot)) = (s, sunrise_client::save::saved_slot(equipment_slot)) {
                level
                    .inventory
                    .push((slot, s.item.to_string(), s.count, s.damage));
                if let Some(extra) = s.extra.clone() {
                    level.inventory_components.insert(slot, extra);
                }
            }
        }
        // Vanilla stores armor in slots 100 (feet) to 103 (head).
        for (i, s) in self.armor.iter().enumerate() {
            if let Some(s) = s {
                level
                    .inventory
                    .push((103 - i as u8, s.item.to_string(), s.count, s.damage));
                if let Some(extra) = s.extra.clone() {
                    level.inventory_components.insert(103 - i as u8, extra);
                }
            }
        }
        level.dimension = self.dim.id().to_owned();
        // An open ender chest holds the live copy.
        if let Some(key) = self.server.ender_open
            && let Some(BlockEntity::Chest(items)) = self.world.block_entities.get(&key)
        {
            self.server.ender_items = items.clone();
        }
        level.ender_items = self.server.ender_items.clone();
        level.active_effects = self.potions.effects.saved();
    }
    /// A translated message for the player the game acts for: `overlay` is
    /// `sendOverlayMessage` (the action bar), else `sendSystemMessage` (chat).
    /// A guest gets a packet, so its own client translates it.
    pub(in crate::game::server_side) fn tell_sleeper(
        &mut self,
        key: &str,
        _fallback: &str,
        overlay: bool,
    ) {
        assert!(
            self.is_logical_server(),
            "tell_sleeper called on Client role"
        );
        if overlay {
            self.server_send_overlay_message(sunrise_client::text::Text::translate(key, &[]));
        } else {
            self.server_system_message(sunrise_client::text::Text::translate(key, &[]));
        }
    }
    pub(in crate::game::server_side) fn tick_mining(&mut self) -> Result<()> {
        if self.server.destroy_delay > 0 {
            self.server.destroy_delay -= 1;
        }
        if !self.server_acting_attack() {
            self.mining = None;
            return Ok(());
        }
        let Some(hit) = self.pick() else {
            self.mining = None;
            return Ok(());
        };
        // Player.blockActionRestricted: spectators, and adventure players
        // without a matching can_break item.
        if self.block_action_restricted(hit.pos)
            || !self
                .res
                .items()
                .can_destroy_block(self.held(), self.creative)
        {
            self.mining = None;
            return Ok(());
        }
        if self.creative {
            if self.server.destroy_delay == 0 {
                self.break_block(hit.pos)?;
                self.server.destroy_delay = 5;
            }
            return Ok(());
        }
        if self.server.destroy_delay > 0 {
            return Ok(());
        }
        if self.mining.as_ref().is_none_or(|m| m.pos != hit.pos) {
            self.attack_block(hit.pos)?;
            self.mining = Some(Mining {
                pos: hit.pos,
                progress: 0.0,
                per_tick: 0.0,
                ticks: 0,
                crack: None,
            });
        }
        let state = self.world.block(hit.pos[0], hit.pos[1], hit.pos[2]);
        if self.res.facts().get(state).hardness < 0.0 {
            // The server still counts its destroy ticks on an unbreakable block.
            let m = self.mining.as_mut().ok_or("mining")?;
            m.per_tick = 0.0;
            m.ticks += 1;
            let (pos, ticks) = (m.pos, m.ticks);
            self.destroy_progress_event(pos, hit.face, ticks);
            return Ok(());
        }
        let progress = self.destroy_progress(hit.pos);
        self.swing_arm();
        let m = self.mining.as_mut().ok_or("mining")?;
        m.per_tick = progress;
        m.progress += progress;
        m.ticks += 1;
        // An instant break at START sends no destroy events.
        if m.progress < 1.0 || m.ticks > 1 {
            let (pos, ticks) = (m.pos, m.ticks);
            self.destroy_progress_event(pos, hit.face, ticks);
        }
        let m = self.mining.as_mut().ok_or("mining")?;
        // ServerPlayerGameMode breaks a player's block at once only when it is instant (START's
        // insta-mine); otherwise it waits for the client's STOP_DESTROY_BLOCK
        // (`guest_stop_digging`), never breaking it on its own count. Breaking it here would
        // take the block from under the player's crack before the player's client predicts the
        // break, so it gets neither particles nor sound.
        if m.progress >= 1.0 && (self.server.lan_acting.is_none() || m.ticks == 1) {
            let pos = m.pos;
            self.mining = None;
            self.break_block(pos)?;
            self.server.destroy_delay = 5;
        }
        self.update_local_crack();
        Ok(())
    }
    pub(in crate::game) fn tick_sleep(&mut self) {
        self.sleep_timer = tick_sleep_timer(self.sleep_timer, self.sleeping.is_some());
        if let Some(head) = self.sleeping {
            // LivingEntity.tick's checkBedExists: a sleeper whose bed is
            // gone gets up.
            let bed = self.world.block(head[0], head[1], head[2]);
            let bed_gone = !self.res.facts().get(bed).name.ends_with("_bed");
            // Vanilla has no Shift wake-up: the chat screen's capitals must
            // not get the sleeper up.
            let wake_immediately = bed_gone
                || self
                    .player
                    .as_ref()
                    .is_none_or(|p| p.health.dead_or_dying());
            if wake_immediately || self.server_acting_sneak() {
                self.wake_up(wake_immediately, true);
                return;
            }
            // setPosToBed: the bed's top (`getSleepHeight`) and 0.125 above
            // it. A sleeper whose bed no longer gives a shape to lie on stays
            // where it is: vanilla only ever places a sleeper when it starts.
            let at = self.sleeping_position(head);
            if let (Some(p), Some(at)) = (self.player.as_mut(), at) {
                p.position = at;
                p.velocity = Vec3::ZERO;
                p.pitch = 0.0;
            }
            // LivingEntity.tick: a sleeper's xRot is 0.
            self.pitch = 0.0;
            self.snap_player_model_pitch();
        }
        // ServerLevel.tick: once enough of the players have slept 100 ticks,
        // skip to morning if `advance_time` is on, wake everyone, and
        // resetWeatherCycle if `advance_weather` is on and it is raining.
        if self.enough_slept() {
            if self
                .ui
                .rules
                .bool_at(sunrise_client::gamerules::vanilla::ADVANCE_TIME)
            {
                // SleepFinishedTimeEvent on the level's own clock (game/world/wake_time.rs): None
                // when a mod canceled, so `native_time_skip` is not asked, or no time fits.
                if let Some((clock, t, target)) = self.sleep_finished_move() {
                    let mut cancel = false;
                    self.native_time_skip(self.dim.id(), t, target, "sleep", &mut cancel);
                    if !cancel {
                        self.set_command_clock_time(&clock, target);
                    }
                }
            }
            // Level.isRaining: the rain level is above 0.2.
            if self
                .ui
                .rules
                .bool_at(sunrise_client::gamerules::vanilla::ADVANCE_WEATHER)
                && self.weather_levels(1.0).0 > 0.2
            {
                self.world.weather.raining = false;
                self.world.weather.thundering = false;
                self.world.weather.rain_time = 0;
                self.world.weather.thunder_time = 0;
            }
            if self.sleeping.is_some() {
                self.wake_up(false, false);
            }
            self.wake_guests();
            // SleepStatus.removeAllSleepers: waking everyone is not announced.
            self.server.sleep_seen.1 = 0;
        }
    }
    /// ServerPlayer.startSleepInBed: in reach, not obstructed, night or thunder,
    /// and (outside creative) no monsters within 8 blocks; the bed becomes the
    /// respawn point either way.
    pub(in crate::game::server_side) fn try_sleep(&mut self, pos: [i32; 3]) {
        if self.fake_blocks(mod_api::FakeEffect::Sleep) {
            return;
        }
        let Some((position, yaw)) = self.player.as_ref().map(|p| (p.position, p.yaw)) else {
            return;
        };
        let state = self.world.block(pos[0], pos[1], pos[2]);
        let props = self
            .pack
            .assets
            .registry
            .state(state)
            .map(|s| s.properties.clone())
            .unwrap_or_default();
        // The head and the foot: the block faces from the foot to the head.
        let d = facing_offset(props.get("facing").map_or("north", String::as_str));
        let (head, foot) = if props.get("part").map(String::as_str) == Some("foot") {
            ([pos[0] + d[0], pos[1], pos[2] + d[2]], pos)
        } else {
            (pos, [pos[0] - d[0], pos[1], pos[2] - d[2]])
        };
        // AbstractBedBlock.useWithoutItem: in an occupied bed a villager
        // asleep there is woken (kickVillagerOutOfBed), else someone else
        // sleeps in it; the player stays up either way.
        let head_state = self.world.block(head[0], head[1], head[2]);
        let occupied = self
            .pack
            .assets
            .registry
            .state(head_state)
            .is_some_and(|s| s.properties.get("occupied").is_some_and(|v| v == "true"));
        if occupied {
            if !self.kick_villager_out_of_bed(head) {
                self.tell_sleeper("block.minecraft.bed.occupied", "This bed is occupied", true);
            }
            return;
        }
        // ServerPlayer.bedInRange / isReachableBedBlock: within 3 blocks
        // sideways and 2 vertically of the head or the foot.
        let reachable = |bed: [i32; 3]| {
            (position.x - f64::from(bed[0]) - 0.5).abs() <= 3.0
                && (position.y - f64::from(bed[1])).abs() <= 2.0
                && (position.z - f64::from(bed[2]) - 0.5).abs() <= 3.0
        };
        let mut problem: Option<&'static str> = if !reachable(head) && !reachable(foot) {
            Some("too_far_away")
        } else {
            None
        };
        // ServerPlayer.bedBlocked: Player.freeAt (not suffocating) for the
        // blocks above the head and the foot, as `in_wall` reads suffocating.
        let blocked = problem.is_none()
            && [head, foot].iter().any(|bed| {
                let f = self
                    .res
                    .facts()
                    .get(self.world.block(bed[0], bed[1] + 1, bed[2]));
                f.full_collision && f.solid_render
            });
        if blocked {
            problem = Some("obstructed");
        }
        // In reach (bedInRange), day or night: setRespawnPosition(
        // RespawnConfig(the head, the player's yaw), not forced). It replaces
        // an anchor or a spawnpoint; respawning checks the bed is still there
        // and finds where to stand up (findRespawnAndUseSpawnBlock).
        // Straw beds never set it (`straw_bed_rule`).
        if problem.is_none() && self.bed_sets_spawn(state) {
            let config = respawn_anchor::RespawnConfig::wrapped(self.dim, head, yaw, false);
            // ServerPlayer.setRespawnPosition: the chat message only when the
            // point moved (`isSamePosition`).
            let moved = self
                .server
                .respawn_config
                .is_none_or(|c| (c.dim, c.pos) != (config.dim, config.pos));
            // `canSetSpawn` calls the setter every time, the same bed too (it
            // refreshes the angle).
            let cancel = self.spawn_point_change(self.dim, Some(head), false);
            if !cancel {
                self.server.respawn_config = Some(config);
                if moved {
                    self.tell_sleeper("block.minecraft.set_spawn", "Respawn point set", false);
                }
                // The fallback becomes the world spawn: a guest's is none of its
                // own, so it follows `setworldspawn`.
                self.spawn = if self.server.lan_acting.is_some() {
                    None
                } else {
                    self.level
                        .world_spawn
                        .map(|[x, y, z]| {
                            Vec3::new(f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5)
                        })
                        .or(self.spawn)
                };
            }
        }
        if problem.is_none() && !self.bed_can_sleep_now() {
            problem = Some("not_possible_now");
        }
        // Creative players skip the monster check (`!this.isCreative()`).
        if problem.is_none()
            && !self.creative
            && self.mobs.iter().any(|m| {
                // `Monster.class` entities that `isPreventingPlayerRest`: a
                // zombified piglin only while it is angry.
                sunrise_client::mobs::ai::is_monster(m.species())
                    && (m.species() != sunrise_client::mobs::Species::ZombifiedPiglin
                        || (m.extra.anger > 0 && m.ai.target.is_some()))
                    && m.alive()
                    && (m.position.x - position.x).abs() < 8.0
                    && (m.position.y - position.y).abs() < 5.0
                    && (m.position.z - position.z).abs() < 8.0
            })
        {
            problem = Some("not_safe");
        }
        // NeoForge's CanPlayerSleepEvent: a mod may clear or replace the
        // problem vanilla found, or add one.
        let mut problem = problem.map(str::to_owned);
        if let (Some(player), Some(name)) = (
            self.server.lan_acting_uuid,
            self.server.lan_acting_name.clone(),
        ) {
            self.native_can_player_sleep(player, &name, head, &mut problem);
        }
        if let Some(problem) = problem {
            match problem.as_str() {
                "too_far_away" => self.tell_sleeper(
                    "block.minecraft.bed.too_far_away",
                    "You may not rest now; the bed is too far away",
                    true,
                ),
                "obstructed" => self.tell_sleeper(
                    "block.minecraft.bed.obstructed",
                    "This bed is obstructed",
                    true,
                ),
                "not_safe" => self.tell_sleeper(
                    "block.minecraft.bed.not_safe",
                    "You may not rest now; there are monsters nearby",
                    true,
                ),
                "not_possible_now" => self.tell_sleeper(
                    "block.minecraft.bed.no_sleep",
                    "You can sleep only at night or during thunderstorms",
                    true,
                ),
                // `other_problem` and any name a mod invents: refused without a message.
                _ => {}
            }
            return;
        }
        // ServerPlayer.startSleepInBed: a percentage above 100 cannot skip the night.
        if self
            .ui
            .rules
            .int_at(sunrise_client::gamerules::vanilla::PLAYERS_SLEEPING_PERCENTAGE)
            > 100
        {
            self.tell_sleeper(
                "sleep.not_possible",
                "No amount of rest can pass this night",
                true,
            );
        }
        if let (Some(player), Some(name)) = (
            self.server.lan_acting_uuid,
            self.server.lan_acting_name.clone(),
        ) {
            let mut cancel = false;
            self.native_player_bed_enter(player, &name, head, &mut cancel);
            if cancel {
                return;
            }
        }
        if self.lie_in_bed(head) {
            let bed = self.world.block(head[0], head[1], head[2]);
            self.award_custom_stat(self.slept_in_bed_stat(bed), 1);
            self.trigger_player("minecraft:slept_in_bed");
            self.stats.reset("custom", "minecraft:time_since_rest");
        }
    }
    /// `BlockTransformer.transformBlock` for a held axe, hoe or shovel.
    pub(in crate::game) fn use_block_transformer(
        &mut self,
        stack: &ItemStack,
        hit: &Hit,
    ) -> Result<bool> {
        let pos = hit.pos;
        let face = hit.face;
        let Some((new, sound, loot, damage, particle, transform_type)) =
            self.block_transform(stack, hit, self.server_acting_sneak())
        else {
            return Ok(false);
        };
        // BlockTransformer.transformBlock: ITEM_USED_ON_BLOCK before the swap.
        self.trigger_block("minecraft:item_used_on_block", pos, stack);
        // A loot table drops on the clicked face, before the block changes.
        if loot.as_deref() == Some("minecraft:till/rooted_dirt") {
            self.pop_resource_from_face(pos, face, ItemStack::new("minecraft:hanging_roots", 1));
        }
        let old = self.world.block(pos[0], pos[1], pos[2]);
        self.level_set(pos, new)?;
        // BlockTransformer.TransformParticle.send.
        self.transform_particle(&particle, pos);
        // `Level.playSound(player, ...)`: the actor's client already played
        // the strip/scrape/till sound in `predict_item_on_block` (#4681).
        let at = [
            f64::from(pos[0]) + 0.5,
            f64::from(pos[1]) + 0.5,
            f64::from(pos[2]) + 0.5,
        ];
        if self.server.lan_acting.is_some() {
            self.server
                .guest_player_sounds
                .push((sound.clone(), at, 1.0, 1.0));
        } else {
            self.server_play_sound(&sound, at, 1.0, 1.0);
        }
        // ItemStack.useOn: a use that acted counts as one of the item.
        self.award_stat("used", &stack.item, 1);
        self.damage_held(damage);
        self.post_block_game_event("minecraft:block_change", pos, self.sculk_actor(), Some(new));
        if transform_type == "copper_chest" {
            self.transform_copper_chest_neighbour(pos, old, &particle);
        }
        Ok(true)
    }
    /// Block tool use: distinguish success, failure and fall-through for hand feedback.
    pub(in crate::game::server_side) fn use_tool(
        &mut self,
        stack: &ItemStack,
        allow_block_actions: bool,
    ) -> Result<sunrise_client::first_person::UseResult> {
        use sunrise_client::first_person::UseResult;
        let item = stack.item.trim_start_matches("minecraft:").to_owned();
        let at = |p: [i32; 3]| {
            [
                f64::from(p[0]) + 0.5,
                f64::from(p[1]) + 0.5,
                f64::from(p[2]) + 0.5,
            ]
        };
        // BucketItem has no useOn: a guest's UseItemOn leaves it to UseItem.
        let bucket = sunrise_client::items::bucket_contents(&stack.item);
        if matches!(self.server.guest_use, Some(GuestUse::OnBlock { .. }))
            && (item == "bucket" || bucket.is_some())
        {
            return Ok(UseResult::Pass);
        }
        if item == "bucket" {
            let Some(hit) = self.bucket_hit() else {
                return Ok(UseResult::Pass);
            };
            // `BucketItem.use`: `level.mayInteract` and `mayUseItemAt`.
            if self.bucket_use_refused(hit.pos) {
                return Ok(UseResult::Fail);
            }
            let mut canceled = false;
            self.native_fill_bucket(stack, hit.pos, &mut canceled);
            if canceled {
                return Ok(UseResult::Fail);
            }
            if self.is_powder_snow(hit.pos) {
                self.pick_up_powder_snow(hit.pos)?;
                return Ok(UseResult::Success { swing: true });
            }
            // `BucketPickup.pickupBlock`: a fluid source, a waterlogged block
            // or powder snow. A block that gives nothing fails the click.
            let pos = BlockPos::new(hit.pos[0], hit.pos[1], hit.pos[2]);
            let creative = self.creative;
            let Some(filled) =
                self.with_level(|b, level, ticks| b.bucket_pickup(level, ticks, pos, creative))
            else {
                return Ok(UseResult::Fail);
            };
            self.server_process_level_events()?;
            // `player.awardStat(Stats.ITEM_USED.get(this))`,
            // `getPickupSound` and `level.gameEvent(player, FLUID_PICKUP)`.
            self.award_stat("used", "minecraft:bucket", 1);
            let sound = if filled == "minecraft:lava_bucket" {
                "minecraft:item.bucket.fill_lava"
            } else {
                "minecraft:item.bucket.fill"
            };
            self.server_play_sound(sound, at(hit.pos), 1.0, 1.0);
            self.post_block_game_event(
                "minecraft:fluid_pickup",
                hit.pos,
                Some(sunrise_client::mobs::Who::Player),
                None,
            );
            let filled = ItemStack::new(filled, 1);
            self.filled_result(filled.clone());
            self.trigger_item("minecraft:filled_bucket", &filled);
            return Ok(UseResult::Success { swing: true });
        }
        // BucketItem.use has its own ray, even when the crosshair hits an entity.
        if !allow_block_actions && bucket.is_none() {
            return Ok(UseResult::Pass);
        }
        let hit = if bucket.is_some() {
            self.item_pick()
        } else {
            self.pick()
        };
        let Some(hit) = hit else {
            return Ok(UseResult::Pass);
        };
        let clicked = self.world.block(hit.pos[0], hit.pos[1], hit.pos[2]);
        if let Some((content, sound, mob)) = bucket {
            if self.bucket_use_refused(hit.pos) {
                return Ok(UseResult::Fail);
            }
            let target = self.bucket_target(content, &hit);
            let mut canceled = false;
            self.native_fill_bucket(stack, target, &mut canceled);
            // `mayUseItemAt(blockPos2)`: a block that refuses the fluid passes
            // it to the block beside, so a mod hears that block too (powder
            // snow places like a block and never goes there).
            let beside_too = content != BucketContent::Powder && hit.beside() != target;
            if !canceled && beside_too {
                self.native_fill_bucket(stack, hit.beside(), &mut canceled);
            }
            if canceled
                || self.bucket_empty_cancelled(stack, target)
                || beside_too && self.bucket_empty_cancelled(stack, hit.beside())
            {
                return Ok(UseResult::Fail);
            }
            if let Some(result) = self.player_empty_contents(stack, target, &hit) {
                return Ok(result);
            }
            let pos = BlockPos::new(target[0], target[1], target[2]);
            let [x, y, z] = hit.beside();
            let beside = Some(BlockPos::new(x, y, z));
            // `Player.isShiftKeyDown`: a block that refuses the fluid (any
            // non-air block under Shift) passes it to the block beside, where
            // Shift no longer counts.
            let sneaking = self.server_acting_sneak();
            let emptied = self.empty_player_bucket(pos, beside, content, sound, sneaking);
            self.server_process_level_events()?;
            let Some(emptied) = emptied else {
                return Ok(UseResult::Fail);
            };
            if self.dimension_type().water_evaporates && content.is_water() {
                self.evaporation_smoke([emptied.x, emptied.y, emptied.z]);
            }
            // `BucketItem.emptyContents` uses `playSound(player, …)`: the
            // actor's client already played the empty / evaporate sound from
            // its DryLevel prediction (#4684).
            self.keep_guest_bucket_sounds(sound);
            if let Some(kind) = mob {
                if let Some(id) = self.spawn_mob_at(kind, (target[0], target[1], target[2]), true) {
                    self.mob_placed(id, stack)?;
                }
                self.post_block_game_event(
                    "minecraft:entity_place",
                    target,
                    self.sculk_actor(),
                    None,
                );
            }
            self.trigger_block("minecraft:placed_block", target, stack);
            self.bucket_emptied(&stack.item);
            return Ok(UseResult::Success { swing: true });
        }
        if self.may_use_on(hit.pos) && self.use_block_transformer(stack, &hit)? {
            return Ok(UseResult::Success { swing: true });
        }
        if self.may_use_on(hit.pos) && item == "honeycomb" {
            let registry = self.pack.assets.registry;
            let old = registry.state(clicked);
            if let Some(state) = self.honeycomb_waxed(clicked) {
                let other = old.and_then(|old| storage::chest_connected_pos(old, hit.pos));
                // HoneycombItem.useOn: ITEM_USED_ON_BLOCK before the swap.
                self.trigger_block("minecraft:item_used_on_block", hit.pos, stack);
                self.level_set(hit.pos, state)?;
                let source = self
                    .server
                    .lan_acting_uuid
                    .map(sunrise_world::level::LevelEventPlayer);
                self.level_event_from(source, 3003, hit.pos, 0);
                self.wax_on_sound(at(hit.pos));
                // HoneycombItem.useOn's game event, and ItemStack.useOn's used statistic.
                self.post_block_game_event(
                    "minecraft:block_change",
                    hit.pos,
                    self.sculk_actor(),
                    Some(state),
                );
                // A double chest's other half: its game event and particles, and
                // a second sound from the clicked block.
                if let Some(other) = other {
                    let now = self.world.block(other[0], other[1], other[2]);
                    self.post_block_game_event(
                        "minecraft:block_change",
                        other,
                        self.sculk_actor(),
                        Some(now),
                    );
                    self.server_level_event_at(3003, other, 0);
                    self.wax_on_sound(at(hit.pos));
                }
                self.award_stat("used", &stack.item, 1);
                if !self.creative {
                    self.consume_held();
                }
                return Ok(UseResult::Success { swing: true });
            }
        }
        if item == "flint_and_steel" || item == "fire_charge" {
            if !self.may_build() && !self.can_place_on_in_adventure(hit.pos) {
                return Ok(UseResult::Fail);
            }
            let name = self.res.facts().get(clicked).name.clone();
            if self.can_light(clicked, &name) {
                let registry = self.pack.assets.registry;
                let mut state = registry
                    .state(clicked)
                    .cloned()
                    .ok_or("missing lit block state")?;
                state.properties.insert("lit".into(), "true".into());
                self.level_set(hit.pos, registry.resolve(&state)?)?;
                let sound = if item == "fire_charge" {
                    "minecraft:item.firecharge.use"
                } else {
                    "minecraft:item.flintandsteel.use"
                };
                let pitch = sunrise_client::items::fire_starter_pitch(
                    item == "fire_charge",
                    &mut self.world.random,
                );
                self.play_block_use_sound(sound, at(hit.pos), pitch);
                // FlintAndSteelItem / FireChargeItem.useOn: BLOCK_CHANGE at the lit block,
                // and ItemStack.useOn's used statistic.
                self.post_block_game_event(
                    "minecraft:block_change",
                    hit.pos,
                    self.sculk_actor(),
                    None,
                );
                self.award_stat("used", &stack.item, 1);
                if item == "fire_charge" {
                    self.consume_held();
                } else {
                    self.damage_held(1);
                }
                return Ok(UseResult::Success { swing: true });
            }
            let target = [
                hit.pos[0] + hit.face[0],
                hit.pos[1] + hit.face[1],
                hit.pos[2] + hit.face[2],
            ];
            if self.can_place_used_fire(target) {
                // With mods, the fire goes in and `BaseFireBlock.onPlace`
                // fills the frame, as vanilla's order is, so that
                // `portal_spawn` is asked once, where vanilla asks it.
                if !mod_api::__private::mods().is_empty() || !self.try_light_portal(target)? {
                    self.place_used_fire(target)?;
                }
                let sound = sunrise_client::items::fire_starter_sound(item == "fire_charge");
                let pitch = sunrise_client::items::fire_starter_pitch(
                    item == "fire_charge",
                    &mut self.world.random,
                );
                self.play_block_use_sound(sound, at(target), pitch);
                // BLOCK_PLACE: flint and steel names the clicked block, a fire charge the
                // fire. Then ItemStack.useOn's used statistic.
                let placed_at = if item == "fire_charge" {
                    target
                } else {
                    hit.pos
                };
                self.post_block_game_event(
                    "minecraft:block_place",
                    placed_at,
                    self.sculk_actor(),
                    None,
                );
                self.award_stat("used", &stack.item, 1);
                if item == "fire_charge" {
                    self.consume_held();
                } else {
                    self.damage_held(1);
                }
                return Ok(UseResult::Success { swing: true });
            }
            return Ok(UseResult::Fail);
        }
        Ok(UseResult::Pass)
    }
    /// `Player.stopSleepInBed`: preserve the flags of the owning wake caller.
    pub(in crate::game) fn wake_up(&mut self, wake_immediately: bool, update_level: bool) {
        // PlayerWakeUpEvent: heard at the start of stopSleepInBed, still in bed;
        // a listener may have woken the player, so the sleep is read after it.
        self.native_acting_bed_leave(wake_immediately, update_level);
        let Some(head) = self.sleeping.take() else {
            return;
        };
        self.sleep_timer = if wake_immediately { 0 } else { 100 };
        // The active guest's live body wakes here; the hub owns its wake
        // animation and sleeping metadata, not the suspended outer actor.
        if let Some(i) = self
            .server
            .lan_acting
            .and_then(|conn| self.guest_index(conn))
            && let Some(host) = self.lan.as_mut()
        {
            // The live turn may have changed its bed since entry.
            host.hub.guests.update(i, |guest| guest.bed = Some(head));
            host.hub.wake(i, wake_immediately);
            // This live body applies the stand-up pose below, not the later
            // wake_woken_guests pass over stored guest bodies.
            host.hub.guests.update(i, |guest| guest.woke_from = None);
        }
        let stand = self.bed_wake_pose(head, self.yaw);
        if let Some(p) = self.player.as_mut() {
            if let Some((at, yaw)) = stand {
                p.position = at;
                p.velocity = Vec3::ZERO;
                p.yaw = yaw;
                p.pitch = 0.0;
                p.teleported = true;
                p.teleported_delta = [false; 3];
            }
            let _ = p.set_sleeping(false);
        }
        if let Some((at, yaw)) = stand {
            self.yaw = yaw;
            self.pitch = 0.0;
            if self.server.lan_acting.is_none() {
                self.previous_position = at;
                self.snap_player_model_pitch();
            }
        }
    }
    /// `HoneycombItem.useOn`'s `Level.playSound(player, ...)`: the actor's client
    /// already played it in `predict_item_on_block`.
    pub(in crate::game::server_side) fn wax_on_sound(&mut self, at: [f64; 3]) {
        let sound = "minecraft:item.honeycomb.wax_on";
        if self.server.lan_acting.is_some() {
            self.server
                .guest_block_sounds
                .push((sound.to_owned(), at, 1.0, 1.0));
        } else {
            self.server_play_sound(sound, at, 1.0, 1.0);
        }
    }
    pub(in crate::game) fn with_level_access<R>(
        &mut self,
        placement_preview: bool,
        f: impl FnOnce(&Behaviours, &mut GameLevel<'_>, &mut Ticks) -> R,
    ) -> R {
        let darken = self.sky_darken();
        let (rain, thunder) = self.weather_levels(1.0);
        let (
            player_box,
            player_collision,
            cart_boxes,
            arrow_boxes,
            guest_boxes,
            building_player_boxes,
            fire_players,
            trigger_hooks,
            trigger_extras,
            open_containers,
            golem_chests,
        ) = if placement_preview {
            Default::default()
        } else {
            // Spectators ignore block triggers (Entity.isIgnoringBlockTriggers).
            let spectator = self.spectator();
            let player_box = self.player.as_ref().filter(|_| !spectator).map(|p| {
                let w = f64::from(p.dimensions.width) / 2.0;
                (
                    [p.position.x - w, p.position.y, p.position.z - w],
                    [
                        p.position.x + w,
                        p.position.y + f64::from(p.dimensions.height),
                        p.position.z + w,
                    ],
                )
            });
            let player_collision = self.player.as_ref().filter(|_| !spectator).map(|p| {
                let main_hand = self.inventory.get(self.selected).and_then(Option::as_ref);
                PlayerCollision {
                    y: p.position.y,
                    descending: p.velocity.y < 0.0,
                    fall_distance: p.fall_distance,
                    main_hand_item: main_hand.map_or_else(
                        || Arc::from("minecraft:air"),
                        |stack| Arc::clone(&stack.item),
                    ),
                    main_hand_count: main_hand.map_or(0, |stack| stack.count as i32),
                    leather_boots: worn_boots::leather_boots(&self.armor),
                }
            });
            let cart_boxes = self
                .carts
                .iter()
                .map(|c| {
                    let p = c.position;
                    (
                        [p.x - 0.49, p.y, p.z - 0.49],
                        [p.x + 0.49, p.y + 0.7, p.z + 0.49],
                    )
                })
                .collect();
            let arrow_boxes = self
                .creatures
                .projectiles
                .iter()
                .filter(|p| p.arrow.is_some())
                .map(|p| p.position)
                .chain(self.tridents.iter().map(|t| t.position))
                .map(|at| {
                    let b = sunrise_client::arrows::arrow_bounds(at);
                    (b.min_position().components(), b.max_position().components())
                })
                .collect();
            let guest_boxes = self.lan_block_trigger_boxes();
            let building_player_boxes = self
                .server
                .lan_acting
                .is_some()
                .then(|| self.lan_building_boxes.0.boxes(self.dim));
            let fire_players: Vec<[f64; 3]> = self
                .player
                .iter()
                .filter(|_| !spectator)
                .map(|p| [p.position.x, p.position.y, p.position.z])
                .chain(self.lan.iter().flat_map(|h| {
                    h.hub
                        .guests
                        .simulation_iter()
                        .filter(|g| h.hub.here(g) && !g.spectator())
                        .map(|g| g.position)
                }))
                .collect();
            let mut trigger_hooks: Vec<(Aabb, bool)> = self
                .hook
                .iter()
                .map(|h| (h.bounds(), h.applies_block_effects()))
                .collect();
            self.lan_hook_triggers(&mut trigger_hooks);
            // EntityType.LIGHTNING_BOLT: 0 x 0.
            let trigger_extras: Vec<Aabb> = self
                .bolts
                .iter()
                .map(|b| {
                    let at = Vec3::new(b.at[0], b.at[1], b.at[2]);
                    Aabb::new(at, at)
                })
                .collect();
            // `ContainerOpenersCounter.getEntitiesWithContainerOpen`: the players
            // whose menu still holds a container of the block at a position. A
            // double chest answers for both halves (`ChestBlockEntity$1` takes a
            // `CompoundContainer`), and a spectator's menu counts for nothing.
            let mut open_containers: Vec<(i32, i32, i32)> = Vec::new();
            for key in self.container.into_iter().filter(|_| !spectator).chain(
                self.lan
                    .iter()
                    .flat_map(|h| {
                        h.hub
                            .guests
                            .simulation_iter()
                            .filter(|g| h.hub.here(g) && !g.spectator())
                    })
                    .filter_map(|g| g.menu.as_ref().and_then(|m| m.pos)),
            ) {
                open_containers.push(key);
                if let Some(other) = self.chest_other_half(key) {
                    open_containers.push(other);
                }
            }
            // CopperGolem.hasContainerOpen: the chest a golem opened, and its other half.
            let mut golem_chests: Vec<(i32, i32, i32)> = Vec::new();
            for &key in self.world.chest_openers.keys() {
                golem_chests.push(key);
                golem_chests.extend(self.chest_other_half(key));
            }
            (
                player_box,
                player_collision,
                cart_boxes,
                arrow_boxes,
                guest_boxes,
                building_player_boxes,
                fire_players,
                trigger_hooks,
                trigger_extras,
                open_containers,
                golem_chests,
            )
        };
        let dimension = self.dim.level();
        let rules = self.level_rules();
        let native_authoritative = self.native_authoritative();
        let liquid_transaction_open = self.has_level_transaction();
        let client_side = !self.is_logical_server();
        let environment = self.res.environment_of(&self.dimension_type().id);
        let other_entities = entity::entity_lists_of!(self, without_carts);
        let mut level = GameLevel {
            cake_probe: false,
            cake_requested: std::cell::Cell::new(false),
            cake_answer: None,
            api_level: if native_authoritative {
                self.level(self.dim.id())
            } else {
                self.native_client_level()
            },
            part_state_index: self.attachment_indexes.get(&self.dim),
            attachment_query_scopes: &self.transactions.attachment_query_scopes,
            command_block_transition: None,
            positions: piston_carry_state::PositionState {
                gateways: &mut self.be_anims.gateway_servers,
                hoppers: &mut self.server.hoppers,
            },
            border: self.border.geometry(),
            redstone_experiments: self
                .level
                .enabled_features
                .iter()
                .any(|feature| feature == "minecraft:redstone_experiments"),
            cart_boxes,
            item_frames: &self.item_frames,
            carts: &mut self.carts,
            arrow_boxes,
            other_entities,
            crystals: &self.crystals,
            trigger_hooks,
            trigger_extras,
            world: &mut self.world,
            facts: self.res.facts(),
            tables: native_block_tables::BlockTables::new(&self.res, &self.pack),
            collision: self.collision,
            events: &mut self.level_events,
            dimension,
            darken,
            environment,
            raining: rain > 0.2,
            thundering: thunder > 0.9,
            enabled_features: self
                .remote
                .as_ref()
                .map_or(self.level.enabled_features.as_arc(), |remote| {
                    &remote.features
                }),
            player_box,
            guest_boxes,
            building_player_boxes,
            player_collision,
            biomes: world_biomes!(self),
            biome_zoom: self.streaming.biome_zoom,
            seed: self.level.seed,
            behaviours: self.res.behaviours(),
            items: world_items!(self),
            recipes: world_recipes!(self),
            // A client rolls no loot (LootTable.getRandomItems needs a
            // ServerLevel): it doesn't load the tables on its game thread.
            loot: if self.remote.is_some() {
                None
            } else {
                self.server_loot.0.get()
            },
            loot_storage: self.score.storage.snapshot(),
            registry: self.pack.assets.registry,
            block_registries: &self.pack.block_registries,
            rules,
            difficulty: i32::from(self.ui.difficulty),
            fire_players,
            rand_sequences: &mut self.server.rand_sequences,
            pending: &mut self.pending,
            spill: &mut self.spill.0,
            tickets: self.tickets.0.levels(),
            random_tick_sections: &mut self.random_tick_sections.0,
            lid_openers: &mut self.server.lid_openers,
            open_containers: &open_containers,
            golem_chests: &golem_chests,
            client_side,
            native_authoritative,
            native_player: self.server.lan_acting_uuid,
            native_player_entity: self.server.lan_acting_entity.map(|id| id as i32),
            liquid_transaction_open,
            debug: self.server.world_debug,
            native_sounds: &mut self.server.pending_native_sounds,
            loop_context: &self.loop_context,
            native_particles: &mut self.server.pending_particles,
            sound_seeds: self.lan.as_ref().map(|host| &host.hub),
            drop_tool: None,
            drop_entity: None,
            interacting: None,
            location: location_world::LocationRefs {
                dimension: self.dim.id(),
                end: &self.end,
                tags: advancements::TagSources {
                    pack_tags: self.world_tables.0.as_ref().map(|t| &t.pack_tags),
                    items: world_items!(self),
                    mob_data: self.res.mob_data(),
                    damage: &self.health_resources.damage,
                    resources: &self.options.resources,
                },
            },
            drop_breaker_at: None,
            drop_into: None,
        };
        f(self.res.behaviours(), &mut level, &mut self.block_ticks)
    }
}

#[cfg(test)]
pub(in crate::game) mod nested_orb_spawn_tests {
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::nested_orb_spawn_tests::*;

    #[test]
    #[ignore = "daily"]
    fn experience_pickup_keeps_nested_orbs_until_the_next_ordered_pass() {
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)),
            );
            game.block_ticks
                .add_empty_chunk(sunrise_core::position::ChunkPos::new(0, 0))
                .unwrap();
            for (x, value) in [(7.5, 1), (9.5, 3), (11.5, 7)] {
                game.add_orb([x, 80.0, 3.5], value);
            }
            let _ = game.orbs.remove(1);
            let mut expected: Vec<_> = game.orbs.iter().map(|o| (o.id, o.join)).collect();
            crate::game::entity::taken_pass_tests::with_spawn(
                |g| g.add_orb([3.5, 80.0, 3.5], 1),
                || game.tick_experience(),
            );
            assert_eq!(game.orbs.len(), 3);
            let join = game.orbs[2].join;
            expected.push((game.orbs[2].id, join));
            assert_eq!(
                game.orbs.iter().map(|o| (o.id, o.join)).collect::<Vec<_>>(),
                expected
            );
            assert_eq!(game.orbs[2].age, 0);
            game.server.ordered.announced = sunrise_client::join_seq::JoinSeq::MAX;
            game.tickets.0.update(None);
            game.begin_ordered_ticks();
            game.run_ordered_ticks(sunrise_client::join_seq::JoinSeq::MAX, false)
                .unwrap();
            let spawned = game.orbs.iter().find(|o| o.join == join).unwrap();
            assert_eq!(spawned.age, 1);
            assert_eq!(
                game.orbs.iter().map(|o| (o.id, o.join)).collect::<Vec<_>>(),
                expected
            );
        });
    }
}
