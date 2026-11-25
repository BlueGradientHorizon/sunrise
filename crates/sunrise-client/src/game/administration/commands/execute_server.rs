#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{
    Arg, Context, Dim, Game, R, Target, data::DataRef, err, wrap_degrees,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::execute::ExecState;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::execute::Source;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::execute::StoreEntry;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::execute::StoreTarget;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::execute::fork_limit_reached;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::execute::is_player_in_place;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::execute::java_int;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::execute::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::target_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    command::CommandError, nbt_path::NbtPath, text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::geometry::trig;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::nbt::Tag;

impl Game<'_> {
    /// `ExecuteCommand.isChunkLoaded`: an entity-ticking loading status and
    /// completed entity loading, rather than merely a retained lit column.
    pub(in crate::game::server_side) fn command_chunk_loaded(&self, x: i32, z: i32) -> bool {
        self.tickets.0.levels().loaded_entity_ticking(x, z)
            && self.world.chunks.contains_key(&(x, z))
            && self.streaming.is_lit(x, z)
            // Streaming queues the saved tags before making the column resident;
            // spawn_loaded_entities applies them before this chunk's entities run.
            && !self.world.loaded_entities.iter().any(|(key, _)| *key == (x, z))
    }
    pub(in crate::game::server_side) fn condition_matches(
        &mut self,
        c: &Context,
    ) -> R<(bool, i32)> {
        let flag = |b: bool| (b, i32::from(b));
        if c.has("data") {
            let r = self.data_ref_named(c, 0, "source")?;
            let data = self.data_of(&r)?;
            let Some(Arg::Str(p)) = c.get("path") else {
                return Ok((false, 0));
            };
            let path = NbtPath::parse(p).map_err(sunrise_client::data_cmd::path_error)?;
            let n = path.count(&data) as i32;
            return Ok((n > 0, n));
        }
        if c.has("items") || c.has("slots") {
            return self.condition_items(c);
        }
        if c.has("entity") {
            let Some(Arg::Entity(sel)) = c.get("entities").cloned() else {
                return Ok((false, 0));
            };
            let n = self.select(&sel).len() as i32;
            return Ok((n > 0, n));
        }
        if c.has("loaded") {
            let p = self
                .coords(c, "pos")
                .map(|co| co.block(self.source_position(), self.source_rotation()));
            let dimension = self.server.command_source.map_or(self.dim, |s| s.dimension);
            return Ok(flag(
                dimension == self.dim
                    && p.is_some_and(|p| self.command_chunk_loaded(p[0] >> 4, p[2] >> 4)),
            ));
        }
        if c.has("dimension") {
            let dimension = self.server.command_source.map_or(self.dim, |s| s.dimension);
            return Ok(flag(matches!(c.get("dimension"), Some(Arg::Resource(d))
                if d.as_str() == dimension.id())));
        }
        if c.has("block") {
            let p = self.block_pos_arg(c, "pos")?;
            let Some(Arg::Block(b)) = c.get("block").cloned() else {
                return Ok((false, 0));
            };
            return Ok(flag(self.block_matches(&b, p)));
        }
        if c.has("biome") {
            let p = self.block_pos_arg(c, "pos")?;
            let Some(Arg::Resource(want_biome)) = c.get("biome").cloned() else {
                return Ok((false, 0));
            };
            let name = self.biome_name_at_block(p);
            return Ok(flag(
                name.is_some_and(|n| self.biome_matches(&want_biome, &n)),
            ));
        }
        if c.has("score") {
            return self.condition_score(c).map(flag);
        }
        if c.has("blocks") {
            return self.condition_blocks(c);
        }
        if c.has("range") && c.has("id") {
            return self.condition_stopwatch(c).map(flag);
        }
        if c.has("predicate") {
            let Some(Arg::Resource(id)) = c.get("predicate").cloned() else {
                return Ok((false, 0));
            };
            return Ok(flag(self.check_predicate(&id)?));
        }
        Ok((false, 0))
    }
    /// `CommandResultCallback`s of a source's `store` chain, oldest first.
    pub(in crate::game::server_side) fn deliver(
        &mut self,
        store: Option<u32>,
        success: bool,
        value: i32,
    ) {
        // The outermost source's own CommandResultCallback (harness record).
        if self.exec.0.functions.is_empty()
            && let Some(results) = self.cmds.0.results.as_mut()
        {
            results.push((success, value));
        }
        let mut chain = Vec::new();
        let mut at = store;
        while let Some(i) = at {
            let Some(e) = self.exec.0.stores.get(i as usize) else {
                break;
            };
            chain.push(e.clone());
            at = e.parent;
        }
        for e in chain.into_iter().rev() {
            let v = if e.result { value } else { i32::from(success) };
            self.store_value(&e.target, v);
        }
    }
    /// `Frame.returnSuccess`/`returnFailure` then `discard`.
    pub(in crate::game::server_side) fn finish_return(&mut self, value: Option<i32>) {
        self.exec.0.returned = Some(value);
    }
    /// The players among `Entity.getPassengers` of `vehicle`: the player in
    /// place when it rides it, then the other players of the server riding it.
    /// A server's players are its guests, so a datapack running with no player
    /// in place still finds them.
    pub(in crate::game::server_side) fn player_passengers(&self, vehicle: Target) -> Vec<Target> {
        let id = match vehicle {
            Target::Mob(id) | Target::Boat(id) | Target::Cart(id) => id,
            _ => return Vec::new(),
        };
        let in_place = self.player.is_some() && self.riding == Some(id);
        let guests = self.lan.iter().flat_map(|host| {
            host.hub
                .guests
                .iter()
                .filter(move |g| {
                    g.placed
                        && Some(g.conn) != self.server.lan_acting
                        && host.hub.here(g)
                        && g.vehicle == Some(id)
                })
                .map(|g| Target::Guest(g.conn))
        });
        in_place
            .then_some(Target::Player)
            .into_iter()
            .chain(guests)
            .collect()
    }
    /// `ExecuteCommand.createRelationOperations`: the entities `on <relation>` reaches.
    pub(in crate::game::server_side) fn relation(&mut self, c: &Context, s: Source) -> Vec<Source> {
        let Some(entity) = s.entity else {
            return Vec::new();
        };
        let found: Vec<Target> = match c.literals.last().map(String::as_str) {
            Some("owner") => match entity {
                // OwnableEntity.getOwner → EntityReference.getLivingEntity:
                // a marker or a display with that UUID is not an owner.
                Target::Mob(id) => self
                    .mobs
                    .iter()
                    .find(|m| m.id == id)
                    .and_then(|m| m.extra.owner)
                    // A guest owner too, not only the player in place.
                    .and_then(|u| {
                        self.target_by_uuid(sunrise_client::lan::playerdata::ints_uuid(u))
                    })
                    .filter(|owner| {
                        matches!(
                            owner,
                            Target::Player
                                | Target::Guest(_)
                                | Target::Mob(_)
                                | Target::Stand(_)
                                | Target::Dragon(_)
                        )
                    })
                    .into_iter()
                    .collect(),
                _ => Vec::new(),
            },
            // Leashable.getLeashHolder: the server's resolved holder, not a
            // saved UUID that is still waiting for tickLeash to restore it.
            Some("leasher") => {
                let leash =
                    match entity {
                        Target::Mob(id) => self.mobs.iter().find(|m| m.id == id).and_then(|m| {
                            m.extra.leash.map(|holder| (holder, m.extra.leash_player))
                        }),
                        Target::Boat(id) => self
                            .boats
                            .iter()
                            .find(|b| b.id == id)
                            .and_then(|b| b.leash.map(|holder| (holder, b.leash_player))),
                        _ => None,
                    };
                leash
                    .and_then(|(holder, player)| match holder {
                        // The player who tied it (the host's when unrecorded).
                        sunrise_client::mobs::leash::Holder::Player => {
                            let uuid = player.or(self.level.player_uuid)?;
                            self.target_by_uuid(sunrise_client::lan::playerdata::ints_uuid(uuid))
                        }
                        sunrise_client::mobs::leash::Holder::Mob(i) => Some(Target::Mob(i)),
                        sunrise_client::mobs::leash::Holder::Boat(_)
                        | sunrise_client::mobs::leash::Holder::Knot(_) => None,
                    })
                    .into_iter()
                    .collect()
            }
            // TraceableEntity.getOwner: who shot a projectile, who threw an
            // item, who summoned a vex.
            Some("origin") => match entity {
                Target::Projectile(id) => self
                    .creatures
                    .projectiles
                    .iter()
                    .find(|p| p.id == id)
                    .and_then(|p| {
                        // Projectile.getOwner resolves EntityReference<Entity>,
                        // including loaded owners that cannot be a Who.
                        let Some(owner) = p.owner else {
                            return p.owner_uuid.and_then(|uuid| {
                                self.target_by_uuid(sunrise_client::lan::playerdata::ints_uuid(
                                    uuid,
                                ))
                            });
                        };
                        match (owner, p.guest) {
                            // A guest's shot: `Who::Player` is them.
                            (sunrise_client::mobs::Who::Player, Some(conn))
                                if self.server.lan_acting == Some(conn) =>
                            {
                                Some(Target::Player)
                            }
                            (sunrise_client::mobs::Who::Player, Some(conn)) => {
                                Some(Target::Guest(conn))
                            }
                            (sunrise_client::mobs::Who::Player, None) => Some(Target::Player),
                            (sunrise_client::mobs::Who::Mob(id), _) => Some(Target::Mob(id)),
                        }
                    })
                    .into_iter()
                    .collect(),
                Target::Item(id) => self
                    .dropped
                    .iter()
                    .find(|d| d.id == id)
                    .and_then(|d| d.thrower)
                    .and_then(|words| {
                        let uuid = words
                            .iter()
                            .fold(0u128, |all, w| all << 32 | u128::from(*w as u32));
                        self.target_by_uuid(uuid)
                    })
                    .into_iter()
                    .collect(),
                Target::Mob(id) => self
                    .mobs
                    .iter()
                    .find(|m| m.id == id && m.kind.id == "minecraft:vex")
                    .map(|m| m.extra.counter2 as u32)
                    .filter(|owner| *owner != 0)
                    .map(Target::Mob)
                    .into_iter()
                    .collect(),
                _ => Vec::new(),
            },
            Some("vehicle") => self.ride_vehicle(entity).into_iter().collect(),
            Some("passengers") => {
                let mut passengers = self.player_passengers(entity);
                // Direct runtime children only. Complete mount insertion order is
                // not modeled; retain the existing mob collection order here.
                if let Target::Mob(id) | Target::Boat(id) = entity {
                    passengers.extend(
                        self.mobs
                            .iter()
                            .filter(|m| m.extra.mount == Some(id) && m.alive() && !m.removed)
                            .map(|m| Target::Mob(m.id)),
                    );
                }
                if let Some(id) = self.resident_target_id(entity) {
                    passengers.extend(
                        self.bolts
                            .iter()
                            .filter(|b| b.mount == Some(id))
                            .map(|b| Target::Bolt(b.id)),
                    );
                }
                passengers
            }
            Some("controller") => match entity {
                Target::Mob(id) if self.mobs.iter().any(|m| m.id == id && m.rider.is_some()) => {
                    vec![Target::Player]
                }
                _ => Vec::new(),
            },
            Some("target") | Some("attacker") => {
                let attacker = c.literals.last().is_some_and(|l| l == "attacker");
                match entity {
                    Target::Mob(id) => self
                        .mobs
                        .iter()
                        .find(|m| m.id == id)
                        .and_then(|m| {
                            use sunrise_client::mobs::{Species, Who, ai::brain_target};
                            if !attacker && m.species() == Species::Warden {
                                return self
                                    .warden_attack_target(m, None)
                                    .map(|(_, target)| target);
                            }
                            let who = if attacker {
                                m.ai.last_hurt_by
                            } else {
                                // `Mob.getTarget`, as brain-backed mobs override it.
                                brain_target(m).unwrap_or(m.ai.target)
                            }?;
                            match who {
                                Who::Player => {
                                    let conn = if attacker {
                                        // `getLastHurtByMob`: the player who hit it,
                                        // not whichever client is reading it.
                                        self.server.last_hurt_by_player.get(&id).and_then(
                                            |credit| self.resolve_player_credit(*credit).flatten(),
                                        )
                                    } else {
                                        None
                                    };
                                    Some(
                                        conn.filter(|&conn| self.server.lan_acting != Some(conn))
                                            .map_or(Target::Player, Target::Guest),
                                    )
                                }
                                Who::Mob(i) if attacker => Some(Target::Mob(i)),
                                Who::Mob(i) => self.level_entity(i as i32),
                            }
                        })
                        .into_iter()
                        .collect(),
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        };
        found
            .into_iter()
            .filter(|t| self.target_position(*t).is_some())
            .map(|t| Source {
                entity: Some(t),
                ..s
            })
            .collect()
    }
    /// Run a parsed command (`root` is the first context; `ctx` the one with
    /// the executable node): `execute` and `return` walk their own chains.
    pub(in crate::game) fn run_chain(&mut self, root: &Context, ctx: &Context) -> R<i32> {
        self.exec.0.commands += 1;
        let limit = self
            .ui
            .rules
            .int_at(sunrise_client::gamerules::vanilla::MAX_COMMAND_SEQUENCE_LENGTH)
            .max(1) as u32;
        if self.exec.0.commands > limit {
            return Err(err("command.failed", &[]));
        }
        match root.literals.first().map(String::as_str) {
            Some("execute") => self.run_execute(root),
            Some("return") if root.child.is_some() => self.run_return_run(root),
            _ => {
                let id = ctx.command.unwrap_or("");
                self.run_leaf(id, ctx)
            }
        }
    }
    /// A conditional that ends the chain runs as a command: a test per source.
    /// `CommandSourceStack.handleError`: error delivery depends on the fork flag,
    /// including a fork that yielded only one source; success feedback is unchanged.
    pub(in crate::game::server_side) fn run_conditional_terminal(
        &mut self,
        c: &Context,
        sources: Vec<Source>,
        want: bool,
        forked: bool,
    ) -> R<i32> {
        let numeric = ["entity", "data", "blocks", "items", "slots"]
            .iter()
            .any(|k| c.has(k));
        let many = sources.len() > 1;
        let mut total = 0;
        let mut failure: Option<CommandError> = None;
        for s in sources {
            self.exec.0.callback = None;
            let (keep, count) = match self.with_source(s, |g| g.condition(c, want)) {
                Ok(v) => v,
                Err(e) => {
                    self.deliver(s.store, false, 0);
                    if !forked {
                        if many {
                            self.command_failure(e.message);
                        } else {
                            failure = Some(e);
                        }
                    }
                    continue;
                }
            };
            if keep {
                let n = if want && numeric { count } else { 1 };
                let message = if want && numeric {
                    Text::translate(
                        "commands.execute.conditional.pass_count",
                        &[Text::literal(count.to_string())],
                    )
                } else {
                    Text::translate("commands.execute.conditional.pass", &[])
                };
                self.with_source(s, |g| g.feedback(message));
                self.deliver(s.store, true, n);
                total += n;
            } else {
                self.deliver(s.store, false, 0);
                let e = if !want && numeric {
                    err("commands.execute.conditional.fail_count", &[&count])
                } else {
                    err("commands.execute.conditional.fail", &[])
                };
                if !forked {
                    if many {
                        self.command_failure(e.message);
                    } else {
                        failure = Some(e);
                    }
                }
            }
        }
        match failure {
            Some(e) => Err(e),
            None => Ok(total),
        }
    }
    /// ExecuteCommand: `ctx` is the context holding the `execute` literal.
    pub(in crate::game::server_side) fn run_execute(&mut self, ctx: &Context) -> R<i32> {
        // BuildContexts: a modifier may not bring the sources to this many.
        let fork_limit =
            self.ui
                .rules
                .int_at(sunrise_client::gamerules::vanilla::MAX_COMMAND_FORKS) as usize;
        let mut sources = vec![
            self.server
                .command_source
                .unwrap_or_else(|| self.player_source()),
        ];
        let mut c = ctx;
        // A fork modifier (as, at, positioned as, rotated as, facing entity,
        // on, a non-terminal if/unless) makes later errors silent
        // (CommandSourceStack.handleError with `forked`).
        let mut forked = false;
        // Nested execute/conditional/other modifier variants retain the old path.
        let mut function_batch_path = ctx.range.0 <= 1 && self.exec.0.functions.is_empty();
        let mut function_as_seen = false;
        loop {
            let word = c
                .literals
                .iter()
                .map(String::as_str)
                .find(|w| *w != "execute")
                .unwrap_or("");
            match word {
                "run" => {
                    let Some(command) = c.child.as_deref() else {
                        return Err(err("command.unknown.command", &[]));
                    };
                    let last = command.last_child();
                    if last.command.is_none() {
                        return Err(err("command.unknown.command", &[]));
                    }
                    let mut prepared = if function_batch_path
                        && function_as_seen
                        && command.literals.first().map(String::as_str) == Some("function")
                    {
                        self.prepare_function_batch(last, &sources)
                            .map(Vec::into_iter)
                    } else {
                        None
                    };
                    let mut total = 0;
                    let many = sources.len() > 1;
                    let mut failure = None;
                    for s in sources {
                        let result = if let Some(tasks) = prepared.as_mut() {
                            self.exec.0.prepared_function = tasks.next();
                            let result = self.with_source(s, |g| g.run_chain(command, last));
                            // Includes a command-cost rejection before the leaf and
                            // a return/error after the slot was consumed.
                            self.exec.0.prepared_function = None;
                            result
                        } else {
                            self.with_source(s, |g| g.run_chain(command, last))
                        };
                        match result {
                            Ok(n) => total += n,
                            // `handleError` with `forked` silences a command
                            // syntax error.
                            Err(e) if forked && !e.unexpected => {}
                            // Not a `CommandSyntaxException`: it escapes to
                            // `Commands.performCommand`'s catch, once, and the
                            // chain stops there.
                            Err(e) if e.unexpected => {
                                failure = Some(e);
                                break;
                            }
                            // Several sources: report each failure and carry on.
                            Err(e) if many => self.command_failure(e.message),
                            Err(e) => failure = Some(e),
                        }
                        if self.exec.0.returned.is_some() {
                            break;
                        }
                    }
                    return match failure {
                        Some(e) => Err(e),
                        None => Ok(total),
                    };
                }
                "if" | "unless" => {
                    function_batch_path = false;
                    let want = word == "if";
                    if c.child.is_none() {
                        return self.run_conditional_terminal(c, sources, want, forked);
                    }
                    forked = true;
                    let mut kept = Vec::new();
                    for s in sources {
                        // `BuildContexts`: an error in a fork modifier is
                        // silent and drops its source (`handleError` with
                        // `forked`); one that is not a `CommandSyntaxException`
                        // still reaches `Commands.performCommand`.
                        let (keep, _) = match self.with_source(s, |g| g.condition(c, want)) {
                            Ok(v) => v,
                            Err(e) if !e.unexpected => continue,
                            Err(e) => return Err(e),
                        };
                        if kept.len() + usize::from(keep) >= fork_limit {
                            return fork_limit_reached(forked, fork_limit);
                        }
                        if keep {
                            kept.push(s);
                        }
                    }
                    sources = kept;
                }
                _ => {
                    if word == "as" {
                        function_batch_path &= !function_as_seen;
                        function_as_seen = true;
                    } else if word != "store" {
                        function_batch_path = false;
                    }
                    forked |= match word {
                        "as" | "at" | "on" => true,
                        "positioned" | "rotated" => c.has("as"),
                        "facing" => c.has("entity"),
                        _ => false,
                    };
                    let mut next = Vec::new();
                    for s in sources {
                        let new = self.with_source(s, |g| g.modify(word, c, s))?;
                        if next.len() + new.len() >= fork_limit {
                            return fork_limit_reached(forked, fork_limit);
                        }
                        next.extend(new);
                    }
                    sources = next;
                }
            }
            match c.child.as_deref() {
                Some(child) => c = child,
                None => return Err(err("command.unknown.command", &[])),
            }
        }
    }
    /// Run a leaf command and report its outcome to the `store` callbacks.
    pub(in crate::game::server_side) fn run_leaf(&mut self, id: &str, ctx: &Context) -> R<i32> {
        self.exec.0.callback = None;
        let store = self.server.command_source.and_then(|s| s.store);
        let result = self.execute_command(id, ctx);
        let custom = self.exec.0.callback.take();
        let outcome = match (&result, custom) {
            (_, Some(c)) => c,
            (Ok(v), None) => Some((true, *v)),
            // `ContextChain.runExecutable` reports a `CommandSyntaxException`
            // to the result consumer; any other exception skips it.
            (Err(e), None) if e.unexpected => None,
            (Err(_), None) => Some((false, 0)),
        };
        if let Some((success, value)) = outcome {
            self.deliver(store, success, value);
        }
        result
    }
    /// `return run <command>`: run it and return its result from the frame.
    pub(in crate::game::server_side) fn run_return_run(&mut self, root: &Context) -> R<i32> {
        let Some(command) = root.child.as_deref() else {
            return Err(err("command.unknown.command", &[]));
        };
        let last = command.last_child();
        let result = self.run_chain(command, last);
        // The command's own callbacks do not fire; the function frame gets
        // its result instead.
        match result {
            Ok(v) => self.finish_return(Some(v)),
            Err(e) => {
                self.finish_return(None);
                self.command_failure(e.message);
            }
        }
        self.exec.0.callback = Some(None);
        Ok(0)
    }
    /// A `store` modifier: a new callback in the source's chain.
    pub(in crate::game::server_side) fn store_source(
        &mut self,
        c: &Context,
        s: Source,
    ) -> R<Source> {
        let result = c.has("result");
        let target = if c.has("score") {
            let holders = self.score_holder_names(c, "targets", true)?;
            let Some(Arg::Str(objective)) = c.get("objective") else {
                return Err(err("command.failed", &[]));
            };
            if self.score.board.objective(objective).is_none() {
                return Err(err("arguments.objective.notFound", &[objective]));
            }
            StoreTarget::Score(holders, objective.clone())
        } else if c.has("bossbar") {
            let Some(Arg::Resource(id)) = c.get("id") else {
                return Err(err("command.failed", &[]));
            };
            if !self.bossbar_exists(id) {
                return Err(err("commands.bossbar.unknown", &[id]));
            }
            StoreTarget::Bossbar(id.clone(), c.has("value"))
        } else {
            let r = self.data_ref(c, 0)?;
            let path = match c.get("path") {
                Some(Arg::Str(p)) => {
                    NbtPath::parse(p).map_err(sunrise_client::data_cmd::path_error)?
                }
                _ => return Err(err("command.failed", &[])),
            };
            let kind = c
                .literals
                .iter()
                .find(|l| {
                    matches!(
                        l.as_str(),
                        "int" | "float" | "short" | "long" | "double" | "byte"
                    )
                })
                .cloned()
                .unwrap_or_else(|| "int".into());
            let scale = match c.get("scale") {
                Some(Arg::Double(v)) => *v,
                _ => 1.0,
            };
            StoreTarget::Data(r, path, kind, scale)
        };
        self.exec.0.stores.push(StoreEntry {
            result,
            target,
            parent: s.store,
        });
        Ok(Source {
            store: Some(self.exec.0.stores.len() as u32 - 1),
            ..s
        })
    }
    /// `Entity.getYRot` and `getXRot`: the entity's angles, not the direction its model faces.
    pub(in crate::game) fn target_rotation(&self, t: Target) -> [f32; 2] {
        use crate::game::entity::Entity;
        use sunrise_client::entity_nbt::loaded_rotation;
        let saved_rotation = |id| self.persist.raw.get(&id).map_or([0.0; 2], loaded_rotation);
        match t {
            Target::Player => [self.yaw, self.pitch],
            Target::Guest(conn) => self
                .command_guest(conn)
                .map_or([0.0; 2], |g| [g.yaw, g.pitch]),
            Target::Mob(id) => self
                .mobs
                .iter()
                .find(|m| m.id == id)
                .map_or([0.0; 2], Entity::rotation),
            Target::Item(id) => self
                .dropped
                .iter()
                .find(|a| a.id == id)
                .map_or([0.0; 2], Entity::rotation),
            Target::Orb(id) => self
                .orbs
                .iter()
                .find(|a| a.id == id)
                .map_or([0.0; 2], Entity::rotation),
            Target::Projectile(id) => self
                .creatures
                .projectiles
                .iter()
                .find(|a| a.id == id)
                .map_or([0.0; 2], Entity::rotation),
            Target::Stand(id) => self
                .armor_stands
                .iter()
                .find(|a| a.id == id)
                .map_or([0.0; 2], Entity::rotation),
            Target::Frame(id) => self
                .item_frames
                .iter()
                .find(|a| a.id == id)
                .map_or([0.0; 2], crate::entities::ItemFrame::entity_rotation),
            Target::Painting(id) => self
                .paintings
                .iter()
                .find(|a| a.id == id)
                .map_or([0.0; 2], |a| {
                    [f32::from(a.facing) * 90.0, saved_rotation(id)[1]]
                }),
            Target::Boat(id) => self
                .boats
                .iter()
                .find(|b| b.id == id)
                .map_or([0.0; 2], Entity::rotation),
            Target::Cart(id) => self
                .carts
                .iter()
                .find(|c| c.id == id)
                .map_or([0.0; 2], Entity::rotation),
            Target::Fangs(id) => self
                .fangs
                .list
                .iter()
                .find(|f| f.id == id)
                .map_or([0.0; 2], |f| [f.yaw, saved_rotation(id)[1]]),
            Target::Crystal(id) => self
                .crystals
                .iter()
                .find(|c| c.id == id)
                .map_or([0.0; 2], |c| c.rotation),
            Target::Cloud(id) => saved_rotation(id),
            Target::Bolt(id) => self
                .bolts
                .iter()
                .find(|b| b.id == id)
                .map_or([0.0; 2], |b| b.rotation),
            Target::Dragon(id) => self
                .dragon
                .as_ref()
                .filter(|a| a.id == id)
                .map_or([0.0; 2], |a| [a.yaw, a.pitch]),
            Target::Tnt(id) => self
                .tnts
                .iter()
                .find(|a| a.id == id)
                .map_or([0.0; 2], Entity::rotation),
            Target::Cushion(id) => self
                .cushions
                .iter()
                .find(|a| a.id == id)
                .map_or([0.0; 2], |a| [a.yaw, saved_rotation(id)[1]]),
            Target::Falling(id) => self
                .falling
                .iter()
                .find(|a| a.id == id)
                .map_or([0.0; 2], Entity::rotation),
            Target::Rocket(id) => self
                .rockets
                .iter()
                .find(|a| a.id == id)
                .map_or([0.0; 2], Entity::rotation),
            Target::Trident(id) => self
                .tridents
                .iter()
                .find(|a| a.id == id)
                .map_or([0.0; 2], Entity::rotation),
            Target::Spawner(id) => self
                .ominous_spawners
                .iter()
                .find(|a| a.id == id)
                .and_then(|a| a.raw.as_deref())
                .map_or([0.0; 2], sunrise_client::entity_nbt::rotation),
            Target::Hook(id) => self
                .fishing_hook_by_id(id)
                .map_or([0.0; 2], |(hook, _)| [hook.yaw, hook.pitch]),
            Target::Other(id) => self
                .others
                .0
                .iter()
                .find(|o| o.id == id)
                .map_or([0.0; 2], |o| [o.yaw, o.pitch]),
            Target::General(id) => self
                .resident_entities
                .general
                .row(id)
                .map(|row| row.rotation)
                .or_else(|| {
                    self.remote
                        .as_ref()
                        .and_then(|r| r.general.rows.get(&(id as i32)))
                        .map(|row| row.rotation)
                })
                .unwrap_or([0.0; 2]),
            // Dragon parts aren't admitted by UUID. Natural fence knots never turn.
            Target::DragonPart(_) | Target::Knot(_) => [0.0; 2],
        }
    }
    /// `ServerLevel.getEntity(UUID)`: any entity of the level in place, not
    /// only mobs (displays, armor stands, items...). The map is keyed by id,
    /// so a killed entity and a later summon of the same UUID can both be
    /// in it; the one the level still holds is the entity.
    pub(in crate::game) fn target_with_uuid(&self, uuid: [i32; 4]) -> Option<Target> {
        if is_player_in_place(uuid, self.server.lan_acting_uuid, self.level.player_uuid) {
            if self.server.native_command_filter {
                return self
                    .player(sunrise_client::lan::playerdata::ints_uuid(uuid))
                    .and_then(|player| self.native_entity_target(player.entity()));
            }
            return Some(Target::Player);
        }
        self.persist.uuids.iter().find_map(|(&id, found)| {
            if *found != uuid {
                return None;
            }
            self.level_entity(i32::try_from(id).ok()?)
        })
    }
    /// Evaluate `f` with `source` as the command source, in the source's
    /// level (`CommandSourceStack.getLevel`): a server puts that level in
    /// place for it.
    pub(in crate::game) fn with_source<T>(
        &mut self,
        source: Source,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let previous = self.server.command_source.replace(source);
        let back = self.enter_command_level(source.dimension);
        let out = f(self);
        if let Some(back) = back {
            self.enter_command_level(back);
        }
        self.server.command_source = previous;
        out
    }
}

#[cfg(test)]
pub(in crate::game) mod passenger_relation_tests {
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::commands::execute::passenger_relation_tests::actor;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::commands::execute::passenger_relation_tests::value;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::commands::execute::passenger_relation_tests::vehicle;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::commands::execute::passenger_relation_tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::commands::target_entity_mut;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::native_entity_refs::tests::with_game;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_core::nbt::Tag;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_core::position::BlockPos;

    #[test]
    #[ignore = "daily"]
    fn h150_actual_registered_passengers_select_direct_live_mob_and_boat_children() {
        with_game(|game| {
            game.build_registries();
            game.set_allow_commands(true);
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)),
            );
            game.block_ticks
                .add_empty_chunk(BlockPos::new(0, 0, 0).chunk())
                .unwrap();
            assert!(game.run_command("forceload add 0 0"));
            assert!(game.run_command("fill 0 199 0 15 199 15 stone"));
            for objective in ["h150u", "h150ur", "h150us"] {
                assert!(game.run_command(&format!("scoreboard objectives add {objective} dummy")));
            }
            let clock = game.world.game_time;
            // 1->2->3 is a nested mob tree; 4->5 a boat; 6->7,8 two direct children.
            let kinds = [
                "cow", "pig", "chicken", "oak_boat", "cow", "cow", "pig", "chicken", "pig", "cow",
            ];
            let mut ids = Vec::new();
            for (offset, kind) in kinds.into_iter().enumerate() {
                let i = offset + 1;
                let extra = if kind == "oak_boat" {
                    ""
                } else {
                    ",NoAI:1b,Health:1f"
                };
                assert!(game.run_command(&format!(
                "summon {kind} 8.5 200 8.5 {{UUID:[I;150,2,0,{i}],Tags:[\"h150u\"],NoGravity:1b,Silent:1b,Motion:[0d,0d,0d]{extra}}}"
            )));
                let words = [150, 2, 0, i as i32];
                let id = *game
                    .persist
                    .uuids
                    .iter()
                    .find(|(_, u)| **u == words)
                    .unwrap()
                    .0;
                let target = if kind == "oak_boat" {
                    Target::Boat(id)
                } else {
                    Target::Mob(id)
                };
                assert_eq!(game.target_type(target), format!("minecraft:{kind}"));
                assert_eq!(game.target_position(target), Some([8.5, 200.0, 8.5]));
                let saved = game.command_target_nbt(target).unwrap();
                assert_eq!(saved.get("UUID"), Some(&Tag::IntArray(words.to_vec())));
                if kind != "oak_boat" {
                    assert_eq!(saved.get("Health"), Some(&Tag::Float(1f32.to_bits())));
                    assert_eq!(saved.get("NoAI"), Some(&Tag::Byte(1)));
                }
                assert!(game.world.loaded(8, 8));
                assert!(game.run_command("execute if block 8 199 8 stone"));
                // Membership must come from runtime links; no retained Passengers body remains.
                game.persist.raw.remove(&id);
                ids.push(id);
            }
            for (child, parent) in [(2, 1), (3, 2), (5, 4), (7, 6), (8, 6)] {
                if child == 8 {
                    // Normal admission refuses the cow's second passenger; /ride forces it.
                    assert!(!game.start_riding_target(
                        Target::Mob(ids[child - 1]),
                        Target::Mob(ids[parent - 1]),
                        false,
                    ));
                }
                assert!(game.run_command(&format!(
                    "ride {} mount {}",
                    actor(child),
                    actor(parent)
                )));
                assert_eq!(
                    game.mobs
                        .iter()
                        .find(|m| m.id == ids[child - 1])
                        .unwrap()
                        .extra
                        .mount,
                    Some(ids[parent - 1])
                );
                vehicle(game, child, parent);
            }
            let mut failures = Vec::new();
            for (phase, parent, depth, selected) in [
                ("direct", 1, 1, vec![2]),
                ("nested", 1, 2, vec![3]),
                ("boat", 4, 1, vec![5]),
                ("multiple", 6, 1, vec![7, 8]),
                ("empty", 9, 1, vec![]),
            ] {
                for (objective, n) in [("h150u", 0), ("h150ur", 91), ("h150us", 17)] {
                    for i in 1..=10 {
                        assert!(game.run_command(&format!(
                            "scoreboard players set {} {objective} {n}",
                            actor(i)
                        )));
                    }
                }
                let nested = if depth == 2 { " on passengers" } else { "" };
                game.cmds.0.results = Some(Vec::new());
                game.server.chat_capture = Some(Vec::new());
                assert!(game.run_command(&format!(
                "execute as {} on passengers{nested} store result score @s h150ur store success score @s h150us run scoreboard players add @s h150u 1", actor(parent)
            )));
                let callbacks = game.cmds.0.results.take().unwrap();
                let messages: Vec<_> = game
                    .server
                    .chat_capture
                    .take()
                    .unwrap()
                    .iter()
                    .map(|t| t.plain())
                    .collect();
                println!("H150 actual {phase}: callbacks={callbacks:?}, feedback={messages:?}");
                let expected = vec![(true, 1); selected.len()];
                if selected.is_empty() {
                    assert_eq!(callbacks, expected, "independent empty parent callback");
                } else if callbacks != expected {
                    failures.push(format!("{phase} callbacks {callbacks:?} != {expected:?}"));
                }
                for i in 1..=10 {
                    let yes = selected.contains(&i);
                    let got = (
                        value(game, i, "h150u"),
                        value(game, i, "h150ur"),
                        value(game, i, "h150us"),
                    );
                    let want = if yes { (1, 1, 1) } else { (0, 91, 17) };
                    if phase == "empty" || !yes {
                        assert_eq!(
                            got, want,
                            "independent empty/unrelated/no-descendant control: {phase} {i}"
                        );
                    } else if got != want {
                        failures.push(format!("{phase} {i}: {got:?} != {want:?}"));
                    }
                }
            }
            assert!(game.run_command(&format!("ride {} dismount", actor(2))));
            assert_eq!(
                game.mobs
                    .iter()
                    .find(|m| m.id == ids[1])
                    .unwrap()
                    .extra
                    .mount,
                None
            );
            vehicle(game, 3, 2);
            for phase in ["dismounted", "remounted"] {
                if phase == "remounted" {
                    assert!(game.run_command(&format!("ride {} mount {}", actor(2), actor(1))));
                    vehicle(game, 2, 1);
                }
                for (objective, n) in [("h150u", 0), ("h150ur", 91), ("h150us", 17)] {
                    for i in 1..=10 {
                        assert!(game.run_command(&format!(
                            "scoreboard players set {} {objective} {n}",
                            actor(i)
                        )));
                    }
                }
                assert!(game.run_command(&format!(
                "execute as {} on passengers store result score @s h150ur store success score @s h150us run scoreboard players add @s h150u 1", actor(1)
            )));
                for i in [1, 2, 3, 10] {
                    let got = (
                        value(game, i, "h150u"),
                        value(game, i, "h150ur"),
                        value(game, i, "h150us"),
                    );
                    let want = if phase == "remounted" && i == 2 {
                        (1, 1, 1)
                    } else {
                        (0, 91, 17)
                    };
                    if phase == "dismounted" || i != 2 {
                        assert_eq!(
                            got, want,
                            "independent dismount/unrelated control: {phase} {i}"
                        );
                    } else if got != want {
                        failures.push(format!("{phase} {i}: {got:?} != {want:?}"));
                    }
                }
            }
            assert_eq!(game.world.game_time, clock, "zero actual observation ticks");
            assert!(
                failures.is_empty(),
                "actual registered direct passenger branch/callback/store failures: {failures:?}"
            );
        });
    }
}

#[cfg(test)]
pub(in crate::game) mod terminal_fork_tests {
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::Game;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::commands::execute::terminal_fork_tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::commands::target_entity_mut;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::{
        Audio, Options, Progress, Settings, load_resources,
    };
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use std::sync::Arc;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_client::chunk::Chunk;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_core::position::ChunkPos;

    pub(in crate::game) fn capture(
        game: &mut Game<'_>,
        command: &str,
    ) -> (Vec<String>, Vec<(bool, i32)>) {
        game.server.chat_capture = Some(Vec::new());
        game.cmds.0.results = Some(Vec::new());
        game.as_console(|g| g.run_command(command));
        let messages = game
            .server
            .chat_capture
            .take()
            .unwrap()
            .iter()
            .map(|t| t.plain())
            .collect();
        (messages, game.cmds.0.results.take().unwrap())
    }
}

#[cfg(test)]
pub(in crate::game) mod score_condition_tests {
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::commands::execute::score_condition_tests::game;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::commands::execute::score_condition_tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::commands::target_entity_mut;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::game_tests::tests::server_game;

    pub(in crate::game) fn say(game: &mut Game<'_>, command: &str) -> (bool, Vec<String>) {
        game.server.chat_capture = Some(Vec::new());
        let ok = game.run_command(command);
        let said = game.server.chat_capture.take().unwrap_or_default();
        (
            ok,
            said.iter().map(sunrise_client::text::Text::plain).collect(),
        )
    }
}
