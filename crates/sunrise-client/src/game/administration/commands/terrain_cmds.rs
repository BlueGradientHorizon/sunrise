//! `/forceload`, `/fillbiome`, `/worldborder` and `/loot`.
use super::{Arg, Context, Game, R, err};
use sunrise_core::position::ChunkPos;
#[allow(unused_imports)]
use sunrise_game_data::text::Text;
#[allow(unused_imports)]
use sunrise_items::items::ItemStack;
#[allow(unused_imports)]
use sunrise_level::worldborder::{MAX_CENTER, MAX_SIZE};
#[allow(unused_imports)]
use sunrise_sim::scoreboard_cmd::{Holder, Tracker};

/// `LocationCheck.test` / `BlockPredicate`: the measured loaded block-state subset.
pub(in crate::game) fn loot_location_block(
    condition: &serde_json::Value,
    origin: [i32; 3],
    chunks: &sunrise_level::chunk_map::ChunkMap,
    registry: &sunrise_world::registry::BlockRegistry,
    items: &sunrise_items::items::Items,
) -> Option<bool> {
    let condition = condition.as_object()?;
    if condition.keys().any(|k| {
        !matches!(
            k.as_str(),
            "type" | "predicate" | "offsetX" | "offsetY" | "offsetZ"
        )
    }) {
        return None;
    }
    let predicate = condition.get("predicate")?.as_object()?;
    if predicate.len() != 1 || !predicate.contains_key("block") {
        return None;
    }
    let block = predicate.get("block")?.as_object()?;
    if block
        .keys()
        .any(|k| !matches!(k.as_str(), "blocks" | "state"))
        || block.get("blocks")?.as_str().is_none()
    {
        return None;
    }
    if let Some(state) = block.get("state")
        && state.as_object()?.values().any(|v| !v.is_string())
    {
        return None;
    }
    let mut at = origin;
    for (axis, key) in ["offsetX", "offsetY", "offsetZ"].iter().enumerate() {
        let offset = match condition.get(*key) {
            None => 0,
            Some(value) => i32::try_from(value.as_i64()?).ok()?,
        };
        // floor(integer block origin + .5 + integer offset), also for negatives.
        at[axis] = origin[axis].checked_add(offset)?;
    }
    // Unloaded positions are unsupported: never load or invent a block here.
    // `LocationPredicate.matches`: a block check fails where `Level.isLoaded` is false.
    let Some(chunk) = chunks.get(&(at[0] >> 4, at[2] >> 4)) else {
        return Some(false);
    };
    let state = registry.state(chunk.block(at[0] & 15, at[1], at[2] & 15))?;
    Some(items.matches_block_state(&predicate["block"], &state.name, &state.properties))
}

pub(in crate::game) fn num(n: impl std::fmt::Display) -> Text {
    Text::literal(n.to_string())
}
pub(in crate::game) fn chunk_text(x: i32, z: i32) -> Text {
    Text::literal(format!("[{x}, {z}]"))
}
pub(in crate::game) fn holder(text: Text) -> Holder {
    Holder {
        name: String::new(),
        text,
        display: None,
    }
}
pub(in crate::game) fn ticks_to_seconds(ticks: i64) -> String {
    super::percent_f(ticks as f64 / 20.0, 2)
}
/// `WorldBorderCommand.ERROR_TOO_BIG`'s argument: vanilla hands the message the
/// double `5.9999968E7` (the value `setSize` compares against too), and a `%s`
/// substitution prints a double with `Double.toString`.
pub(in crate::game) fn size_limit_text() -> String {
    sunrise_core::java_number::double_to_string(MAX_SIZE)
}

impl Game<'_, crate::game::server_side::ServerState> {
    // ---- /forceload ----------------------------------------------------------

    pub(in crate::game::server_side) fn column_arg(
        &self,
        ctx: &Context,
        name: &str,
    ) -> R<[i32; 2]> {
        let c = self
            .coords(ctx, name)
            .ok_or_else(|| err("argument.pos2d.incomplete", &[]))?;
        let p = self.resolve_coords(&c);
        Ok([p[0].floor() as i32, p[2].floor() as i32])
    }
    /// `ForceLoadCommand.changeForceLoad`: update and load each newly forced column in order.
    pub(in crate::game::server_side) fn cmd_forceload(
        &mut self,
        id: &str,
        ctx: &Context,
    ) -> R<i32> {
        let dim = Text::literal(self.dim.id());
        match id {
            "forceload.query" if ctx.get("pos").is_none() => {
                let mut forced: Vec<(i32, i32)> =
                    self.side.streaming.commanded().iter().copied().collect();
                // ForceLoadCommand sorts its LongStream before formatting chunks.
                forced.sort_unstable_by_key(|&(x, z)| ChunkPos::new(x, z).pack());
                if forced.is_empty() {
                    self.command_failure(Text::translate("commands.forceload.added.none", &[dim]));
                    return Ok(0);
                }
                let list = forced
                    .iter()
                    .map(|(x, z)| format!("[{x}, {z}]"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let message = if forced.len() == 1 {
                    Text::translate(
                        "commands.forceload.list.single",
                        &[dim, Text::literal(list)],
                    )
                } else {
                    Text::translate(
                        "commands.forceload.list.multiple",
                        &[num(forced.len()), dim, Text::literal(list)],
                    )
                };
                self.feedback(message);
                Ok(forced.len() as i32)
            }
            "forceload.query" => {
                let [x, z] = self.column_arg(ctx, "pos")?;
                let key = (x >> 4, z >> 4);
                if self.side.streaming.commanded().contains(&key) {
                    self.feedback(Text::translate(
                        "commands.forceload.query.success",
                        &[chunk_text(key.0, key.1), dim],
                    ));
                    Ok(1)
                } else {
                    Err(err(
                        "commands.forceload.query.failure",
                        &[&chunk_text(key.0, key.1).plain(), &dim.plain()],
                    ))
                }
            }
            "forceload.remove.all" => {
                self.side.streaming.clear_forced();
                self.feedback(Text::translate("commands.forceload.removed.all", &[dim]));
                Ok(0)
            }
            _ => {
                let add = id == "forceload.add";
                let from = self.column_arg(ctx, "from")?;
                let to = if ctx.get("to").is_some() {
                    self.column_arg(ctx, "to")?
                } else {
                    from
                };
                let (min_x, max_x) = (from[0].min(to[0]), from[0].max(to[0]));
                let (min_z, max_z) = (from[1].min(to[1]), from[1].max(to[1]));
                if min_x < -30_000_000
                    || min_z < -30_000_000
                    || max_x >= 30_000_000
                    || max_z >= 30_000_000
                {
                    return Err(err("argument.pos.outofworld", &[]));
                }
                let (cx0, cx1, cz0, cz1) = (min_x >> 4, max_x >> 4, min_z >> 4, max_z >> 4);
                let count = i64::from(cx1 - cx0 + 1) * i64::from(cz1 - cz0 + 1);
                if count > 256 {
                    return Err(err("commands.forceload.toobig", &[&256, &count]));
                }
                let mut tracker = Tracker::default();
                for x in cx0..=cx1 {
                    for z in cz0..=cz1 {
                        let changed =
                            self.side
                                .streaming
                                .set_forced_loaded(&mut self.world, x, z, add);
                        tracker.track(&holder(chunk_text(x, z)), i32::from(changed));
                    }
                }
                let (lo, hi) = (chunk_text(cx0, cz0), chunk_text(cx1, cz1));
                let verb = if add { "added" } else { "removed" };
                let (message, total) = tracker.message(
                    false,
                    Some(err(&format!("commands.forceload.{verb}.failure"), &[])),
                    |h, _| {
                        Text::translate(
                            &format!("commands.forceload.{verb}.single"),
                            &[h.text.clone(), dim.clone()],
                        )
                    },
                    |c, _| {
                        Text::translate(
                            &format!("commands.forceload.{verb}.multiple"),
                            &[num(c), dim.clone(), lo.clone(), hi.clone()],
                        )
                    },
                )?;
                self.feedback(message);
                Ok(total)
            }
        }
    }

    // ---- /fillbiome ----------------------------------------------------------------

    pub(in crate::game::server_side) fn cmd_fillbiome(&mut self, ctx: &Context) -> R<i32> {
        let Some(Arg::Resource(biome)) = ctx.get("biome").cloned() else {
            return Ok(0);
        };
        // BlockPosArgument.getLoadedBlockPos on both corners first.
        let (a, b) = (
            self.block_pos_arg(ctx, "from")?,
            self.block_pos_arg(ctx, "to")?,
        );
        // Quantize to biome cells (`QuartPos`), then work with the box.
        let q = |v: i32| (v >> 2) << 2;
        let lo = [
            q(a[0]).min(q(b[0])),
            q(a[1]).min(q(b[1])),
            q(a[2]).min(q(b[2])),
        ];
        let hi = [
            q(a[0]).max(q(b[0])),
            q(a[1]).max(q(b[1])),
            q(a[2]).max(q(b[2])),
        ];
        let volume = i64::from(hi[0] - lo[0] + 1)
            * i64::from(hi[1] - lo[1] + 1)
            * i64::from(hi[2] - lo[2] + 1);
        let limit = self
            .ui
            .rules
            .int_at(sunrise_game_data::gamerules::vanilla::MAX_BLOCK_MODIFICATIONS);
        if volume > i64::from(limit) {
            return Err(err("commands.fillbiome.toobig", &[&limit, &volume]));
        }
        let Some(target) = world_biomes!(self)
            .list
            .iter()
            .position(|b| b.name == biome)
        else {
            return Err(err(
                "argument.resource.not_found",
                &[&biome, &"minecraft:worldgen/biome"],
            ));
        };
        let filter = match ctx.get("filter") {
            Some(Arg::Resource(f)) => Some(f.clone()),
            _ => None,
        };
        for cz in (lo[2] >> 4)..=(hi[2] >> 4) {
            for cx in (lo[0] >> 4)..=(hi[0] >> 4) {
                if !self.world.chunks.contains_key(&(cx, cz)) {
                    return Err(err("argument.pos.unloaded", &[]));
                }
            }
        }
        let mut changed = 0;
        let mut touched = std::collections::HashSet::new();
        for z in (lo[2]..=hi[2]).step_by(4) {
            for y in (lo[1]..=hi[1]).step_by(4) {
                for x in (lo[0]..=hi[0]).step_by(4) {
                    let key = (x >> 4, z >> 4);
                    let Some(chunk) = self.world.chunks.get(&key) else {
                        continue;
                    };
                    let current = chunk.biome(x, y, z);
                    let current_name = world_biomes!(self).name(current);
                    if let Some(f) = &filter
                        && !self.biome_matches(f, current_name)
                    {
                        continue;
                    }
                    if usize::from(current) == target {
                        continue;
                    }
                    let qy = ((y - self.world.bottom_y()) >> 2)
                        .clamp(0, self.world.layout().height() / 4 - 1);
                    if let Some(chunk) = self.world.chunks.get_mut(&key) {
                        let chunk = std::sync::Arc::make_mut(chunk);
                        chunk.set_biome_cell((x & 15) >> 2, qy, (z & 15) >> 2, target as u16);
                    }
                    touched.insert(key);
                    changed += 1;
                }
            }
        }
        if changed == 0 {
            return Err(err("commands.fillbiome.no_changes", &[]));
        }
        let columns: Vec<(i32, i32)> = ((lo[2] >> 4)..=(hi[2] >> 4))
            .flat_map(|cz| ((lo[0] >> 4)..=(hi[0] >> 4)).map(move |cx| (cx, cz)))
            .filter(|key| touched.contains(key))
            .collect();
        self.commit_biome_columns(&touched, &columns);
        self.feedback(Text::translate(
            "commands.fillbiome.success.count",
            &[
                num(changed),
                num(lo[0]),
                num(lo[1]),
                num(lo[2]),
                num(hi[0]),
                num(hi[1]),
                num(hi[2]),
            ],
        ));
        Ok(changed)
    }

    /// The tail of a biome change: the columns in `touched` are saved and re-meshed, and the
    /// players who see `columns` are sent their new biomes (`ChunkMap.resendBiomesForChunks`), in
    /// the order the columns were gathered (z, then x). Shared by `/fillbiome` and
    /// `level_set_biome`.
    pub(in crate::game) fn commit_biome_columns(
        &mut self,
        touched: &std::collections::HashSet<(i32, i32)>,
        columns: &[(i32, i32)],
    ) {
        for key in touched {
            self.world.modified.insert(*key);
            for sy in 0..self.world.layout().sections() as i32 {
                self.world.dirty_sections.insert((key.0, sy, key.1));
            }
        }
        if let Some(host) = self.lan.as_mut() {
            host.hub.resend_biomes(&self.world, columns);
        }
    }
    /// Set the biome cell holding `pos` (a 4x4x4 cell, `QuartPos`) to the biome named `biome`
    /// (`minecraft:plains`), as `/fillbiome` does for one cell: the chunk is saved, re-meshed and
    /// resent. False for an unknown biome, an unloaded or out-of-height position, or a cell that
    /// already has it.
    /// Tentative reads see the new cells inside a transaction. Abort restores the cells
    /// without publishing. Saving, remeshing and resending wait for the outermost commit,
    /// once per column. Outside a transaction they run immediately.
    pub(in crate::game) fn set_biome_at(&mut self, pos: [i32; 3], biome: &str) -> bool {
        let Some(target) = world_biomes!(self)
            .list
            .iter()
            .position(|b| b.name == biome)
        else {
            return false;
        };
        let key = (pos[0] >> 4, pos[2] >> 4);
        let Some(chunk) = self.world.chunks.get(&key) else {
            return false;
        };
        if !self.world.shape.contains_y(pos[1])
            || usize::from(chunk.biome(pos[0], pos[1], pos[2])) == target
        {
            return false;
        }
        let old = chunk.biome(pos[0], pos[1], pos[2]);
        self.journal_biome(pos, old);
        let qy =
            ((pos[1] - self.world.bottom_y()) >> 2).clamp(0, self.world.layout().height() / 4 - 1);
        if let Some(chunk) = self.world.chunks.get_mut(&key) {
            let chunk = std::sync::Arc::make_mut(chunk);
            chunk.set_biome_cell((pos[0] & 15) >> 2, qy, (pos[2] & 15) >> 2, target as u16);
        }
        self.after_commit(super::super::transactions::After::Biome(key));
        true
    }

    // ---- /worldborder -----------------------------------------------------------------------

    /// Every tick: the size change, the wall the player cannot pass and
    /// damage outside the border.
    /// `BorderChangeListener.DelegateBorderChangeListener`'s packets: what
    /// changed on the border since the guests were last told of it.
    pub(in crate::game) fn lan_sync_border(&mut self) {
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        let now = sunrise_sim::lan::border::Sent::of(&self.border);
        if let Some(sent) = host.border_sent.replace(now) {
            for packet in sent.changes(&self.border) {
                // PlayerList$1: only the players of the border's own level.
                host.hub.broadcast_level(&packet);
            }
        }
    }
    pub(in crate::game::server_side) fn tick_worldborder(&mut self) {
        let remaining = self.border.remaining_ticks();
        self.border.tick();
        if remaining > 0
            && self.border.remaining_ticks() == 0
            && (self.serving.is_some() || self.options.dedicated.is_some())
        {
            self.native_world_border_bounds_change_finish();
        }
        self.player_border();
        self.border_damage();
    }
    /// `LivingEntity.baseTick`: a player outside the border, further than
    /// its safe zone, is hurt by the blocks beyond it. The game's player, or
    /// a guest's in their turn (`as_guest`).
    pub(in crate::game::server_side) fn border_damage(&mut self) {
        let Some((px, pz, half)) = self.player.as_ref().map(|p| {
            (
                p.position.x,
                p.position.z,
                f64::from(p.dimensions.width) / 2.0,
            )
        }) else {
            return;
        };
        let inside = self.border.contains(px - half, pz - half)
            && self.border.contains(px + half, pz + half);
        if inside {
            return;
        }
        let dist = self.border.distance(px, pz) + self.border.safe_zone;
        if dist < 0.0 && self.border.damage_per_block > 0.0 && self.ticks.is_multiple_of(10) {
            let amount = (-dist * self.border.damage_per_block).floor().max(1.0) as f32;
            self.hurt_player_by("minecraft:outside_border", amount);
        }
    }
    pub(in crate::game::server_side) fn cmd_worldborder(
        &mut self,
        id: &str,
        ctx: &Context,
    ) -> R<i32> {
        let float2 = |v: f64| super::percent_f(v, 2);
        let double = match ctx.get("distance") {
            Some(Arg::Double(d)) => *d,
            _ => 0.0,
        };
        let time = match ctx.get("time") {
            Some(Arg::Int(t)) => Some(i64::from(*t)),
            _ => None,
        };
        match id {
            "worldborder.get" => {
                let size = self.border.size();
                self.feedback(Text::translate(
                    "commands.worldborder.get",
                    &[Text::literal(super::percent_f(size, 0))],
                ));
                Ok((size + 0.5).floor() as i32)
            }
            "worldborder.set" | "worldborder.add" => {
                let current = self.border.size();
                // `add` measures from the size the border has now and, given a
                // time, keeps a change in flight going for its remaining ticks
                // plus the argument (`getLerpTime() + time`); `set` replaces it.
                let (distance, ticks) = match (id, time) {
                    ("worldborder.add", Some(t)) => {
                        (current + double, self.border.remaining_ticks() + t)
                    }
                    ("worldborder.add", None) => (current + double, 0),
                    (_, t) => (double, t.unwrap_or(0)),
                };
                if current == distance {
                    return Err(err("commands.worldborder.set.failed.nochange", &[]));
                }
                if distance < 1.0 {
                    return Err(err("commands.worldborder.set.failed.small", &[]));
                }
                if distance > MAX_SIZE {
                    return Err(err(
                        "commands.worldborder.set.failed.big",
                        &[&size_limit_text()],
                    ));
                }
                let shown = Text::literal(super::percent_f(distance, 1));
                if ticks > 0 {
                    self.border.lerp_to(distance, ticks);
                    let key = if distance > current {
                        "commands.worldborder.set.grow"
                    } else {
                        "commands.worldborder.set.shrink"
                    };
                    self.feedback(Text::translate(
                        key,
                        &[shown, Text::literal(ticks_to_seconds(ticks))],
                    ));
                } else {
                    self.border.set_size(distance);
                    self.feedback(Text::translate(
                        "commands.worldborder.set.immediate",
                        &[shown],
                    ));
                }
                self.native_world_border_bounds_changed();
                Ok((distance - current) as i32)
            }
            "worldborder.center" => {
                let p = self
                    .position_arg(ctx, "pos")
                    .unwrap_or_else(|| self.source_position());
                let (x, z) = (p[0], p[2]);
                if self.border.center() == [x, z] {
                    return Err(err("commands.worldborder.center.failed", &[]));
                }
                if x.abs() > MAX_CENTER || z.abs() > MAX_CENTER {
                    return Err(err(
                        "commands.worldborder.set.failed.far",
                        &[&"2.9999984E7"],
                    ));
                }
                self.border.set_center([x, z]);
                self.native_world_border_center_changed();
                self.feedback(Text::translate(
                    "commands.worldborder.center.success",
                    &[Text::literal(float2(x)), Text::literal(float2(z))],
                ));
                Ok(0)
            }
            "worldborder.damage.amount" | "worldborder.damage.buffer" => {
                let amount = id.ends_with("amount");
                let value = match ctx.get(if amount { "damagePerBlock" } else { "distance" }) {
                    Some(Arg::Float(v)) => f64::from(*v),
                    _ => 0.0,
                };
                let slot = if amount {
                    &mut self.border.damage_per_block
                } else {
                    &mut self.border.safe_zone
                };
                if *slot == value {
                    return Err(err(
                        if amount {
                            "commands.worldborder.damage.amount.failed"
                        } else {
                            "commands.worldborder.damage.buffer.failed"
                        },
                        &[],
                    ));
                }
                *slot = value;
                self.feedback(Text::translate(
                    if amount {
                        "commands.worldborder.damage.amount.success"
                    } else {
                        "commands.worldborder.damage.buffer.success"
                    },
                    &[Text::literal(float2(value))],
                ));
                Ok(value as i32)
            }
            "worldborder.warning.distance" => {
                let d = match ctx.get("distance") {
                    Some(Arg::Int(v)) => *v,
                    _ => 0,
                };
                if self.border.warning_distance == d {
                    return Err(err("commands.worldborder.warning.distance.failed", &[]));
                }
                self.border.warning_distance = d;
                self.feedback(Text::translate(
                    "commands.worldborder.warning.distance.success",
                    &[num(d)],
                ));
                Ok(d)
            }
            _ => {
                let ticks = match ctx.get("time") {
                    Some(Arg::Int(v)) => *v,
                    _ => 0,
                };
                if self.border.warning_time == ticks {
                    return Err(err("commands.worldborder.warning.time.failed", &[]));
                }
                self.border.warning_time = ticks;
                self.feedback(Text::translate(
                    "commands.worldborder.warning.time.success",
                    &[Text::literal(ticks_to_seconds(i64::from(ticks)))],
                ));
                Ok(ticks)
            }
        }
    }

    // ---- /loot ---------------------------------------------------------------------------------
}

#[cfg(test)]
pub(in crate::game) mod tests {
    /// `WorldBorderCommand.ERROR_TOO_BIG` is built with the double
    /// `5.9999968E7`, the same value `setSize` compares `distance` against, and
    /// the `%s` substitution of a translatable renders a double with
    /// `Double.toString`: the message names `5.9999968E7`, not a rounded
    /// `5.999997E7`.
    #[test]
    fn the_too_big_message_names_the_limit_as_vanilla_prints_it() {
        assert_eq!(super::MAX_SIZE, 5.9999968E7);
        assert_eq!(super::size_limit_text(), "5.9999968E7");
    }
}

#[cfg(test)]
pub(in crate::game) mod loot_mine_component_tests {
    use super::*;
    use crate::game::{Options, Progress, Settings, load_resources};
    use std::sync::Arc;
    use sunrise_core::nbt::{Compound, Tag};
    use sunrise_level::{chunk::Chunk, containers::BlockEntity};
    use sunrise_sim::save;

    pub(in crate::game) fn slot(game: &crate::game::ServerGame<'_>, index: usize) -> ItemStack {
        let Some(BlockEntity::Chest(items)) = game.world.block_entities.get(&(12, 200, 4)) else {
            panic!("actual command-created chest required");
        };
        items[index]
            .clone()
            .expect("actual inventory slot must be occupied")
    }

    pub(in crate::game) fn decorations(stack: &ItemStack) -> Option<Tag> {
        save::item_to_nbt(stack, &Default::default())
            .get("components")?
            .get("minecraft:pot_decorations")
            .cloned()
    }

    #[test]
    #[ignore = "daily"]
    fn loot_mine_copies_actual_empty_and_right_pot_components_into_the_chest() {
        let root = std::path::PathBuf::from(
            std::env::var_os("SUNRISE_TEST_RESOURCES").expect("prepared resources required"),
        );
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
        options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
        let mut game = crate::game::ServerGame::new(
            &loaded.resources,
            loaded.pack,
            options,
            settings.sim_settings(),
            None,
        )
        .unwrap();
        game.build_registries();
        game.set_allow_commands(true);
        game.world
            .chunks
            .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
        game.block_ticks
            .add_empty_chunk(ChunkPos::new(0, 0))
            .unwrap();
        assert!(game.run_command("forceload add 0 0"));
        assert!(game.run_command("fill 0 199 0 15 199 15 stone"));
        assert!(game.run_command(
            "setblock 12 200 4 chest[facing=north,type=single,waterlogged=false] strict"
        ));
        assert!(
            game.run_command("item replace block 12 200 4 container.26 with minecraft:diamond 7")
        );
        assert!(game.run_command("item replace block 12 200 4 container.2 with minecraft:decorated_pot[minecraft:pot_decorations={}] 1"));
        let empty = Tag::Compound(Compound::new());
        assert_eq!(
            (&*slot(&game, 26).item, slot(&game, 26).count),
            ("minecraft:diamond", 7)
        );
        assert_eq!(
            decorations(&slot(&game, 2)),
            Some(empty.clone()),
            "direct-item parser/encoder positive control"
        );
        assert!(
            slot(&game, 2)
                .extra
                .as_ref()
                .unwrap()
                .pot_decorations
                .is_some()
        );
        let start = game.world.game_time;
        let right = "minecraft:angler_pottery_sherd";
        let mut failures = Vec::new();
        for (x, nbt, decorated) in [
            (4, "", false),
            (
                7,
                r#"{sherds:{right:{id:"minecraft:angler_pottery_sherd"}}}"#,
                true,
            ),
        ] {
            assert!(game.run_command(&format!(
                "setblock {x} 200 4 decorated_pot[facing=north,cracked=false,waterlogged=false]{nbt} strict"
            )));
            let pos = [x, 200, 4];
            let state = game.world.block(x, 200, 4);
            let actual = game.pack.assets.registry.state(state).unwrap();
            assert_eq!(&*actual.name, "minecraft:decorated_pot");
            assert_eq!(
                actual.properties.get("cracked").map(String::as_str),
                Some("false")
            );
            assert_eq!(
                game.world.block(x, 199, 4),
                game.pack
                    .assets
                    .registry
                    .default_state("minecraft:stone")
                    .unwrap()
            );
            let before = game.pot_data(pos).expect("actual source block entity read");
            assert_eq!(
                before.get("id"),
                Some(&Tag::String("minecraft:decorated_pot".into()))
            );
            for (key, value) in [("x", x), ("y", 200), ("z", 4)] {
                assert_eq!(before.get(key), Some(&Tag::Int(value)));
            }
            if decorated {
                assert_eq!(
                    before
                        .get("sherds")
                        .and_then(|t| t.get("right"))
                        .and_then(|t| t.get("id")),
                    Some(&Tag::String(right.into())),
                    "actual admitted source sherd"
                );
            } else {
                assert!(before.get("sherds").is_none());
            }
            assert!(before.get("item").is_none() && before.get("LootTable").is_none());
            assert!(
                game.run_command("item replace block 12 200 4 container.0 with minecraft:stone 3")
            );
            let initial = slot(&game, 0);
            assert_eq!((&*initial.item, initial.count), ("minecraft:stone", 3));
            assert!(game.run_command(&format!(
                "loot replace block 12 200 4 container.0 1 mine {x} 200 4"
            )));
            let out = slot(&game, 0);
            assert_eq!(
                (&*out.item, out.count),
                ("minecraft:decorated_pot", 1),
                "inventory is the count authority"
            );
            let expected = if decorated {
                Tag::compound([("right", Tag::compound([("id", Tag::String(right.into()))]))])
            } else {
                empty.clone()
            };
            if decorations(&out) != Some(expected) {
                failures.push(format!(
                    "source {x}: actual saved output pot_decorations missing/wrong"
                ));
            }
            let modeled = out.extra.as_ref().and_then(|d| d.pot_decorations.as_ref());
            if modeled.is_none_or(|sides| {
                sides.iter().enumerate().any(|(i, item)| {
                    if decorated && i == 2 {
                        item.as_ref()
                            .is_none_or(|s| &*s.item != right || s.count != 1)
                    } else {
                        item.is_some()
                    }
                })
            }) {
                failures.push(format!(
                    "source {x}: actual output component state missing/wrong"
                ));
            }
            assert_eq!(
                game.pot_data(pos),
                Some(before),
                "mine leaves the actual source unchanged"
            );
            assert_eq!(game.world.block(x, 200, 4), state);
            assert_eq!(
                (&*slot(&game, 26).item, slot(&game, 26).count),
                ("minecraft:diamond", 7)
            );
            assert_eq!(decorations(&slot(&game, 2)), Some(empty.clone()));
            assert_eq!(game.world.game_time, start);
        }
        assert!(game.run_command("setblock 10 200 4 dirt strict"));
        assert!(game.run_command("loot replace block 12 200 4 container.0 1 mine 10 200 4"));
        let plain = slot(&game, 0);
        assert_eq!((&*plain.item, plain.count), ("minecraft:dirt", 1));
        assert!(decorations(&plain).is_none());
        assert_eq!(game.world.game_time, start);
        assert!(
            failures.is_empty(),
            "actual loot-mine component-copy failures: {failures:#?}"
        );
    }
}

#[cfg(test)]
pub(in crate::game) mod loot_position_tests {
    use crate::game::{Options, Progress, Settings, load_resources};

    #[test]
    #[ignore = "daily"]
    fn actual_loot_replacement_retains_the_emitted_zero_slot() {
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
        options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
        let mut game = crate::game::ServerGame::new(
            &loaded.resources,
            loaded.pack,
            options,
            settings.sim_settings(),
            None,
        )
        .unwrap();
        game.build_registries();

        game.set_allow_commands(true);
        game.world.chunks.insert(
            (0, 0),
            std::sync::Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)),
        );
        game.block_ticks
            .add_empty_chunk(sunrise_core::position::ChunkPos::new(0, 0))
            .unwrap();
        assert!(game.run_command("forceload add 0 0"));
        assert!(game.run_command("setblock 4 200 4 chest"));
        let slot = |game: &mut crate::game::ServerGame<'_>, i| {
            game.slot_get(&crate::game::commands::items::Slot::Block([4, 200, 4], i))
        };
        let reset = |game: &mut crate::game::ServerGame<'_>| {
            assert!(game.run_command("item replace block 4 200 4 container.0 with diamond 3"));
            assert!(game.run_command("item replace block 4 200 4 container.1 with emerald 4"));
            assert!(game.run_command("item replace block 4 200 4 container.2 with gold_ingot 5"));
            assert_eq!(
                slot(game, 0).map(|s| (s.item.to_string(), s.count)),
                Some(("minecraft:diamond".into(), 3))
            );
            assert_eq!(
                slot(game, 1).map(|s| (s.item.to_string(), s.count)),
                Some(("minecraft:emerald".into(), 4))
            );
        };
        let table = |counts: &[i32]| {
            serde_json::json!({"pools":counts.iter().map(|count| serde_json::json!({
            "rolls":1,"entries":[{"type":"minecraft:item","name":"minecraft:iron_ingot",
                "modifier":{"type":"minecraft:set_count","count":count}}]
        })).collect::<Vec<_>>()})
            .to_string()
        };
        let time = game.world.game_time;
        reset(&mut game);
        assert!(game.run_command(&format!(
            "loot replace block 4 200 4 container.0 loot {}",
            table(&[1])
        )));
        assert_eq!(
            slot(&mut game, 0).map(|s| (s.item.to_string(), s.count)),
            Some(("minecraft:iron_ingot".into(), 1))
        );
        reset(&mut game);
        assert!(game.run_command(&format!(
            "loot replace block 4 200 4 container.0 loot {}",
            table(&[0, 1])
        )));
        assert!(
            slot(&mut game, 0).is_none(),
            "the emitted zero occupies and clears slot0"
        );
        assert_eq!(
            slot(&mut game, 1).map(|s| (s.item.to_string(), s.count)),
            Some(("minecraft:iron_ingot".into(), 1))
        );
        assert_eq!(slot(&mut game, 2).unwrap().count, 5);
        reset(&mut game);
        assert!(game.run_command(&format!(
            "loot replace block 4 200 4 container.0 2 loot {}",
            table(&[])
        )));
        assert!(slot(&mut game, 0).is_none());
        assert!(slot(&mut game, 1).is_none());
        assert_eq!(slot(&mut game, 2).unwrap().count, 5);
        assert_eq!(game.world.game_time, time);
    }
}

#[cfg(test)]
pub(in crate::game) mod loot_kill_target_tests {
    use super::*;
    use crate::game::{Options, Progress, Settings, load_resources};
    use std::sync::Arc;
    use sunrise_level::{chunk::Chunk, containers::BlockEntity};
    #[test]
    #[ignore = "daily"]
    fn loot_kill_uses_actual_sheep_state_without_killing_the_target() {
        let root = std::path::PathBuf::from(
            std::env::var_os("SUNRISE_TEST_RESOURCES").expect("prepared resources required"),
        );
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
        options.game_dir = Some(
            std::path::PathBuf::from(std::env::var_os("SUNRISE_HOME").unwrap()).join("h036-game"),
        );
        options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
        let mut game = crate::game::ServerGame::new(
            &loaded.resources,
            loaded.pack,
            options,
            settings.sim_settings(),
            None,
        )
        .unwrap();
        game.build_registries();
        // A real world open publishes its owned loot evaluator before commands run.
        game.load_server_loot();
        game.set_allow_commands(true);
        game.world
            .chunks
            .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
        game.block_ticks
            .add_empty_chunk(ChunkPos::new(0, 0))
            .unwrap();
        assert!(game.run_command("forceload add 0 0"));
        assert!(game.run_command("fill 0 199 0 15 199 15 stone"));
        let clock = game.world.game_time;
        let mut differences = Vec::new();
        for (n, color, sheared, wool) in [
            (1, 0, true, "minecraft:white_wool"),
            (2, 14, true, "minecraft:red_wool"),
            (3, 0, false, "minecraft:white_wool"),
            (4, 14, false, "minecraft:red_wool"),
        ] {
            let uuid = format!("00000024-0000-0000-0000-{n:012x}");
            assert!(game.run_command(&format!(
                "summon sheep 4.5 200 4.5 {{UUID:[I;36,0,0,{n}],Health:8f,NoAI:1b,NoGravity:1b,Age:0,Color:{color}b,Sheared:{}b,Motion:[0d,0d,0d]}}", i32::from(sheared)
            )));
            let mob = game
                .mobs
                .iter()
                .find(|m| game.persist.uuid_of(m.id) == Some([36, 0, 0, n]))
                .unwrap();
            assert_eq!(mob.kind.id, "minecraft:sheep");
            assert_eq!(mob.health, 8.0);
            assert!(mob.no_ai && mob.no_gravity);
            assert_eq!(mob.extra.color, color);
            assert_eq!(mob.extra.sheared, sheared);
            let before = (mob.id, mob.position, mob.velocity, mob.health);
            let x = 2 + n * 3;
            assert!(game.run_command(&format!(
                "setblock {x} 200 8 chest[facing=north,type=single,waterlogged=false] strict"
            )));
            let Some(BlockEntity::Chest(initial)) = game.world.block_entities.get(&(x, 200, 8))
            else {
                panic!("real chest required")
            };
            assert!(initial.iter().all(Option::is_none));
            assert!(game.run_command(&format!("loot insert {x} 200 8 kill {uuid}")));
            let Some(BlockEntity::Chest(items)) = game.world.block_entities.get(&(x, 200, 8))
            else {
                panic!("real output chest required")
            };
            let output: Vec<_> = items.iter().flatten().collect();
            assert!(
                output
                    .iter()
                    .any(|s| s.item.as_ref() == "minecraft:mutton" && (1..=2).contains(&s.count)),
                "nonempty meat proves the command actually rolled"
            );
            let actual: u32 = output
                .iter()
                .filter(|s| s.item.as_ref() == wool)
                .map(|s| s.count)
                .sum();
            if actual != u32::from(!sheared) {
                differences.push((n, sheared, wool, actual));
            }
            let mob = game.mobs.iter().find(|m| m.id == before.0).unwrap();
            assert_eq!((mob.id, mob.position, mob.velocity, mob.health), before);
            assert_eq!(game.persist.uuid_of(mob.id), Some([36, 0, 0, n]));
            assert_eq!((mob.extra.color, mob.extra.sheared), (color, sheared));
        }
        assert_eq!(game.world.game_time, clock);
        assert!(
            differences.is_empty(),
            "actual command wool-state differences: {differences:?}"
        );
    }
}

#[cfg(test)]
pub(in crate::game) mod loot_mine_location_tests {
    pub(in crate::game) const TABLE: &str = r###"{"type":"minecraft:block","pools":[{"rolls":1,"entries":[{"type":"minecraft:item","name":"minecraft:emerald","modifier":{"type":"minecraft:set_count","count":2}}]},{"rolls":1,"condition":{"type":"minecraft:location_check","offsetX":1,"predicate":{"block":{"blocks":"minecraft:oak_log","state":{"axis":"x"}}}},"entries":[{"type":"minecraft:item","name":"minecraft:diamond"}]}]}"###;
}

#[cfg(test)]
pub(in crate::game) mod loot_location_shape_tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;
    use sunrise_level::{chunk::Chunk, chunk_map::ChunkMap};
    use sunrise_world::registry::BlockRegistry;

    #[test]
    fn location_shape_uses_loaded_facts_and_rejects_whole_unsupported_conjunctions() {
        let registry = BlockRegistry::from_facts_json(
            &br#"{"defaults":{"minecraft:air":0,"minecraft:oak_log":1},"states":[
            ["air",true,false,"","minecraft:air"],
            ["log",false,false,"",{"id":"minecraft:oak_log","properties":{"axis":"x"}}]
        ]}"#[..],
        )
        .unwrap();
        let mut positive = Chunk::empty(0, 0);
        positive.set(5, 200, 4, 1);
        let mut negative = Chunk::empty(-1, -1);
        negative.set(13, 200, 12, 1);
        let mut chunks = ChunkMap::default();
        chunks.insert((0, 0), Arc::new(positive));
        chunks.insert((-1, -1), Arc::new(negative));
        let items = sunrise_items::items::Items::for_tests(Default::default());
        let valid = json!({"type":"minecraft:location_check","offsetX":1,"predicate":{"block":{"blocks":"minecraft:oak_log","state":{"axis":"x"}}}});
        assert_eq!(
            loot_location_block(&valid, [4, 200, 4], &chunks, &registry, &items),
            Some(true)
        );
        assert_eq!(
            loot_location_block(&valid, [-4, 200, -4], &chunks, &registry, &items),
            Some(true)
        );
        assert_eq!(
            loot_location_block(&valid, [8, 200, 4], &chunks, &registry, &items),
            Some(false)
        );
        for (key, value) in [
            ("biomes", json!("minecraft:plains")),
            ("light", json!({"light":15})),
            ("dimension", json!("minecraft:overworld")),
        ] {
            let mut unsupported = valid.clone();
            unsupported["predicate"][key] = value;
            assert_eq!(
                loot_location_block(&unsupported, [4, 200, 4], &chunks, &registry, &items),
                None
            );
        }
        for value in [json!(1.5), json!(2147483648_i64), json!("1")] {
            let mut unsupported = valid.clone();
            unsupported["offsetX"] = value;
            assert_eq!(
                loot_location_block(&unsupported, [4, 200, 4], &chunks, &registry, &items),
                None
            );
        }
        let count = chunks.len();
        assert_eq!(
            loot_location_block(&valid, [32, 200, 32], &chunks, &registry, &items),
            Some(false),
            "an unloaded counterpart fails, as Level.isLoaded does"
        );
        assert_eq!(chunks.len(), count, "no chunk is loaded as a side effect");
    }
}

#[cfg(test)]
#[path = "loot_slot_tests.rs"]
pub(in crate::game) mod loot_slot_tests;
