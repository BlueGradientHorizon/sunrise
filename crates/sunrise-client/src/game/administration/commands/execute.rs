//! `/execute` (ExecuteCommand): walk the parsed redirect chain, forking and
//! filtering command sources, then run the final command once per source,
//! reporting each result to the `store` callbacks of its source.
#[allow(unused_imports)]
use super::{Arg, Context, Dim, Game, R, Target, data::DataRef, err, wrap_degrees};
#[allow(unused_imports)]
use sunrise_client::{command::CommandError, nbt_path::NbtPath, text::Text};
#[allow(unused_imports)]
use sunrise_core::geometry::trig;
#[allow(unused_imports)]
use sunrise_core::nbt::Tag;

/// `CommandSourceStack` fields Sunrise models: the executing entity (None for
/// the server), position, rotation, anchor, whether feedback is suppressed
/// and the chain of `store` callbacks.
#[derive(Clone, Copy, Debug)]
pub(in crate::game) struct Source {
    pub entity: Option<Target>,
    pub pos: [f64; 3],
    pub rot: [f32; 2],
    /// EntityAnchorArgument.Anchor: eyes (true) or feet.
    pub eyes: bool,
    /// `withSuppressedOutput`: functions do not print feedback.
    pub silent: bool,
    /// Index of the newest `store` in `ExecState::stores`.
    pub store: Option<u32>,
    /// The level this command source uses. Sunrise has only this dimension's
    /// chunks loaded in the single-player client.
    pub dimension: Dim,
    /// Output origin, unchanged by `execute in` or `execute at`.
    pub origin_dimension: Dim,
    /// Whether a player submitted this source, unchanged by executor modifiers.
    pub origin_player: bool,
    /// The dimension `execute in` named, if any: a server teleports its
    /// players there (`TeleportCommand` into the source's level).
    pub level: Option<super::super::Dim>,
    /// The command block whose output this source is
    /// (`BaseCommandBlock.CloseableCommandBlockSource`).
    pub block: Option<(i32, i32, i32)>,
    /// The command block minecart whose output this source is.
    pub cart: Option<u32>,
}

/// Where `execute store` writes.
#[derive(Clone, Debug)]
pub(in crate::game) enum StoreTarget {
    Score(Vec<String>, String),
    Bossbar(String, bool),
    Data(DataRef, NbtPath, String, f64),
}
#[derive(Clone, Debug)]
pub(in crate::game) struct StoreEntry {
    /// `store result` (true) or `store success`.
    pub(in crate::game) result: bool,
    pub(in crate::game) target: StoreTarget,
    pub(in crate::game) parent: Option<u32>,
}
/// Command execution state shared across nested commands and functions.
#[derive(Default)]
pub(in crate::game) struct ExecState {
    pub(in crate::game) stores: Vec<StoreEntry>,
    /// Commands run since the outermost one began.
    pub commands: u32,
    /// What the last leaf command reports to its `store` callbacks: None for
    /// the default, `Some(None)` for nothing (a function that did not
    /// `return`), `Some(Some((success, result)))` for an explicit value.
    pub callback: Option<Option<(bool, i32)>>,
    /// The innermost function frame's `return`.
    pub returned: Option<Option<i32>>,
    /// Function IDs in execution order, with the active frame last.
    pub functions: Vec<String>,
    pub prepared_function: Option<super::super::functions::PreparedFunctionCall>,
}

/// A modifier that brings the sources to `max_command_forks` ends the chain
/// (`BuildContexts.ERROR_FORK_LIMIT_REACHED`); the error is only shown when
/// no fork came before it (`CommandSourceStack.handleError`).
pub(in crate::game) fn fork_limit_reached(forked: bool, limit: usize) -> R<i32> {
    if forked {
        Ok(0)
    } else {
        Err(err("command.forkLimit", &[&limit]))
    }
}

/// Java's `(int)`/`(short)`/`(byte)` narrowing of a double.
pub(in crate::game) fn java_int(v: f64) -> i32 {
    v as i32
}

impl Game<'_, crate::game::server_side::ServerState> {
    /// The source a command typed by the player starts from.
    pub(in crate::game) fn player_source(&self) -> Source {
        Source {
            // The server console has no entity (CommandSourceStack of the server).
            entity: self.player.is_some().then_some(Target::Player),
            pos: self.source_position(),
            rot: self.source_rotation(),
            eyes: false,
            silent: false,
            store: None,
            dimension: self.dim,
            origin_dimension: self.dim,
            origin_player: self.player.is_some(),
            level: None,
            block: None,
            cart: None,
        }
    }
    /// The server as a source (function ticks and schedules,
    /// `MinecraftServer.createCommandSourceStack`): the world spawn, in its level
    /// (`findRespawnDimension`), whichever level is in place.
    pub(in crate::game) fn server_source(&self) -> Source {
        let level = self.world_spawn_level();
        Source {
            entity: None,
            pos: self.server_position(),
            rot: [0.0, 0.0],
            eyes: false,
            silent: true,
            store: None,
            dimension: level,
            origin_dimension: level,
            origin_player: false,
            level: None,
            block: None,
            cart: None,
        }
    }
    /// Server-side `Entity.getEyeHeight`: reads server guest state, entity tables, and saved interaction height.
    pub(in crate::game) fn server_target_eye_height(&self, t: Target) -> f64 {
        match t {
            Target::Player => self
                .player
                .as_ref()
                .map_or(1.62, |p| f64::from(p.dimensions.eye_height)),
            Target::Guest(conn) => self.command_guest(conn).map_or(1.62, |g| g.eye_height()),
            Target::Mob(id) => self
                .mobs
                .iter()
                .find(|m| m.id == id)
                .map_or(0.0, |m| m.eye_y() - m.position.y),
            // ArmorStand.getDefaultDimensions: a marker's are empty, a small
            // stand's are BABY_DIMENSIONS, whose eye height is 0.9875F.
            Target::Stand(id) => match self.armor_stands.iter().find(|a| a.id == id) {
                Some(a) if a.marker => 0.0,
                Some(a) if a.small => f64::from(0.9875f32),
                _ => self.type_eye_height(&self.target_type(t)),
            },
            Target::Other(id) => self
                .side
                .others
                .0
                .iter()
                .find(|o| o.id == id)
                .and_then(|o| o.interaction)
                .map_or_else(
                    || self.type_eye_height(&self.target_type(t)),
                    |i| f64::from(i.height * 0.85),
                ),
            // Entity.getEyeHeight: the type's dimensions.
            _ => self.type_eye_height(&self.target_type(t)),
        }
    }
    /// EntityAnchorArgument.Anchor.apply.
    pub(in crate::game::server_side) fn anchor_position(&self, s: &Source) -> [f64; 3] {
        match s.entity {
            Some(t) if s.eyes => [
                s.pos[0],
                s.pos[1] + self.server_target_eye_height(t),
                s.pos[2],
            ],
            _ => s.pos,
        }
    }

    // ---- running commands -----------------------------------------------

    /// Run one command line with `source` (no error reporting): the entry
    /// for functions and `execute run`.
    pub(in crate::game) fn execute_line(&mut self, line: &str, source: Source) -> R<i32> {
        let d = self.ui.dispatcher.0.clone();
        let parse = d.parse(line, &self.ui.registries);
        let (ctx, _) = parse.executable()?;
        let ctx = ctx.clone();
        let root = parse.context.clone();
        self.with_source(source, |g| g.run_chain(&root, &ctx))
    }
    pub(in crate::game::server_side) fn store_value(&mut self, target: &StoreTarget, value: i32) {
        match target {
            StoreTarget::Score(holders, objective) => {
                for h in holders {
                    self.score.board.set(h, objective, value);
                }
            }
            StoreTarget::Bossbar(id, is_value) => self.set_bossbar_value(id, *is_value, value),
            StoreTarget::Data(r, path, kind, scale) => {
                let Ok(mut data) = self.data_of(r) else {
                    return;
                };
                let v = f64::from(value) * scale;
                let tag = match kind.as_str() {
                    "int" => Tag::Int(java_int(v)),
                    "float" => Tag::Float((v as f32).to_bits()),
                    "short" => Tag::Short(java_int(v) as i16),
                    "long" => Tag::Long(v as i64),
                    "double" => Tag::Double(v.to_bits()),
                    _ => Tag::Byte(java_int(v) as i8),
                };
                if path.set(&mut data, &tag).is_ok() {
                    let _ = self.set_data_of(r, data);
                }
            }
        }
    }

    /// The sources one modifier produces from `s`.
    pub(in crate::game) fn modify(&mut self, word: &str, c: &Context, s: Source) -> R<Vec<Source>> {
        let targets = |g: &mut Self| -> R<Vec<Target>> {
            // An empty selection forks to nothing rather than failing.
            match c.get("targets") {
                Some(Arg::Entity(sel)) => {
                    let sel = sel.clone();
                    Ok(g.select(&sel))
                }
                _ => Ok(Vec::new()),
            }
        };
        Ok(match word {
            "as" => targets(self)?
                .into_iter()
                .map(|t| Source {
                    entity: Some(t),
                    ..s
                })
                .collect(),
            "at" => targets(self)?
                .into_iter()
                .filter_map(|t| {
                    // CommandSourceStack.withLevel(entity.level()).
                    let level = self.target_level(t);
                    Some(Source {
                        pos: self.target_position(t)?,
                        rot: self.target_rotation(t),
                        dimension: level,
                        level: (level != self.dim).then_some(level),
                        ..s
                    })
                })
                .collect(),
            "positioned" if c.has("as") => targets(self)?
                .into_iter()
                .filter_map(|t| {
                    // withPosition keeps the anchor (only `positioned <pos>`
                    // resets it to the feet).
                    Some(Source {
                        pos: self.target_position(t)?,
                        ..s
                    })
                })
                .collect(),
            "positioned" if c.has("over") => {
                let Some(Arg::Str(kind)) = c.get("heightmap") else {
                    return Ok(vec![s]);
                };
                let (x, z) = (s.pos[0].floor() as i32, s.pos[2].floor() as i32);
                if !self.world.chunks.contains_key(&(x >> 4, z >> 4)) {
                    return Err(err("argument.pos.unloaded", &[]));
                }
                let y = self.heightmap_at(kind, x, z);
                vec![Source {
                    pos: [s.pos[0], f64::from(y), s.pos[2]],
                    ..s
                }]
            }
            "positioned" => {
                let pos = self
                    .position_arg(c, "pos")
                    .ok_or_else(|| err("argument.pos.missing.double", &[]))?;
                vec![Source {
                    pos,
                    eyes: false,
                    ..s
                }]
            }
            "rotated" if c.has("as") => targets(self)?
                .into_iter()
                .map(|t| Source {
                    rot: self.target_rotation(t),
                    ..s
                })
                .collect(),
            "rotated" => {
                let rot = match c.get("rot") {
                    Some(Arg::Coords(r)) => r.rotation(s.rot),
                    _ => s.rot,
                };
                vec![Source { rot, ..s }]
            }
            "facing" if c.has("entity") => {
                let eyes = matches!(c.get("anchor"), Some(Arg::Anchor(true)));
                targets(self)?
                    .into_iter()
                    .filter_map(|t| {
                        let p = self.target_position(t)?;
                        let y = if eyes {
                            p[1] + self.server_target_eye_height(t)
                        } else {
                            p[1]
                        };
                        Some(self.facing(s, [p[0], y, p[2]]))
                    })
                    .collect()
            }
            "facing" => {
                let to = self
                    .position_arg(c, "pos")
                    .ok_or_else(|| err("argument.pos.missing.double", &[]))?;
                vec![self.facing(s, to)]
            }
            "align" => {
                let Some(Arg::Str(axes)) = c.get("axes") else {
                    return Ok(vec![s]);
                };
                let mut pos = s.pos;
                for (i, axis) in ['x', 'y', 'z'].into_iter().enumerate() {
                    if axes.contains(axis) {
                        pos[i] = pos[i].floor();
                    }
                }
                vec![Source { pos, ..s }]
            }
            "anchored" => vec![Source {
                eyes: matches!(c.get("anchor"), Some(Arg::Anchor(true))),
                ..s
            }],
            "in" => {
                // CommandSourceStack.withLevel scales the source position;
                // absolute coordinates passed to /tp are unaffected.
                let Some(Arg::Resource(dim)) = c.get("dimension") else {
                    return Ok(vec![s]);
                };
                if !Dim::exists(dim) {
                    return Err(err("argument.dimension.invalid", &[dim]));
                }
                let dimension = Dim::from_id(dim);
                let scale = sunrise_world::dimension_type::DimensionFacts::teleportation_scale(
                    self.dimension_type_of(s.dimension),
                    self.dimension_type_of(dimension),
                );
                vec![Source {
                    pos: [s.pos[0] * scale, s.pos[1], s.pos[2] * scale],
                    dimension,
                    level: Some(dimension),
                    ..s
                }]
            }
            "summon" => {
                let Some(Arg::Resource(kind)) = c.get("entity").cloned() else {
                    return Ok(vec![s]);
                };
                // ExecuteCommand.spawnEntityAndRedirect calls SummonCommand.createEntity.
                if matches!(
                    kind.as_str(),
                    "minecraft:player" | "minecraft:fishing_bobber"
                ) {
                    return Err(err("entity.not_summonable", &[&kind]));
                }
                Self::check_summon_bounds(s.pos)?;
                if self.difficulty_id() == 0
                    && self
                        .res
                        .mob_types()
                        .get(&kind)
                        .is_some_and(|t| !t.allowed_in_peaceful())
                {
                    return Err(err("commands.summon.failed.peaceful", &[]));
                }
                let target = self.summon_with_nbt(&kind, s.pos, "{}")?;
                let Some(t) = target else {
                    return Err(err("commands.summon.failed", &[]));
                };
                vec![Source {
                    entity: Some(t),
                    ..s
                }]
            }
            "on" => self.relation(c, s),
            "store" => vec![self.store_source(c, s)?],
            _ => return Err(err("command.unknown.command", &[])),
        })
    }
    /// The highest block of a heightmap type at a column (`Level.getHeight`).
    pub(in crate::game::server_side) fn heightmap_at(&self, kind: &str, x: i32, z: i32) -> i32 {
        let top = self.world.top_y();
        let facts = self.res.facts();
        for y in (self.world.bottom_y()..top).rev() {
            let state = self.world.block(x, y, z);
            // `Heightmap.NOT_AIR` is `!state.isAir()`, so cave air and void
            // air are skipped as well as air.
            if facts.is_air(state) {
                continue;
            }
            let facts = facts.get(state);
            let blocks = facts.legacy_solid || facts.fluid != sunrise_client::blocks::Fluid::None;
            let leaves = self
                .pack
                .assets
                .registry
                .state(state)
                .is_some_and(|s| s.name.ends_with("_leaves"));
            let found = match kind {
                "ocean_floor" => facts.legacy_solid,
                "motion_blocking" => blocks,
                "motion_blocking_no_leaves" => blocks && !leaves,
                _ => true,
            };
            if found {
                return y + 1;
            }
        }
        self.world.bottom_y()
    }
    /// CommandSourceStack.facing: rotation from the anchor point to `to`.
    pub(in crate::game) fn facing(&self, s: Source, to: [f64; 3]) -> Source {
        let from = self.anchor_position(&s);
        let (dx, dy, dz) = (to[0] - from[0], to[1] - from[1], to[2] - from[2]);
        let horizontal = (dx * dx + dz * dz).sqrt();
        // `CommandSourceStack.facing`: `Mth.atan2` times the folded
        // `Mth.RAD_TO_DEG`, cast to float, then `Mth.wrapDegrees`. The yaw
        // stays in -180..180, so a south-east target is -135 and not 225.
        let pitch = wrap_degrees(-trig::atan2_degrees(dy, horizontal));
        let yaw = wrap_degrees(trig::atan2_degrees(dz, dx) - 90.0);
        Source {
            rot: [yaw, pitch],
            ..s
        }
    }

    // ---- conditions ----------------------------------------------------------

    /// Whether the source stays for `if`/`unless` (`want`), and the count the
    /// numeric conditions report.
    pub(in crate::game) fn condition(&mut self, c: &Context, want: bool) -> R<(bool, i32)> {
        if c.has("function") {
            return self.condition_function(c, want);
        }
        let (matched, count) = self.condition_matches(c)?;
        Ok((matched == want, count))
    }
    /// `FallthroughTask.execute`: a fallthrough supplies failure/zero to the
    /// function-condition continuation. Preserve explicit-result decisions
    /// and direct function callbacks.
    pub(in crate::game::server_side) fn condition_function(
        &mut self,
        c: &Context,
        want: bool,
    ) -> R<(bool, i32)> {
        let Some(Arg::Resource(name)) = c.get("name").cloned() else {
            return Ok((false, 0));
        };
        let results = self.call_function_ref(&name, None)?;
        let keep = results.iter().any(|r| r.is_some_and(|v| (v != 0) == want));
        let fallthrough = !results.is_empty() && results.iter().all(Option::is_none);
        Ok((keep || (!want && fallthrough), 0))
    }

    pub(in crate::game::server_side) fn condition_stopwatch(&mut self, c: &Context) -> R<bool> {
        let Some(Arg::Resource(id)) = c.get("id").cloned() else {
            return Ok(false);
        };
        let Some(Arg::DoubleRange((lo, hi))) = c.get("range").cloned() else {
            return Ok(false);
        };
        let Some(t) = self.score.stopwatches.elapsed_seconds(&id) else {
            return Err(err("commands.stopwatch.does_not_exist", &[&id]));
        };
        Ok(lo.is_none_or(|l| t >= l) && hi.is_none_or(|h| t <= h))
    }
    pub(in crate::game::server_side) fn condition_score(&mut self, c: &Context) -> R<bool> {
        let target = self.score_holder_names(c, "target", false)?;
        let Some(Arg::Str(to)) = c.get("targetObjective").cloned() else {
            return Ok(false);
        };
        if self.score.board.objective(&to).is_none() {
            return Err(err("arguments.objective.notFound", &[&to]));
        }
        if c.has("matches") {
            let Some(Arg::IntRange((lo, hi))) = c.get("range") else {
                return Ok(false);
            };
            let Some(a) = target
                .first()
                .and_then(|t| self.score.board.score(t, &to))
                .map(|s| s.value)
            else {
                return Ok(false);
            };
            return Ok(lo.is_none_or(|l| a >= l) && hi.is_none_or(|h| a <= h));
        }
        let source = self.score_holder_names(c, "source", false)?;
        let Some(Arg::Str(so)) = c.get("sourceObjective").cloned() else {
            return Ok(false);
        };
        if self.score.board.objective(&so).is_none() {
            return Err(err("arguments.objective.notFound", &[&so]));
        }
        let Some(a) = target
            .first()
            .and_then(|t| self.score.board.score(t, &to))
            .map(|s| s.value)
        else {
            return Ok(false);
        };
        let Some(b) = source
            .first()
            .and_then(|t| self.score.board.score(t, &so))
            .map(|s| s.value)
        else {
            return Ok(false);
        };
        let op = c.literals.last().map_or("", String::as_str);
        Ok(match op {
            "=" => a == b,
            "<" => a < b,
            "<=" => a <= b,
            ">" => a > b,
            ">=" => a >= b,
            _ => false,
        })
    }
    /// `if blocks`: the region at `start..end` equals the one at `destination`.
    pub(in crate::game::server_side) fn condition_blocks(&mut self, c: &Context) -> R<(bool, i32)> {
        let (a, b, d) = (
            self.block_pos_arg(c, "start")?,
            self.block_pos_arg(c, "end")?,
            self.block_pos_arg(c, "destination")?,
        );
        let masked = c.has("masked");
        let lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
        let hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
        let area = i64::from(hi[0] - lo[0] + 1)
            * i64::from(hi[1] - lo[1] + 1)
            * i64::from(hi[2] - lo[2] + 1);
        if area > 32768 {
            return Err(err("commands.execute.blocks.toobig", &[&32768, &area]));
        }
        let offset = [d[0] - lo[0], d[1] - lo[1], d[2] - lo[2]];
        let mut count = 0;
        for z in lo[2]..=hi[2] {
            for y in lo[1]..=hi[1] {
                for x in lo[0]..=hi[0] {
                    let src = self.world.block(x, y, z);
                    if masked && src == self.world.air {
                        continue;
                    }
                    let (dx, dy, dz) = (x + offset[0], y + offset[1], z + offset[2]);
                    if src != self.world.block(dx, dy, dz) {
                        return Ok((false, 0));
                    }
                    let (se, de) = (
                        self.world.block_entities.get(&(x, y, z)),
                        self.world.block_entities.get(&(dx, dy, dz)),
                    );
                    if let Some(se) = se {
                        let name = self.data_block_name([x, y, z]);
                        let Some(de) = de else {
                            return Ok((false, 0));
                        };
                        let st =
                            self.encode_live_block_entity((x, y, z), se, &name, self.res.items());
                        let dt = self.encode_live_block_entity(
                            (dx, dy, dz),
                            de,
                            &name,
                            self.res.items(),
                        );
                        // Each capture receives its real position; saveCustomOnly compares payloads.
                        let without_pos = |mut tag: sunrise_core::nbt::Tag| {
                            if let sunrise_core::nbt::Tag::Compound(fields) = &mut tag {
                                for key in ["x", "y", "z"] {
                                    fields.remove(&key.into());
                                }
                            }
                            tag
                        };
                        if without_pos(st) != without_pos(dt) {
                            return Ok((false, 0));
                        }
                    }
                    count += 1;
                }
            }
        }
        Ok((true, count))
    }
    /// The biome name at a block position.
    pub(in crate::game::server_side) fn biome_name_at_block(&self, p: [i32; 3]) -> Option<String> {
        let chunk = self.world.chunks.get(&(p[0] >> 4, p[2] >> 4))?;
        // The world's table (`world_biomes!`): a packed biome's id is past
        // the built-in list, and `Biomes::get` would clamp it to the last
        // vanilla one.
        Some(
            world_biomes!(self)
                .get(chunk.biome(p[0], p[1], p[2]))
                .name
                .clone(),
        )
    }
    /// A biome id or `#tag` matched against a biome name.
    pub(in crate::game) fn biome_matches(&self, wanted: &str, biome: &str) -> bool {
        match wanted.strip_prefix('#') {
            Some(tag) => self.biome_tag_has(tag, biome),
            None => wanted == biome,
        }
    }
    /// `#tag`: the biome tags the game's files and the world's packs define,
    /// nested tags expanded; an unknown tag falls back to the game's built-in
    /// table.
    pub(in crate::game::server_side) fn biome_tag_has(&self, tag: &str, biome: &str) -> bool {
        let entries = self.tag_entries("worldgen/biome", tag);
        entries.iter().any(|id| id == biome)
            || (entries.is_empty() && self.res.mob_data().biome_has(tag, biome))
    }
}

/// Whether `uuid` is the player in place (`Target::Player`): during a guest's turn that is the guest, not the
/// level's own player (the world owner), whom `Level.getEntity(UUID)` finds only while they are in the level.
pub(in crate::game) fn is_player_in_place(
    uuid: [i32; 4],
    acting: Option<u128>,
    level_player: Option<[i32; 4]>,
) -> bool {
    match acting {
        Some(guest) => sunrise_client::lan::playerdata::ints_uuid(uuid) == guest,
        None => Some(uuid) == level_player,
    }
}

#[cfg(test)]
#[path = "execute_facing_tests.rs"]
pub(in crate::game) mod execute_facing_tests;

#[cfg(test)]
pub(in crate::game) mod tests {
    use super::is_player_in_place;
    use sunrise_client::lan::playerdata::ints_uuid;

    const OWNER: [i32; 4] = [0, 0, 0, 12];
    const GUEST: [i32; 4] = [0, 0, 0, 13];

    /// A owns the world, B is a guest: the sequence of one command each way.
    #[test]
    fn the_owners_uuid_is_not_the_guest_in_place() {
        // A's own command, or the single-player one: no guest in place, the level's player is.
        assert!(is_player_in_place(OWNER, None, Some(OWNER)));
        assert!(!is_player_in_place(GUEST, None, Some(OWNER)));
        // B's turn: only B is in place, the owner's UUID names someone else.
        let b = Some(ints_uuid(GUEST));
        assert!(is_player_in_place(GUEST, b, Some(OWNER)));
        assert!(!is_player_in_place(OWNER, b, Some(OWNER)));
        // A's own turn as a guest of an integrated server.
        let a = Some(ints_uuid(OWNER));
        assert!(is_player_in_place(OWNER, a, Some(OWNER)));
        assert!(!is_player_in_place(GUEST, a, Some(OWNER)));
        // No level player at all (a dedicated server): nobody is in place without a guest.
        assert!(!is_player_in_place(OWNER, None, None));
    }
}

#[cfg(test)]
pub(in crate::game) mod passenger_relation_tests {
    #[allow(unused_imports)]
    use super::*;
    #[allow(unused_imports)]
    use crate::game::native_entity_refs::tests::with_game;
    #[allow(unused_imports)]
    use sunrise_core::nbt::Tag;
    #[allow(unused_imports)]
    use sunrise_core::position::BlockPos;

    pub(in crate::game) fn actor(i: usize) -> String {
        format!("00000096-0000-0002-0000-{i:012x}")
    }
    pub(in crate::game) fn value(
        game: &crate::game::ServerGame<'_>,
        i: usize,
        objective: &str,
    ) -> i32 {
        game.score.board.score(&actor(i), objective).unwrap().value
    }
    pub(in crate::game) fn vehicle(
        game: &mut crate::game::ServerGame<'_>,
        child: usize,
        parent: usize,
    ) {
        assert!(game.run_command("scoreboard players set #r h150u 91"));
        assert!(game.run_command("scoreboard players set #s h150u 17"));
        assert!(game.run_command(&format!(
            "execute store result score #r h150u store success score #s h150u as {} on vehicle if entity @s[nbt={{UUID:[I;150,2,0,{parent}]}}]", actor(child)
        )));
        assert_eq!(game.score.board.score("#r", "h150u").unwrap().value, 1);
        assert_eq!(game.score.board.score("#s", "h150u").unwrap().value, 1);
    }
}

#[cfg(test)]
pub(in crate::game) mod terminal_fork_tests {
    #[allow(unused_imports)]
    use super::Game;
    #[allow(unused_imports)]
    use crate::game::{Audio, Options, Progress, Settings, load_resources};
    #[allow(unused_imports)]
    use std::sync::Arc;
    #[allow(unused_imports)]
    use sunrise_client::chunk::Chunk;
    #[allow(unused_imports)]
    use sunrise_core::position::ChunkPos;

    #[allow(unused_imports)]
pub(in crate::game) use crate::game::server_side::commands_server::execute::terminal_fork_tests::capture;

    #[test]
    #[ignore = "daily"]
    fn terminal_conditions_keep_the_fork_state_and_callbacks() {
        let root = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            crate::game::execution_role::ExecutionRole::IntegratedServer,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.game_dir = Some(std::env::current_dir().unwrap().join("game"));
        options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
        let mut game = crate::game::ServerGame::new(
            &loaded.resources,
            loaded.pack,
            loaded.gui.unwrap(),
            options,
            settings,
            None,
        )
        .unwrap();
        game.build_registries();
        game.set_allow_commands(true);
        let mut chunk = Chunk::empty(0, 0);
        let stone = game
            .pack
            .assets
            .registry
            .default_state("minecraft:stone")
            .unwrap();
        for x in 0..16 {
            for z in 0..16 {
                chunk.set(x, 199, z, stone.0);
            }
        }
        game.world.chunks.insert((0, 0), Arc::new(chunk));
        game.block_ticks
            .add_empty_chunk(ChunkPos::new(0, 0))
            .unwrap();
        assert!(game.run_command("forceload add 0 0"));
        assert!(game.run_command("scoreboard objectives add h147 dummy"));
        assert!(game.run_command("scoreboard players set zero h147 0"));
        for n in [1, 2] {
            assert!(game.run_command(&format!("summon pig {} 200 4.5 {{UUID:[I;147,0,0,{n}],Health:10f,NoAI:1b,NoGravity:1b,Tags:[\"h147\"]}}", n as f64 + 0.5)));
            assert!(
                game.mobs
                    .iter()
                    .any(|m| game.persist.uuids.get(&m.id) == Some(&[147, 0, 0, n])
                        && m.kind.id == "minecraft:pig"
                        && m.health == 10.0)
            );
        }
        let time = game.world.game_time;
        let a = "00000093-0000-0000-0000-000000000001";
        let positive = capture(
            &mut game,
            &format!("execute as {a} if score zero h147 matches 0"),
        );
        assert_eq!(positive.0.len(), 1);
        assert_eq!(positive.1, vec![(true, 1)]);
        let direct = capture(&mut game, "execute if score zero h147 matches 1");
        assert_eq!(direct.0.len(), 1);
        assert_eq!(direct.1, vec![(false, 0)]);
        for tail in [
            format!("as {a} if score zero h147 matches 1"),
            format!("at {a} if block ~ ~-1 ~ air"),
            format!("as {a} if score zero h147_missing matches 1"),
        ] {
            assert!(game.run_command("scoreboard players set result h147 -99"));
            assert!(game.run_command("scoreboard players set success h147 -99"));
            let (messages, callbacks) = capture(
                &mut game,
                &format!(
                    "execute store result score result h147 store success score success h147 {tail}"
                ),
            );
            assert_eq!(callbacks, vec![(false, 0)]);
            assert_eq!(
                game.score.board.score("result", "h147").map(|s| s.value),
                Some(0)
            );
            assert_eq!(
                game.score.board.score("success", "h147").map(|s| s.value),
                Some(0)
            );
            assert!(messages.is_empty(), "forked terminal error: {messages:?}");
        }
        let many = capture(
            &mut game,
            "execute as @e[type=pig,tag=h147] if score zero h147 matches 1",
        );
        assert!(many.0.is_empty());
        assert_eq!(many.1, vec![(false, 0), (false, 0)]);
        let missing = capture(&mut game, "execute if score zero h147_missing matches 1");
        assert_eq!(missing.0.len(), 1);
        assert_eq!(missing.1, vec![(false, 0)]);
        let empty = capture(
            &mut game,
            "execute as @e[tag=h147_none] if score zero h147 matches 1",
        );
        assert!(empty.0.is_empty() && empty.1.is_empty());
        assert_eq!(game.world.game_time, time);
    }
}

#[cfg(test)]
pub(in crate::game) mod score_condition_tests {
    #[allow(unused_imports)]
    use super::*;
    #[allow(unused_imports)]
    use crate::game::game_tests::tests::server_game;

    pub(in crate::game) fn game(
        resources: &mut Option<std::sync::Arc<crate::game::Resources>>,
    ) -> crate::game::ServerGame<'_> {
        let mut game = server_game(resources);
        game.build_registries();
        game.set_allow_commands(true);
        game
    }

    #[allow(unused_imports)]
pub(in crate::game) use crate::game::server_side::commands_server::execute::score_condition_tests::say;

    /// `checkScore` resolves the source holder and objective before it reads
    /// the target's score, so a terminal condition names the bad objective
    /// even when the target has no score at all.
    #[test]
    #[ignore = "daily"]
    fn an_unknown_source_objective_fails_a_terminal_condition() {
        let mut resources = None;
        let mut game = game(&mut resources);
        assert!(game.run_command("scoreboard objectives add a dummy"));
        for sign in ["if", "unless"] {
            let (ok, said) = say(
                &mut game,
                &format!("execute {sign} score zero a = zero nope"),
            );
            assert!(!ok, "{sign}: {said:?}");
            assert!(
                said.iter().any(|m| m.contains("nope")),
                "{sign}: the source objective is named: {said:?}"
            );
        }
    }

    /// A non-terminal `if`/`unless` is a fork: its error is silent and its
    /// source is dropped (`CommandSourceStack.handleError` with `forked`), so
    /// `unless` does not run the branch either.
    #[test]
    #[ignore = "daily"]
    fn an_unknown_source_objective_drops_a_forked_condition_silently() {
        let mut resources = None;
        let mut game = game(&mut resources);
        assert!(game.run_command("scoreboard objectives add a dummy"));
        for sign in ["if", "unless"] {
            let (ok, said) = say(
                &mut game,
                &format!("execute {sign} score zero a = zero nope run say hi"),
            );
            assert!(ok, "{sign}: {said:?}");
            assert!(said.is_empty(), "{sign}: {said:?}");
        }
    }
}
