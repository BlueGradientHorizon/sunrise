//! Brushing suspicious sand and gravel: `BrushItem.onUseTick` and
//! `BrushableBlockEntity`. The block's loot table stays in its saved block
//! entity (like a decorated pot's) until the first counted stroke rolls it.
#[allow(unused_imports)]
use super::{
    Game, Hit, decorated_pot,
    item_draw::{BlockItemDraw, block_entity_pose},
    view_vector,
};
#[allow(unused_imports)]
use sunrise_client::{
    block_entity_render::vertex_light,
    camera::rotate_degrees,
    containers::BlockEntity,
    mesher::Vertex,
    particles::{ParticleOptions, ParticleType},
    save,
};
#[allow(unused_imports)]
use sunrise_client_render::block_entity_reach::Reach;
#[allow(unused_imports)]
use sunrise_core::random::RandomSource;

#[allow(unused_imports)]
pub(in crate::game) use sunrise_gameplay::brushing::{Brushing, USE_DURATION, completion_state};

impl Game<'_> {
    /// `BrushItem.onUseTick` for a brush used `ticks` ticks so far, the
    /// hand of `slot` acting.
    pub(in crate::game) fn tick_brush(
        &mut self,
        stack: &sunrise_client::items::ItemStack,
        slot: usize,
        ticks: u32,
    ) {
        // No block under the crosshair: vanilla calls releaseUsingItem, which
        // stops the use and causes the interaction vibration.
        let Some(hit) = self.item_pick() else {
            self.stop_using_item(stack);
            return;
        };
        // `BrushItem.onUseTick` aims with `getHitResultOnViewVector` out to the
        // block interaction range, for mobs too: a mob between the player and
        // the block is nearer, so the hit is not a block and the use releases
        // the same way. A guest acts on its client's targeted block, whose view
        // we cannot trace here, so only the local aim runs the entity test.
        if self.server.forced_hit.is_none()
            && let Some((eye, _)) = self.aim()
            && let Some((_, mob, _)) = self.mob_hit_within(self.block_reach)
        {
            let block = (
                hit.point[0] - eye[0],
                hit.point[1] - eye[1],
                hit.point[2] - eye[2],
            );
            if mob <= (block.0 * block.0 + block.1 * block.1 + block.2 * block.2).sqrt() {
                self.stop_using_item(stack);
                return;
            }
        }
        let elapsed = (USE_DURATION as i32)
            .wrapping_sub(super::item_use::ticks_left(self.use_duration, ticks))
            .wrapping_add(1);
        // The last tick before the backswing.
        if elapsed % 10 == 5 {
            self.brush_stroke(&hit, slot);
        }
        // updateUsingItem owns the countdown and completion, including a
        // replacement remaining counter from the Tick event.
        self.using = Some((slot, ticks + 1));
    }

    /// One stroke on the block aimed at: dust (this window only), the brush
    /// sound, and the block entity's count. The brush wears when the block
    /// changed state.
    pub(in crate::game::server_side) fn brush_stroke(&mut self, hit: &Hit, slot: usize) {
        let state = self.world.block(hit.pos[0], hit.pos[1], hit.pos[2]);
        let name = self.res.facts().get(state).name.clone();
        let (sound, brushable) = match &*name {
            "minecraft:suspicious_sand" => ("minecraft:item.brush.brushing.sand", true),
            "minecraft:suspicious_gravel" => ("minecraft:item.brush.brushing.gravel", true),
            _ => ("minecraft:item.brush.brushing.generic", false),
        };
        if self.server.lan_acting.is_none() {
            self.brush_dust(hit, slot == super::shield::OFFHAND_SLOT);
        }
        let at = hit.pos.map(|v| f64::from(v) + 0.5);
        self.client_audio_play(sound, Some(at), 1.0, 1.0);
        if brushable && self.brush_block(hit.pos, hit.face, slot) {
            self.damage_held(1);
        }
    }

    /// `BrushableBlockEntity.brush`: whether the block changed (the brush wears).
    pub(in crate::game::server_side) fn brush_block(
        &mut self,
        pos: [i32; 3],
        face: [i32; 3],
        slot: usize,
    ) -> bool {
        let time = self.world.game_time;
        let mut brush = self.server.brushing.remove(&pos).unwrap_or_default();
        brush.hit_face.get_or_insert(face);
        let direction = match brush.hit_face.unwrap() {
            [0, -1, 0] => 0,
            [0, 1, 0] => 1,
            [0, 0, -1] => 2,
            [0, 0, 1] => 3,
            [-1, 0, 0] => 4,
            [1, 0, 0] => 5,
            _ => return false,
        };
        if let Some(BlockEntity::Brushable(b)) =
            self.world.block_entities.get_mut(&(pos[0], pos[1], pos[2]))
        {
            b.hit_direction = Some(direction);
        }
        brush.resets_at = time + 40;
        if time < brush.cooldown_ends_at {
            self.server.brushing.insert(pos, brush);
            return false;
        }
        brush.cooldown_ends_at = time + 10;
        self.unpack_brush_loot(pos, slot);
        let before = completion_state(brush.count);
        brush.count += 1;
        if brush.count >= 10 {
            self.brushing_completed(pos, brush.hit_face, slot);
            return true;
        }
        self.schedule_brushable_tick(pos);
        let after = completion_state(brush.count);
        self.server.brushing.insert(pos, brush);
        if before != after {
            self.set_dusted(pos, after);
        }
        false
    }

    /// `BrushableBlockEntity.unpackLootTable`: the table's roll becomes the
    /// block entity's item. `LootContextParamSets.ARCHAEOLOGY`: the brusher is
    /// THIS_ENTITY, the brush in `slot` the TOOL and the block's centre the
    /// ORIGIN, with the brusher's luck.
    pub(in crate::game::server_side) fn unpack_brush_loot(&mut self, pos: [i32; 3], slot: usize) {
        let key = (pos[0], pos[1], pos[2]);
        let Some(BlockEntity::Brushable(b)) = self.world.block_entities.get(&key) else {
            return;
        };
        let Some(sunrise_core::nbt::Tag::String(table)) = &b.loot_table else {
            return;
        };
        let table = table.to_string_lossy();
        let mut tag = save::block_entity_encode::encode(
            key,
            &BlockEntity::Brushable(b.clone()),
            "minecraft:brushable_block",
            self.res.items(),
        );
        self.trigger_loot_table(&table);
        let loot = self.server_loot.0.get();
        let brusher = self.killer_loot_entity(None);
        let luck = self.acting_luck();
        let tool = self.stack_in(slot).map(super::creatures::to_loot);
        let caller = self.caller_loot(Some(pos.map(|c| f64::from(c) + 0.5)), tool);
        caller.with(|| {
            decorated_pot::unpack_loot(
                &mut tag,
                loot.as_deref(),
                &mut self.world.random,
                &mut self.server.rand_sequences,
                self.level.seed,
                self.res.items(),
                sunrise_world::loot::ContainerLootContext {
                    luck,
                    entity: Some(&brusher),
                    ..Default::default()
                },
            )
        });
        if let Some(BlockEntity::Brushable(b)) = self.world.block_entities.get_mut(&key) {
            b.loot_table = tag.get("LootTable").cloned();
            b.table_seed_tag = tag.get("LootTableSeed").cloned();
            b.item = tag.get("item").and_then(save::item_from_nbt);
        }
        self.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
    }

    /// `BrushableBlockEntity.brushingCompleted`: the item drops out of the
    /// brushed face and the block turns into its sand or gravel.
    pub(in crate::game::server_side) fn brushing_completed(
        &mut self,
        pos: [i32; 3],
        face: Option<[i32; 3]>,
        slot: usize,
    ) {
        let state = self.world.block(pos[0], pos[1], pos[2]);
        let turns_into = match &*self.res.facts().get(state).name {
            "minecraft:suspicious_sand" => "minecraft:sand",
            "minecraft:suspicious_gravel" => "minecraft:gravel",
            _ => "minecraft:air",
        };
        // dropContent: the item at the middle of the block beside that face.
        let item = match self.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
            Some(BlockEntity::Brushable(b)) => b.item.clone(),
            _ => None,
        };
        if let Some(mut item) = item {
            let face = face.unwrap_or([0, 1, 0]);
            let take = 10 + self.world.random.next_int_bound(21).unwrap_or(0);
            item.count = item.count.min(take as u32);
            let at = [
                f64::from(pos[0] + face[0]) + 0.5,
                f64::from(pos[1] + face[1]) + 0.625,
                f64::from(pos[2] + face[2]) + 0.5,
            ];
            let mut drops = vec![item];
            let tool = self.stack_in(slot).cloned();
            let origin = pos.map(|c| f64::from(c) + 0.5);
            let player = Some(self.native_acting_id());
            self.native_use_drops(
                "",
                origin,
                Some((pos, state)),
                None,
                player,
                tool.as_ref(),
                &mut drops,
            );
            for item in drops {
                self.spawn_item(item, at, [0.0; 3], 0);
            }
        }
        // Level event 3008 for every player, then the block change.
        self.level_event_at(3008, pos, state.0 as i32);
        if let Some(base) = self.pack.assets.registry.default_state(turns_into) {
            let _ = self.level_set(pos, base);
        }
    }

    pub(in crate::game::server_side) fn set_dusted(&mut self, pos: [i32; 3], dusted: i32) {
        let registry = self.pack.assets.registry;
        let Some(mut state) = registry
            .state(self.world.block(pos[0], pos[1], pos[2]))
            .cloned()
        else {
            return;
        };
        state.properties.insert("dusted".into(), dusted.to_string());
        if let Ok(state) = registry.resolve(&state) {
            let _ = self.level_set(pos, state);
        }
    }

    pub(in crate::game::server_side) fn schedule_brushable_tick(&mut self, pos: [i32; 3]) {
        let at = sunrise_core::position::BlockPos::new(pos[0], pos[1], pos[2]);
        self.with_level(|b, level, ticks| b.schedule_tick(level, ticks, at, 2));
    }

    /// `BrushableBlockEntity.checkReset`, from `BrushableBlock.tick`: a block
    /// left alone loses two counts, again every four ticks.
    pub(in crate::game::server_side) fn brushable_tick(&mut self, pos: [i32; 3]) {
        let time = self.world.game_time;
        let Some(mut brush) = self.server.brushing.remove(&pos) else {
            return;
        };
        let before = completion_state(brush.count);
        if brush.count != 0 && time >= brush.resets_at {
            brush.count = (brush.count - 2).max(0);
            brush.resets_at = time + 4;
        }
        let after = completion_state(brush.count);
        if brush.count != 0 {
            self.server.brushing.insert(pos, brush);
            self.schedule_brushable_tick(pos);
        } else if let Some(BlockEntity::Brushable(b)) =
            self.world.block_entities.get_mut(&(pos[0], pos[1], pos[2]))
        {
            b.hit_direction = None;
        }
        if before != after {
            self.set_dusted(pos, after);
        }
    }

    /// `BrushableBlockRenderer`: reveal the item from the first brushed face.
    pub(in crate::game) fn brushable_geometry(&self, camera: glam::DVec3, out: &mut Vec<Vertex>) {
        self.brushable_items(camera, |d| {
            self.submit_item_world(d.state, d.pose, d.light, out);
        });
    }

    /// The items [`Self::brushable_geometry`] draws.
    pub(in crate::game::server_side) fn brushable_items(
        &self,
        camera: glam::DVec3,
        mut draw: impl FnMut(BlockItemDraw),
    ) {
        use sunrise_assets::item_model::DisplayContext;
        let _items = sunrise_client::spans::raw_flight_guard("block_entity_items");
        let reach = Reach::Default;
        sunrise_client::spans::note_block_entity_item_visits(|| {
            self.world.block_entities.len() as u64
        });
        for (key, entity) in &self.world.block_entities {
            let BlockEntity::Brushable(b) = entity else {
                continue;
            };
            let (Some(stack), Some(face)) = (&b.item, b.hit_direction) else {
                continue;
            };
            if !reach.should_render([key.0, key.1, key.2], camera, 0) {
                continue;
            }
            let state = self.world.block(key.0, key.1, key.2);
            let progress = self
                .pack
                .assets
                .registry
                .state(state)
                .and_then(|s| s.properties.get("dusted"))
                .and_then(|s| s.parse::<u8>().ok())
                .unwrap_or(0);
            if progress == 0 {
                continue;
            }
            // `BrushableBlockRenderer.translations`.
            let offset = f32::from(progress) / 10.0 * 0.75;
            let (translation, adjacent, east_west) = match face {
                0 => ([0.5, -0.23 - offset, 0.5], [0, -1, 0], false),
                1 => ([0.5, 0.25 + offset, 0.5], [0, 1, 0], false),
                2 => ([0.5, 0.0, 0.25 - offset], [0, 0, -1], false),
                3 => ([0.5, 0.0, 0.73 + offset], [0, 0, 1], false),
                4 => ([0.25 - offset, 0.0, 0.5], [-1, 0, 0], true),
                5 => ([0.73 + offset, 0.0, 0.5], [1, 0, 0], true),
                _ => continue,
            };
            let item = self.resolve_item(stack, DisplayContext::Fixed, None, 0);
            if item.is_empty() {
                continue;
            }
            let mut pose = block_entity_pose(*key, camera);
            pose.translate(0.0, 0.5, 0.0);
            pose.translate(translation[0], translation[1], translation[2]);
            rotate_degrees(&mut pose, 1, 75.0);
            rotate_degrees(&mut pose, 1, if east_west { 101.0 } else { 11.0 });
            pose.scale(0.5, 0.5, 0.5);
            let light = self.world.light(
                key.0 + adjacent[0],
                key.1 + adjacent[1],
                key.2 + adjacent[2],
            );
            draw(BlockItemDraw {
                #[cfg(test)]
                stack,
                #[cfg(test)]
                display: DisplayContext::Fixed,
                #[cfg(test)]
                seed: 0,
                state: &item,
                pose: pose.to_glam(),
                light: vertex_light(light),
            });
        }
    }
}
