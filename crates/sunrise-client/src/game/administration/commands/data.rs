//! `/data` (DataCommands), `/tag` (TagCommand) and `/summon` with NBT: entity,
//! block entity and storage data accessors over the game state.
use super::{Arg, Context, Game, R, Target, err};
use sunrise_core::nbt::{Compound, JavaString, Tag, parse_snbt};
use sunrise_game_data::{nbt_path::NbtPath, snbt, text::Text};
use sunrise_sim::{
    data_cmd::{self, Op, path_error},
    entity_nbt,
    save::{self},
    scoreboard_cmd::{Holder, Tracker, format_list},
};

/// A `DataAccessor`.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::game) enum DataRef {
    Entity(Target),
    Block([i32; 3]),
    Storage(String),
}

pub(in crate::game) fn empty() -> Tag {
    Tag::Compound(Compound::new())
}
pub(in crate::game) fn set_key(tag: &mut Tag, key: &str, value: Tag) {
    if let Tag::Compound(c) = tag {
        c.insert(JavaString::from(key), value);
    }
}
/// `SummonCommand.spawnEntity`'s name for a kind no command target stands for:
/// its entity type's. An item is the exception: `EntityType`'s factory leaves
/// `ItemStack.EMPTY`, which `ItemEntity.readAdditionalSaveData` discards, and
/// `ItemEntity.getName` reads that empty stack's `getItemName` — an empty
/// component, not the type's name.
pub(in crate::game) fn summon_type_name(kind: &str) -> Text {
    if kind.trim_start_matches("minecraft:") == "item" {
        return Text::empty();
    }
    Text::translate(&super::entity_name_key(kind), &[])
}
/// `EntityType.loadEntityRecursive` snaps every entity of the stack,
/// passengers included, to the position (`/summon`, `summon`).
pub(in crate::game) fn place_summoned(tag: &mut Tag, pos: [f64; 3]) {
    let p = pos.map(|v| Tag::Double(v.to_bits()));
    set_key(tag, "Pos", Tag::List(p.to_vec()));
    // Entity.load: an absent Fire reads as 0 (until the entity moves).
    if tag.get("Fire").is_none() {
        set_key(tag, "Fire", Tag::Short(0));
    }
    if tag.get("Rotation").is_none() {
        set_key(
            tag,
            "Rotation",
            Tag::List(vec![
                Tag::Float(0.0f32.to_bits()),
                Tag::Float(0.0f32.to_bits()),
            ]),
        );
    }
    if let Tag::Compound(c) = tag
        && let Some(Tag::List(riders)) = c.get_mut(&"Passengers".into())
    {
        for r in riders {
            if matches!(r, Tag::Compound(_)) {
                place_summoned(r, pos);
            }
        }
    }
}

impl Game<'_, crate::game::server_side::ServerState> {
    // ---- entity data ---------------------------------------------------

    /// `NbtPredicate.getEntityTagToCompare`: the entity as saved. An entity
    /// in another level is read in the level that holds it.
    pub(in crate::game) fn command_target_nbt(&mut self, t: Target) -> Option<Tag> {
        self.in_target_level(t, |g| g.entity_nbt_in_level(t))
            .flatten()
    }
    /// [`Self::entity_nbt`] of the player whose turn it is.
    pub(in crate::game) fn player_nbt(&mut self) -> Option<Tag> {
        self.player.as_ref()?;
        self.sync_level();
        let mut tag = self.level.player_tag(self.res.items());
        if let Some(u) = self.saved_player_uuid() {
            set_key(&mut tag, "UUID", Tag::IntArray(u.to_vec()));
        }
        // NbtPredicate.getEntityTagToCompare: a player's held stack.
        let selected = self.level.selected_slot;
        let held = match tag.get("Inventory") {
            Some(Tag::List(items)) => items
                .iter()
                .find(|i| matches!(i.get("Slot"), Some(Tag::Byte(b)) if i32::from(*b) == selected))
                .cloned(),
            _ => None,
        };
        if let Some(mut stack) = held {
            if let Tag::Compound(c) = &mut stack {
                c.remove(&"Slot".into());
            }
            set_key(&mut tag, "SelectedItem", stack);
        }
        Some(tag)
    }
    pub(in crate::game) fn entity_nbt_in_level(&mut self, t: Target) -> Option<Tag> {
        self.entity_tag_in_level(t, true)
    }

    /// `Entity.load` with `AbstractArrow.readAdditionalSaveData` on the resident arrow or
    /// spectral arrow at `index` of the projectile list: the loader's decoding
    /// (`arrow_from_nbt`), over the entity it already is. Its type, id, join,
    /// random, UUID and tick count stay; so does what no save holds (its pierced
    /// entities, `inGroundTime`, fluid state). None when the data has no position.
    pub(in crate::game::server_side) fn load_resident_arrow(
        &mut self,
        index: usize,
        spectral: bool,
        tag: &Tag,
    ) -> Option<()> {
        // `Entity.load` cannot change the type, and `/data`'s compound has no `id`.
        let mut tag = tag.clone();
        let kind = if spectral {
            "minecraft:spectral_arrow"
        } else {
            "minecraft:arrow"
        };
        set_key(&mut tag, "id", Tag::String(kind.into()));
        let (mut new, in_block) = entity_nbt::arrow_from_nbt(
            &tag,
            self.saved_player_uuid(),
            crate::game::dispenser::tipped_effect,
        )?;
        // Projectile.readAdditionalSaveData: `setOwner(EntityReference.read(..))`.
        self.claim_owner(&mut new);
        let last_state = in_block.and_then(|t| self.state_from_nbt(&t));
        let read = new.arrow.take()?;
        self.creatures.projectiles.update(index, |p| {
            p.kind = new.kind;
            p.position = new.position;
            // Entity.load: setOldPosAndRot.
            p.previous = new.position;
            p.velocity = new.velocity;
            (p.yaw, p.pitch) = (new.yaw, new.pitch);
            (p.previous_yaw, p.previous_pitch) = (new.yaw, new.pitch);
            p.left_owner = new.left_owner;
            (p.owner, p.guest, p.owner_vehicle) = (new.owner, new.guest, new.owner_vehicle);
            p.owner_uuid = new.owner_uuid;
            p.no_gravity = new.no_gravity;
            p.ground.on_ground = new.ground.on_ground;
            p.air_supply = new.air_supply;
            p.stuck = new.stuck;
            let Some(a) = p.arrow.as_mut() else {
                return;
            };
            a.life = read.life;
            a.last_state = last_state;
            a.shake = read.shake;
            a.in_ground = read.in_ground;
            a.pickup = read.pickup;
            a.pierce = read.pierce;
            a.has_been_shot = read.has_been_shot;
            a.can_break = read.can_break;
            a.spectral_duration = read.spectral_duration;
            a.pickup_item = read.pickup_item;
            a.fired_from_weapon = read.fired_from_weapon;
            a.crossbow_hit_sound = read.crossbow_hit_sound;
            a.fire_ticks = read.fire_ticks;
            a.fall_distance = read.fall_distance;
        });
        Some(())
    }
    pub(in crate::game::server_side) fn raw_mut(&mut self, t: Target) -> Option<&mut Tag> {
        match t {
            Target::Player => Some(self.level.player_extra.get_or_insert_with(empty)),
            Target::Guest(conn) => self
                .side.lan
                .as_mut()?
                .hub
                .guests
                .saved_data_mut(conn)
                .map(|saved| saved.get_or_insert_with(empty)),
            Target::Mob(id)
            | Target::Item(id)
            | Target::Stand(id)
            | Target::Frame(id)
            | Target::Painting(id)
            | Target::Boat(id)
            | Target::Cart(id)
            // `tags_of` reads a primed TNT's compound; without this arm
            // `/tag` wrote nothing and still reported success.
            | Target::Tnt(id)
            | Target::Cushion(id)
            | Target::Orb(id)
            | Target::Projectile(id)
            | Target::Crystal(id)
            | Target::Dragon(id)
            | Target::Other(id)
            | Target::Fangs(id)
            | Target::Cloud(id)
            | Target::Bolt(id) => Some(self.persist.raw.entry(id).or_insert_with(empty)),
            // `tags_of` reads these two from the same map and a falling
            // block from its saved compound; without these arms `/tag`
            // wrote nothing and still reported success.
            Target::Rocket(id) | Target::Trident(id) => {
                Some(self.persist.raw.entry(id).or_insert_with(empty))
            }
            _ => None,
        }
    }
    /// Writes `t`'s `Tags`; false when its kind keeps no saved compound to
    /// write them to.
    pub(in crate::game) fn set_entity_tags(&mut self, t: Target, tags: &[String]) -> bool {
        if let Target::Falling(id) = t {
            let uuid = self.uuid_for(id);
            let Some(i) = self.falling.iter().position(|f| f.id == id) else {
                return false;
            };
            self.falling.update(i, |f| {
                let raw = &mut f.saved.get_or_insert_with(|| Box::new((uuid, empty()))).1;
                set_key(
                    raw,
                    "Tags",
                    Tag::List(
                        tags.iter()
                            .map(|s| Tag::String(s.as_str().into()))
                            .collect(),
                    ),
                );
            });
            return true;
        }
        if let Target::Spawner(id) = t {
            let Some(i) = self.ominous_spawners.iter().position(|s| s.id == id) else {
                return false;
            };
            self.ominous_spawners.update(i, |spawner| {
                let raw = &mut **spawner.raw.get_or_insert_with(|| Box::new(empty()));
                if tags.is_empty() {
                    if let Tag::Compound(map) = raw {
                        map.remove(&"Tags".into());
                    }
                } else {
                    set_key(
                        raw,
                        "Tags",
                        Tag::List(
                            tags.iter()
                                .map(|s| Tag::String(s.as_str().into()))
                                .collect(),
                        ),
                    );
                }
            });
            return true;
        }
        let Some(raw) = self.raw_mut(t) else {
            return false;
        };
        // `Entity.saveWithoutId` writes `Tags` only for a nonempty live set:
        // removing the last tag leaves no key.
        if tags.is_empty() {
            if let Tag::Compound(map) = raw {
                map.remove(&"Tags".into());
            }
            return true;
        }
        let list = tags
            .iter()
            .map(|s| Tag::String(s.as_str().into()))
            .collect();
        set_key(raw, "Tags", Tag::List(list));
        true
    }

    /// `ComparatorBlockEntity.loadAdditional` (`getIntOr("OutputSignal", 0)`)
    /// for a comparator at `p`: None for any other block; else whether the
    /// loaded output differs from the one it had (None inside when the tag
    /// does not decode). The block's own metadata comes from `comparator_data`.
    pub(in crate::game::server_side) fn load_comparator_output(
        &mut self,
        p: [i32; 3],
        tag: &Tag,
    ) -> Option<Option<bool>> {
        let mut input = self.comparator_data(p)?;
        let before = input.get("OutputSignal").cloned();
        if let Some(value) = tag.get("OutputSignal") {
            set_key(&mut input, "OutputSignal", value.clone());
        } else if let Tag::Compound(map) = &mut input {
            map.remove(&"OutputSignal".into());
        }
        let Some(normalized) = sunrise_game_data::block_entity_nbt::canonical(&input) else {
            return Some(None);
        };
        let output = normalized.get("OutputSignal").cloned();
        if let Some(Tag::Int(value)) = output {
            let pos = sunrise_core::position::BlockPos::new(p[0], p[1], p[2]);
            self.block_ticks.comparator_outputs.insert(pos, value);
        }
        Some(Some(output != before))
    }

    /// `ComparatorBlockEntity.saveAdditional`: expose the circuit's live output,
    /// rather than keeping a second copy in the ordinary block entity map.
    pub(in crate::game::server_side) fn comparator_data(&self, p: [i32; 3]) -> Option<Tag> {
        if self.data_block_name(p) != "minecraft:comparator" {
            return None;
        }
        let pos = sunrise_core::position::BlockPos::new(p[0], p[1], p[2]);
        let output = self
            .block_ticks
            .comparator_outputs
            .get(&pos)
            .copied()
            .or_else(|| {
                let saved = self
                    .world
                    .chunks
                    .get(&(p[0] >> 4, p[2] >> 4))?
                    .saved
                    .as_deref()?;
                let tag = saved.block_entities.iter().find(|t| {
                    [t.get("x"), t.get("y"), t.get("z")]
                        .map(|v| save::number(v).unwrap_or(0) as i32)
                        == p
                        && save::text(t.get("id")).as_deref() == Some("minecraft:comparator")
                })?;
                let tag = sunrise_game_data::block_entity_nbt::canonical(tag)?;
                match tag.get("OutputSignal") {
                    Some(Tag::Int(v)) => Some(*v),
                    _ => None,
                }
            })
            .unwrap_or(0);
        Some(Tag::compound([
            ("OutputSignal", Tag::Int(output)),
            ("components", empty()),
            ("id", Tag::String("minecraft:comparator".into())),
            ("x", Tag::Int(p[0])),
            ("y", Tag::Int(p[1])),
            ("z", Tag::Int(p[2])),
        ]))
    }

    // ---- accessors -----------------------------------------------------

    /// The `role`th provider of the command (`entity`, `block` or `storage`).
    pub(in crate::game::server_side) fn data_ref(
        &mut self,
        ctx: &Context,
        role: usize,
    ) -> R<DataRef> {
        let prefix = if role == 0 { "target" } else { "source" };
        self.data_ref_named(ctx, role, prefix)
    }
    /// `PistonMovingBlockEntity.saveAdditional`, with full command metadata.
    pub(in crate::game::server_side) fn moving_piston_data(&self, p: [i32; 3]) -> Option<Tag> {
        let pos = sunrise_core::position::BlockPos::new(p[0], p[1], p[2]);
        let (_, piston) = self
            .block_ticks
            .moving_pistons
            .iter()
            .find(|(at, _)| *at == pos)?;
        Some(self.moving_piston_to_nbt(pos, piston))
    }
    pub(in crate::game) fn data_ref_named(
        &mut self,
        ctx: &Context,
        role: usize,
        prefix: &str,
    ) -> R<DataRef> {
        let kind = ctx
            .literals
            .iter()
            .filter(|l| matches!(l.as_str(), "entity" | "block" | "storage"))
            .nth(role)
            .cloned()
            .unwrap_or_default();
        match kind.as_str() {
            "entity" => {
                let Some(Arg::Entity(sel)) = ctx.get(prefix).cloned() else {
                    return Err(err("argument.entity.notfound.entity", &[]));
                };
                let found = self.select(&sel);
                found
                    .first()
                    .map(|t| DataRef::Entity(*t))
                    .ok_or_else(|| err("argument.entity.notfound.entity", &[]))
            }
            "block" => {
                let p = self.block_pos_arg(ctx, &format!("{prefix}Pos"))?;
                if !self.world.block_entities.contains_key(&(p[0], p[1], p[2]))
                    && self.comparator_data(p).is_none()
                    && self.pot_data(p).is_none()
                    && self.moving_piston_data(p).is_none()
                {
                    return Err(err("commands.data.block.invalid", &[]));
                }
                Ok(DataRef::Block(p))
            }
            _ => match ctx.get(prefix) {
                Some(Arg::Resource(id)) => Ok(DataRef::Storage(id.clone())),
                _ => Err(err("command.failed", &[])),
            },
        }
    }
    pub(in crate::game) fn data_of(&mut self, r: &DataRef) -> R<Tag> {
        match r {
            // EntityDataAccessor.getData: Entity.saveWithoutId.
            DataRef::Entity(t) => self
                .in_target_level(*t, |g| g.entity_tag_in_level(*t, false))
                .flatten()
                .ok_or_else(|| err("argument.entity.notfound.entity", &[])),
            DataRef::Block(p) => self
                .block_entity_nbt(*p)
                .ok_or_else(|| err("commands.data.block.invalid", &[])),
            DataRef::Storage(id) => Ok(self.score.storage.get(id)),
        }
    }
    /// `BlockEntity.saveWithFullMetadata` of the block entity at `p`: no
    /// chunk-only keepPacked, and the lock and custom name the model does
    /// not hold. `None` where there is no block entity.
    pub(in crate::game) fn block_entity_nbt(&self, p: [i32; 3]) -> Option<Tag> {
        // A live moving piston is the level's block entity there (#9617).
        if let Some(tag) = self.moving_piston_data(p) {
            return Some(tag);
        }
        self.full_block_entity_data(p)
    }
    /// `BlockEntity.saveWithFullMetadata` of the stored block entity at `p`,
    /// for [`Self::block_entity_nbt`].
    /// Comparators and decorated pots come from their own views; any other
    /// entry gets what the model keeps outside it: the hopper's
    /// `TransferCooldown` (`HopperBlockEntity.saveAdditional`), the custom
    /// name and the pending loot table.
    pub(in crate::game::server_side) fn full_block_entity_data(&self, p: [i32; 3]) -> Option<Tag> {
        if let Some(tag) = self.comparator_data(p).or_else(|| self.pot_data(p)) {
            return Some(tag);
        }
        let key = (p[0], p[1], p[2]);
        let name = self.data_block_name(p);
        let e = self.world.block_entities.get(&key)?;
        let pending = self
            .world
            .chunks
            .get(&(key.0 >> 4, key.2 >> 4))
            .and_then(|chunk| chunk.saved.as_ref())
            .and_then(|saved| saved.pending_loot.get(&key));
        let items = self.items();
        Some(save::block_entity_encode::data_with(
            key,
            e,
            &name,
            save::block_entity_encode::DataFields {
                lock: self.world.block_lock(key),
                custom_name: self.world.block_names.get(&key),
                pending,
                hopper_cooldown: (name == "minecraft:hopper").then(|| self.hopper_cooldown(key)),
            },
            items,
            |row| self.capture_block_entity_payload(key, &name, row, items),
        ))
    }
    /// `BlockEntity.loadWithComponents` for the block entity at `key` from its full saved
    /// `tag` (id and position included): false when the tag doesn't decode.
    pub(in crate::game::server_side) fn load_block_entity_data(
        &mut self,
        key: (i32, i32, i32),
        tag: &Tag,
    ) -> bool {
        let state = self.world.block(key.0, key.1, key.2);
        let name = &self.res.facts().get(state).name;
        let previous = self.world.block_entities.get(&key);
        let Some((_, mut new)) = save::decode_block_entity_replacing(tag, name, previous) else {
            return false;
        };
        self.take_loot_table(key, tag, &mut new);
        self.load_block_name(key, &new, tag);
        self.world.block_entities.insert(key, new);
        self.world.set_block_lock(key, tag);
        true
    }
    /// What `BlockDataAccessor.setData` tells the world after a load: `setChanged` and
    /// `sendBlockUpdated(3)`, even when the block state stays the same.
    pub(in crate::game::server_side) fn block_entity_updated(&mut self, key: (i32, i32, i32)) {
        self.mark_block_entity_updated(key);
        let state = self.world.block(key.0, key.1, key.2);
        // Redraw what shows the block entity (signs, banners).
        let _ = self.world.set(key.0, key.1, key.2, state);
    }
    pub(in crate::game::server_side) fn data_name(&self, r: &DataRef) -> Text {
        match r {
            DataRef::Entity(t) => self.target_name(*t),
            DataRef::Block(p) => Text::literal(format!("{}, {}, {}", p[0], p[1], p[2])),
            DataRef::Storage(id) => Text::literal(id.clone()),
        }
    }
    pub(in crate::game::server_side) fn modified_text(&self, r: &DataRef) -> Text {
        match r {
            DataRef::Entity(_) => {
                Text::translate("commands.data.entity.modified", &[self.data_name(r)])
            }
            DataRef::Block(p) => Text::translate(
                "commands.data.block.modified",
                &[
                    Text::literal(p[0].to_string()),
                    Text::literal(p[1].to_string()),
                    Text::literal(p[2].to_string()),
                ],
            ),
            DataRef::Storage(id) => Text::translate(
                "commands.data.storage.modified",
                &[Text::literal(id.clone())],
            ),
        }
    }
    pub(in crate::game::server_side) fn query_text(&self, r: &DataRef, data: &Tag) -> Text {
        let pretty = snbt::pretty(data);
        match r {
            DataRef::Entity(_) => {
                Text::translate("commands.data.entity.query", &[self.data_name(r), pretty])
            }
            DataRef::Block(p) => Text::translate(
                "commands.data.block.query",
                &[
                    Text::literal(p[0].to_string()),
                    Text::literal(p[1].to_string()),
                    Text::literal(p[2].to_string()),
                    pretty,
                ],
            ),
            DataRef::Storage(id) => Text::translate(
                "commands.data.storage.query",
                &[Text::literal(id.clone()), pretty],
            ),
        }
    }
    pub(in crate::game::server_side) fn get_text(
        &self,
        r: &DataRef,
        path: &NbtPath,
        scale: f64,
        value: i32,
    ) -> Text {
        let p = Text::literal(path.as_str());
        let (s, v) = (
            data_cmd::scale_text(scale),
            Text::literal(value.to_string()),
        );
        match r {
            DataRef::Entity(_) => {
                Text::translate("commands.data.entity.get", &[p, self.data_name(r), s, v])
            }
            DataRef::Block(b) => Text::translate(
                "commands.data.block.get",
                &[
                    p,
                    Text::literal(b[0].to_string()),
                    Text::literal(b[1].to_string()),
                    Text::literal(b[2].to_string()),
                    s,
                    v,
                ],
            ),
            DataRef::Storage(id) => Text::translate(
                "commands.data.storage.get",
                &[p, Text::literal(id.clone()), s, v],
            ),
        }
    }

    // ---- /data -----------------------------------------------------------

    pub(in crate::game::server_side) fn path_arg(&self, ctx: &Context, name: &str) -> R<NbtPath> {
        let Some(Arg::Str(text)) = ctx.get(name) else {
            return Err(err("command.failed", &[]));
        };
        NbtPath::parse(text).map_err(path_error)
    }
    pub(in crate::game::server_side) fn cmd_data(&mut self, id: &str, ctx: &Context) -> R<i32> {
        let target = self.data_ref(ctx, 0)?;
        // An entity in another level is read and written in the level that
        // holds it.
        if let DataRef::Entity(t) = &target {
            let t = *t;
            let target = target.clone();
            return match self.in_target_level(t, |g| g.cmd_data_in(id, ctx, target)) {
                Some(r) => r,
                None => Err(err("argument.entity.notfound.entity", &[])),
            };
        }
        self.cmd_data_in(id, ctx, target)
    }
    /// `DataCommands.mergeData`: literal values keep the modern argument
    /// parser's decoded Tag contract through execution.
    pub(in crate::game) fn cmd_data_in(
        &mut self,
        id: &str,
        ctx: &Context,
        target: DataRef,
    ) -> R<i32> {
        match id {
            "data.merge" => {
                let Some(Arg::Str(nbt)) = ctx.get("nbt") else {
                    return Ok(0);
                };
                let nbt = sunrise_command::snbt::parse_fully(nbt)?;
                let old = self.data_of(&target)?;
                if sunrise_game_data::nbt_path::too_deep(&nbt, 0) {
                    return Err(err("arguments.nbtpath.too_deep", &[]));
                }
                let result = data_cmd::merged(&old, &nbt);
                if old == result {
                    return Err(err("commands.data.merge.failed", &[]));
                }
                self.set_data_of(&target, result)?;
                let message = self.modified_text(&target);
                self.feedback(message);
                Ok(1)
            }
            "data.get" => {
                let data = self.data_of(&target)?;
                if ctx.get("path").is_none() {
                    let message = self.query_text(&target, &data);
                    self.system_feedback(message);
                    return Ok(1);
                }
                let path = self.path_arg(ctx, "path")?;
                let tag = data_cmd::single_tag(&path, &data)?;
                if let Some(Arg::Double(scale)) = ctx.get("scale") {
                    let value = data_cmd::scaled_value(&tag, &path, *scale)?;
                    let message = self.get_text(&target, &path, *scale, value);
                    self.system_feedback(message);
                    return Ok(value);
                }
                let value = data_cmd::get_value(&tag, &path)?;
                let message = self.query_text(&target, &tag);
                self.system_feedback(message);
                Ok(value)
            }
            "data.remove" => {
                let path = self.path_arg(ctx, "path")?;
                let mut data = self.data_of(&target)?;
                let count = path.remove(&mut data);
                if count == 0 {
                    return Err(err("commands.data.merge.failed", &[]));
                }
                self.set_data_of(&target, data)?;
                let message = self.modified_text(&target);
                self.feedback(message);
                Ok(count as i32)
            }
            "data.modify" => self.cmd_data_modify(ctx, target),
            _ => Err(err("command.failed", &[])),
        }
    }
    /// A message sent without the feedback gamerule check (`sendSuccess(.., false)`).
    pub(in crate::game::server_side) fn system_feedback(&mut self, message: Text) {
        self.feedback(message);
    }
    /// `DataCommands.manipulateData`: literal sources use the same SNBT
    /// decode contract as command admission; stored sources are unchanged.
    pub(in crate::game::server_side) fn cmd_data_modify(
        &mut self,
        ctx: &Context,
        target: DataRef,
    ) -> R<i32> {
        let target_path = self.path_arg(ctx, "targetPath")?;
        let after_modify: Vec<&String> = ctx
            .literals
            .iter()
            .skip_while(|l| l.as_str() != "modify")
            .collect();
        let op = after_modify
            .iter()
            .find_map(|l| match l.as_str() {
                "prepend" => Some(Op::Prepend),
                "append" => Some(Op::Append),
                "set" => Some(Op::Set),
                "merge" => Some(Op::Merge),
                "insert" => Some(Op::Insert(match ctx.get("index") {
                    Some(Arg::Int(i)) => *i,
                    _ => 0,
                })),
                _ => None,
            })
            .unwrap_or(Op::Set);
        let sources: Vec<Tag> = if ctx.has("value") {
            let Some(Arg::Str(text)) = ctx.get("value") else {
                return Ok(0);
            };
            vec![sunrise_command::snbt::parse_fully(text)?]
        } else if ctx.get("provider").is_some() {
            // `LootContextSources`' compute source: the provider's value in the
            // command's loot context, as a one-tag source.
            let compute = self.compute_context(ctx)?;
            let integer = ctx.literals.iter().any(|l| l == "integer");
            let registry = if integer {
                "minecraft:context_int_provider"
            } else {
                "minecraft:context_float_provider"
            };
            let (_, provider) = self.provider_arg(ctx, registry)?;
            if integer {
                // ContextIntProvider.getInt: an ArithmeticException reads as 0.
                let value = self.provider_int(&provider, &compute).unwrap_or(0);
                vec![Tag::Int(value)]
            } else {
                // ContextFloatProvider.getFloat: a failure or a non-finite
                // value reads as 0.
                let value = self.provider_float(&provider, &compute).unwrap_or(0.0);
                let value = if value.is_finite() { value } else { 0.0 };
                vec![Tag::Float(value.to_bits())]
            }
        } else {
            let source = self.data_ref(ctx, 1)?;
            let data = self.data_of(&source)?;
            let tags = if ctx.get("sourcePath").is_some() {
                self.path_arg(ctx, "sourcePath")?
                    .get(&data)
                    .map_err(path_error)?
            } else {
                vec![data]
            };
            if ctx.has("string") {
                let start = match ctx.get("start") {
                    Some(Arg::Int(i)) => Some(*i),
                    _ => None,
                };
                let end = match ctx.get("end") {
                    Some(Arg::Int(i)) => Some(*i),
                    _ => None,
                };
                let mut out = Vec::new();
                for t in &tags {
                    let text = data_cmd::as_text(t)?;
                    let text = match start {
                        Some(s) => data_cmd::substring(&text, s, end)?,
                        None => text,
                    };
                    out.push(Tag::String(text.as_str().into()));
                }
                out
            } else {
                tags
            }
        };
        let mut data = self.data_of(&target)?;
        let changed = data_cmd::modify(op, &mut data, &target_path, &sources)?;
        if changed == 0 {
            // StorageDataAccessor.getData is the stored compound itself: an
            // edit that changed it and still counted nothing (a wildcard set
            // that clears an array, then rejects the value) stays. An entity's
            // or a block's data is a copy.
            if let DataRef::Storage(id) = &target {
                self.score.storage.set_unsaved(id, data);
            }
            return Err(err("commands.data.merge.failed", &[]));
        }
        self.set_data_of(&target, data)?;
        let message = self.modified_text(&target);
        self.feedback(message);
        Ok(changed as i32)
    }

    // ---- /tag ------------------------------------------------------------

    pub(in crate::game::server_side) fn cmd_tag(&mut self, id: &str, ctx: &Context) -> R<i32> {
        let targets = self.entities(ctx, "targets", false)?;
        let name = match ctx.get("name") {
            Some(Arg::Str(n)) => n.clone(),
            _ => String::new(),
        };
        let mut tracker = Tracker::default();
        let holder = |g: &Self, t: Target| Holder {
            name: String::new(),
            text: g.target_name(t),
            display: None,
        };
        // A natural knot is listed but keeps no tags yet: `tag add`/`remove` leave it out.
        let targets = if matches!(id, "tag.add" | "tag.remove") {
            super::without_knots(targets)?
        } else {
            targets
        };
        let mut all: Vec<String> = Vec::new();
        for &t in &targets {
            let mut tags = self.tags_of(t);
            let h = holder(self, t);
            match id {
                "tag.add" => {
                    let added = tags.len() < 1024 && !tags.contains(&name);
                    if added {
                        tags.push(name.clone());
                        self.set_entity_tags(t, &tags);
                    }
                    tracker.track(&h, i32::from(added));
                }
                "tag.remove" => {
                    let before = tags.len();
                    tags.retain(|x| x != &name);
                    let removed = tags.len() != before;
                    if removed {
                        self.set_entity_tags(t, &tags);
                    }
                    tracker.track(&h, i32::from(removed));
                }
                _ => {
                    tracker.track(&h, tags.len() as i32);
                    for tag in tags {
                        if !all.contains(&tag) {
                            all.push(tag);
                        }
                    }
                }
            }
        }
        let num = |n: usize| Text::literal(n.to_string());
        let n = name.clone();
        let (message, total) = match id {
            "tag.add" => tracker.message(
                false,
                Some(err("commands.tag.add.failed", &[])),
                |h, _| {
                    Text::translate(
                        "commands.tag.add.success.single",
                        &[Text::literal(n.clone()), h.text.clone()],
                    )
                },
                |c, _| {
                    Text::translate(
                        "commands.tag.add.success.multiple",
                        &[Text::literal(n.clone()), num(c)],
                    )
                },
            ),
            "tag.remove" => tracker.message(
                false,
                Some(err("commands.tag.remove.failed", &[])),
                |h, _| {
                    Text::translate(
                        "commands.tag.remove.success.single",
                        &[Text::literal(n.clone()), h.text.clone()],
                    )
                },
                |c, _| {
                    Text::translate(
                        "commands.tag.remove.success.multiple",
                        &[Text::literal(n.clone()), num(c)],
                    )
                },
            ),
            _ => {
                let count = all.len();
                let message = if all.is_empty() {
                    tracker
                        .message(
                            false,
                            None,
                            |h, _| {
                                Text::translate(
                                    "commands.tag.list.single.empty",
                                    std::slice::from_ref(&h.text),
                                )
                            },
                            |c, _| Text::translate("commands.tag.list.multiple.empty", &[num(c)]),
                        )?
                        .0
                } else {
                    let list = {
                        let mut sorted = all.clone();
                        sorted.sort();
                        format_list(
                            sorted
                                .into_iter()
                                .map(|s| {
                                    Text::literal(s).with_color(sunrise_game_data::text::GREEN)
                                })
                                .collect(),
                        )
                    };
                    // `TagCommand.listTags` tracks each entity's tag count and
                    // sends with `sendFeedback`'s default element type, NON_ZERO:
                    // the single form names the one entity that has tags.
                    tracker
                        .message(
                            false,
                            None,
                            |h, _| {
                                Text::translate(
                                    "commands.tag.list.single.success",
                                    &[h.text.clone(), num(count), list.clone()],
                                )
                            },
                            |c, _| {
                                Text::translate(
                                    "commands.tag.list.multiple.success",
                                    &[num(c), num(count), list.clone()],
                                )
                            },
                        )?
                        .0
                };
                // `TagCommand.listTags` reports with `sendFeedback(source, false, …)`:
                // the operators are not told, unlike `addTag`/`removeTag` (`true`).
                self.feedback_unbroadcast(message);
                return Ok(count as i32);
            }
        }?;
        self.feedback(message);
        Ok(total)
    }

    // ---- /summon ---------------------------------------------------------

    /// The `summon <entity> <pos> <nbt>` form.
    pub(in crate::game) fn summon_with_nbt(
        &mut self,
        kind: &str,
        pos: [f64; 3],
        nbt: &str,
    ) -> R<Option<Target>> {
        let (mut tag, _) = parse_snbt(nbt).map_err(|e| err("argument.nbt.invalid", &[&e]))?;
        if !matches!(tag, Tag::Compound(_)) {
            return Err(err("argument.nbt.expected.compound", &[]));
        }
        // SummonCommand.createEntity with an NBT argument passes
        // `finalize = false`: the mob is what its compound says, and the
        // `sunrise:finalize` marker worldgen leaves on a structure mob is not
        // the player's to set.
        strip_finalize_tree(&mut tag);
        set_key(&mut tag, "id", Tag::String(kind.into()));
        // Turtle.readAdditionalSaveData chooses its home before SummonCommand's snapTo.
        // Preserve that load-stage default while the shared loader receives the final Pos.
        if kind == "minecraft:turtle"
            && !matches!(tag.get("home_pos"), Some(Tag::IntArray(home)) if home.len() == 3)
        {
            let home = entity_nbt::position(&tag).map_or([0; 3], |p| {
                [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32]
            });
            set_key(&mut tag, "home_pos", Tag::IntArray(home.to_vec()));
        }
        if self.difficulty_id() == 0 {
            self.prune_peaceful_summon_passengers(&mut tag);
        }
        place_summoned(&mut tag, pos);
        // ServerLevel.tryAddFreshEntityWithPassengers: a UUID of the tree that
        // an entity (including a player) in the destination level holds refuses
        // the whole tree before any of it joins.
        if self
            .summon_tree_uuids(&tag)
            .into_iter()
            .any(|u| self.is_loaded_uuid_in_level(u))
        {
            return Err(err("commands.summon.failed.uuid", &[]));
        }
        let from = self.mobs.len();
        let canceled = self.persist.joins_canceled;
        let target = self.spawn_entity_from_nbt(tag);
        self.native_mobs_joined(from, false);
        // A mod kept the entity out: tryAddFreshEntityWithPassengers is false.
        if self.persist.joins_canceled != canceled
            && target.is_none_or(|t| self.target_position(t).is_none())
        {
            return Err(err("commands.summon.failed.uuid", &[]));
        }
        Ok(target)
    }

    /// Checked `EntityType.loadEntityRecursive` drops an ineligible passenger
    /// and its entire subtree before UUID admission. Filter only the summon
    /// input; the shared loader must still restore those entities from saves.
    pub(in crate::game::server_side) fn prune_peaceful_summon_passengers(&self, tag: &mut Tag) {
        let Tag::Compound(compound) = tag else { return };
        if let Some(Tag::List(riders)) = compound.get_mut(&"Passengers".into()) {
            riders.retain_mut(|rider| {
                if let Some(kind) = entity_nbt::kind(rider)
                    && self
                        .res
                        .mob_types()
                        .get(&kind)
                        .is_some_and(|t| !t.allowed_in_peaceful())
                {
                    return false;
                }
                self.prune_peaceful_summon_passengers(rider);
                true
            });
        }
    }
}

/// `sunrise:finalize` off `tag` and off the riders under its `Passengers`:
/// only worldgen leaves it, for a structure mob the game finishes when its
/// chunk loads (`Game::load_mob`), never for a player to ask for. Taken off the
/// compounds a player's NBT arrives in: `/summon`'s argument, `/data`'s and the
/// `minecraft:entity_data` component's (`Game::apply_entity_nbt`), and
/// `native_summon`, which takes `/summon`'s argument.
pub(in crate::game) fn strip_finalize_tree(tag: &mut Tag) {
    let Tag::Compound(c) = tag else { return };
    c.remove(&"sunrise:finalize".into());
    if let Some(Tag::List(passengers)) = c.get_mut(&"Passengers".into()) {
        for passenger in passengers {
            strip_finalize_tree(passenger);
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    use crate::game::native_entity_refs::tests::with_game;

    /// `lan_guest_joined`'s placement-only pass, between a forced write and
    /// the tracker pass, must not spend the marks in either dimension.
    #[test]
    #[ignore = "daily"]
    fn guest_placement_keeps_forced_metadata_in_each_dimension() {
        use sunrise_sim::lan::entity::{EntityState, States};
        with_game(|game| {
            for elsewhere in [false, true] {
                game.persist.force_meta(12, 8);
                let mut parked = elsewhere.then(|| std::mem::take(&mut game.persist));
                let mut placement = States::default();
                placement.push(EntityState {
                    id: 12,
                    ..Default::default()
                });
                game.lan_entity_uuids(&mut placement);
                if let Some(ref mut saved) = parked {
                    std::mem::swap(&mut game.persist, saved);
                }
                assert_eq!(game.persist.forced_meta.get(&12), Some(&[1 << 8, 0, 0, 0]));
                let mut snapshot = States::default();
                snapshot.push(EntityState {
                    id: 12,
                    ..Default::default()
                });
                game.lan_entity_uuids(&mut snapshot);
                assert_eq!(snapshot.as_slice()[0].forced_meta, [1 << 8, 0, 0, 0]);
                game.spend_forced_meta();
                assert!(game.persist.forced_meta.is_empty());
            }
        });
    }
}

#[cfg(test)]
#[path = "data/orb_uuid_tests.rs"]
pub(in crate::game) mod orb_uuid_tests;

#[cfg(test)]
#[path = "data/eye_item_tests.rs"]
pub(in crate::game) mod eye_item_tests;

#[cfg(test)]
pub(in crate::game) mod wolf_live_anger_tests {
    use super::Target;
    use crate::game::{Options, Progress, Settings, load_resources};
    use sunrise_core::nbt::Tag;

    #[test]
    #[ignore = "daily"]
    fn live_wolf_anger_writes_use_the_level_clock_and_keep_bee_state() {
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
            crate::game::profile_resolver,
        )
        .unwrap();
        game.build_registries();
        game.world.game_time = 12_345;
        // Bee's independently modeled end is the first control, before the Wolf regression.
        for (kind, health, number) in [("bee", 10.0_f32, 1), ("wolf", 8.0, 2)] {
            game.mobs.clear();
            let tag = Tag::Compound(
                [
                    (
                        "id".into(),
                        Tag::String(format!("minecraft:{kind}").as_str().into()),
                    ),
                    ("UUID".into(), Tag::IntArray(vec![316, 1, 0, number])),
                    (
                        "Pos".into(),
                        Tag::List(vec![
                            Tag::Double(10.5_f64.to_bits()),
                            Tag::Double(200.0_f64.to_bits()),
                            Tag::Double(10.5_f64.to_bits()),
                        ]),
                    ),
                    ("Health".into(), Tag::Float(health.to_bits())),
                    ("NoAI".into(), Tag::Byte(1)),
                    ("NoGravity".into(), Tag::Byte(1)),
                    ("Silent".into(), Tag::Byte(1)),
                    ("PersistenceRequired".into(), Tag::Byte(1)),
                    ("anger_end_time".into(), Tag::Long(-1)),
                ]
                .into_iter()
                .collect(),
            );
            let id = game.load_mob(tag, None).unwrap();
            let i = game.mobs.iter().position(|m| m.id == id).unwrap();
            game.mobs[i].invulnerable = 17;
            game.mobs[i].last_hurt = 2.0;
            game.mobs[i].age = 6;
            game.mobs[i].in_powder_snow = true;
            for end in [12_395, 12_395, 12_420, -1] {
                let mut tag = game.mob_tree_nbt(i);
                let Tag::Compound(fields) = &mut tag else {
                    panic!("mob compound")
                };
                fields.insert("anger_end_time".into(), Tag::Long(end));
                game.apply_entity_nbt(Target::Mob(id), tag).unwrap();
                let saved = game.mob_tree_nbt(i);
                assert_eq!(
                    saved.get("anger_end_time"),
                    Some(&Tag::Long(end)),
                    "{kind} end={end}"
                );
                assert_eq!(game.world.game_time, 12_345);
                assert_eq!(game.persist.uuids.get(&id), Some(&[316, 1, 0, number]));
                assert_eq!(game.mobs[i].health, health);
                assert!(game.mobs[i].no_ai);
                assert_eq!(
                    (
                        game.mobs[i].invulnerable,
                        game.mobs[i].last_hurt,
                        game.mobs[i].age
                    ),
                    (17, 2.0, 6)
                );
                assert!(game.mobs[i].in_powder_snow, "{kind} end={end}");
            }
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod boat_passenger_tree_tests {
    use super::*;
    use crate::game::native_entity_refs::tests::with_game;
    #[test]
    #[ignore = "daily"]
    fn actual_boat_getter_projects_runtime_children_and_existing_bookkeeping() {
        with_game(|game| {
            game.build_registries();
            game.set_allow_commands(true);
            // Supply the synthetic chunk before forceload's synchronous load barrier.
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_level::chunk::Chunk::empty(0, 0)),
            );
            assert!(game.run_command("forceload add 0 0 15 15"));
            assert!(game.world.loaded(8, 8));
            assert!(game.run_command("setblock 8 199 8 stone"));
            assert!(game.run_command("summon oak_boat 8.5 200 8.5 {UUID:[I;7,151,1,1],NoGravity:1b,Silent:1b,CustomName:\"Hull\"}"));
            let bi = game.boats.iter().position(|b| b.position.x == 8.5).unwrap();
            let bid = game.boats[bi].id;
            let child = game
                .spawn_mob(
                    "minecraft:cow",
                    [8.5, 200.0, 8.5],
                    true,
                    crate::game::creatures::SpawnSite::Plain,
                )
                .unwrap();
            let ci = game.mobs.iter().position(|m| m.id == child).unwrap();
            game.mobs[ci].no_ai = true;
            game.mobs[ci].no_gravity = true;
            assert!(
                !game.persist.raw.contains_key(&child),
                "fresh actual runtime child has no raw NBT"
            );
            assert_eq!(
                game.mobs[ci].extra.anger_end, 0,
                "unprimed constructor state"
            );
            game.ride_mount(Target::Mob(child), Target::Boat(bid))
                .unwrap();
            assert_eq!(game.mobs[ci].extra.mount, Some(bid));
            let clock = game.world.game_time;
            let chunks = game.world.chunks.len();
            let raw = game.persist.raw.get(&bid).cloned();
            assert!(raw.as_ref().unwrap().get("Passengers").is_none());
            let before = (
                game.persist.uuids.len(),
                game.persist.uuid_of(child),
                game.mobs[ci].extra.anger_end,
                game.mobs[ci].extra.angry_at,
            );
            let first = game.command_target_nbt(Target::Boat(bid)).unwrap();
            let after = (
                game.persist.uuids.len(),
                game.persist.uuid_of(child),
                game.mobs[ci].extra.anger_end,
                game.mobs[ci].extra.angry_at,
            );
            println!("parent-first bookkeeping before={before:?} after={after:?}");
            let mut failures = Vec::new();
            if after.2 != -1 {
                failures.push("first parent must use existing 0->-1 normalization".to_owned());
            }
            if after.1.is_none() || after.0 != before.0 + usize::from(before.1.is_none()) {
                failures.push("first parent must use existing UUID registration once".into());
            }
            let control = game
                .spawn_mob(
                    "minecraft:cow",
                    [12.5, 200.0, 8.5],
                    true,
                    crate::game::creatures::SpawnSite::Plain,
                )
                .unwrap();
            let di = game.mobs.iter().position(|m| m.id == control).unwrap();
            game.mobs[di].no_ai = true;
            game.mobs[di].no_gravity = true;
            assert!(!game.persist.raw.contains_key(&control));
            assert_eq!(game.mobs[di].extra.anger_end, 0);
            let direct_before = (
                game.persist.uuids.len(),
                game.persist.uuid_of(control),
                game.mobs[di].extra.anger_end,
            );
            let direct_control = game.command_target_nbt(Target::Mob(control)).unwrap();
            let direct_after = (
                game.persist.uuids.len(),
                game.persist.uuid_of(control),
                game.mobs[di].extra.anger_end,
            );
            println!(
                "existing child-getter bookkeeping before={direct_before:?} after={direct_after:?}"
            );
            assert_eq!(direct_after.2, -1);
            assert_eq!(
                direct_after.0,
                direct_before.0 + usize::from(direct_before.1.is_none())
            );
            assert!(direct_after.1.is_some());
            assert_eq!(
                direct_control.get("UUID"),
                Some(&Tag::IntArray(direct_after.1.unwrap().to_vec()))
            );
            let direct_child = game.command_target_nbt(Target::Mob(child)).unwrap();
            let saved_child = |tag: &Tag| match tag.get("Passengers") {
                Some(Tag::List(v)) if v.len() == 1 => Some(v[0].clone()),
                _ => None,
            };
            if saved_child(&first).as_ref() != Some(&direct_child) {
                failures.push("first parent typed child must equal actual child getter".into());
            }
            let stable = game.persist.uuids.len();
            for _ in 0..2 {
                let again = game.command_target_nbt(Target::Boat(bid)).unwrap();
                if saved_child(&again).as_ref() != Some(&direct_child) {
                    failures.push("repeated parent must keep exactly one actual child".into());
                }
                assert_eq!(game.persist.uuids.len(), stable);
                assert_eq!(game.mobs[ci].extra.anger_end, -1);
                assert_eq!(game.mobs[ci].extra.mount, Some(bid));
            }
            assert!(game.run_command("scoreboard objectives add h151 dummy"));
            assert!(game.run_command("scoreboard players set #r h151 91"));
            assert!(game.run_command("scoreboard players set #s h151 17"));
            let accepted=game.run_command("execute store result score #r h151 store success score #s h151 run data get entity @e[nbt={UUID:[I;7,151,1,1]},limit=1] Passengers[0].UUID");
            let score = |name| game.score.board.score(name, "h151").unwrap().value;
            if (accepted, score("#r"), score("#s")) != (true, 4, 1) {
                failures.push("registered data command must reach live tree".into());
            }
            game.boats.update(bi, |boat| {
                boat.leash = Some(sunrise_sim::mobs::leash::Holder::Mob(control))
            });
            let leashed = game.command_target_nbt(Target::Boat(bid)).unwrap();
            assert_eq!(
                leashed.get("leash").and_then(|t| t.get("UUID")),
                direct_control.get("UUID")
            );
            assert_eq!(leashed.get("CustomName"), Some(&Tag::String("Hull".into())));
            assert_eq!(leashed.get("Silent"), Some(&Tag::Byte(1)));
            game.dismount_mob(child);
            assert!(
                game.command_target_nbt(Target::Boat(bid))
                    .unwrap()
                    .get("Passengers")
                    .is_none()
            );
            game.ride_mount(Target::Mob(child), Target::Boat(bid))
                .unwrap();
            let remount_child = game.command_target_nbt(Target::Mob(child)).unwrap();
            if saved_child(&game.command_target_nbt(Target::Boat(bid)).unwrap()).as_ref()
                != Some(&remount_child)
            {
                failures.push("actual remount tree".into());
            }
            assert_eq!(
                game.persist.raw.get(&bid),
                raw.as_ref(),
                "getter must not replay/mutate raw passenger input"
            );
            assert_eq!(game.world.chunks.len(), chunks, "no chunk loading");
            assert_eq!(game.world.game_time, clock, "no scene ticks");
            assert!(
                failures.is_empty(),
                "actual boat passenger getter mismatches: {failures:?}"
            );
        });
    }
}

#[cfg(test)]
pub(in crate::game) mod loot_table_tests {
    use crate::game::{Options, Progress, Settings, load_resources};
    use std::sync::Arc;
    use sunrise_core::nbt::Tag;
    use sunrise_items::items::ItemStack;
    use sunrise_level::{chunk::Chunk, containers::BlockEntity};
    use sunrise_sim::save::{self, ChunkExtras, PendingContainerLoot};

    const TABLE: &str = "minecraft:chests/simple_dungeon";

    /// The chest as a chunk save writes it, and its slots when that save loads.
    fn saved_and_reloaded(
        game: &crate::game::ServerGame<'_>,
        pos: (i32, i32, i32),
    ) -> (Tag, Vec<Option<ItemStack>>) {
        let live = game.world.chunks.get(&(0, 0)).unwrap().clone();
        let chunk = game.with_entities([&live]).remove(0);
        let registry = game.pack.assets.registry;
        let encoded = save::encode(&chunk, registry, game.res.biomes(), &Default::default());
        let Some(Tag::List(list)) = encoded.tag.get("block_entities") else {
            panic!("no block entities");
        };
        let saved = list
            .iter()
            .find(|t| save::number(t.get("x")) == Some(i64::from(pos.0)))
            .unwrap()
            .clone();
        let reloaded = save::decode(&encoded, registry, game.res.biomes()).unwrap();
        let (_, BlockEntity::Chest(slots)) = reloaded
            .block_entities
            .iter()
            .find(|(p, _)| *p == pos)
            .unwrap()
        else {
            panic!("chest");
        };
        (saved, slots.clone())
    }

    /// RandomizableContainer.tryLoadLootTable: a tag without a table loads an
    /// empty chest that stays so, and one with a table keeps it.
    #[test]
    #[ignore = "daily"]
    fn data_commands_on_an_unopened_loaded_chest_survive_a_save_and_reload() {
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
            crate::game::profile_resolver,
        )
        .unwrap();
        game.build_registries();
        game.set_allow_commands(true);

        // Two unopened chests, with one owner for their pending table tags.
        let removed = (1, 64, 2);
        let merged = (2, 64, 2);
        let chest = game
            .pack
            .assets
            .registry
            .default_state("minecraft:chest")
            .unwrap();
        let mut chunk = Chunk::empty(0, 0);
        let mut extras = ChunkExtras::default();
        for pos in [removed, merged] {
            let slots = vec![None; 27];

            chunk.set(pos.0, pos.1, pos.2, chest.0);
            extras.pending_loot.insert(
                pos,
                PendingContainerLoot {
                    table: Tag::String(TABLE.into()),
                    seed: Some(Tag::Long(42)),
                },
            );
            game.world
                .block_entities
                .insert(pos, BlockEntity::Chest(slots));
        }
        chunk.saved = Some(Arc::new(extras));
        game.world.chunks.insert((0, 0), Arc::new(chunk));

        for field in ["LootTable", "LootTableSeed"] {
            assert!(game.run_command(&format!("data get block 2 64 2 {field}")));
        }
        let raw = game.block_entity_nbt([2, 64, 2]).unwrap();
        assert_eq!(raw.get("LootTable"), Some(&Tag::String(TABLE.into())));
        assert_eq!(raw.get("LootTableSeed"), Some(&Tag::Long(42)));
        assert!(
            game.world
                .block_entities
                .get(&merged)
                .unwrap()
                .slots()
                .iter()
                .all(Option::is_none)
        );
        assert!(game.run_command("data remove block 1 64 2 LootTable"));
        let (saved, slots) = saved_and_reloaded(&game, removed);
        assert!(
            saved.get("LootTable").is_none(),
            "the emptied chest rolls again on load: {saved:?}"
        );
        assert!(saved.get("LootTableSeed").is_none());
        assert!(slots.iter().all(Option::is_none));

        assert!(game.run_command("data merge block 2 64 2 {LootTableSeed:7L}"));
        let (saved, _) = saved_and_reloaded(&game, merged);
        assert_eq!(save::text(saved.get("LootTable")).as_deref(), Some(TABLE));
        assert_eq!(save::number(saved.get("LootTableSeed")), Some(7));
        assert!(saved.get("Items").is_none());
    }
}

#[cfg(test)]
#[path = "data/portal_cooldown_tests.rs"]
pub(in crate::game) mod portal_cooldown_tests;

#[cfg(test)]
pub(in crate::game) mod compute_tests;

#[cfg(test)]
pub(in crate::game) mod saved_tag_tests;

#[cfg(test)]
#[path = "data/stand_living_tests.rs"]
pub(in crate::game) mod stand_living_tests;

#[cfg(test)]
pub(in crate::game) mod mod_block_entity_tests;

#[cfg(test)]
#[path = "data/fire_tests.rs"]
pub(in crate::game) mod fire_tests;

#[cfg(test)]
pub(in crate::game) mod residency_tests;

#[cfg(test)]
pub(in crate::game) mod hopper_cooldown_tests;

#[cfg(test)]
#[path = "data/rotation_tests.rs"]
pub(in crate::game) mod rotation_tests;

#[cfg(test)]
pub(in crate::game) mod arrow_reload_tests;

#[cfg(test)]
pub(in crate::game) mod arrow_owner_tests {
    use super::Target;
    use crate::game::{Options, Progress, Settings, load_resources};
    use sunrise_core::nbt::Tag;

    #[test]
    #[ignore = "daily"]
    fn loaded_arrow_getter_uses_registered_owner_reference_without_raw() {
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
            crate::game::profile_resolver,
        )
        .unwrap();
        game.build_registries();

        let position = Tag::List(vec![
            Tag::Double(8.5_f64.to_bits()),
            Tag::Double(202.0_f64.to_bits()),
            Tag::Double(4.5_f64.to_bits()),
        ]);
        let mob_uuid = [213, 1, 2, 3];
        let mob = Tag::compound([
            ("id", Tag::String("minecraft:pig".into())),
            ("UUID", Tag::IntArray(mob_uuid.to_vec())),
            ("Pos", position.clone()),
            ("Health", Tag::Float(10.0_f32.to_bits())),
            ("NoAI", Tag::Byte(1)),
        ]);
        game.load_mob(mob, None).unwrap();
        assert!(game.mob_by_uuid(mob_uuid).is_some());
        let time = game.world.game_time;
        for (index, owner) in [Some(mob_uuid), Some([213, 91, 92, 93]), None]
            .into_iter()
            .enumerate()
        {
            let uuid = [213, 10, 0, index as i32 + 1];
            let mut tag = Tag::compound([
                ("id", Tag::String("minecraft:arrow".into())),
                ("UUID", Tag::IntArray(uuid.to_vec())),
                ("Pos", position.clone()),
                ("NoGravity", Tag::Byte(1)),
                ("CustomName", Tag::String("kept name".into())),
                ("Tags", Tag::List(vec![Tag::String("kept_tag".into())])),
            ]);
            if let (Some(owner), Tag::Compound(fields)) = (owner, &mut tag) {
                fields.insert("Owner".into(), Tag::IntArray(owner.to_vec()));
            }
            game.load_arrow(tag, Some(uuid), None);
            let p = game.creatures.projectiles.last().unwrap();
            // Prove actual loader adoption before reading any saved NBT.
            assert_eq!(p.owner_uuid, owner);
            let id = p.id;
            assert_eq!(game.persist.uuids.get(&id), Some(&uuid));
            let expected = owner.map(|u| Tag::IntArray(u.to_vec()));
            let loaded_raw = game.persist.raw.get(&id).cloned();
            let mut stale = loaded_raw.clone().unwrap();
            if let Tag::Compound(fields) = &mut stale {
                fields.insert("Owner".into(), Tag::IntArray(vec![99; 4]));
            }
            let mut raw_cases = vec![None, loaded_raw];
            // A positive modeled reference must overwrite a conflicting raw UUID.
            // Generic stale optional-key cleanup is outside this getter bridge.
            if owner.is_some() {
                raw_cases.push(Some(stale));
            }
            for raw in raw_cases {
                if let Some(raw) = &raw {
                    game.persist.raw.insert(id, raw.clone());
                } else {
                    game.persist.raw.remove(&id);
                }
                let saved = game.command_target_nbt(Target::Projectile(id)).unwrap();
                assert_eq!(saved.get("Owner"), expected.as_ref());
                assert_eq!(saved.get("UUID"), Some(&Tag::IntArray(uuid.to_vec())));
                if let Some(raw) = &raw {
                    assert_eq!(saved.get("CustomName"), raw.get("CustomName"));
                    assert_eq!(saved.get("Tags"), raw.get("Tags"));
                }
                assert_eq!(game.creatures.projectiles.last().unwrap().owner_uuid, owner);
                assert_eq!(game.persist.raw.get(&id), raw.as_ref());
                assert_eq!(game.world.game_time, time);
            }
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod modern_literal_tests {}

#[cfg(test)]
pub(in crate::game) mod bell_command_identity_tests {
    use super::*;
    use std::sync::Arc;
    use sunrise_core::{geometry::Vec3, position::ChunkPos};
    use sunrise_level::chunk::Chunk;
    use sunrise_level::containers::BlockEntity;
    use sunrise_sim::motion::PlayerMotion;
    fn with_game(test: impl FnOnce(&mut crate::game::ServerGame<'_>)) {
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.build_registries();
            game.set_allow_commands(true);

            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            game.block_ticks
                .add_empty_chunk(ChunkPos::new(0, 0))
                .unwrap();
            game.player = Some(PlayerMotion::new(game.motion, Vec3::new(2.5, 200.0, 2.5)).unwrap());
            game.creative = true;
            assert!(game.run_command("forceload add 0 0"));
            assert!(game.run_command("tick freeze"));
            assert!(game.run_command("fill 0 198 0 15 199 15 stone"));
            test(game);
        });
    }

    #[test]
    #[ignore = "daily"]
    fn placed_bell_is_a_real_stored_entity_with_command_metadata() {
        with_game(|game| {
            let clock = game.world.game_time;
            assert!(game.run_command("setblock 8 200 4 chest[facing=north] strict"));
            assert!(game.world.block_entities.contains_key(&(8, 200, 4)));
            assert!(game.run_command("setblock 10 200 4 stone strict"));
            assert!(game.block_entity_nbt([10, 200, 4]).is_none());
            assert!(game.run_command(
                "setblock 4 200 4 bell[facing=north,attachment=floor,powered=false] strict"
            ));
            assert!(game.run_command(
                "execute if block 4 200 4 bell[facing=north,attachment=floor,powered=false]"
            ));
            assert!(game.world.chunks.contains_key(&(0, 0)));
            let entity = game
                .world
                .block_entities
                .get(&(4, 200, 4))
                .expect("actual placed bell stored in collection");
            assert_eq!(entity.id(), "minecraft:bell");
            assert!(entity.fits("minecraft:bell"));
            assert!(!entity.fits("minecraft:chest"));
            for _ in 0..2 {
                let tag = game.block_entity_nbt([4, 200, 4]).unwrap();
                for (key, value) in [
                    ("id", Tag::String("minecraft:bell".into())),
                    ("x", Tag::Int(4)),
                    ("y", Tag::Int(200)),
                    ("z", Tag::Int(4)),
                ] {
                    assert_eq!(tag.get(key), Some(&value));
                }
                assert!(game.run_command(
                    "execute if data block 4 200 4 {id:\"minecraft:bell\",x:4,y:200,z:4}"
                ));
            }
            // No raw input: factory/model and the real common writer supply metadata.
            let fresh = BlockEntity::for_block("minecraft:bell").unwrap();
            let tag = save::block_entity_encode::data(
                (-3, 204, 11),
                &fresh,
                "minecraft:bell",
                Default::default(),
                &Default::default(),
            );
            assert_eq!(tag.get("x"), Some(&Tag::Int(-3)));
            assert_eq!(tag.get("y"), Some(&Tag::Int(204)));
            assert_eq!(tag.get("z"), Some(&Tag::Int(11)));
            let (pos, back) = save::decode_block_entity(&tag).unwrap();
            assert_eq!(pos, (-3, 204, 11));
            assert_eq!(back.id(), "minecraft:bell");
            assert!(game.run_command(
                "setblock 4 200 4 bell[facing=east,attachment=floor,powered=false] strict"
            ));
            assert_eq!(
                game.world.block_entities.get(&(4, 200, 4)).unwrap().id(),
                "minecraft:bell"
            );
            assert!(game.run_command("setblock 4 200 4 air strict"));
            assert!(!game.world.block_entities.contains_key(&(4, 200, 4)));
            assert!(game.world.block_entities.contains_key(&(8, 200, 4)));
            assert!(game.run_command(
                "setblock 4 200 4 bell[facing=south,attachment=floor,powered=false] strict"
            ));
            assert_eq!(
                game.world.block_entities.get(&(4, 200, 4)).unwrap().id(),
                "minecraft:bell"
            );
            assert_eq!(game.world.game_time, clock);
        });
    }
}

#[cfg(test)]
pub(in crate::game) mod falling_live_adoption_tests {
    use crate::game::{Options, Progress, Settings, load_resources};
    use std::sync::Arc;
    use sunrise_core::position::ChunkPos;
    use sunrise_level::chunk::Chunk;
    pub(in crate::game) fn with_game(test: impl FnOnce(&mut crate::game::ServerGame<'_>)) {
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
        options.game_dir = Some(
            std::path::PathBuf::from(std::env::var_os("SUNRISE_HOME").unwrap())
                .join(format!("h098-{:?}", std::thread::current().id())),
        );
        options.execution_role = crate::game::execution_role::ExecutionRole::IntegratedServer;
        let mut game = crate::game::ServerGame::new(
            &loaded.resources,
            loaded.pack,
            options,
            settings.sim_settings(),
            None,
            crate::game::profile_resolver,
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
        assert!(game.run_command("difficulty normal"));
        if !game.side.streaming.forced.contains(&(0, 0)) {
            assert!(game.run_command("forceload add 0 0"));
        }
        test(&mut game);
    }
}

#[cfg(test)]
pub(in crate::game) mod vehicle_container_tests {
    use super::*;

    #[test]
    #[ignore = "daily"]
    fn loaded_vehicle_saves_own_only_their_container_fields() {
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.build_registries();
            game.set_allow_commands(true);
            let cases = [
                (
                    "boat_plain",
                    "minecraft:oak_boat",
                    "Items:[{Slot:0b,id:\"minecraft:diamond\",count:3,components:{\"minecraft:custom_data\":{h052:9}}}],LootTable:\"minecraft:empty\",LootTableSeed:73L",
                    None,
                    false,
                    "",
                ),
                (
                    "cart_plain",
                    "minecraft:minecart",
                    "Items:[{Slot:0b,id:\"minecraft:diamond\",count:3,components:{\"minecraft:custom_data\":{h052:9}}}],LootTable:\"minecraft:empty\",LootTableSeed:73L",
                    None,
                    false,
                    "",
                ),
                // Only container carts read a loot table: one on any other cart is dropped.
                (
                    "tnt_cart_plain",
                    "minecraft:tnt_minecart",
                    "Items:[{Slot:0b,id:\"minecraft:diamond\",count:3,components:{\"minecraft:custom_data\":{h052:9}}}],LootTable:\"minecraft:empty\",LootTableSeed:73L",
                    None,
                    false,
                    "",
                ),
                (
                    "boat_items",
                    "minecraft:oak_chest_boat",
                    "Items:[{Slot:0b,id:\"minecraft:diamond\",count:3,components:{\"minecraft:custom_data\":{h052:9}}}],LootTableSeed:73L",
                    Some(1usize),
                    false,
                    "",
                ),
                (
                    "cart_items",
                    "minecraft:chest_minecart",
                    "Items:[{Slot:0b,id:\"minecraft:diamond\",count:3,components:{\"minecraft:custom_data\":{h052:9}}}],LootTableSeed:73L",
                    Some(1usize),
                    false,
                    "",
                ),
                (
                    "boat_table",
                    "minecraft:oak_chest_boat",
                    "LootTable:\"minecraft:empty\",LootTableSeed:73L",
                    None,
                    true,
                    "minecraft:empty",
                ),
                (
                    "cart_table",
                    "minecraft:chest_minecart",
                    "LootTable:\"minecraft:empty\",LootTableSeed:73L",
                    None,
                    true,
                    "minecraft:empty",
                ),
                (
                    "boat_empty_table",
                    "minecraft:oak_chest_boat",
                    "LootTable:\"\",LootTableSeed:73L",
                    None,
                    true,
                    "minecraft:",
                ),
                (
                    "cart_empty_table",
                    "minecraft:chest_minecart",
                    "LootTable:\"\",LootTableSeed:73L",
                    None,
                    true,
                    "minecraft:",
                ),
                (
                    "boat_empty",
                    "minecraft:oak_chest_boat",
                    "",
                    Some(0usize),
                    false,
                    "",
                ),
                (
                    "cart_empty",
                    "minecraft:chest_minecart",
                    "",
                    Some(0usize),
                    false,
                    "",
                ),
            ];
            let clock = game.world.game_time;
            let mut differences = Vec::new();
            for (i, (name, kind, fields, expected_items, pending, table_text)) in
                cases.iter().enumerate()
            {
                let uuid = [52, 0, 0, i as i32 + 1];
                let source = [
                    r#"{Pos:[4.5d,200d,4.5d],NoGravity:1b,Tags:["h052"]"#,
                    if fields.is_empty() { "" } else { "," },
                    fields,
                    "}",
                ]
                .concat();
                let (tag, _) = parse_snbt(&source).unwrap();
                let target = if kind.contains("boat") {
                    game.load_boat(tag, Some(uuid), kind);
                    let b = game.boats.last().unwrap();
                    assert_eq!(
                        b.items
                            .first()
                            .and_then(Option::as_ref)
                            .map_or(0, |s| s.count),
                        if *expected_items == Some(1) { 3 } else { 0 },
                        "{name}: actual boat inventory"
                    );
                    Target::Boat(b.id)
                } else {
                    game.load_cart(tag, Some(uuid), kind);
                    let c = game.carts.last().unwrap();
                    assert_eq!(
                        c.items
                            .first()
                            .and_then(Option::as_ref)
                            .map_or(0, |s| s.count),
                        if *expected_items == Some(1) { 3 } else { 0 },
                        "{name}: actual cart inventory"
                    );
                    Target::Cart(c.id)
                };
                let id = match target {
                    Target::Boat(id) | Target::Cart(id) => id,
                    _ => unreachable!(),
                };
                let saved = game.command_target_nbt(target).unwrap();
                assert_eq!(saved.get("UUID"), Some(&Tag::IntArray(uuid.to_vec())));
                assert_eq!(
                    saved.get("Tags"),
                    Some(&Tag::List(vec![Tag::String("h052".into())]))
                );
                let actual_items = match saved.get("Items") {
                    Some(Tag::List(items)) => Some(items.len()),
                    None => None,
                    other => panic!("{name}: {other:?}"),
                };
                if actual_items != *expected_items {
                    differences.push(format!(
                        "{name}: Items {actual_items:?} != {expected_items:?}"
                    ));
                }
                let table = pending.then(|| Tag::String((*table_text).into()));
                let seed = pending.then_some(Tag::Long(73));
                for (key, expected) in [
                    ("LootTable", table.as_ref()),
                    ("LootTableSeed", seed.as_ref()),
                ] {
                    if saved.get(key) != expected {
                        differences.push(format!(
                            "{name}: {key} {:?} != {expected:?}",
                            saved.get(key)
                        ));
                    }
                }
                if *expected_items == Some(1) {
                    // The model, not a stale loaded Items list, owns the inventory.
                    match target {
                        Target::Boat(_) => {
                            game.boats.update(game.boats.len() - 1, |boat| {
                                boat.items[0].as_mut().unwrap().count = 4
                            });
                        }
                        Target::Cart(_) => {
                            game.carts.update(game.carts.len() - 1, |cart| {
                                cart.items[0].as_mut().unwrap().count = 4
                            });
                        }
                        _ => unreachable!(),
                    }
                }
                // A normal model can save without any raw input. Boat pending tables
                // already live in the retained source; no replacement pending state is invented.
                if !pending {
                    game.persist.raw.remove(&id);
                    let saved = game.command_target_nbt(target).unwrap();
                    let expected = if *expected_items == Some(1) { 4 } else { 0 };
                    let count = match saved.get("Items") {
                        Some(Tag::List(items)) => {
                            items.first().and_then(|t| t.get("count")).cloned()
                        }
                        _ => None,
                    };
                    assert_eq!(
                        count,
                        (expected != 0).then_some(Tag::Int(expected)),
                        "{name}: no-raw model count"
                    );
                    assert!(
                        saved.get("LootTable").is_none() && saved.get("LootTableSeed").is_none()
                    );
                    assert_eq!(saved.get("UUID"), Some(&Tag::IntArray(uuid.to_vec())));
                }
            }
            // Retained loader diagnostic, not a writer parity assertion: an input
            // containing both a table and Items currently keeps modeled contents.
            for kind in ["minecraft:oak_chest_boat", "minecraft:chest_minecart"] {
                let source = r#"{Pos:[4.5d,200d,4.5d],LootTable:"minecraft:empty",LootTableSeed:73L,Items:[{Slot:0b,id:"minecraft:diamond",count:3}]}"#;
                let (tag, _) = parse_snbt(source).unwrap();
                let count = if kind.contains("boat") {
                    game.load_boat(tag, Some([52, 1, 0, 1]), kind);
                    game.boats.last().unwrap().items[0]
                        .as_ref()
                        .map_or(0, |s| s.count)
                } else {
                    game.load_cart(tag, Some([52, 1, 0, 2]), kind);
                    game.carts.last().unwrap().items[0]
                        .as_ref()
                        .map_or(0, |s| s.count)
                };
                eprintln!(
                    "separate loader diagnostic {kind}: supplied table+Items retains modeled count {count}"
                );
            }
            assert_eq!(game.world.game_time, clock);
            assert!(
                differences.is_empty(),
                "class-owned save differences:\n{}",
                differences.join("\n")
            );
        });
    }
}

#[cfg(test)]
#[path = "finalize_marker_tests.rs"]
pub(in crate::game) mod finalize_marker_tests;
