//! Item frames and glow item frames: `ItemFrameItem` placement on any face,
//! `ItemFrame.survives`, `interact` (put an item in, then rotate it),
//! `hurtServer` (take the item out, then break the frame) and
//! `ItemFrameRenderer` (the frame model and the item at a 45 degree step).
use super::{Game, Hit, InteractTarget};
use sunrise_assets::{bake::bake_model, item_model::OwnerState, resolve::ModelLibrary};
use sunrise_core::{
    geometry::{Aabb, Vec3},
    position::BlockPos,
    random::RandomSource,
};
use sunrise_items::items::ItemStack;
use sunrise_sim::entities::ItemFrame;
/// A baked frame model: (corners in block units, unit UVs, sprite) per quad.
pub(in crate::game) type FrameModel = Vec<([[f32; 3]; 4], [[f32; 2]; 4], String)>;

/// Item-frame variants prepared together for one resource generation.
pub(in crate::game) fn bake_frames(library: Option<&ModelLibrary>) -> [FrameModel; 4] {
    [
        "item_frame",
        "glow_item_frame",
        "item_frame_map",
        "glow_item_frame_map",
    ]
    .map(|name| library.map_or_else(Vec::new, |library| bake_frame(library, name)))
}

/// Bake `block/<name>` (item_frame / glow_item_frame) from the resources.
pub(in crate::game::server_side) fn bake_frame(library: &ModelLibrary, name: &str) -> FrameModel {
    let Ok(resolved) = library.resolve(&format!("minecraft:block/{name}")) else {
        return Vec::new();
    };
    let Ok(quads) = bake_model(&resolved) else {
        return Vec::new();
    };
    let unit = if quads
        .iter()
        .flat_map(|q| q.positions)
        .any(|p| p.iter().any(|v| *v > 1.5))
    {
        16.0
    } else {
        1.0
    };
    quads
        .into_iter()
        .map(|q| {
            (
                q.positions.map(|p| p.map(|v| v / unit)),
                q.uv,
                q.material
                    .sprite
                    .trim_start_matches("minecraft:")
                    .to_owned(),
            )
        })
        .collect()
}
pub(in crate::game::server_side) fn facing_of(face: [i32; 3]) -> u8 {
    match face {
        [0, -1, 0] => 0,
        [0, 1, 0] => 1,
        [0, 0, -1] => 2,
        [0, 0, 1] => 3,
        [-1, 0, 0] => 4,
        _ => 5,
    }
}
pub(in crate::game) fn sound(f: &ItemFrame, what: &str) -> String {
    let prefix = if f.glow {
        "glow_item_frame"
    } else {
        "item_frame"
    };
    format!("minecraft:entity.{prefix}.{what}")
}
pub(in crate::game) fn center(f: &ItemFrame) -> [f64; 3] {
    let (lo, hi) = f.bounds();
    std::array::from_fn(|i| (lo[i] + hi[i]) / 2.0)
}
/// `HangingEntity.spawnAtLocation`: drops sit 0.15 out from the wall.
pub(in crate::game) fn hanging_drop_position(center: [f64; 3], step: [i32; 3]) -> [f64; 3] {
    [
        center[0] + f64::from(step[0]) * 0.15,
        center[1],
        center[2] + f64::from(step[2]) * 0.15,
    ]
}
/// A frame's item, waiting for its model: the frames' borrow of `self` ends
/// before `resolve_item` reads the pack.
pub(in crate::game) struct FramedItem {
    pub(in crate::game) stack: ItemStack,
    pub(in crate::game) id: u32,
    pub(in crate::game) matrix: glam::Mat4,
    pub(in crate::game) light: u8,
    pub(in crate::game) cull: bool,
    pub(in crate::game) kind: &'static str,
    pub(in crate::game) owner: OwnerState,
}
/// `ItemFrameRenderer.extractRenderState`'s `ItemOwner` (`updateForNonLiving`
/// with the frame entity): `ItemFrame.position` and
/// `ItemFrame.getVisualRotationYInDegrees`.
pub(in crate::game) fn frame_owner(f: &ItemFrame) -> OwnerState {
    super::item_draw::owner_at(center(f), visual_rotation_y_degrees(f), f.kind())
}
/// `ItemFrame.getVisualRotationYInDegrees`: the frame's own yaw, as an item
/// model's owner reads it (a framed compass's needle turns with it).
pub(in crate::game::server_side) fn visual_rotation_y_degrees(f: &ItemFrame) -> f32 {
    // `Direction.get2DDataValue` of a wall frame, and the axis direction's
    // step for the two flat ones.
    let s = f.step();
    let (two_d, correction) = match (s[0], s[1], s[2]) {
        (0, 0, 1) => (0, 0),
        (-1, 0, 0) => (1, 0),
        (0, 0, -1) => (2, 0),
        (1, 0, 0) => (3, 0),
        (0, y, _) => (-1, 90 * y),
        _ => (0, 0),
    };
    sunrise_level::camera_math::wrap_degrees(
        180.0 + (two_d * 90) as f32 + f32::from(f.rotation) * 45.0 + correction as f32,
    )
}
impl Game<'_, crate::game::server_side::ServerState> {
    /// `ItemFrame.survives` (not fixed): no collision, a solid block behind
    /// (or a diode for wall frames), no overlapping hanging entity.
    pub(in crate::game) fn frame_survives(&self, f: &ItemFrame) -> bool {
        if f.fixed {
            return true;
        }
        let (lo, hi) = f.bounds();
        let bounds = Aabb::new(
            Vec3::new(lo[0], lo[1], lo[2]),
            Vec3::new(hi[0], hi[1], hi[2]),
        );
        let mut scene = sunrise_level::collisions::SceneCollisions {
            world: &self.world,
            binding: self.collision,
            subject: None,
        };
        if !scene.unobstructed(bounds).unwrap_or(false) {
            return false;
        }
        let s = f.step();
        let behind = self.res.facts().get(self.world.block(
            f.pos[0] - s[0],
            f.pos[1] - s[1],
            f.pos[2] - s[2],
        ));
        let supported = behind.legacy_solid || (s[1] == 0 && behind.has_class("DiodeBlock"));
        // canCoexist(true): only hanging entities facing the same way block it.
        supported && super::paintings::frame_coexists(f, &self.item_frames, &self.paintings)
    }
    /// Asks the mods whether the acting guest may place a hanging entity
    /// of type `kind` at `pos` (`native_hanging_place`): true when one
    /// cancelled it. A game's own player is not asked.
    pub(in crate::game) fn hanging_place_cancelled(&mut self, kind: &str, pos: [i32; 3]) -> bool {
        let (Some(uuid), Some(name)) =
            (self.side.lan_acting_uuid, self.side.lan_acting_name.clone())
        else {
            return false;
        };
        let mut cancel = false;
        self.native_hanging_place(uuid, &name, kind, pos, &mut cancel);
        cancel
    }
    /// Asks the mods whether a hanging entity of type `kind` at `pos` may
    /// break (`native_hanging_break`): true when one cancelled it. A break by
    /// a player is asked only for the acting guest, with them as the player;
    /// a break by anything else is asked with none.
    pub(in crate::game) fn hanging_break_cancelled(
        &mut self,
        kind: &str,
        pos: [i32; 3],
        by_player: bool,
    ) -> bool {
        let player = if by_player {
            let Some(uuid) = self.side.lan_acting_uuid else {
                return false;
            };
            Some(uuid)
        } else {
            None
        };
        let mut cancel = false;
        self.native_hanging_break(player, kind, pos, &mut cancel);
        cancel
    }
    /// `ItemFrameItem.useOn`.
    pub(in crate::game::server_side) fn place_item_frame(
        &mut self,
        hit: &Hit,
        stack: &ItemStack,
    ) -> bool {
        let glow = match &*stack.item {
            "minecraft:item_frame" => false,
            "minecraft:glow_item_frame" => true,
            _ => return false,
        };
        let pos: [i32; 3] = std::array::from_fn(|i| hit.pos[i] + hit.face[i]);
        // `ItemFrameItem.mayPlace`: `Level.isInsideBuildHeight(blockPos)`, so a frame is not
        // placed on the top face of the highest block or under the lowest one.
        if !(sunrise_level::chunk::MIN_Y
            ..sunrise_level::chunk::MIN_Y + sunrise_level::chunk::HEIGHT)
            .contains(&pos[1])
        {
            // ItemFrameItem.mayPlace's FAIL.
            self.side.use_failed = true;
            return true;
        }
        let mut frame = ItemFrame {
            attachment_lifetime: Default::default(),
            join: Default::default(),
            id: self.side.next_entity + 1,
            pos,
            facing: facing_of(hit.face),
            glow,
            item: None,
            rotation: 0,
            drop_chance: 1.0,
            fixed: false,
            invisible: false,
            check_interval: 0,
            ticks: 0,
            // `applyComponentsFromItemStack` sets `CUSTOM_NAME`.
            custom_name: stack.data().and_then(|d| d.custom_name.clone()),
            random: sunrise_level::random_source::entity(),
        };
        // `EntityType.createDefaultStackConfig`: the item's `custom_data`, then its
        // `entity_data` merged over the frame (`TypedEntityData.loadInto`), before
        // `survives` runs on the configured frame.
        let kind = frame.kind();
        let own =
            sunrise_sim::entity_nbt::item_frame_to_nbt(&frame, [0; 4], None, self.res.items());
        let configured = sunrise_sim::block_entity_item::stack_configured_entity(stack, kind, &own);
        if let Some(mut loaded) =
            sunrise_sim::entity_nbt::item_frame_from_nbt(&configured, frame.id)
        {
            loaded.glow = glow;
            frame = loaded;
        }
        // HangingEntity.survives false: CONSUME, which ItemStack.useOn still counts as a use.
        if !self.frame_survives(&frame) {
            self.award_stat("used", &stack.item, 1);
            return true;
        }
        if self.hanging_place_cancelled(kind, pos) {
            return true;
        }
        self.side.next_entity += 1;
        self.server_play_sound(&sound(&frame, "place"), center(&frame), 1.0, 1.0);
        // HangingEntityItem.useOn: ENTITY_PLACE at the frame, by the player, before it is
        // added; ItemStack.useOn then counts the use.
        let ctx = self.player_context(None);
        self.post_game_event("minecraft:entity_place", center(&frame), ctx);
        self.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
        // The unmodelled rest of the loaded data (`data`, `Tags`...) stays with the frame.
        let mut raw = configured;
        if let sunrise_core::nbt::Tag::Compound(map) = &mut raw {
            map.remove(&"UUID".into());
        }
        self.persist.raw.insert(frame.id, raw);
        self.item_frames.push(frame);
        if !self.creative {
            self.consume_held();
        }
        self.award_stat("used", &stack.item, 1);
        true
    }
    /// `ItemFrame.interact` for the frame under the crosshair, or the one a
    /// guest's interact packet named.
    pub(in crate::game::server_side) fn interact_frame(
        &mut self,
        target: Option<InteractTarget>,
    ) -> bool {
        let Some((i, d)) = (match target {
            Some(t) => Self::named_entity(&self.item_frames, t.id, |f| f.id).map(|i| (i, 0.0)),
            None => self.frame_hit(),
        }) else {
            return false;
        };
        // Only a client's own pick is stopped by a block: the range check passed.
        if (target.is_none() && !self.before_block(d)) || self.item_frames[i].fixed {
            return false;
        }
        let at = center(&self.item_frames[i]);
        let entity_id = self.item_frames[i].id;
        let action;
        if self.item_frames[i].item.is_none() {
            let Some(held) = self.held().cloned() else {
                return false;
            };
            // A map tracking more than 256 counted markers stays in the hand
            // (InteractionResult.FAIL): nothing else is tried, no swing (#11383).
            if let Some(id) = super::maps::map_id(&held)
                && self.map(id).is_some_and(|m| m.tracked_over_limit(256))
            {
                return true;
            }
            self.item_frames[i].set_item(Some(held));
            action = "put";
            self.server_play_sound(&sound(&self.item_frames[i], "add_item"), at, 1.0, 1.0);
        } else {
            let s = sound(&self.item_frames[i], "rotate_item");
            self.server_play_sound(&s, at, 1.0, 1.0);
            let f = &mut self.item_frames[i];
            f.rotation = (f.rotation + 1) % 8;
            action = "rotate";
        }
        let p = self.item_frames[i].pos;
        self.world.modified.insert((p[0] >> 4, p[2] >> 4));
        self.frame_output_changed(p);
        // ItemFrame.interact: BLOCK_CHANGE by the player after setItem or
        // setRotation, then the held item is consumed.
        let ctx = self.who_context(Some(sunrise_sim::mobs::Who::Player), None);
        self.post_game_event("minecraft:block_change", at, ctx);
        if action == "put" && !self.creative {
            self.consume_held();
        }
        if let (Some(player), Some(name)) =
            (self.side.lan_acting_uuid, self.side.lan_acting_name.clone())
        {
            let frame = self.item_frames.iter().find(|f| f.id == entity_id);
            let item = frame
                .and_then(|f| f.item.as_ref())
                .map_or_else(String::new, |s| s.item.to_string());
            let rotation = frame.map_or(0, |f| f.rotation);
            self.native_player_item_frame_change(
                player,
                &name,
                entity_id,
                action,
                &item,
                rotation.rem_euclid(8) as u8,
            );
        }
        self.swing_arm();
        true
    }
    /// `ItemFrame.hurtServer` from the player: the item comes out first.
    pub(in crate::game::server_side) fn attack_frame(&mut self) -> bool {
        let Some((i, d)) = self.frame_hit() else {
            return false;
        };
        if !self.before_block(d) || self.mob_hit().is_some_and(|m| m.1 < d) {
            return false;
        }
        if self.item_frames[i].fixed {
            // A fixed frame refuses the hit of a player who isn't in creative
            // (canHurtWhenFixed), so skipAttackInteraction is false and
            // Player.attack goes on to onAttack.
            if !self.creative {
                self.side.attack_ticks = 0;
            }
            return true;
        }
        // `isInvulnerableToBase`: a refused hit goes on to onAttack, as a fixed frame's does.
        if self.native_player_hit_invulnerable(self.item_frames[i].id, self.item_frames[i].kind()) {
            self.side.attack_ticks = 0;
            return true;
        }
        self.hit_frame_by(
            i,
            true,
            self.creative,
            Some(super::commands::Target::Player),
            false,
        );
        true
    }
    /// `ItemFrame.hurtServer` from a projectile with full source attribution.
    pub(in crate::game::server_side) fn projectile_hurts_frame_by(
        &mut self,
        id: u32,
        by_player: bool,
        creative: bool,
        source: Option<super::commands::Target>,
        damage_type: &str,
    ) -> bool {
        let Some(i) = self.item_frames.iter().position(|f| f.id == id) else {
            return false;
        };
        let bypasses = self
            .health_resources
            .damage
            .id(damage_type)
            .is_some_and(|k| {
                self.health_resources
                    .damage
                    .in_tag(k, "minecraft:bypasses_invulnerability")
            });
        let mob_blocked = !self
            .ui
            .rules
            .bool_at(sunrise_client_core::gamerules::vanilla::MOB_GRIEFING)
            && matches!(source, Some(super::commands::Target::Mob(_)));
        let fixed = self.item_frames[i].fixed;
        if fixed {
            if (!creative && !bypasses) || mob_blocked {
                return false;
            }
        } else if self.item_frames[i].item.is_none() && mob_blocked {
            return false;
        }
        self.hit_frame_by(i, by_player, creative, source, fixed);
        true
    }
    /// Non-fixed part of `ItemFrame.hurtServer`: filled drops item, empty or fixed breaks.
    pub(in crate::game::server_side) fn hit_frame_by(
        &mut self,
        mut i: usize,
        by_player: bool,
        creative: bool,
        source: Option<super::commands::Target>,
        force_break: bool,
    ) {
        let with_frame = force_break || self.item_frames[i].item.is_none();
        let entity_id = self.item_frames[i].id;
        if with_frame {
            let (kind, pos) = (self.item_frames[i].kind(), self.item_frames[i].pos);
            if self.hanging_break_cancelled(kind, pos, by_player) {
                return;
            }
            // The listener had the game in its hands: find the frame again by id.
            match self.item_frames.iter().position(|f| f.id == entity_id) {
                Some(now) if force_break || self.item_frames[now].item.is_none() => i = now,
                _ => return,
            }
        }
        let old_item = self.item_frames[i]
            .item
            .as_ref()
            .map_or("", |s| &*s.item)
            .to_owned();
        let rotation = self.item_frames[i].rotation;
        let at = center(&self.item_frames[i]);
        let mut f = if with_frame {
            self.item_frames.remove(i)
        } else {
            self.item_frames[i].clone()
        };
        if !with_frame {
            self.item_frames[i].item = None;
        } else {
            // BlockAttachedEntity.kill precedes ItemFrame.dropItem's break sound.
            self.entity_killed_event_by(at, source);
            self.server_play_sound(&sound(&f, "break"), at, 1.0, 1.0);
        }
        // ItemFrame.dropItem clears the item (updating comparators) before drops.
        if f.item.is_some() {
            self.frame_output_changed(f.pos);
        }
        self.drop_frame_items(&mut f, with_frame, by_player, creative);
        // Drops run listeners: the random state goes back to the frame with
        // this id, wherever it is now.
        if !with_frame
            && let Some(frame) = self
                .item_frames
                .iter_mut()
                .find(|frame| frame.id == entity_id)
        {
            frame.random = f.random.clone();
        }
        // BLOCK_CHANGE by the attacker after the drops: ItemFrame.hurtServer
        // for the item, ItemFrame.dropItem(level, causedBy) for the frame.
        let ctx = match source {
            Some(super::commands::Target::Player) => {
                self.who_context(Some(sunrise_sim::mobs::Who::Player), None)
            }
            Some(super::commands::Target::Guest(conn)) => {
                let who = self
                    .command_guest(conn)
                    .map(|g| sunrise_sim::mobs::Who::Mob(g.entity_id as u32))
                    .or(Some(sunrise_sim::mobs::Who::Player));
                self.who_context(who, None)
            }
            Some(super::commands::Target::Mob(id)) => {
                self.who_context(Some(sunrise_sim::mobs::Who::Mob(id)), None)
            }
            _ => {
                let who = by_player.then_some(sunrise_sim::mobs::Who::Player);
                self.who_context(who, None)
            }
        };
        self.post_game_event("minecraft:block_change", at, ctx);
        if !with_frame {
            // ItemFrame.hurtServer plays this after the event.
            self.server_play_sound(&sound(&f, "remove_item"), at, 1.0, 1.0);
        }
        self.world.modified.insert((f.pos[0] >> 4, f.pos[2] >> 4));
        if by_player
            && let (Some(player), Some(name)) =
                (self.side.lan_acting_uuid, self.side.lan_acting_name.clone())
        {
            self.native_player_item_frame_change(
                player,
                &name,
                entity_id,
                if with_frame { "break" } else { "remove_item" },
                if with_frame { "" } else { &old_item },
                rotation.rem_euclid(8) as u8,
            );
        }
    }
    /// `ItemFrame.setItem` / `setRotation`: `updateNeighbourForOutputSignal`, so
    /// the comparators reading the frame take its new signal.
    pub(in crate::game) fn frame_output_changed(&mut self, pos: [i32; 3]) {
        let at = BlockPos::new(pos[0], pos[1], pos[2]);
        self.with_level(|b, level, ticks| b.update_neighbour_for_output_signal(level, ticks, at));
    }
    /// `ItemFrame.dropItem`: the frame (when breaking) and the held item.
    pub(in crate::game) fn drop_frame_items(
        &mut self,
        f: &mut ItemFrame,
        with_frame: bool,
        by_player: bool,
        creative: bool,
    ) {
        // The framed map forgets the frame, except when the drops are off and
        // something caused the break (`removeFramedMap` runs only for a null
        // `causedBy` then).
        if self
            .ui
            .rules
            .bool_at(sunrise_client_core::gamerules::vanilla::ENTITY_DROPS)
            || !by_player
        {
            self.remove_framed_map(f);
        }
        if !self
            .ui
            .rules
            .bool_at(sunrise_client_core::gamerules::vanilla::ENTITY_DROPS)
            || (by_player && creative)
        {
            return;
        }
        let c = center(f);
        let at = hanging_drop_position(c, f.step());
        let mut drops = Vec::new();
        if with_frame {
            // `ItemFrame.getFrameItemStackWithData` (glow frames share it).
            drops.push(f.item_stack_with_data());
        }
        if let Some(item) = f.item.clone()
            && f.random.next_float() < f.drop_chance
        {
            drops.push(item);
        }
        if by_player && !drops.is_empty() {
            let tool = self.held().cloned();
            let player = Some(self.native_acting_id());
            self.native_use_drops(
                "",
                c,
                None,
                Some((f.id, f.kind())),
                player,
                tool.as_ref(),
                &mut drops,
            );
        }
        for stack in drops {
            self.spawn_item(stack, at, [0.0; 3], 10);
        }
    }
    /// `ServerExplosion.hurtEntities` for item frames: a frame within the blast
    /// is killed and dropped with its item (`BlockAttachedEntity.hurtServer`).
    /// A fixed frame is not hurt by an explosion, and `blast_spares_decorations`
    /// keeps every frame without `mobGriefing`.
    pub(in crate::game::server_side) fn explosion_breaks_frames(
        &mut self,
        blast: [f64; 3],
        diameter: f64,
        interaction: super::explosion::BlockInteraction,
    ) {
        if self.blast_spares_decorations(interaction) {
            return;
        }
        let mut i = 0;
        while i < self.item_frames.len() {
            let c = center(&self.item_frames[i]);
            let dist =
                ((c[0] - blast[0]).powi(2) + (c[1] - blast[1]).powi(2) + (c[2] - blast[2]).powi(2))
                    .sqrt()
                    / diameter;
            if dist > 1.0
                || self.item_frames[i].fixed
                || !self.side.combat.0.visits(self.item_frames[i].id)
                || !self.break_frame_by(i, self.explosion_kill_credit())
            {
                i += 1;
            }
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod source_tests {
    #[test]
    fn frame_model_reads_resolved_resources_without_files() {
        use sunrise_assets::resource_source::{Resource, ResourceSource};
        let source = ResourceSource::indexed(std::collections::BTreeMap::from([
            ("assets/minecraft/models/block/item_frame.json".into(), Resource::bytes(br##"{"textures":{"x":"example:block/frame"},"elements":[{"from":[0,0,0],"to":[16,16,16],"faces":{"north":{"texture":"#x"}}}]}"##.to_vec())),
        ]));
        let library = sunrise_client::model_library::ParsedModels::load(&source)
            .into_item_library()
            .unwrap();
        let [model, ..] = super::bake_frames(Some(&library));
        assert_eq!(model.len(), 1);
        assert_eq!(model[0].2, "example:block/frame");
    }
}

#[cfg(test)]
pub(in crate::game) mod map_limit_tests {
    use super::super::maps::{Decoration, Key, MapData};
    use super::*;

    fn frame(id: u32) -> ItemFrame {
        ItemFrame {
            attachment_lifetime: Default::default(),
            join: Default::default(),
            id,
            pos: [0, 64, id as i32],
            facing: 3,
            glow: false,
            item: None,
            rotation: 0,
            drop_chance: 1.0,
            fixed: false,
            invisible: false,
            check_interval: 0,
            ticks: 0,
            custom_name: None,
            random: sunrise_core::random::LegacyRandom::new(0),
        }
    }
    /// A map whose `count` frame markers all count toward the limit.
    fn map_tracking(count: i32) -> MapData {
        let mut map = MapData::fresh(0.0, 0.0, 0, "minecraft:overworld");
        map.decorations = (0..count)
            .map(|i| Decoration {
                sprite: "map/decorations/frame",
                x: 0,
                y: 0,
                rot: 0,
                name: None,
                key: Key::Frame(1000 + i),
            })
            .collect();
        map
    }
    fn filled_map(id: i32) -> ItemStack {
        let mut stack = ItemStack::new("minecraft:filled_map", 1);
        stack.data_mut().map_id = Some(id);
        stack
    }
    fn at(id: u32) -> Option<InteractTarget> {
        Some(InteractTarget {
            id: id as i32,
            location: [0.0; 3],
        })
    }

    #[test]
    #[ignore = "daily"]
    fn a_map_tracking_more_than_256_markers_stays_in_the_hand() {
        super::super::native_entity_refs::tests::with_game(|game| {
            game.item_frames.extend([frame(1), frame(2)]);
            game.maps.data.insert(7, map_tracking(257));
            game.maps.data.insert(8, map_tracking(256));

            // ItemFrame.interact: FAIL, the frame stays empty and the map is kept.
            *game.held_mut() = Some(filled_map(7));
            assert!(
                game.interact_frame(at(1)),
                "the refusal ends the interaction"
            );
            assert!(game.item_frames[0].item.is_none());
            assert_eq!(game.held().and_then(|s| s.data()?.map_id), Some(7));

            // At the limit (not over it) the map goes in.
            *game.held_mut() = Some(filled_map(8));
            assert!(game.interact_frame(at(2)));
            let framed = game.item_frames[1].item.as_ref();
            assert_eq!(framed.and_then(|s| s.data()?.map_id), Some(8));
        });
    }
}

#[cfg(test)]
pub(in crate::game) mod game_event_tests {
    use super::super::game_tests::tests::server_game;
    use super::*;
    use sunrise_sim::mobs::Who;

    fn frame(id: u32, pos: [i32; 3], item: Option<ItemStack>) -> ItemFrame {
        ItemFrame {
            attachment_lifetime: Default::default(),
            join: Default::default(),
            id,
            pos,
            facing: 3,
            glow: false,
            item,
            rotation: 0,
            drop_chance: 1.0,
            fixed: false,
            invisible: false,
            check_interval: 0,
            ticks: 0,
            custom_name: None,
            random: sunrise_core::random::LegacyRandom::new(0),
        }
    }
    fn at(id: u32) -> Option<InteractTarget> {
        Some(InteractTarget {
            id: id as i32,
            location: [0.0; 3],
        })
    }
    /// The events posted since [`reset`].
    fn posted() -> Vec<(&'static str, [f64; 3], Option<Who>)> {
        super::super::sculk::TEST_POSTED_EVENTS.with(|e| e.borrow().clone().unwrap_or_default())
    }
    fn reset() {
        super::super::sculk::TEST_POSTED_EVENTS.with(|e| *e.borrow_mut() = Some(Vec::new()));
    }

    /// `ItemFrame.interact`: putting an item in and turning it each post
    /// `GameEvent.BLOCK_CHANGE` from the player, at the frame's position.
    #[test]
    #[ignore = "daily"]
    fn putting_and_rotating_a_frame_item_posts_block_change() {
        let mut resources = None;
        let mut game = server_game(&mut resources);
        game.item_frames.clear();
        game.item_frames.extend(vec![frame(1, [0, 64, 1], None)]);
        let center = [0.5, 64.5, 1.03125];
        *game.held_mut() = Some(ItemStack::new("minecraft:stick", 3));

        let expected = [("minecraft:block_change", center, Some(Who::Player))];
        reset();
        assert!(game.interact_frame(at(1)));
        assert_eq!(posted(), expected);
        assert_eq!(game.held().map(|s| s.count), Some(2), "one goes in");
        assert_eq!(game.item_frames[0].item.as_ref().map(|s| s.count), Some(1));

        reset();
        assert!(game.interact_frame(at(1)));
        assert_eq!(posted(), expected);
        assert_eq!(game.item_frames[0].rotation, 1);
    }

    /// `ItemFrame.hurtServer`: the item coming out posts `BLOCK_CHANGE` from
    /// the player hitting the frame.
    #[test]
    #[ignore = "daily"]
    fn taking_a_frame_item_out_posts_block_change_from_the_player() {
        let mut resources = None;
        let mut game = server_game(&mut resources);
        game.item_frames.clear();
        game.item_frames.extend(vec![frame(
            1,
            [0, 64, 1],
            Some(ItemStack::new("minecraft:stick", 1)),
        )]);
        let center = [0.5, 64.5, 1.03125];
        // A player looking straight at the frame's centre from two blocks away
        // (eye height 1.62 above the feet).
        let feet = Vec3::new(0.5, 64.5 - 1.62, -1.0);
        game.player =
            Some(sunrise_sim::motion::PlayerMotion::new(game.motion, feet).expect("a player"));
        game.yaw = 0.0;
        game.pitch = 0.0;

        let expected = [("minecraft:block_change", center, Some(Who::Player))];
        reset();
        assert!(game.attack_frame(), "the frame is in reach");
        assert!(game.item_frames[0].item.is_none());
        assert_eq!(posted(), expected);

        // The empty frame itself then breaks (`BlockAttachedEntity.hurtServer`
        // then `ItemFrame.dropItem`), with the same event.
        reset();
        assert!(game.attack_frame());
        assert!(game.item_frames.is_empty());
        assert_eq!(
            posted(),
            [
                ("minecraft:entity_die", center, Some(Who::Player)),
                ("minecraft:block_change", center, Some(Who::Player))
            ]
        );
    }

    /// `ItemFrame.dropItem` from `BlockAttachedEntity.tick`, when a frame
    /// loses the block it hangs on: the event has no source.
    #[test]
    #[ignore = "daily"]
    fn a_frame_that_falls_posts_block_change_without_source() {
        let mut resources = None;
        let mut game = server_game(&mut resources);
        let mut f = frame(1, [0, 64, 1], None);
        // BlockAttachedEntity.tick checks every 100 ticks.
        f.check_interval = 99;
        game.item_frames.clear();
        game.item_frames.extend(vec![f]);
        let center = [0.5, 64.5, 1.03125];

        reset();
        game.tick_item_frames();
        assert!(game.item_frames.is_empty(), "nothing holds the frame");
        assert_eq!(posted(), [("minecraft:block_change", center, None)]);
    }
}

impl Game<'_, crate::game::server_side::ServerState> {
    /// `BlockAttachedEntity.kill`, then `ItemFrame.dropItem(level, causedBy)`
    /// for the frame at `index`, the kill credited to `source`: the break
    /// sound, the frame and its item, then `BLOCK_CHANGE`. False when a mod
    /// kept it (`native_hanging_break`): it stays at `index`.
    pub(in crate::game) fn break_frame_by(
        &mut self,
        index: usize,
        source: Option<super::commands::Target>,
    ) -> bool {
        let (id, kind, pos, was_empty) = (
            self.item_frames[index].id,
            self.item_frames[index].kind(),
            self.item_frames[index].pos,
            self.item_frames[index].item.is_none(),
        );
        if self.hanging_break_cancelled(kind, pos, false) {
            return false;
        }
        // Found again by id after the listener: gone, or empty before and
        // filled now, nothing breaks.
        let Some(index) = self
            .item_frames
            .iter()
            .position(|f| f.id == id && !(was_empty && f.item.is_some()))
        else {
            return false;
        };
        let mut f = self.item_frames.remove(index);
        self.entity_killed_event_by(center(&f), source);
        self.server_play_sound(&sound(&f, "break"), center(&f), 1.0, 1.0);
        self.drop_frame_items(&mut f, true, false, false);
        // ItemFrame.dropItem(level, causedBy): BLOCK_CHANGE after the drops.
        let ctx = self.who_context(None, None);
        self.post_game_event("minecraft:block_change", center(&f), ctx);
        self.world.modified.insert((f.pos[0] >> 4, f.pos[2] >> 4));
        if f.item.is_some() {
            self.frame_output_changed(f.pos);
        }
        true
    }
    /// `ItemFrame.setItem(stack, true)`: keeps one item, plays the add sound
    /// for a non-empty stack and updates the comparators.
    pub(in crate::game::server_side) fn set_frame_item(
        &mut self,
        i: usize,
        stack: Option<ItemStack>,
    ) {
        let f = &mut self.item_frames[i];
        f.set_item(stack);
        let (pos, filled) = (f.pos, f.item.is_some());
        if filled {
            let (name, at) = (
                sound(&self.item_frames[i], "add_item"),
                center(&self.item_frames[i]),
            );
            self.server_play_sound(&name, at, 1.0, 1.0);
        }
        self.frame_output_changed(pos);
    }
    /// BlockAttachedEntity.tick for frames.
    pub(in crate::game) fn tick_item_frames(&mut self) {
        // ServerLevel.tick: only entities in entity-ticking chunks tick.
        let ticking = self.entity_ticking_area();
        let mut i = 0;
        while i < self.item_frames.len() {
            let at = center(&self.item_frames[i]);
            if !ticking(at[0].floor() as i32, at[2].floor() as i32) {
                i += 1;
                continue;
            }
            // `Entity.tickCount` is raised before the rest of the tick, and is not saved.
            self.item_frames[i].ticks = self.item_frames[i].ticks.saturating_add(1);
            self.item_frames[i].check_interval += 1;
            if self.item_frames[i].check_interval >= 100 {
                self.item_frames[i].check_interval = 0;
                let f = self.item_frames[i].clone();
                if !self.frame_survives(&f) && !self.hanging_break_cancelled(f.kind(), f.pos, false)
                {
                    // Found again by id after the listener: gone, nothing is
                    // removed; empty before and filled now, it stays.
                    let Some(now) = self.item_frames.iter().position(|g| g.id == f.id) else {
                        continue;
                    };
                    if f.item.is_none() && self.item_frames[now].item.is_some() {
                        i = now + 1;
                        continue;
                    }
                    i = now;
                    let mut f = self.item_frames.remove(i);
                    // BlockAttachedEntity.tick discards without a death event.
                    self.server_play_sound(&sound(&f, "break"), center(&f), 1.0, 1.0);
                    if f.item.is_some() {
                        self.frame_output_changed(f.pos);
                    }
                    self.drop_frame_items(&mut f, true, false, false);
                    // ItemFrame.dropItem(level, null): sound, drops, then event.
                    let ctx = self.who_context(None, None);
                    self.post_game_event("minecraft:block_change", center(&f), ctx);
                    continue;
                }
            }
            i += 1;
        }
    }
}

#[cfg(test)]
#[path = "tests/lodestone_compass_tests.rs"]
pub(in crate::game) mod lodestone_compass_tests;
