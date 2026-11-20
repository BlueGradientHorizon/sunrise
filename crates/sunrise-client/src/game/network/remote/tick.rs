use super::*;

impl Game<'_> {
    /// One client tick while connected (Minecraft.tick with a ClientLevel).
    pub(in crate::game) fn tick_remote(&mut self) -> Result<()> {
        self.tick_remote_level()?;
        // The rockets boosting this player tick after it, also while it waits
        // for chunks.
        self.tick_own_rockets();
        self.tick_book_snapshot_retry();
        // Far tiles the samplers asked for (`lod_net`).
        if let Some(r) = &self.remote {
            r.session.send_far_terrain();
        }
        // Minecraft.tick sends this even when the player cannot tick yet,
        // including while waiting for chunks. Keep it outside early returns.
        if self.remote.as_ref().is_some_and(Remote::has_level) {
            self.send(S::ClientTickEnd(Empty));
        }
        Ok(())
    }

    pub(in crate::game) fn tick_remote_player(&mut self, running: bool, hold: bool) -> Result<()> {
        // Inventory.tick's ItemStack.inventoryTick: the pop time runs down.
        if !hold {
            for pop in &mut self.hotbar_pop {
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
            self.tick_particles();
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
        }
        if let Some(input) = sent? {
            self.remote_send_state(input);
        }
        Ok(())
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
        let camera = self.player.as_ref().map(|p| {
            [
                p.position.x,
                p.position.y + f64::from(p.dimensions.eye_height),
                p.position.z,
            ]
        });
        let mut looks = std::mem::take(&mut self.guardian_look_scratch);
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
        self.guardian_look_scratch = looks;
    }

    pub(in crate::game) fn tick_remote_entities(&mut self, running: bool) {
        use crate::game::native_ticks::{PLAYER, TickKind};

        if running {
            self.tick_text_displays();
            self.tick_remote_ominous_item_spawners();
        }
        // EntityTickEvent.Pre for the mobs and players this client ticks, once for the family
        // before their tick, and the Post after it (below). A canceled one keeps its state. One
        // of a type a mod named is told alone, right before and right after its own turn below.
        let mut round = self.native_tick_round();
        // The mount this client moves ticks in `remote_ride` instead, as its
        // authoritative side (LivingEntity.tick with travelRidden).
        let driven = self.driven_mount().map(|m| m.id);
        let alone_player = round.alone_kind(PLAYER);
        let mut alone_any = false;
        if round.listening() {
            let players = self.remote.as_ref().map(|r| &r.level_players);
            for m in self.mobs.iter() {
                let player = players.is_some_and(|p| p.contains_key(&(m.id as i32)));
                if (running || player) && driven != Some(m.id) {
                    let alone = if player {
                        alone_player
                    } else {
                        round.alone_mob(&m.kind)
                    };
                    if alone {
                        alone_any = true;
                    } else if player {
                        round.add(m.id, TickKind::Fixed(PLAYER));
                    } else {
                        round.add(m.id, TickKind::Mob(m.kind.clone()));
                    }
                }
            }
            self.native_tick_pre_batch(&mut round);
        }
        // EntityFluidInteraction.update for the entities this window only displays: the
        // server never sends the fluid contact, and the renderers read it (cod, salmon and
        // tropical fish lie on their side without `in_water`, #1591).
        // A frozen level ticks only the players, as below.
        // Entity.isInRain for the wolves, which dry off by their own `Wolf.tick`.
        let (mut fluids, mut rain, mut standings, mut water_fx) = {
            let r = self
                .remote
                .as_mut()
                .expect("remote entity tick without remote state");
            (
                std::mem::take(&mut r.tick_fluids),
                std::mem::take(&mut r.tick_rain),
                std::mem::take(&mut r.tick_standings),
                std::mem::take(&mut r.tick_water_fx),
            )
        };
        let thundering = {
            let w = self.mob_world_base(&[]);
            let players = self.remote.as_ref().map(|r| &r.level_players);
            fluids.clear();
            fluids.extend(self.mobs.iter().map(|m| {
                (running || players.is_some_and(|p| p.contains_key(&(m.id as i32))))
                    .then(|| w.fluids(m.bounds(), m.eye_y()))
            }));
            rain.clear();
            rain.extend(self.mobs.iter().map(|m| {
                m.species() == sunrise_client::mobs::Species::Wolf
                    && sunrise_client::mobs::tame::in_rain(m, &w)
            }));
            // The block under each mob the packs' random-model rules read (a grazing
            // cow), one hair below the feet like `Entity.getBlockStateOn`; nothing
            // without rules.
            standings.clear();
            standings.extend(self.mobs.iter().map(|m| {
                sunrise_client::mobs::cem::tracks_standing(&m.kind.id).then(|| {
                    let (x, y, z) = (
                        m.position.x.floor() as i32,
                        (m.position.y - 0.0001).floor() as i32,
                        m.position.z.floor() as i32,
                    );
                    w.block_name(x, y, z).to_owned()
                })
            }));
            w.thundering
        };
        // The window's own player already splashes from its motion tick.
        let own = self.remote.as_ref().map(|r| r.entity_id);
        for i in 0..self.mobs.len() {
            let mob = &self.mobs[i];
            let id = mob.id;
            let told_alone = alone_any
                && if self
                    .remote
                    .as_ref()
                    .unwrap()
                    .level_players
                    .contains_key(&(id as i32))
                {
                    alone_player
                } else {
                    round.alone_mob(&mob.kind)
                };
            // One told alone takes this step in its own turn, after its Pre (below).
            if round.canceled(id) || told_alone && fluids[i].is_some() && driven != Some(id) {
                continue;
            }
            client_fluid_step(
                &mut self.mobs[i],
                fluids[i].take(),
                standings[i].take(),
                own,
                &mut water_fx,
            );
        }
        let game_time = self.world.game_time;
        // `AbstractPiglin.isConverting`: the dimension is not piglin safe.
        let nether = !self.dimension_type().piglins_zombify;
        // What a wither's side heads can look at, by network entity id: the
        // mobs and the player (position and eye height).
        let wither_targets: Vec<(i32, [f64; 3], f64)> = if self
            .mobs
            .iter()
            .any(|m| m.species() == sunrise_client::mobs::Species::Wither)
        {
            let own = self
                .remote
                .as_ref()
                .zip(self.eye_position())
                .map(|(r, eye)| (r.entity_id, eye, eye[1]));
            self.mobs
                .iter()
                .map(|m| {
                    (
                        m.id as i32,
                        [m.position.x, m.position.y, m.position.z],
                        m.eye_y(),
                    )
                })
                .chain(own)
                .collect()
        } else {
            Vec::new()
        };
        // ExperienceOrb.followNearbyPlayer: the players an orb may go for,
        // their feet and the point `eyeHeight / 2` above them.
        let followed: Vec<([f64; 3], [f64; 3])> = {
            let own = self
                .player
                .as_ref()
                .filter(|p| !p.health.dead_or_dying() && !self.spectator())
                .map(|p| {
                    let feet = [p.position.x, p.position.y, p.position.z];
                    let eye = f64::from(p.dimensions.eye_height);
                    (feet, [feet[0], feet[1] + eye / 2.0, feet[2]])
                });
            let others = self.remote.as_ref().into_iter().flat_map(|r| {
                self.mobs
                    .iter()
                    .filter(move |m| m.alive() && r.level_players.contains_key(&(m.id as i32)))
                    .filter(move |m| {
                        let listed = r.level_players.get(&(m.id as i32));
                        !listed
                            .and_then(|uuid| r.tab_players.get(uuid))
                            .is_some_and(|t| t.game_mode == SPECTATOR)
                    })
                    .map(|m| {
                        let feet = [m.position.x, m.position.y, m.position.z];
                        (
                            feet,
                            [feet[0], feet[1] + (m.eye_y() - feet[1]) / 2.0, feet[2]],
                        )
                    })
            });
            own.into_iter().chain(others).collect()
        };
        // Cat.handleLieDown: the sleeping players (another's, drawn lying in
        // a bed, and this client's own) a lying cat can lie on top of.
        let sleepers: Vec<sunrise_core::geometry::Aabb> = self
            .remote
            .as_ref()
            .map(|r| {
                self.mobs
                    .iter()
                    .filter(|m| {
                        r.level_players.contains_key(&(m.id as i32)) && m.extra.ints[1] != 0
                    })
                    .map(|m| {
                        let p = m.position;
                        sunrise_core::geometry::Aabb::new(
                            sunrise_core::geometry::Vec3::new(p.x - 0.1, p.y, p.z - 0.1),
                            sunrise_core::geometry::Vec3::new(p.x + 0.1, p.y + 0.2, p.z + 0.1),
                        )
                    })
                    .chain(self.sleeping.map(|bed| {
                        let p = sunrise_core::geometry::Vec3::new(
                            f64::from(bed[0]) + 0.5,
                            f64::from(bed[1]) + 0.6875,
                            f64::from(bed[2]) + 0.5,
                        );
                        sunrise_core::geometry::Aabb::new(
                            sunrise_core::geometry::Vec3::new(p.x - 0.1, p.y, p.z - 0.1),
                            sunrise_core::geometry::Vec3::new(p.x + 0.1, p.y + 0.2, p.z + 0.1),
                        )
                    }))
                    .collect()
            })
            .unwrap_or_default();
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        let mut lerps = std::mem::take(&mut r.lerps);
        let mut cart_lerps = std::mem::take(&mut r.cart_lerps);
        let mut using = Vec::new();
        let mut mob_fx = water_fx;
        let tick_listened = mod_api::__private::listened(mod_api::HookId::PlayerTickPre)
            || mod_api::__private::listened(mod_api::HookId::PlayerTickPost);
        // A mob told alone hears its hooks with the list in place: each slot's mob is kept by id,
        // and where it is once a hook changed the list.
        let slots: Vec<u32> = if alone_any {
            self.mobs.iter().map(|m| m.id).collect()
        } else {
            Vec::new()
        };
        let mut places = None;
        for (slot, rain) in rain.iter().copied().enumerate() {
            let Some(mut i) = crate::game::native_ticks::slot_place(&places, &slots, slot) else {
                continue;
            };
            let id = self.mobs[i].id;
            let player = self
                .remote
                .as_ref()
                .unwrap()
                .level_players
                .contains_key(&(id as i32));
            if (!running && !player) || driven == Some(id) || round.canceled(id) {
                continue;
            }
            // Of a type a mod named: its own Pre, then its fluid step and its tick.
            let alone = if player {
                alone_player.then_some(TickKind::Fixed(PLAYER))
            } else {
                (alone_any && round.alone_mob(&self.mobs[i].kind))
                    .then(|| TickKind::Mob(self.mobs[i].kind.clone()))
            };
            if let Some(kind) = &alone {
                let stamp = self.mobs.stamp();
                let ticks = self.native_tick_pre_one(&mut round, id, kind.id());
                if self.mobs.stamp() != stamp {
                    places = Some(crate::game::native_ticks::places_of(
                        self.mobs.iter().map(|m| m.id),
                    ));
                }
                match crate::game::native_ticks::slot_place(&places, &slots, slot) {
                    Some(at) if ticks => i = at,
                    _ => continue,
                }
                client_fluid_step(
                    &mut self.mobs[i],
                    fluids[slot].take(),
                    standings[slot].take(),
                    own,
                    &mut mob_fx,
                );
            }
            let animation = if player && self.mobs[i].extra.using_item > 0 {
                let extra = &self.mobs[i].extra;
                let slot = if extra.using_off_hand {
                    sunrise_items::equipment::OFFHAND
                } else {
                    sunrise_items::equipment::MAINHAND
                };
                extra
                    .slot_stack(slot)
                    .map_or_else(Default::default, |stack| self.item_use_animation(stack))
            } else {
                Default::default()
            };
            let player_ref = tick_listened
                .then(|| {
                    self.remote.as_ref().and_then(|r| {
                        let uuid = *r.level_players.get(&(id as i32))?;
                        Some(mod_api::__private::game::player_ref(
                            mod_api::EntityRef::new(self.native_lifecycle.epoch()?, id),
                            uuid,
                        ))
                    })
                })
                .flatten();
            let r = self.remote.as_mut().unwrap();
            let stepped = &mut r.stepped;
            let players = &r.level_players;
            let tab_players = &r.tab_players;
            let m = &mut self.mobs[i];
            let section_from = m.position;
            if player {
                // AvatarRenderer.extractRenderState: isSpectator, from the player list.
                m.extra.spectator = players
                    .get(&(m.id as i32))
                    .and_then(|uuid| tab_players.get(uuid))
                    .is_some_and(|t| t.game_mode == SPECTATOR);
            }
            m.previous = m.position;
            // `Entity.deltaMovementOnPreviousTick`: the elytra's flight yaw
            // lerps from it, so another player's model does not snap when a
            // new tick's velocity arrives.
            m.extra.previous_velocity = m.velocity;
            m.previous_yaw = m.yaw;
            m.previous_body_yaw = m.body_yaw;
            m.previous_head_yaw = m.head_yaw;
            m.previous_head_pitch = m.head_pitch;
            m.previous_limb_speed = m.limb_speed;
            m.age += 1;
            if m.species() == sunrise_client::mobs::Species::Rabbit {
                sunrise_client::mobs::rabbit::base_tick(m);
            }
            if matches!(
                m.species(),
                sunrise_client::mobs::Species::Horse
                    | sunrise_client::mobs::Species::Donkey
                    | sunrise_client::mobs::Species::Mule
                    | sunrise_client::mobs::Species::SkeletonHorse
                    | sunrise_client::mobs::Species::ZombieHorse
            ) {
                sunrise_client::mobs::equine::tail_tick(m);
            }
            if m.species() == sunrise_client::mobs::Species::Creeper {
                sunrise_client::mobs::hostile::creeper_client_tick(m);
            }
            if matches!(
                m.species(),
                sunrise_client::mobs::Species::Guardian
                    | sunrise_client::mobs::Species::ElderGuardian
            ) {
                sunrise_client::mobs::guardian::client_tick(m);
            }
            // PolarBear.tick: the stand animation follows the synched flag.
            if m.species() == sunrise_client::mobs::Species::PolarBear {
                sunrise_client::mobs::polar_bear::client_tick(m);
            }
            // The particles these mobs' own ticks draw on the client side.
            {
                use sunrise_client::mobs::{
                    Species as S, cube, illager, illusioner, nautilus, tame, wither,
                };
                match m.species() {
                    S::Wolf => tame::client_tick(m, rain, &mut mob_fx),
                    S::Slime | S::MagmaCube | S::SulfurCube => cube::client_tick(m, &mut mob_fx),
                    S::Nautilus | S::ZombieNautilus => nautilus::client_tick(m, &mut mob_fx),
                    S::Illusioner => {
                        illusioner::client_tick(m, &mut mob_fx);
                        illager::cast_particles(m, &mut mob_fx);
                    }
                    S::Evoker => illager::cast_particles(m, &mut mob_fx),
                    S::Wither => {
                        wither::client_smoke(m, &mut mob_fx);
                        // `getInvulnerableTicks() > 0`: the charge's cloud,
                        // after the heads' smoke as in `aiStep`.
                        wither::charging_particles(m, &mut mob_fx);
                    }
                    _ => {}
                }
            }
            if m.hurt_time > 0 {
                m.hurt_time -= 1;
            }
            if !m.alive() {
                m.death_time += 1;
            }
            // LivingEntity.updateSwingTime.
            m.anim.attack_anim_o = m.anim.attack_anim;
            sunrise_client::mobs::ai::tick_swing(m);
            // Entity.tick: the SteppedInterpolationHandler's step.
            if let Some(at) = stepped
                .get_mut(&(m.id as i32))
                .and_then(|h| h.interpolate())
            {
                place_interpolated(m, at);
            }
            // `ClientLevel.tickNonPassenger`'s `commonTick` interpolated the
            // entity before `Player.tick`, which `Pre` precedes.
            if let Some(player) = player_ref {
                self.native_player_tick_pre(mod_api::__private::PlayerTickPreCall { player });
            }
            let head_lerps = &mut self.remote.as_mut().unwrap().head_lerps;
            let m = &mut self.mobs[i];
            // LivingEntity.aiStep: the head's own interpolation.
            if let Some((yaw, steps)) = head_lerps.get_mut(&(m.id as i32))
                && *steps > 0
            {
                m.head_yaw += wrap_degrees(*yaw - m.head_yaw) / *steps as f32;
                *steps -= 1;
            }
            // Fox.tick: the head tilt and crouch follow the synched flags.
            if m.species() == sunrise_client::mobs::Species::Fox {
                sunrise_client::mobs::fox::client_tick(m);
            }
            // Cat.tick: the lying and relaxing amounts follow the synched flags.
            if m.species() == sunrise_client::mobs::Species::Cat {
                m.anim.vo = m.anim.v;
                sunrise_client::mobs::tame::cat_amounts(m);
                sunrise_client::mobs::tame::cat_lying_on_sleeper(m, &sleepers);
            }
            // Chicken.aiStep: the wings follow the server's on-ground flag.
            if m.species() == sunrise_client::mobs::Species::Chicken {
                sunrise_client::mobs::farm::chicken_flap(m);
            }
            // Parrot.calculateFlapping: the wings follow the server's
            // on-ground flag and the shoulder it sits on (the glide is the
            // server's).
            if m.species() == sunrise_client::mobs::Species::Parrot {
                sunrise_client::mobs::parrot::flap(m);
            }
            // LivingEntity.updatingUsingItem: the ticks the item has been
            // used before this one. Another player's use particles; a mob's
            // drawn bow only needs the count (its pull follows it).
            if player {
                // AvatarRenderer.getArmPose: the used item's use animation
                // poses the arm.
                m.extra.use_animation = animation;
            }
            if m.extra.using_item > 0 {
                if player {
                    let item = if m.extra.using_off_hand {
                        m.extra.slot_stack(sunrise_items::equipment::OFFHAND)
                    } else {
                        m.extra.slot_stack(sunrise_items::equipment::MAINHAND)
                    }
                    .map(|s| s.item.clone());
                    if let Some(item) = item {
                        let eye = m.eye_height();
                        let at = [m.position.x, m.position.y, m.position.z];
                        using.push((
                            item,
                            m.extra.using_item as u32 - 1,
                            at,
                            eye,
                            m.head_pitch,
                            m.yaw,
                        ));
                    }
                }
                m.extra.using_item += 1;
            }
            // LivingEntity.tick's tickHeadTurn: a mob's BodyRotationControl,
            // a player's body following its walk and look.
            if player {
                remote_player_head_turn(m);
                // LivingEntity.tick: the glide count, wings and swim amount.
                m.tick_avatar_pose(m.fluids.in_water);
                // LivingEntity.tick: a sleeper's xRot is 0.
                if m.extra.ints[1] != 0 {
                    m.head_pitch = 0.0;
                    m.previous_head_pitch = 0.0;
                }
            } else if m.alive() {
                m.body_rotation_tick();
            }
            // Squid.aiStep; WalkAnimationState.update from the horizontal distance.
            sunrise_client::mobs::aquatic::squid_client_step(m, &mut mob_fx);
            let d = sunrise_core::geometry::length(
                m.position.x - m.previous.x,
                m.position.z - m.previous.z,
            ) as f32;
            let (target, factor) = if m.species() == sunrise_client::mobs::Species::Frog {
                (sunrise_client::mobs::frog::walk_target(m, d), 0.4)
            } else if sunrise_client::mobs::camel::is_camel(m.species()) {
                sunrise_client::mobs::camel::walk_target(m, d)
            } else {
                ((d * 4.0).min(1.0), 0.4)
            };
            m.limb_speed += (target - m.limb_speed) * factor;
            m.limb_position += m.limb_speed;
            if m.species() == sunrise_client::mobs::Species::Rabbit {
                sunrise_client::mobs::rabbit::post_tick(m);
            }
            if m.species() == sunrise_client::mobs::Species::Bat {
                sunrise_client::mobs::bat::setup_animation_states(m);
            }
            // The animation states a server computes in its own `ai_step`,
            // and the particles vanilla draws on the client.
            sunrise_client::mobs::sync::client_tick(m, game_time, &mut mob_fx);
            match m.species() {
                // WitherBoss.aiStep: the side heads turn toward their targets.
                sunrise_client::mobs::Species::Wither => {
                    sunrise_client::mobs::wither::client_tick(m, |id| {
                        wither_targets
                            .iter()
                            .find(|t| t.0 == id)
                            .map(|t| (t.1, t.2))
                    });
                }
                // Hoglin and piglin `isConverting` outside the Nether.
                sunrise_client::mobs::Species::Hoglin
                | sunrise_client::mobs::Species::Piglin
                | sunrise_client::mobs::Species::PiglinBrute => {
                    sunrise_client::mobs::piglin::client_converting(m, nether);
                }
                sunrise_client::mobs::Species::Panda => {
                    sunrise_client::mobs::panda::client_tick(m, thundering);
                }
                // Shulker.tick's onPeekAmountChange: the lid moves this
                // client's player, whose movement is its own.
                sunrise_client::mobs::Species::Shulker => {
                    sunrise_client::mobs::shulker::client_push(m, &mut mob_fx);
                }
                _ => {}
            }
            let section_id = m.id;
            let section_to = m.position;
            if let Some(player) = player_ref {
                self.native_player_tick_post(mod_api::__private::PlayerTickPostCall { player });
            }
            if section_from != section_to {
                self.remote_entity_section_step(section_id, section_from, section_to);
            }
            // Its own Post, right after its turn.
            if let Some(kind) = &alone {
                let stamp = self.mobs.stamp();
                self.native_tick_post_one(&round, id, kind.id());
                if self.mobs.stamp() != stamp {
                    places = Some(crate::game::native_ticks::places_of(
                        self.mobs.iter().map(|m| m.id),
                    ));
                }
            }
        }
        self.native_tick_post_batch(round);
        // ExperienceOrb.tick runs on a client too (all but the merges and the
        // push out of a block, which are the server's): gravity, buoyancy,
        // collisions and the pull of the nearest player between the server's
        // updates; the server removes it on pickup.
        let (world, facts, collision) = (&self.world, self.res.facts(), self.collision);
        let border = Some(self.border.geometry());
        let mut lava_random = None;
        let mut next_float = || {
            lava_random
                .get_or_insert_with(random_source::entity)
                .next_float()
        };
        if running {
            self.orbs.retain_mut(|o| {
                let distance = |feet: &[f64; 3]| -> f64 {
                    (feet[0] - o.position.x).powi(2)
                        + (feet[1] - o.position.y).powi(2)
                        + (feet[2] - o.position.z).powi(2)
                };
                let target = followed
                    .iter()
                    .filter(|(feet, _)| distance(feet) < 64.0)
                    .min_by(|a, b| distance(&a.0).total_cmp(&distance(&b.0)))
                    .map(|(_, target)| *target);
                // Entity.commonTick: setOldPosAndRot, then
                // LinearInterpolationHandler.doInterpolate moves 1/remaining of
                // the way to the server's position, so the frame draws the step
                // from the old position.
                let old = o.position;
                if let Some(l) = lerps.get_mut(&(o.id as i32)).filter(|l| l.steps > 0) {
                    let alpha = 1.0 / f64::from(l.steps);
                    o.position = Vec3::new(
                        o.position.x + (l.target[0] - o.position.x) * alpha,
                        o.position.y + (l.target[1] - o.position.y) * alpha,
                        o.position.z + (l.target[2] - o.position.z) * alpha,
                    );
                    l.steps -= 1;
                }
                let before = o.position;
                let side = game_side!(self);
                let kept = o.tick(
                    world,
                    facts,
                    collision,
                    target,
                    false,
                    &mut next_float,
                    border,
                    &self.res.motion().fluids,
                    &self.loop_context,
                    &side,
                    &mut |_, _| {},
                );
                // `tick` set `previous` to the stepped position: keep the
                // pre-step one, as `xo` is in vanilla.
                o.previous = old;
                // AbstractInterpolationHandler.applyPredictedMovement: the orb's
                // own move carries the target along (vanilla also checks the
                // moved target for collisions; that check is left out here).
                if let Some(l) = lerps.get_mut(&(o.id as i32)).filter(|l| l.steps > 0) {
                    l.target[0] += o.position.x - before.x;
                    l.target[1] += o.position.y - before.y;
                    l.target[2] += o.position.z - before.z;
                }
                kept
            });
        }
        // ItemEntity.tick runs on a client too: gravity, collisions and
        // friction move the item between the server's updates (every 20
        // ticks). Ordinary removal remains server-authoritative. Mods can opt
        // into predicting consumption at block contact.
        self.remote
            .as_mut()
            .unwrap()
            .head_lerps
            .retain(|_, (_, steps)| *steps > 0);
        let mut entered = Vec::new();
        if running {
            entity_section_steps::tick_items(self, &mut entered);
        }
        // Entity.doWaterSplashEffect on this window: Level.addParticle draws
        // the bubbles and splashes. ClientLevel.playSeededSound drops
        // Entity.playSound, so the sound stays the server's.
        for splash in entered {
            self.apply_item_splash(splash);
        }
        let _ = self.handle_mob_events_reusable(&mut mob_fx);
        if let Some(r) = self.remote.as_mut() {
            r.tick_fluids = fluids;
            r.tick_rain = rain;
            r.tick_standings = standings;
            r.tick_water_fx = mob_fx;
        }
        for (item, ticks, at, eye, pitch, yaw) in using {
            self.remote_player_eating(&item, ticks, at, eye, pitch, yaw);
        }
        // ClientLevel.tick: the sky flash fades in a frozen level too.
        self.flash = self.flash.saturating_sub(1);
        if running {
            self.guardian_bubbles();
            // The dragon: LivingEntity's interpolation, inside its aiStep.
            let dragon_lerp = self
                .dragon
                .as_ref()
                .and_then(|d| lerps.get_mut(&(d.id as i32)))
                .filter(|l| l.steps > 0)
                .map(|l| {
                    let steps = l.steps;
                    l.steps -= 1;
                    (l.target, l.rotation.map(|r| r.0), steps)
                });
            self.tick_remote_dragon(dragon_lerp);
            // A mod's general rows: the non-living interpolation, and no mod code.
            if let Some(r) = self.remote.as_mut() {
                general::tick_remote_general(&mut r.general.rows, &mut lerps);
            }
            // Armor stands: ClientLevel.tickNonPassenger's setOldPosAndRot,
            // then their SteppedInterpolationHandler's step, as the mobs'.
            if let Some(r) = self.remote.as_mut() {
                for a in &mut self.armor_stands {
                    let step = r
                        .stepped
                        .get_mut(&(a.id as i32))
                        .and_then(|h| h.interpolate());
                    a.interpolate(step.map(|at| {
                        let p = at.position;
                        (Vec3::new(p[0], p[1], p[2]), at.yaw)
                    }));
                }
            }
            // The yaw of a boat eases toward the server's over the same steps
            // as its position, read before `step` counts one off.
            let turns: HashMap<i32, (f32, u32)> = lerps
                .iter()
                .filter(|(_, l)| l.steps > 0)
                .filter_map(|(id, l)| l.rotation.map(|r| (*id, (r.0, l.steps))))
                .collect();
            // Boats, minecarts, projectiles and the rest ease toward the
            // server's position over three ticks (`lerps`).
            let mut step = |id: u32, at: Vec3| match lerps.get_mut(&(id as i32)) {
                Some(l) if l.steps > 0 => {
                    let n = f64::from(l.steps);
                    l.steps -= 1;
                    Vec3::new(
                        at.x + (l.target[0] - at.x) / n,
                        at.y + (l.target[1] - at.y) / n,
                        at.z + (l.target[2] - at.z) / n,
                    )
                }
                _ => at,
            };
            self.tick_remote_objects(
                &mut step,
                |id, yaw| {
                    turns.get(&(id as i32)).map_or(yaw, |&(target, steps)| {
                        super::super::remote_objects::lerp_yaw_step(yaw, target, steps)
                    })
                },
                |c| {
                    cart_lerps
                        .get_mut(&(c.id as i32))
                        .is_some_and(|l| l.tick(c))
                },
            );
            self.tick_remote_projectiles(&mut step);
            // LightningBolt.tick on a client.
            self.tick_bolts();
        }
        lerps.retain(|_, l| l.steps > 0);
        if let Some(r) = self.remote.as_mut() {
            for (id, l) in lerps {
                r.lerps.entry(id).or_insert(l);
            }
            for (id, l) in cart_lerps {
                r.cart_lerps.entry(id).or_insert(l);
            }
        }
    }
}
