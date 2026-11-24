#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::tick::*;

impl Game<'_> {
    /// The visit of each entity of the client level whose kind is told no tick
    /// (`tick_remote_entities`): its `Entity.commonTick` counts it here. A frozen level visits
    /// only the players and what carries them.
    pub(in crate::game) fn count_untold_client_ticks(
        &mut self,
        running: bool,
        frozen_ticking: &[i32],
    ) {
        let Some(mut r) = self.remote.take() else {
            return;
        };
        let flying: sunrise_client::fast_hash::FxHashSet<u32> = self
            .creatures
            .projectiles
            .iter()
            .map(|p| p.id)
            .chain(self.tridents.iter().map(|t| t.id))
            .collect();
        let told = |id: i32| {
            let id = id as u32;
            self.mob_index.get_ref(&self.mobs, id).is_some()
                || self.text_displays.0.contains_key(&(id as i32))
                || flying.contains(&id)
        };
        if running {
            for (&id, t) in r.entities.iter_mut() {
                if !told(id) {
                    t.tick_count = t.tick_count.wrapping_add(1);
                }
            }
        } else {
            for &id in frozen_ticking {
                if !told(id)
                    && let Some(t) = r.entities.get_mut(&id)
                {
                    t.tick_count = t.tick_count.wrapping_add(1);
                }
            }
        }
        self.remote = Some(r);
    }
    /// `ClientPacketListener.tick`'s `LevelLoadTracker.tickClientLoad`: the
    /// player's section is shown once the renderer has its mesh and it is 30%
    /// faded in (without a renderer: once its column and the 8 around it are
    /// lit, then the same fade); when the level is
    /// ready the loading screen closes and the server hears `PlayerLoaded`
    /// (`notifyPlayerLoaded`).
    pub(in crate::game) fn tick_client_load(&mut self) {
        let Some(player) = self.player.as_ref() else {
            return;
        };
        if !self
            .remote
            .as_ref()
            .is_some_and(|r| r.level_load.is_active())
        {
            return;
        }
        let cam_pos = self.visual_camera_position().unwrap_or([
            player.position.x,
            player.position.y + f64::from(self.eye),
            player.position.z,
        ]);
        let camera = cam_pos.map(|v| v.floor() as i32);
        let camera_y = camera[1];
        // `Util.toMillis(chunkSectionFadeInTime)`.
        let fade_ms = sunrise_client::gfx::fade_millis(self.settings.chunk_fade as f32);
        // `LevelRenderer`'s playerCompiledSectionCallback, run once
        // `isSectionCompiledAndVisible(camera)`: the renderer's own mesh and fade
        // for the camera's section. Without a renderer, the section counts as
        // compiled once `LevelExtractor` would compile it
        // (`SectionUpdateTracker.hasAllNeighbors`: its column and the 8 around
        // it lit, as meshing waits for here).
        let shown = self
            .renderer
            .0
            .as_ref()
            .map(|renderer| renderer.section_compiled_and_visible(camera, fade_ms));
        let (cx, cz) = (camera[0] >> 4, camera[2] >> 4);
        let neighbours_lit = shown.is_none()
            && (-1..=1).all(|dx| {
                (-1..=1).all(|dz| {
                    self.streaming
                        .is_lit(cx.wrapping_add(dx), cz.wrapping_add(dz))
                })
            });
        let shape = self.world.shape;
        let check = sunrise_client::level_loading::PlayerCheck {
            outside_build_height: camera_y < shape.min_y || camera_y >= shape.min_y + shape.height,
            spectator: player.spectator,
            alive: player.health.health() > 0.0,
        };
        let now = client_millis();
        // LevelLoadingScreen.tick.
        self.tick_load_progress();
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        if r.level_load.wants_player_section() {
            match shown {
                Some(true) => r.level_load.player_section_ready(),
                Some(false) => {}
                None => r.level_load.player_section_compiled(
                    [cx, camera_y >> 4, cz],
                    neighbours_lit,
                    now,
                    fade_ms,
                ),
            }
        }
        r.level_load.tick_client_load(now, check);
        if !r.level_load.is_level_ready(now) {
            return;
        }
        r.level_load.finish();
        if !r.loaded_sent {
            r.loaded_sent = true;
            self.send(S::PlayerLoaded(Empty));
        }
        // LevelLoadingScreen.onClose.
        self.say_system_now(&sunrise_client::text::Text::translate(
            "narrator.ready_to_play",
            &[],
        ));
        self.capture_loaded_world();
    }
    /// `LocalPlayer.aiStep` (877-883): water vision reads the local body's `Entity.baseTick`
    /// `isEyeInFluid(FluidTags.WATER)` sample and `this.isSpectator()`.
    pub(in crate::game) fn tick_local_water_vision(&mut self) {
        let in_water = self
            .player
            .as_ref()
            .is_some_and(|p| p.is_eye_in_fluid(sunrise_client::motion::WATER));
        self.client
            .fog
            .tick(in_water, self.own_player_is_spectator());
    }
    /// `ClientLevel.tickEntities`: the section index takes each crossing at its entity's turn.
    pub(in crate::game) fn tick_remote_entities(&mut self, running: bool) {
        self.client
            .client_pick
            .sections
            .get_mut()
            .begin_entity_ticks();
        self.tick_remote_entity_turns(running);
        self.client
            .client_pick
            .sections
            .get_mut()
            .end_entity_ticks();
    }
    /// ClientLevel.tickEntities; a frozen level only ticks the players
    /// (`TickRateManager.isEntityFrozen`).
    /// `GuardianRenderer.getEntityToLookAt`: a guardian's eye follows its
    /// synched attack target, else the camera.
    pub(in crate::game) fn tick_remote_guardian_eyes(&mut self, running: bool) {
        use sunrise_client::mobs::{Species, Who, guardian};
        if !running {
            return;
        }
        let camera = self.camera_eye_position();
        let mut looks = std::mem::take(&mut self.client.guardian_look_scratch);
        looks.clear();
        looks.extend(
            self.mobs
                .iter()
                .enumerate()
                .filter(|(_, m)| {
                    matches!(m.species(), Species::Guardian | Species::ElderGuardian) && m.alive()
                })
                .filter_map(|(i, m)| {
                    let look = match m.ai.target.filter(|_| m.extra.flag) {
                        Some(Who::Mob(id)) => self
                            .mobs
                            .iter()
                            .find(|t| t.id == id)
                            .map(|t| [t.position.x, t.eye_y(), t.position.z]),
                        _ => camera,
                    };
                    look.map(|look| (i, look))
                }),
        );
        for &(i, look) in &looks {
            guardian::look_at(&mut self.mobs[i], look);
        }
        self.client.guardian_look_scratch = looks;
    }
    /// The local player's `LocalPlayer.tick`, once the client has loaded. The
    /// input `sendChanges` sends after the block entities ticked, if it runs.
    pub(in crate::game) fn tick_remote_own_player(&mut self) -> Result<Option<u8>> {
        // ClientLevel.tickEntities skips a removed entity. Keep the last player state
        // for LevelExtractor.extractPlayerState, which still sees it on the death screen.
        let removed = self.local_player_removed_by_death();
        // ClientLevel.tickNonPassenger's Entity.commonTick, before LocalPlayer.tick: the admitted
        // player counts this tick whether it has its body yet, has loaded, sleeps or rides.
        if let Some(r) = self
            .remote
            .as_mut()
            .filter(|r| r.has_logical_player && !removed)
        {
            r.own_tick_count = r.own_tick_count.wrapping_add(1);
        }
        if self.player.is_none() || removed {
            return Ok(None);
        }
        // LocalPlayer.tick and sendChanges run once the client has loaded (hasClientLoaded),
        // wherever the player is: a chunk that hasn't come yet has no blocks here, as vanilla's
        // empty chunk has none, so the player falls through it until the chunk comes.
        if !self.remote.as_ref().is_some_and(|r| r.loaded_sent) {
            return Ok(None);
        }
        self.client.drop_spam_throttler.tick();
        let entity_collisions = self.entity_collisions(Mover::Player);
        // Those added before it tick before it: their pushes reach this move.
        let pushed = self.remote_pushes(true);
        let sprint_window = self.settings.sprint_window;
        // The hook endpoints bracket Player.tick only, not the level pass before it.
        let player = (mod_api::__private::listened(mod_api::HookId::PlayerTickPre)
            || mod_api::__private::listened(mod_api::HookId::PlayerTickPost))
        .then(|| self.local_player())
        .flatten();
        // The entities that tick before the player pushed it before its tick began, so
        // before Pre.
        self.apply_remote_push(pushed);
        // The flag a glide starts from is the one before the hook, so a listener that starts
        // the glide from `player_tick_pre` is reported as this tick's start.
        let fall_flying_before_hooks = self.player.as_ref().is_some_and(|p| p.fall_flying);
        if let Some(player) = player {
            self.native_player_tick_pre(mod_api::__private::PlayerTickPreCall { player });
        }
        self.native_local_player_fit_sizes();
        let result = self.tick_local_player(
            entity_collisions,
            sprint_window,
            Some(fall_flying_before_hooks),
        );
        // `EntityEvent.Size` for the pose the tick set.
        self.native_local_player_size();
        if let Some(player) = player {
            self.native_player_tick_post(mod_api::__private::PlayerTickPostCall { player });
        }
        // The entities added after the player tick after it, so after Post: `Entity.push`
        // adds each to the player's deltaMovement during their tick, from where they and
        // the player ended it. A velocity or teleport the server sends before the next
        // tick replaces that deltaMovement, push included
        // (`ClientPacketListener.handleSetEntityMotion`, `handleMovePlayer`), so the push
        // lands on the velocity now, not on the next move.
        let after = self.remote_pushes(false);
        self.apply_remote_push(after);
        result
    }
    pub(in crate::game) fn tick_remote_player(&mut self, running: bool, hold: bool) -> Result<()> {
        // Inventory.tick's ItemStack.inventoryTick: the pop time runs down.
        if !hold {
            for pop in &mut self.client.hotbar_pop {
                *pop = pop.saturating_sub(1);
            }
        }
        // ClientLevel.tick: the border's size change, while the level runs.
        if running {
            self.border.tick();
        }
        // ClientLevel.tick: the End's sky flash, frozen or not.
        self.tick_end_flash();
        self.player_border();
        self.tick_remote_entities(running);
        self.tick_remote_arm_poses();
        self.tick_random_textures();
        // Breeze.tick's block dust: drawn by each client for the breezes it sees.
        if running {
            self.tick_breeze_effects();
        }
        self.tick_remote_guardian_eyes(running);
        self.tick_hand();
        if running {
            self.tick_particle_sources();
        } else if self.clock.still() {
            self.settle_rain_fog();
        }
        self.tick_frame_visuals(running);
        // Under `SUNRISE_STILL` they wait for the world too: endermen's and blazes' particles
        // draw on the particles' random, and effects count down, on every tick a frozen
        // client runs, as many as the run took to get there.
        if !hold {
            self.tick_client_ambient();
            self.tick_client_effects();
        }
        // Gui.tick: the titles, the action bar and the held item's name.
        if !hold {
            self.tick_hud();
        } else {
            // Only its countdowns wait: lying down still opens the bed's chat, waking closes it.
            self.tick_in_bed_chat();
        }
        self.tick_subtitles();
        self.tick_client_load();
        // Minecraft.tick: Level.tickEntities (this player among them), then
        // Level.tickBlockEntities, then LocalPlayer.sendChanges. A piston's push
        // lands after the player's own move, and the packet carries what it left:
        // the position and the ground and collision flags `Entity.move(PISTON, ...)`
        // set. A frozen level ticks no block entities; a step advances the
        // moving pistons created by the block event once, rather than finishing
        // them during the frozen ticks between steps.
        let sent = self.tick_remote_own_player();
        if running {
            self.tick_remote_block_entities();
            // Minecraft.tick: the particle engine ticks after Level.tickBlockEntities.
            self.tick_particle_engine();
        }
        let input = sent?;
        // Like the block-entity push above, collider displacement belongs in this tick's
        // LocalPlayer.sendChanges, before the client closes the tick on the wire.
        let start = sunrise_client_render::moving_colliders::Scope::placed();
        self.carry_pass(&start);
        self.observe_carry_age(&start);
        if let Some(input) = input {
            self.remote_send_state(input);
        }
        Ok(())
    }
}
