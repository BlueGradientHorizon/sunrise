use super::{Arg, Context, Game, R, Target, err};
use crate::game::commands::world_cmds::holder;
pub(in crate::game::server_side) use crate::game::commands::world_cmds::look_at;
use crate::game::commands::world_cmds::num;
use crate::game::commands::world_cmds::take_new_reinforcements;
use crate::game::commands::world_cmds::text_error;
use crate::game::minecarts::CartHit;
use sunrise_client::{
    mobs::{Damage, Who},
    scoreboard_cmd::Tracker,
    text::Text,
};
use sunrise_core::nbt::Tag;
use sunrise_core::random::RandomSource;
use sunrise_protocol::packets::play::{self, Clientbound as C};

impl Game<'_, crate::game::server_side::ServerState> {
    /// ServerRecipeBook.addRecipes/removeRecipes for the player in place: how
    /// many recipe book displays changed (a transmute recipe has one per
    /// material count).
    pub(in crate::game) fn change_recipes(&mut self, give: bool, recipes: &[String]) -> i32 {
        let (recipes, custom_count) = self.change_mod_crafting_recipes(give, recipes);
        if give {
            return custom_count + self.award_recipes(&recipes) as i32;
        }
        let index = &world_stations!(self).recipe_index;
        let removed = self.level.recipe_book.remove_recipes(index, &recipes);
        let count = custom_count + removed.len() as i32;
        if count != 0 {
            self.invalidate_mod_book_placement_knowledge();
        }
        // A network player's client drops them from its book.
        if self.side.lan_acting.is_some() && !removed.is_empty() {
            self.side.guest_packets.push(C::RecipeBookRemove(
                sunrise_protocol::packets::recipes::RecipeBookRemove { ids: removed },
            ));
        }
        count
    }
    pub(in crate::game::server_side) fn cmd_particle(&mut self, ctx: &Context) -> R<i32> {
        use crate::lan::server_effects::{ServerParticle, ServerQueuedParticle};
        use sunrise_level::particle_options::ParticleType;
        let (name, particle) = match ctx.get("name") {
            Some(Arg::Particle(opts)) => {
                use crate::game::api_values::TryIntoEngine;
                let options = opts
                    .try_into_engine(self.items())
                    .map_err(|_| err("particle.invalidOptions", &[]))?;
                let name = options
                    .mod_particle
                    .as_deref()
                    .unwrap_or(options.ty.name())
                    .to_owned();
                let particle =
                    crate::game::lan_particles::server_particle_from_options(&options, None);
                (name, particle)
            }
            Some(Arg::Str(spec)) => {
                // ParticleArgument: `id` then optional SNBT options (`dust{color:[1,0,0],scale:2}`).
                let (name, options_text) = match spec.find('{') {
                    Some(i) => (&spec[..i], &spec[i..]),
                    None => (spec.as_str(), ""),
                };
                let ty = ParticleType::from_name(name)
                    .ok_or_else(|| err("particle.notFound", &[&name]))?;
                let opts = match sunrise_client::command::snbt::parse_fully(options_text) {
                    Ok(Tag::Compound(c)) if !options_text.is_empty() => Some(c),
                    _ => None,
                };
                let field = |key: &str| opts.as_ref().and_then(|c| c.get(&key.into()).cloned());
                let string = |t: &Tag| match t {
                    Tag::String(s) => Some(s.to_string_lossy()),
                    _ => None,
                };
                let particle = match ty {
                    // ItemParticleOption: an item id, or a stack {id, ...}.
                    ParticleType::Item => {
                        let id = match field("item") {
                            Some(Tag::String(s)) => s.to_string_lossy(),
                            Some(Tag::Compound(c)) => {
                                c.get(&"id".into()).and_then(string).unwrap_or_default()
                            }
                            _ => String::new(),
                        };
                        let id = if id.contains(':') {
                            id
                        } else {
                            format!("minecraft:{id}")
                        };
                        Some(ServerParticle::item(sunrise_client::items::ItemStack::new(
                            &id, 1,
                        )))
                    }
                    ParticleType::ItemSlime
                    | ParticleType::ItemSnowball
                    | ParticleType::ItemCobweb => Some(ServerParticle::simple(ty)),
                    ParticleType::Block
                    | ParticleType::BlockCrumble
                    | ParticleType::FallingDust
                    | ParticleType::DustPillar
                    | ParticleType::BlockMarker => {
                        // BlockParticleOption: a block id, or {Name, Properties}.
                        let text = match field("block_state") {
                            Some(Tag::String(s)) => s.to_string_lossy(),
                            Some(Tag::Compound(c)) => {
                                let name =
                                    c.get(&"Name".into()).and_then(string).unwrap_or_default();
                                let props: Vec<String> = match c.get(&"Properties".into()) {
                                    Some(Tag::Compound(p)) => p
                                        .iter()
                                        .filter_map(|(k, v)| {
                                            string(v)
                                                .map(|v| format!("{}={v}", k.to_string_lossy()))
                                        })
                                        .collect(),
                                    _ => Vec::new(),
                                };
                                if props.is_empty() {
                                    name
                                } else {
                                    format!("{name}[{}]", props.join(","))
                                }
                            }
                            _ => String::new(),
                        };
                        let mut reader = sunrise_client::command::Reader::new(&text);
                        let block = sunrise_client::command::args::ArgType::BlockState
                            .parse(&mut reader, &self.ui.registries);
                        match block {
                            Ok(Arg::Block(b)) => {
                                let state = self.block_state(&b)?;
                                Some(ServerParticle::block(ty, state))
                            }
                            _ => Some(ServerParticle::simple(ty)),
                        }
                    }
                    // Every other type with options decodes its own codec
                    // (`ParticleTypes.CODEC`): dust, spells, sculk charge, shriek,
                    // geysers. A type without options keeps the defaults.
                    _ => {
                        let options = opts
                            .as_ref()
                            .and_then(|c| crate::game::potions::particle_options(ty, c))
                            .unwrap_or_else(|| {
                                sunrise_level::particle_options::ParticleOptions::simple(ty)
                            });
                        crate::game::lan_particles::server_particle_from_options(&options, None)
                    }
                };
                (name.to_owned(), particle)
            }
            _ => return Ok(0),
        };
        let pos = self
            .position_arg(ctx, "pos")
            .unwrap_or_else(|| self.source_position());
        // Vec3Argument.getVec3: the spread as given (relative parts from the
        // source), not a position to measure from the source.
        let delta = self.position_arg(ctx, "delta").unwrap_or([0.0; 3]);
        let speed = match ctx.get("speed") {
            Some(Arg::Float(v)) => *v,
            _ => 0.0,
        };
        let count = match ctx.get("count") {
            Some(Arg::Int(c)) => *c,
            _ => 0,
        };
        // ParticleCommand.sendParticles: the viewers it reached, each within
        // 32 blocks (512 with `force`) of the particle's block centre.
        // Without viewers, every player (PlayerList.getPlayers).
        let viewers = if ctx.get("viewers").is_some() {
            self.entities(ctx, "viewers", true)?
        } else {
            let acting = self.side.lan_acting;
            let guests = self.lan.iter().flat_map(|h| h.hub.online_players());
            self.player
                .as_ref()
                .filter(|_| !self.side.lan_acting_fake)
                .map(|_| Target::Player)
                .into_iter()
                .chain(
                    guests
                        .filter(|g| Some(g.conn) != acting)
                        .map(|g| Target::Guest(g.conn)),
                )
                .collect()
        };
        let force = ctx.has("force");
        let limit = if force { 512.0 } else { 32.0 };
        let reached: Vec<Target> = viewers
            .into_iter()
            .filter(|&t| {
                if particle.as_ref().is_some_and(ServerParticle::is_mod) {
                    let conn = match t {
                        Target::Player => self.side.lan_acting,
                        Target::Guest(conn) => Some(conn),
                        _ => None,
                    };
                    if conn.is_some_and(|conn| {
                        self.lan
                            .as_ref()
                            .and_then(|host| host.hub.guest(conn))
                            .is_none_or(|g| !g.is_sunrise())
                    }) {
                        return false;
                    }
                }
                self.target_position(t).is_some_and(|p| {
                    let c = p.map(|v| v.floor() + 0.5);
                    (c[0] - pos[0]).powi(2) + (c[1] - pos[1]).powi(2) + (c[2] - pos[2]).powi(2)
                        < limit * limit
                })
            })
            .collect();
        if reached.is_empty() {
            return Err(err("commands.particle.failed", &[]));
        }
        // ServerLevel.sendParticles: encode via the portable server particle codec,
        // preserving `override_limiter: force` and the exact reached recipient set.
        let packet = particle
            .as_ref()
            .and_then(|p| self.lan.as_ref()?.hub.server_particle(p))
            .map(|particle| {
                C::LevelParticles(play::LevelParticles {
                    particle,
                    override_limiter: force,
                    always_show: false,
                    pos,
                    dist: delta.map(|d| d as f32),
                    max_speed: [speed; 3],
                    count: count.max(0),
                    randomization: 0,
                })
            });
        let mut sent = 0;
        for t in reached {
            match (t, &packet) {
                (Target::Player, None) if self.lan.is_none() => {
                    if let Some(p) = &particle {
                        self.side.pending_particles.push(ServerQueuedParticle {
                            dimension: self.dim.id(),
                            particle: p.clone(),
                            at: pos,
                            spread: delta,
                            speed: [f64::from(speed); 3],
                            count: count.max(0),
                            randomization: 0,
                            except: None,
                        });
                        sent += 1;
                    }
                }
                (Target::Player, Some(p)) if self.side.lan_acting.is_some() => {
                    self.side.guest_packets.push(p.clone());
                    sent += 1;
                }
                (Target::Guest(conn), Some(p)) => {
                    if let Some(host) = &self.lan {
                        host.hub.send_to(conn, p);
                        sent += 1;
                    }
                }
                _ => {}
            }
        }
        if sent == 0 {
            return Err(err("commands.particle.failed", &[]));
        }
        self.feedback(Text::translate(
            "commands.particle.success",
            &[Text::literal(name)],
        ));
        Ok(sent)
    }
    pub(in crate::game::server_side) fn cmd_rotate(&mut self, ctx: &Context) -> R<i32> {
        let targets = super::without_knots(self.entities(ctx, "target", false)?)?;
        let Some(t) = targets.first().copied() else {
            return Ok(0);
        };
        let Some(pos) = self.target_position(t) else {
            return Ok(0);
        };
        let mut look_at_target = None;
        let rot = if let Some(Arg::Coords(c)) = ctx.get("rotation") {
            // RotateCommand.rotate: relative parts are offsets from the command
            // source's rotation (WorldCoordinates.getRotation); the entity's own
            // rotation cancels out in Entity.forceSetRotation.
            c.rotation(self.source_rotation())
        } else {
            let mut faced_entity = None;
            let to = if ctx.has("entity") {
                let faced = self.entities(ctx, "facingEntity", false)?;
                let Some(f) = faced.first().copied() else {
                    return Ok(0);
                };
                let mut p = self.target_position(f).unwrap_or(pos);
                let eyes = matches!(ctx.get("facingAnchor"), Some(Arg::Anchor(true)));
                if eyes {
                    p[1] += self.server_target_eye_height(f);
                }
                faced_entity = Some((f, eyes));
                p
            } else {
                self.position_arg(ctx, "facingLocation").unwrap_or(pos)
            };
            // LookAt.perform: Entity.lookAt from the source's anchor (the
            // feet, unless `anchored eyes`).
            let anchored_eyes = self.side.command_source.is_some_and(|s| s.eyes);
            let eye = if anchored_eyes {
                self.server_target_eye_height(t)
            } else {
                0.0
            };
            let from = [pos[0], pos[1] + eye, pos[2]];
            look_at_target = Some((anchored_eyes, to, faced_entity));
            look_at(from, to)
        };
        // RotateCommand: Entity.forceSetRotation, not a teleport.
        self.set_rotation(t, rot);
        if let Some((eyes, to, entity)) = look_at_target {
            self.send_player_look_at(t, eyes, to, entity);
        }
        if t == Target::Player {
            self.pitch = rot[1].clamp(-90.0, 90.0);
        }
        let name = self.target_name(t);
        self.feedback(Text::translate("commands.rotate.success", &[name]));
        Ok(1)
    }
    pub(in crate::game::server_side) fn cmd_spreadplayers(&mut self, ctx: &Context) -> R<i32> {
        let centre = self
            .position_arg(ctx, "center")
            .unwrap_or_else(|| self.source_position());
        let float = |name: &str| match ctx.get(name) {
            Some(Arg::Float(v)) => f64::from(*v),
            _ => 0.0,
        };
        let (spread, range) = (float("spreadDistance"), float("maxRange"));
        let respect = matches!(ctx.get("respectTeams"), Some(Arg::Bool(true)));
        let max_y = match ctx.get("maxHeight") {
            Some(Arg::Int(h)) => *h,
            _ => self.world.top_y(),
        };
        if max_y < self.world.bottom_y() {
            return Err(err(
                "commands.spreadplayers.failed.invalid.height",
                &[&max_y, &self.world.bottom_y()],
            ));
        }
        let targets = super::without_knots(self.entities(ctx, "targets", false)?)?;
        if targets.is_empty() {
            return Err(err("argument.entity.notfound.entity", &[]));
        }
        // SpreadPlayersCommand: one position per entity, or per team (every
        // non-player and team-less player sharing one) with respectTeams.
        let team_of = |g: &mut Self, t: Target| -> Option<String> {
            if t != Target::Player {
                return None;
            }
            g.holder_name(t)
                .and_then(|h| g.score.board.players_team(&h).map(|x| x.name.clone()))
        };
        let count = if respect {
            let mut seen: Vec<Option<String>> = Vec::new();
            for &t in &targets {
                let team = team_of(self, t);
                if !seen.contains(&team) {
                    seen.push(team);
                }
            }
            seen.len()
        } else {
            targets.len()
        };
        let centre_f = [centre[0] as f32, centre[2] as f32];
        let (cx, cz) = (f64::from(centre_f[0]), f64::from(centre_f[1]));
        let (min_x, max_x) = (cx - range, cx + range);
        let (min_z, max_z) = (cz - range, cz + range);
        // Mth.nextDouble.
        let next = |g: &mut Self, lo: f64, hi: f64| {
            if lo >= hi {
                lo
            } else {
                g.world.random.next_double() * (hi - lo) + lo
            }
        };
        let mut points: Vec<[f64; 2]> = Vec::with_capacity(count);
        for _ in 0..count {
            let x = next(self, min_x, max_x);
            let z = next(self, min_z, max_z);
            points.push([x, z]);
        }
        let mut collisions = true;
        let mut min_distance = f64::from(f32::MAX);
        let mut iteration = 0;
        while iteration < 10_000 && collisions {
            collisions = false;
            min_distance = f64::from(f32::MAX);
            for i in 0..points.len() {
                let mut neighbours = 0;
                let mut avg = [0.0f64, 0.0f64];
                for j in 0..points.len() {
                    if i == j {
                        continue;
                    }
                    let (dx, dz) = (points[i][0] - points[j][0], points[i][1] - points[j][1]);
                    let dist = (dx * dx + dz * dz).sqrt();
                    min_distance = min_distance.min(dist);
                    if dist < spread {
                        neighbours += 1;
                        avg[0] += points[j][0] - points[i][0];
                        avg[1] += points[j][1] - points[i][1];
                    }
                }
                if neighbours > 0 {
                    avg[0] /= f64::from(neighbours);
                    avg[1] /= f64::from(neighbours);
                    let length = (avg[0] * avg[0] + avg[1] * avg[1]).sqrt();
                    if length > 0.0 {
                        points[i][0] -= avg[0] / length;
                        points[i][1] -= avg[1] / length;
                    } else {
                        points[i][0] = next(self, min_x, max_x);
                        points[i][1] = next(self, min_z, max_z);
                    }
                    collisions = true;
                }
                let p = &mut points[i];
                let before = *p;
                p[0] = if p[0] < min_x {
                    min_x
                } else if p[0] > max_x {
                    max_x
                } else {
                    p[0]
                };
                p[1] = if p[1] < min_z {
                    min_z
                } else if p[1] > max_z {
                    max_z
                } else {
                    p[1]
                };
                if before != *p {
                    collisions = true;
                }
            }
            if !collisions {
                for i in 0..points.len() {
                    if !self.spread_safe(points[i], max_y) {
                        points[i][0] = next(self, min_x, max_x);
                        points[i][1] = next(self, min_z, max_z);
                        collisions = true;
                    }
                }
            }
            iteration += 1;
        }
        if min_distance == f64::from(f32::MAX) {
            min_distance = 0.0;
        }
        let float = |v: f32| Text::literal(sunrise_core::java_number::float_to_string(v));
        if iteration >= 10_000 {
            let key = if respect {
                "commands.spreadplayers.failed.teams"
            } else {
                "commands.spreadplayers.failed.entities"
            };
            return Err(text_error(
                key,
                &[
                    num(points.len()),
                    float(centre_f[0]),
                    float(centre_f[1]),
                    Text::literal(super::percent_f(min_distance, 2)),
                ],
            ));
        }
        // setPlayerPositions.
        let mut total = 0.0;
        let mut index = 0;
        let mut team_points: Vec<(Option<String>, usize)> = Vec::new();
        for &t in &targets {
            let i = if respect {
                let team = team_of(self, t);
                match team_points.iter().find(|(k, _)| *k == team) {
                    Some((_, i)) => *i,
                    None => {
                        team_points.push((team, index));
                        index += 1;
                        index - 1
                    }
                }
            } else {
                index += 1;
                index - 1
            };
            let p = points[i];
            let y = self.spread_spawn_y(p, max_y);
            let mut to = [p[0].floor() + 0.5, f64::from(y), p[1].floor() + 0.5];
            let mut skip = false;
            if mod_api::__private::listened(mod_api::HookId::SpreadPlayersTeleport)
                && let (Some(entity), Some(from)) =
                    (self.network_entity_id(t), self.target_position(t))
            {
                let mut moved = to;
                let uuid = match t {
                    Target::Player => self.side.lan_acting_uuid,
                    Target::Guest(conn) => self.command_guest(conn).map(|g| g.uuid),
                    _ => None,
                };
                self.native_spread_players_teleport(
                    self.dim.id(),
                    entity as u32,
                    uuid,
                    from,
                    &mut moved,
                    &mut skip,
                );
                if moved.iter().all(|v| v.is_finite()) {
                    to = moved;
                }
            }
            if !skip {
                self.teleport_to(t, to, None, false);
                self.teleport_stops(t);
            }
            let closest = points
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, q)| ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt())
                .fold(f64::MAX, f64::min);
            total += closest;
        }
        let average = if targets.len() < 2 {
            0.0
        } else {
            total / targets.len() as f64
        };
        let key = if respect {
            "commands.spreadplayers.success.teams"
        } else {
            "commands.spreadplayers.success.entities"
        };
        self.feedback(Text::translate(
            key,
            &[
                num(points.len()),
                float(centre_f[0]),
                float(centre_f[1]),
                Text::literal(super::percent_f(average, 2)),
            ],
        ));
        Ok(points.len() as i32)
    }
    pub(in crate::game::server_side) fn cmd_swing(&mut self, ctx: &Context) -> R<i32> {
        let targets = if ctx.get("targets").is_some() {
            self.entities(ctx, "targets", false)?
        } else {
            vec![
                self.side
                    .command_source
                    .and_then(|s| s.entity)
                    .unwrap_or(Target::Player),
            ]
        };
        let mut tracker = Tracker::default();
        for t in targets {
            match t {
                Target::Player => self.swing_arm(),
                Target::Mob(id) => {
                    if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) {
                        m.ai.swinging = true;
                        m.ai.swing_time = -1;
                    }
                }
                // An armor stand is a LivingEntity: it swings (invisibly).
                Target::Stand(_) => {}
                _ => continue,
            }
            let h = holder(self.target_name(t));
            tracker.mark(&h);
        }
        let (message, total) = tracker.message(
            false,
            Some(err("commands.swing.failed.notliving", &[])),
            |h, _| {
                Text::translate(
                    "commands.swing.success.single",
                    std::slice::from_ref(&h.text),
                )
            },
            |c, _| Text::translate("commands.swing.success.multiple", &[num(c)]),
        )?;
        self.feedback(message);
        Ok(total)
    }
    /// `DamageCommand.damage`: retain the accepted hit's player context and run
    /// `Zombie.hurtServer`'s reinforcement requests before the command returns.
    /// Health-changing hits also raise their damage vibration before return.
    pub(in crate::game::server_side) fn command_damage_mob(
        &mut self,
        id: u32,
        amount: f32,
        damage: Damage,
    ) -> bool {
        let Some(index) = self.mob_index.find(&self.mobs, id) else {
            return false;
        };
        let plan =
            self.prepare_knockback_credit(id as i32, damage.attacker, self.side.combat.0.attack());
        let attack = self.side.combat.0.attack();
        let mut credit = plan.lend(&mut self.side.last_hurt_by_player);
        let mob = &mut self.mobs[index];
        let (queued, old_health) = (mob.outbox.len(), mob.health);
        let hurt = mob.damage(
            crate::game::entity::damage_level_of!(self, attack, &mut credit, None),
            amount,
            damage,
        );
        let Some((index, hurt)) = self.complete_mob_hurt(index, hurt) else {
            return false;
        };
        let health_changed = hurt && self.mobs[index].health != old_health;
        let reinforcements = if hurt {
            take_new_reinforcements(&mut self.mobs[index].outbox, queued)
        } else {
            Vec::new()
        };
        if hurt {
            if damage.attacker == Some(Who::Player) {
                self.remember_mob_player_credit(id);
                if let Some(host) = self.lan.as_mut() {
                    match self.side.lan_acting {
                        Some(conn) => {
                            host.mob_players.insert(id, conn);
                        }
                        None => {
                            host.mob_players.remove(&id);
                        }
                    }
                }
            }
            if health_changed {
                self.mob_damage_event(id);
            }
        }
        self.finish_mob_damage_at(index);
        for (mob, target) in reinforcements {
            self.call_reinforcements(mob, target);
        }
        self.settle_mob_hurt(id);
        // Enderman.hurtServer's teleport, refused hit or not, before the command returns.
        if let Err(e) = self.settle_hurt_teleports() {
            log::warn!("Sunrise: /damage: enderman teleport: {e}");
        }
        hurt
    }
    /// `VehicleEntity.hurtServer`: report acceptance even when the vehicle survives.
    /// `by` is the `DamageSource`'s direct entity, `causing` its causing entity.
    pub(in crate::game::server_side) fn command_damage_vehicle(
        &mut self,
        target: Target,
        amount: f32,
        damage_type: &str,
        direct: Option<Target>,
        causing: Option<Target>,
        position: Option<[f64; 3]>,
    ) -> bool {
        let registry = &self.health_resources.damage;
        let Some(kind) = registry.id(damage_type) else {
            return false;
        };
        let creative = self.creative_source_of(causing);
        let id = match target {
            Target::Boat(id) | Target::Cart(id) => id,
            _ => return false,
        };
        let vanilla = self.saved_refuses_damage(id, damage_type, creative);
        let ignites = registry.in_tag(kind, "minecraft:is_fire")
            || registry.in_tag(kind, "minecraft:is_explosion");
        let entity_kind = match target {
            Target::Boat(_) => self
                .boats
                .iter()
                .find(|b| b.id == id)
                .map(|b| b.entity_type()),
            Target::Cart(_) => self
                .carts
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.kind.id().to_owned()),
            _ => None,
        };
        let Some(entity_kind) = entity_kind else {
            return false;
        };
        let entity_id = |target| match target {
            Some(Target::Player) => self.own_entity_id(),
            Some(Target::Guest(conn)) => self.command_guest(conn).map_or(0, |g| g.entity_id),
            Some(target) => {
                crate::game::native_entities::target_id(target).map_or(0, |id| id as i32)
            }
            None => 0,
        };
        let source = sunrise_client::health::HurtSource {
            damage_type,
            causing: entity_id(causing),
            direct: entity_id(direct),
            position,
        };
        if crate::game::entity::damage_level_of!(self, self.side.combat.0.attack(), no_push, None)
            .invulnerability_check(
                sunrise_client::health::Hurting::Entity(id as i32, &entity_kind, source),
                vanilla,
            )
        {
            return false;
        }
        match target {
            Target::Boat(_) => self
                .boats
                .iter()
                .position(|b| b.id == id)
                .is_some_and(|i| self.hurt_boat_result(i, amount, creative).is_some()),
            Target::Cart(_) => self.carts.iter().position(|c| c.id == id).is_some_and(|i| {
                self.hurt_cart_result(
                    i,
                    amount,
                    creative,
                    ignites,
                    CartHit::command(direct, causing),
                )
            }),
            _ => false,
        }
    }
    /// The attribute map of a living entity whose attributes Sunrise does
    /// not model, created from its type's defaults on first use.
    pub(in crate::game) fn generic_attributes(
        &mut self,
        t: Target,
        attribute: &str,
    ) -> Option<&mut sunrise_world::attributes::entity::EntityAttributes> {
        if let Target::Mob(id) = t
            && (matches!(attribute, "minecraft:armor" | "minecraft:armor_toughness")
                || self.sulfur_archetype_attribute(t, attribute))
        {
            let mob = self.mobs.iter_mut().find(|m| m.id == id)?;
            if mob.extra.armor_attributes.is_none() {
                mob.extra.armor_attributes =
                    self.motion.attributes.create(&mob.kind.id).map(Box::new);
            }
            return mob.extra.armor_attributes.as_deref_mut();
        }
        if !self.side.cmds.0.attributes.contains_key(&t) {
            let ty = self.target_type(t);
            let map = self.motion.attributes.create(&ty)?;
            self.side.cmds.0.attributes.insert(t, map);
        }
        self.side.cmds.0.attributes.get_mut(&t)
    }
    pub(in crate::game::server_side) fn hurt_target_from_source(
        &mut self,
        target: Target,
        amount: f32,
        ty: &str,
        by: Option<Target>,
        cause: Option<Target>,
        from: Option<[f64; 3]>,
    ) -> bool {
        match target {
            // The causing mob is the one the hit is credited to and thorns
            // answer; the source position gives the knockback its direction.
            Target::Player | Target::Guest(_) => {
                // `DamageSource(type, by, cause)`: the death message names the
                // causing entity, else the direct one. A mob is noted by
                // `hurt_player_from`, a player here.
                let named = match cause.or(by) {
                    Some(t @ (Target::Player | Target::Guest(_))) => Some(self.target_name(t)),
                    _ => None,
                };
                let attacker = match cause.or(by) {
                    Some(Target::Mob(i)) => Some(Some(i)),
                    _ => Some(None),
                };
                // `DamageSource.isCreativePlayer`: a creative player's hit
                // passes the victim's invulnerability (#916).
                let creative_source = self.creative_source_of(cause.or(by));
                // resolvePlayerResponsibleForDamage: remember the causing player's UUID.
                let by_player = match cause.or(by) {
                    Some(Target::Player) => self.side.lan_acting,
                    Some(Target::Guest(conn)) => Some(conn),
                    _ => None,
                }
                .and_then(|conn| self.command_guest(conn))
                .map(|g| crate::game::combat::PlayerCredit(g.uuid));
                let now = self.world.game_time;
                let attack = self.current_attack();
                self.for_player(target, |g| {
                    g.with_attack(attack, |g| {
                        if let Some(name) = named {
                            g.note_direct_entity(ty, name);
                        }
                        let hurt = g.hurt_player_from(amount, from, ty, attacker, creative_source);
                        if hurt && let Some(credit) = by_player {
                            g.side.combat.0.note_hurt_by_player(credit, now);
                        }
                        hurt
                    })
                })
                .unwrap_or(false)
            }
            Target::Mob(id) => {
                let attacker = cause.or(by).and_then(|t| match t {
                    Target::Player | Target::Guest(_) => Some(Who::Player),
                    Target::Mob(i) | Target::Dragon(i) | Target::Stand(i) => Some(Who::Mob(i)),
                    _ => None,
                });
                let attacker_species = match attacker {
                    Some(Who::Mob(i)) => self.mobs.iter().find(|m| m.id == i).map(|m| m.species()),
                    _ => None,
                };
                let damage = Damage {
                    kind: Self::damage_kind(&self.health_resources.damage, ty),
                    attacker,
                    from,
                    attacker_species,
                };
                // DamageSource.getEntity is the causing player, even when
                // another client issued the command or supplied the direct entity.
                let guest = match cause.or(by) {
                    Some(Target::Guest(conn)) if self.side.lan_acting != Some(conn) => Some(conn),
                    _ => None,
                };
                if let Some(conn) = guest {
                    let attack = self.current_attack();
                    let hurt = self
                        .as_guest(conn, |g| {
                            g.with_attack(attack, |g| g.command_damage_mob(id, amount, damage))
                        })
                        .unwrap_or(false);
                    if hurt && let Some(host) = self.lan.as_mut() {
                        host.mob_players.insert(id, conn);
                    }
                    hurt
                } else {
                    self.command_damage_mob(id, amount, damage)
                }
            }
            Target::Item(id) => self.hurt_item_entity(id, amount, ty, by, cause.or(by), from),
            Target::Boat(_) | Target::Cart(_) => {
                self.command_damage_vehicle(target, amount, ty, by, cause.or(by), from)
            }
            Target::Dragon(id) => self.command_damage_dragon(
                id,
                amount,
                ty,
                Self::damage_kind(&self.health_resources.damage, ty),
                (
                    by,
                    cause.or(by),
                    from.map(|[x, y, z]| sunrise_core::geometry::Vec3::new(x, y, z)),
                ),
            ),
            Target::Crystal(id) => self.command_damage_crystal(
                id,
                ty,
                Self::damage_kind(&self.health_resources.damage, ty),
                by,
                cause.or(by),
                from,
            ),
            Target::Stand(id) => self.command_damage_stand(id, ty, cause.or(by)),
            _ => false,
        }
    }
    /// Entity.getVehicle: the player's mount, or a mob's.
    pub(in crate::game::server_side) fn ride_vehicle(&self, t: Target) -> Option<Target> {
        let id = match t {
            Target::Player => self.riding?,
            Target::Guest(conn) => self.command_guest(conn)?.vehicle?,
            Target::Mob(id) => self.mobs.iter().find(|m| m.id == id)?.extra.mount?,
            Target::Bolt(id) => self.bolts.iter().find(|b| b.id == id)?.mount?,
            _ => return None,
        };
        Some(if self.boats.iter().any(|b| b.id == id) {
            Target::Boat(id)
        } else if self.carts.iter().any(|c| c.id == id) {
            Target::Cart(id)
        } else if self.armor_stands.iter().any(|a| a.id == id) {
            Target::Stand(id)
        } else if self.side.others.0.iter().any(|o| o.id == id) {
            Target::Other(id)
        } else {
            Target::Mob(id)
        })
    }
    pub(in crate::game::server_side) fn send_player_sound(
        &mut self,
        t: Target,
        sound: &str,
        at: [f64; 3],
        volume: f32,
        pitch: f32,
        source: usize,
        seed: i64,
        committing: bool,
    ) {
        let guest = match t {
            Target::Guest(conn) => Some(conn),
            Target::Player => self.side.lan_acting,
            _ => return,
        };
        match guest.and_then(|conn| Some((conn, self.lan.as_ref()?))) {
            Some((conn, host)) => {
                let packet = host
                    .hub
                    .sound_packet(sound, at, volume, pitch, source as i32, seed);
                if t == Target::Player && !committing {
                    self.side.guest_packets.push(packet);
                } else {
                    host.hub.send_to(conn, &packet);
                }
            }
            None => {
                assert!(self.lan.is_none(), "/playsound None branch is fixture-only");
                self.server_play_sound_on(source, sound, at, volume, pitch);
            }
        }
    }
    pub(in crate::game::server_side) fn send_player_stop_sound(
        &mut self,
        targets: &[Target],
        sound: Option<&str>,
        source: Option<usize>,
        committing: bool,
    ) {
        let packet = C::StopSound(play::StopSound {
            source: source.map(|s| s as i32),
            sound: sound.map(str::to_owned),
        });
        for &t in targets {
            let guest = match t {
                Target::Guest(conn) => Some(conn),
                Target::Player => self.side.lan_acting,
                _ => continue,
            };
            match guest {
                Some(_) if t == Target::Player && !committing => {
                    self.side.guest_packets.push(packet.clone());
                }
                Some(conn) => {
                    if let Some(host) = self.lan.as_ref() {
                        host.hub.send_to(conn, &packet);
                    }
                }
                None => {
                    assert!(self.lan.is_none(), "/stopsound None branch is fixture-only");
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "attribute_missing_tests_server.rs"]
pub(in crate::game) mod attribute_missing_tests;

#[cfg(test)]
pub(in crate::game) mod allay_owner_damage_tests {
    use crate::game::native_entity_refs::tests::with_game_at;
    use sunrise_client::mobs::{Damage, Mob};
    use sunrise_core::geometry::Vec3;

    #[test]
    #[ignore = "daily"]
    fn allay_owner_adapters_compare_the_actual_player_uuid() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local/sunrise-client")
            .join(sunrise_core::VANILLA_VERSION);
        if !root.join("entity-types.json").exists() {
            eprintln!("Prepared resources unavailable; Allay Game damage coverage did not run");
            return;
        }
        with_game_at(root, |game| {
            game.build_registries();
            let actual = [101, 102, 103, 104];
            let other = [10, 20, 30, 40];
            let mut player =
                sunrise_client::motion::PlayerMotion::new(game.motion, Vec3::ZERO).unwrap();
            player.entity_id = 7;
            game.player = Some(player);
            game.level.player_uuid = Some(actual);
            assert_eq!(game.attack_of(7).player_uuid, Some(actual));
            game.side.lan_acting = Some(12);
            game.side.lan_acting_uuid = Some(sunrise_client::lan::playerdata::ints_uuid(other));
            assert_eq!(
                game.attack_of(7).player_uuid,
                Some(other),
                "the acting guest is the causing player"
            );
            game.side.lan_acting_uuid = None;
            game.level.player_uuid = None;
            assert_eq!(
                game.attack_of(7).player_uuid,
                None,
                "no invented UUID for an unknown source"
            );
            game.level.player_uuid = Some(actual);
            game.side.lan_acting = None;
            let kind = game.res.mob_types().get("minecraft:allay").unwrap().clone();
            let before = game.world.game_time;
            for melee in [true, false] {
                for liked in [Some(actual), None, Some(other)] {
                    game.mobs.clear();
                    let mut mob = Mob::new(1, kind.clone(), Vec3::new(22.5, 101.0, 22.5), 0.0);
                    mob.no_ai = true;
                    mob.extra.tame = liked.is_some();
                    mob.extra.owner = liked;
                    assert_eq!((mob.health, mob.hurt_time), (20.0, 0));
                    game.mobs.push(mob);
                    let source = Damage::player([20.5, 102.0, 20.5]);
                    let accepted = if melee {
                        game.hurt_by_player(0, 2.0, source).0
                    } else {
                        // The command entry point lends its resolved attacker to the damage path.
                        let attack = game.attack_of_who(source.attacker);
                        game.with_attack(attack, |g| g.command_damage_mob(1, 2.0, source))
                    };
                    let expected = liked != Some(actual);
                    assert_eq!(accepted, expected, "melee={melee}, liked={liked:?}");
                    assert_eq!(game.mobs[0].health, if expected { 18.0 } else { 20.0 });
                    assert_eq!(game.mobs[0].extra.owner, liked);
                    assert_eq!(game.world.game_time, before);
                }
            }
        });
    }
}

#[cfg(test)]
#[path = "registered_damage_tests_server.rs"]
pub(in crate::game) mod registered_damage_tests;

#[cfg(test)]
pub(in crate::game) mod datapack_damage_tests {
    use crate::game::commands::world_cmds::datapack_damage_tests::with_pack;
    use crate::game::{commands::Target, native_entity_refs::tests::with_game};
    use sunrise_client::mobs::Mob;
    use sunrise_core::geometry::Vec3;

    pub(in crate::game) fn pig(game: &mut crate::game::ServerGame<'_>, id: u32, at: Vec3) {
        game.side.next_entity = game.side.next_entity.max(id);
        let kind = game.res.mob_types().get("minecraft:pig").unwrap().clone();
        let mut pig = Mob::new(id, kind, at, 0.0);
        pig.no_ai = true;
        game.mobs.push(pig);
    }

    pub(in crate::game) fn run_a_datapack_explosion_type_removes_a_crystal_quietly_and_hurts_the_dragon()
     {
        with_game(|game| {
            with_pack(game);
            let crystal = |game: &mut crate::game::ServerGame<'_>, id: u32| {
                game.side.next_entity = game.side.next_entity.max(id);
                game.crystals.push(crate::game::dragon::Crystal::new(
                    id,
                    Vec3::new(0.5, 64.0, 0.5),
                    false,
                    0,
                ));
            };
            pig(game, 1, Vec3::new(2.5, 64.0, 0.5));
            crystal(game, 50);
            assert!(game.hurt_target(Target::Crystal(50), 1.0, "dmgtest:boom", None, None, None));
            assert!(game.crystals.is_empty());
            assert_eq!(game.mobs[0].health, 10.0, "no blast");
            crystal(game, 51);
            assert!(game.hurt_target(Target::Crystal(51), 1.0, "dmgtest:zap", None, None, None));
            assert!(game.crystals.is_empty());
            assert!(
                game.mobs.first().is_none_or(|pig| pig.health < 10.0),
                "the crystal's blast"
            );

            let dragon = game
                .spawn_dragon_spawned([0.5, 80.0, 0.5], crate::game::creatures::SpawnSite::Plain)
                .unwrap();
            let before = game.dragon.as_ref().unwrap().health;
            assert!(game.hurt_target(
                Target::Dragon(dragon),
                4.0,
                "dmgtest:boom",
                None,
                None,
                None
            ));
            assert!(game.dragon.as_ref().unwrap().health < before);
        });
    }
}

#[cfg(test)]
pub(in crate::game) mod vehicle_damage_command_tests {
    use crate::game::commands::world_cmds::vehicle_damage_command_tests::attribute_chat_count;
    use crate::game::commands::world_cmds::vehicle_damage_command_tests::with_game;
    use std::sync::Arc;

    pub(in crate::game::server_side) fn attribute_guests(
        game: &mut crate::game::ServerGame<'_>,
    ) -> Vec<std::sync::mpsc::Receiver<sunrise_client::lan::Out>> {
        use std::sync::{atomic::AtomicI32, mpsc};
        use sunrise_client::lan::{Joined, NetConfig, PlayerSender, guest::Guest, hub::HubConfig};
        use sunrise_protocol::packets::configuration::ClientInformation;
        game.side.world_root = game.options.game_dir.as_ref().unwrap().join("h082-world");
        std::fs::create_dir_all(&game.side.world_root).unwrap();
        game.lan_host(
            crate::game::lan::LanSettings {
                port: 0,
                guest_commands: true,
                force_game_mode: false,
            },
            NetConfig {
                announce: false,
                ..NetConfig::loopback(0, "attribute-test".into(), 2)
            },
            HubConfig {
                default_game_mode: 1,
                guest_commands: true,
                force_game_mode: false,
                hardcore: false,
                view_distance: 2,
                simulation_distance: 2,
                dimension: "minecraft:overworld".into(),
                allow_flight: false,
                seed: 0,
                max_players: 3,
                integrated: false,
                levels: Default::default(),
                level_seeds: Default::default(),
            },
        )
        .unwrap();
        let mut receivers = Vec::new();
        for (conn, name, position) in [
            (2, "Bob", [7.5, 100.0, 4.5]),
            (3, "Carol", [10.5, 100.0, 4.5]),
        ] {
            let (tx, rx) = mpsc::channel();
            let mut guest = Guest::new(
                Joined {
                    conn,
                    uuid: sunrise_protocol::crypto::offline_uuid(name),
                    name: name.into(),
                    properties: Vec::new(),
                    info: ClientInformation::new(2),
                    brand: "test".into(),
                    sender: PlayerSender::new(tx, Arc::default()),
                    latency: Arc::new(AtomicI32::new(0)),
                    address: "127.0.0.1:25565".parse().unwrap(),
                    expected: false,
                },
                conn as i32,
                position,
                2,
            );
            guest.dimension = "minecraft:overworld".into();
            guest.placed = true;
            guest.loaded = true;
            guest.game_mode = 1;
            game.lan.as_mut().unwrap().hub.guests.push(guest);
            receivers.push(rx);
        }
        let host = game.lan.as_mut().unwrap();
        for name in ["Bob", "Carol"] {
            host.hub
                .ops
                .insert(sunrise_protocol::crypto::offline_uuid(name), 2);
        }
        host.hub.refresh_permissions();
        assert!(host.hub.guests.iter().all(|guest| guest.permission == 2));
        receivers
    }

    #[test]
    #[ignore = "daily"]
    fn direct_guest_attributes_adopt_the_target_and_restore_the_issuer() {
        use crate::game::commands::Target;
        use sunrise_client::motion::PlayerMotion;
        use sunrise_core::geometry::Vec3;
        with_game(|game| {
            game.player = Some(PlayerMotion::new(game.motion, Vec3::new(4.5, 100.0, 4.5)).unwrap());
            crate::game::native_entity_refs::tests::bind_local_player(game, 1, 99);
            game.creative = true;
            assert!(game.run_command("fill 0 99 0 15 99 15 stone"));
            assert!(game.run_command("attribute @s minecraft:armor base set 2"));
            let host_uuid = game.acting_uuid();
            let clock = game.world.game_time;
            let dimension = game.dim;
            let receivers = attribute_guests(game);
            for (conn, name) in [(2, "Bob"), (3, "Carol")] {
                let g = game.command_guest(conn).unwrap();
                assert_eq!(g.uuid, sunrise_protocol::crypto::offline_uuid(name));
                assert_eq!(g.health, 20.0);
                assert!(g.loaded && g.placed);
                assert_eq!(g.game_mode, 1);
                assert_eq!(
                    game.for_player(Target::Guest(conn), |g| g
                        .player
                        .as_mut()
                        .unwrap()
                        .attribute("minecraft:armor")
                        .unwrap()),
                    Some(0.0)
                );
            }
            // Existing guest-issued Player path is positive before direct Guest dispatch.
            assert_eq!(
                game.for_player(Target::Guest(2), |g| g
                    .run_command("attribute @s minecraft:armor base set 3")),
                Some(true)
            );
            assert_eq!(
                game.for_player(Target::Guest(2), |g| g
                    .player
                    .as_mut()
                    .unwrap()
                    .attribute("minecraft:armor")
                    .unwrap()),
                Some(3.0)
            );
            for rx in &receivers {
                attribute_chat_count(rx);
            }
            game.side.chat_capture = Some(Vec::new());
            let accepted = game.run_command("attribute Bob minecraft:armor base set 4");
            let issuer = game.side.chat_capture.take().unwrap();
            // This assertion fails old on the actual guest model, not feedback alone.
            assert_eq!(
                game.for_player(Target::Guest(2), |g| g
                    .player
                    .as_mut()
                    .unwrap()
                    .attribute("minecraft:armor")
                    .unwrap()),
                Some(4.0)
            );
            assert!(accepted);
            assert_eq!(issuer.len(), 1);
            assert!(issuer[0].plain().contains("Bob"));
            assert_eq!(
                attribute_chat_count(&receivers[0]),
                0,
                "success belongs to issuer, not Bob"
            );
            assert_eq!(
                game.player
                    .as_mut()
                    .unwrap()
                    .attribute("minecraft:armor")
                    .unwrap(),
                2.0
            );
            assert_eq!(
                game.for_player(Target::Guest(3), |g| g
                    .player
                    .as_mut()
                    .unwrap()
                    .attribute("minecraft:armor")
                    .unwrap()),
                Some(0.0)
            );
            assert_eq!(
                game.for_player(Target::Guest(2), |g| g
                    .player
                    .as_mut()
                    .unwrap()
                    .attribute("minecraft:max_health")
                    .unwrap()),
                Some(20.0)
            );
            game.side.chat_capture = Some(Vec::new());
            assert!(
                !game.run_command("attribute Bob minecraft:armor modifier remove h082:missing")
            );
            let errors = game.side.chat_capture.take().unwrap();
            assert_eq!(errors.len(), 1);
            assert!(errors[0].plain().contains("h082:missing"));
            assert_eq!(
                attribute_chat_count(&receivers[0]),
                0,
                "error returns to issuer"
            );
            game.side.chat_capture = Some(Vec::new());
            assert!(game.run_command("attribute Bob minecraft:armor base reset"));
            assert_eq!(game.side.chat_capture.take().unwrap().len(), 1);
            assert_eq!(
                game.for_player(Target::Guest(2), |g| g
                    .player
                    .as_mut()
                    .unwrap()
                    .attribute("minecraft:armor")
                    .unwrap()),
                Some(0.0)
            );
            assert_eq!(game.acting_uuid(), host_uuid);
            assert_eq!(game.side.lan_acting, None);
            assert_eq!(game.dim, dimension);
            assert_eq!(game.world.game_time, clock);
            assert_eq!(game.player.as_ref().unwrap().health.health(), 20.0);
        });
    }

    #[test]
    #[ignore = "daily"]
    fn existing_attribute_operations_keep_their_exact_success_messages() {
        use sunrise_client::motion::PlayerMotion;
        use sunrise_core::geometry::Vec3;
        with_game(|game| {
            game.player = Some(PlayerMotion::new(game.motion, Vec3::new(4.5, 200.0, 4.5)).unwrap());
            crate::game::native_entity_refs::tests::bind_local_player(game, 1, 99);
            assert!(
                game.run_command("summon pig 6.5 200 4.5 {UUID:[I;82,0,0,1],NoAI:1b,NoGravity:1b}")
            );
            assert!(
                game.run_command("summon armor_stand 8.5 200 4.5 {UUID:[I;82,0,0,2],NoGravity:1b}")
            );
            for target in [
                "@s",
                "00000052-0000-0000-0000-000000000001",
                "00000052-0000-0000-0000-000000000002",
            ] {
                for operation in [
                    "get",
                    "base get",
                    "base set 2",
                    "base get",
                    "base reset",
                    "modifier add h082:reply 1 add_value",
                    "modifier value get h082:reply",
                    "get",
                    "modifier remove h082:reply",
                ] {
                    let command = format!("attribute {target} minecraft:armor {operation}");
                    game.side.chat_capture = Some(Vec::new());
                    game.side.cmds.0.results = Some(Vec::new());
                    assert!(game.run_command(&command), "{command}");
                    let messages = game.side.chat_capture.take().unwrap();
                    assert_eq!(messages.len(), 1, "{command}");
                    let components: Vec<_> = messages.iter().map(|m| m.component()).collect();
                    println!(
                        "H082_EXISTING {}",
                        serde_json::json!({"command":command,"messages":components,"results":game.side.cmds.0.results.take().unwrap()})
                    );
                }
            }
        });
    }
}

#[cfg(test)]
#[path = "attribute_tests_server.rs"]
pub(in crate::game) mod attribute_tests;

#[cfg(test)]
#[path = "effect_attribute_tests_server.rs"]
pub(in crate::game) mod effect_attribute_tests;
