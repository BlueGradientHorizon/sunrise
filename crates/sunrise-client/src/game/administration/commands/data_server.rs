use super::{Game, R, Target, err};
pub(in crate::game::server_side) use crate::game::commands::data::DataRef;
use crate::game::commands::data::set_key;
use crate::game::commands::data::strip_finalize_tree;
pub(in crate::game::server_side) use crate::game::commands::data::summon_type_name;
#[allow(unused_imports)]
use sunrise_client::{
    containers::BlockEntity,
    entity_nbt,
    save::{self, random_uuid},
    text::Text,
};
use sunrise_core::nbt::{Compound, Tag};

impl Game<'_, crate::game::server_side::ServerState> {
    /// `Entity.load` of new data (`EntityDataAccessor.setData`): the entity is
    /// rebuilt from its compound, keeping its id and UUID. Also what
    /// `TypedEntityData.loadInto` is: a spawn egg's data on the mob it spawned.
    pub(in crate::game) fn apply_entity_nbt(&mut self, t: Target, mut tag: Tag) -> R<()> {
        let stale = || err("commands.data.merge.failed", &[]);
        // `Entity.load` keeps only the fields it reads, so vanilla drops a key it
        // does not know on `EntityDataAccessor.setData` (and on
        // `TypedEntityData.loadInto`). `sunrise:finalize` is worldgen's, and
        // `Game::load_mob` would run the finalize it asks for on the next load.
        strip_finalize_tree(&mut tag);
        // `Entity.load`'s `setPos` moves the entity in its section storage like any other write.
        let before = self.target_position(t);
        // Entity.load does not read Passengers: the riders stay, and so do the
        // passengers the load kept raw (none, when it kept none).
        let kept_passengers = match t {
            Target::Mob(id) | Target::Boat(id) | Target::Cart(id) | Target::Tnt(id) => Some(
                self.persist
                    .raw
                    .get(&id)
                    .and_then(|raw| raw.get("Passengers"))
                    .cloned(),
            ),
            _ => None,
        };
        match t {
            // EntityDataAccessor.setData: players can't be modified, guests neither.
            Target::Player | Target::Guest(_) => {
                return Err(err("commands.data.entity.invalid", &[]));
            }
            Target::Mob(id) => {
                Self::normalize_loaded_mob_tags(&mut tag);
                let uuid = self.uuid_for(id);
                let kind = self
                    .mobs
                    .iter()
                    .find(|m| m.id == id)
                    .map(|m| m.kind.id.clone());
                set_key(&mut tag, "UUID", Tag::IntArray(uuid.to_vec()));
                if let Some(k) = kind {
                    set_key(&mut tag, "id", Tag::String(k.as_str().into()));
                }
                let mut new = entity_nbt::mob_from_nbt(
                    &tag,
                    self.res.mob_types(),
                    id,
                    Some(self.pack.assets.registry),
                )
                .ok_or_else(stale)?;
                sunrise_client::mobs::armor::prepare(&new, &self.motion.attributes);
                self.resolve_loaded_profession(&mut new);
                let unresolved = self
                    .stations()
                    .professions
                    .state_index(&new.extra.profession)
                    .is_none();
                if let Some(slot) = self.mobs.iter_mut().find(|m| m.id == id) {
                    // LivingEntity.readAdditionalSaveData only replaces a decoded
                    // Brain. A missing or unreadable memory map is not an empty map.
                    let unreadable_brain = !matches!(
                        tag.get("Brain").and_then(|brain| brain.get("memories")),
                        Some(Tag::Compound(_))
                    );
                    let keep_liked_player =
                        unreadable_brain && slot.species() == sunrise_client::mobs::Species::Allay;
                    // Runtime identity and lifetime stay on the resident owner.
                    new.join = slot.join;
                    new.random = slot.random.clone();
                    let old = sunrise_client::join_seq::store_back(slot, new);
                    // Entity.load does not replace the live passenger graph or
                    // the runtime seating state, even for an off-level vehicle.
                    slot.extra.mount = old.extra.mount;
                    slot.extra.rider = old.extra.rider;
                    slot.extra.passenger = old.extra.passenger;
                    slot.extra.vehicle_order = old.extra.vehicle_order;
                    slot.extra.seat = old.extra.seat;
                    slot.extra.vehicle_body = old.extra.vehicle_body;
                    slot.extra.boarding_cooldown = old.extra.boarding_cooldown;
                    slot.rider = old.rider;
                    // Runtime-only state (AI, animation) carries over.
                    // Reset saved armadillo sensors from the resident random.
                    if slot.species() == sunrise_client::mobs::Species::Armadillo
                        && tag.get("Brain").is_some()
                    {
                        sunrise_client::mobs::armadillo::delay_sensors(slot);
                    }
                    // Entity.load keeps the unsaved Allay.jukeboxPos and
                    // DATA_DANCING of the entity it loads into.
                    if slot.species() == sunrise_client::mobs::Species::Allay {
                        slot.extra.flag = old.extra.flag;
                        if let (Some(st), Some(was)) =
                            (slot.extra.allay.as_mut(), old.extra.allay.as_ref())
                        {
                            st.jukebox = was.jukebox;
                        }
                    }
                    if keep_liked_player {
                        slot.extra.tame = old.extra.tame;
                        slot.extra.owner = old.extra.owner;
                        if let (Some(st), Some(was)) =
                            (slot.extra.allay.as_mut(), old.extra.allay.as_ref())
                        {
                            st.liked_player_ttl = was.liked_player_ttl;
                        }
                    }
                    // LivingEntity's damage cooldown and previous-hit amount
                    // are runtime state. Entity's separate immunity window is
                    // loaded from the saved `invulnerable_time` key above.
                    slot.invulnerable = old.invulnerable;
                    slot.last_hurt = old.last_hurt;
                    // Entity.isInPowderSnow isn't saved data either: a skeleton in
                    // powder snow stays afflicted through a merge.
                    slot.in_powder_snow = old.in_powder_snow;
                    // Entity.load updates saved data on the existing entity;
                    // lastTradedPlayer is transient and survives a live NoAI edit.
                    if slot.species() == sunrise_client::mobs::Species::Villager {
                        sunrise_client::mobs::villager::ext_mut(slot).last_traded_player =
                            sunrise_client::mobs::villager::ext(&old)
                                .and_then(|ext| ext.last_traded_player);
                    }
                    if slot.species() == sunrise_client::mobs::Species::Shulker {
                        sunrise_client::mobs::shulker::keep_eased_peek(slot, &old);
                    }
                    if unreadable_brain
                        && let (Some(st), Some(was)) =
                            (slot.extra.warden.as_mut(), old.extra.warden)
                    {
                        st.brain = was.brain;
                    }
                    slot.ai = old.ai;
                    // Mob.readAdditionalSaveData updates persistent home fields,
                    // while retaining the rest of the live AI/runtime state.
                    entity_nbt::read_mob_home(slot, &tag);
                    slot.extra.armor_attributes = old.extra.armor_attributes;
                    // LivingEntity.lastEquipmentItems is runtime state Entity.load
                    // keeps: the next tick swaps the archetype modifiers.
                    slot.extra.adopted_body = old.extra.adopted_body;
                    slot.age = old.age;
                    slot.previous = slot.position;
                    // Wolf.readAdditionalSaveData initializes persistent anger at
                    // the live level clock, just as load_mob does on admission.
                    if slot.species() == sunrise_client::mobs::Species::Wolf {
                        sunrise_client::mobs::adopt_anger_end(slot, self.world.game_time);
                    } else {
                        // `readPersistentAngerSaveData` set the end time as written.
                        slot.extra.anger_stamp = Some((slot.extra.anger, slot.age));
                    }
                }
                self.merge_portal_cooldown(id, &tag);
                if unresolved {
                    self.close_opaque_merchant(id);
                }
            }
            Target::Other(id) => {
                let i = self
                    .side
                    .others
                    .0
                    .iter()
                    .position(|o| o.id == id)
                    .ok_or_else(stale)?;
                let slot = &self.side.others.0[i];
                let mut new = Self::other_from_nbt(&mut tag, id, &slot.kind).ok_or_else(stale)?;
                new.join = slot.join;
                let is_display = matches!(
                    slot.kind.as_str(),
                    "minecraft:block_display" | "minecraft:item_display" | "minecraft:text_display"
                );
                self.side.others.0.update(i, |slot| *slot = new);
                // `EntityDataAccessor.setData` keeps the entity's UUID.
                set_key(&mut tag, "UUID", Tag::IntArray(self.uuid_for(id).to_vec()));
                if is_display {
                    self.persist.force_meta(id, 8);
                }
            }
            Target::Item(id) => {
                let mut new = entity_nbt::item_entity_from_nbt(&tag, id, 0.0).ok_or_else(stale)?;
                if let Some(slot) = self.dropped.iter_mut().find(|e| e.id == id) {
                    new.join = slot.join;
                    new.bob_offset = slot.bob_offset;
                    new.random = slot.random.clone();
                    sunrise_client::join_seq::store_back(slot, new);
                }
            }
            Target::Fangs(id) => {
                let owner = self.loaded_fangs_owner(&tag);
                let at = entity_nbt::position(&tag).ok_or_else(stale)?;
                if let Some(i) = self.fangs.list.iter().position(|e| e.id == id) {
                    self.fangs.list.update(i, |f| {
                        f.pos = [at.x, at.y, at.z];
                        f.motion = entity_nbt::loaded_motion(&tag);
                        f.fire_ticks = entity_nbt::fire_ticks_from_nbt(&tag);
                        f.has_visual_fire = entity_nbt::flag(&tag, "HasVisualFire");
                        f.yaw = entity_nbt::rotation(&tag)[0];
                        f.warmup =
                            sunrise_client::save::number(tag.get("Warmup")).unwrap_or(0) as i32;
                        f.owner = owner;
                    });
                }
            }
            Target::Stand(id) => {
                let mut new = entity_nbt::armor_stand_from_nbt(&tag, id).ok_or_else(stale)?;
                if let Some(slot) = self.armor_stands.iter_mut().find(|a| a.id == id) {
                    // Entity.load keeps the existing entity's firstTick state
                    // and its place in the level's join order.
                    new.first_tick = slot.first_tick;
                    new.join = slot.join;
                    sunrise_client::join_seq::store_back(slot, new);
                }
                self.merge_portal_cooldown(id, &tag);
            }
            Target::Frame(id) => {
                let mut new = entity_nbt::item_frame_from_nbt(&tag, id).ok_or_else(stale)?;
                if let Some(i) = self.item_frames.iter().position(|f| f.id == id) {
                    let old = &self.item_frames[i];
                    // Entity.load cannot change the runtime type; saveWithoutId
                    // deliberately omitted the decoder's type discriminator.
                    new.glow = old.glow;
                    new.join = old.join;
                    // ItemFrame.readAdditionalSaveData: ItemStack.matches includes
                    // count, item and components, before adopting the new stack.
                    // It compares the stack as read; setItem keeps one of it
                    // only after, as `new.item` already does.
                    let read = tag.get("Item").and_then(save::item_from_nbt);
                    if old.item.as_ref().is_some_and(|a| {
                        read.as_ref()
                            .is_none_or(|b| a.count != b.count || !a.same_item_same_components(b))
                    }) {
                        let old = old.clone();
                        self.remove_framed_map(&old);
                    }
                    let item = new.item.take();
                    new.set_item(item);
                    sunrise_client::join_seq::store_back(&mut self.item_frames[i], new);
                }
            }
            Target::Boat(id) => {
                // AbstractBoat.readAdditionalSaveData over Entity.load: the
                // position, rotation, motion, ground and gravity flags, Fire and chest items.
                let (position, velocity, yaw) =
                    entity_nbt::boat_from_nbt(&tag).ok_or_else(stale)?;
                let uuid = self.uuid_for(id);
                let i = self
                    .boats
                    .iter()
                    .position(|b| b.id == id)
                    .ok_or_else(stale)?;
                self.boats.update(i, |b| {
                    b.position = position;
                    b.previous = position;
                    b.velocity = velocity;
                    b.yaw = yaw;
                    b.previous_yaw = yaw;
                    b.pitch = entity_nbt::rotation(&tag)[1];
                    b.no_gravity = entity_nbt::flag(&tag, "NoGravity");
                    b.ground.on_ground = entity_nbt::flag(&tag, "OnGround");
                    b.fire_ticks = crate::game::boats::saved_fire(&tag);
                    b.ticks_frozen = crate::game::boats::saved_ticks_frozen(&tag);
                    if b.chest {
                        b.items =
                            save::decode_items(tag.get("Items"), crate::game::boats::CHEST_SLOTS);
                    }
                });
                set_key(&mut tag, "UUID", Tag::IntArray(uuid.to_vec()));
            }
            Target::Falling(id) => {
                // Use load_falling's existing converters, but mutate the resident
                // instance: loading/spawning would allocate a new id and join.
                let (state, position, velocity, age, fall_distance) =
                    entity_nbt::falling_block_from_nbt(&tag).ok_or_else(stale)?;
                let state = self.state_from_nbt(&state).ok_or_else(stale)?;
                let (no_gravity, drop_item) = entity_nbt::falling_block_flags(&tag);
                let damage = sunrise_client::falling_block::Damage::from_nbt(
                    &tag,
                    self.res.facts().get(state).name.ends_with("anvil"),
                );
                let uuid = self.uuid_for(id);
                let index = self
                    .falling
                    .iter()
                    .position(|f| f.id == id)
                    .ok_or_else(stale)?;
                self.falling.update(index, |falling| {
                    falling.state = state;
                    falling.position = position;
                    falling.previous = position;
                    falling.velocity = velocity;
                    falling.rotation = entity_nbt::loaded_rotation(&tag);
                    falling.age = age;
                    falling.no_gravity = no_gravity;
                    falling.drop_item = drop_item;
                    falling.fall_distance = fall_distance;
                    falling.ground.on_ground = entity_nbt::flag(&tag, "OnGround");
                    falling.fire_ticks = crate::game::boats::saved_fire(&tag);
                    falling.damage = damage;
                    // Keep id/join, start_pos, portal state and stuck multiplier.
                    set_key(&mut tag, "UUID", Tag::IntArray(uuid.to_vec()));
                    set_key(
                        &mut tag,
                        "id",
                        Tag::String("minecraft:falling_block".into()),
                    );
                    falling.saved = Some(Box::new((uuid, tag.clone())));
                });
            }
            Target::Tnt(id) => {
                // Entity.load and PrimedTnt.readAdditionalSaveData mutate the
                // resident instance; load_tnt would allocate another id/join.
                let position = entity_nbt::position(&tag).ok_or_else(stale)?;
                let uuid = self.uuid_for(id);
                let block_state = tag
                    .get("block_state")
                    .and_then(|v| entity_nbt::state_from_nbt(self.pack.assets.registry, v));
                let tnt = self
                    .tnts
                    .iter_mut()
                    .find(|tnt| tnt.id == id)
                    .ok_or_else(stale)?;
                tnt.position = position;
                tnt.previous = position;
                tnt.velocity = entity_nbt::motion(&tag);
                tnt.rotation = entity_nbt::rotation(&tag);
                tnt.fuse = super::super::persist::tnt_fuse(&tag);
                tnt.ground.on_ground = entity_nbt::flag(&tag, "OnGround");
                tnt.fall_distance =
                    sunrise_client::save::float(tag.get("fall_distance")).unwrap_or(0.0);
                tnt.no_gravity = entity_nbt::flag(&tag, "NoGravity");
                tnt.power = entity_nbt::tnt_power(&tag);
                tnt.block_state = block_state;
                // Runtime id/join, owner binding and guest attribution stay live.
                set_key(&mut tag, "UUID", Tag::IntArray(uuid.to_vec()));
            }
            Target::Orb(id) => {
                // Entity.load and ExperienceOrb.readAdditionalSaveData on the
                // resident orb: its runtime id, join and tick count stay.
                let new = entity_nbt::orbs_from_nbt(&tag)
                    .into_iter()
                    .next()
                    .ok_or_else(stale)?;
                let uuid = self.uuid_for(id);
                let orb = self
                    .orbs
                    .iter_mut()
                    .find(|o| o.id == id)
                    .ok_or_else(stale)?;
                orb.rotation = new.rotation;
                orb.position = new.position;
                orb.previous = new.previous;
                orb.velocity = new.velocity;
                orb.value = new.value;
                orb.age = new.age;
                orb.ground.on_ground = new.ground.on_ground;
                orb.count = new.count;
                orb.health = new.health;
                orb.no_gravity = new.no_gravity;
                orb.fall_distance = new.fall_distance;
                orb.fire_ticks = new.fire_ticks;
                orb.air = new.air;
                // Entity.load reads portalCooldown and leaves the portal
                // process as it is.
                if let Some(portal) = self.portals.entities.get_mut(&id) {
                    portal.cooldown = match tag.get("PortalCooldown") {
                        Some(Tag::Int(cooldown)) => *cooldown,
                        _ => 0,
                    };
                } else {
                    self.restore_portal_cooldown(id, &tag);
                }
                set_key(&mut tag, "UUID", Tag::IntArray(uuid.to_vec()));
            }
            Target::Trident(id) => {
                let loaded = entity_nbt::trident_from_nbt(&tag).ok_or_else(stale)?;
                let uuid = self.uuid_for(id);
                self.tridents
                    .iter_mut()
                    .find(|trident| trident.id == id)
                    .ok_or_else(stale)?
                    .load_nbt(loaded, &tag, self.pack.assets.registry);
                set_key(&mut tag, "UUID", Tag::IntArray(uuid.to_vec()));
            }
            Target::Rocket(id) => {
                let player = self.saved_player_uuid();
                let loaded =
                    entity_nbt::firework_rocket_from_nbt(&tag, player).ok_or_else(stale)?;
                let uuid = self.uuid_for(id);
                self.rockets
                    .iter_mut()
                    .find(|r| r.id == id)
                    .ok_or_else(stale)?
                    .load_nbt(loaded);
                set_key(&mut tag, "UUID", Tag::IntArray(uuid.to_vec()));
            }
            Target::Cart(id) => {
                let uuid = self.uuid_for(id);
                let i = self
                    .carts
                    .iter()
                    .position(|c| c.id == id)
                    .ok_or_else(stale)?;
                let mut cart = self.carts[i].clone();
                cart.load(&tag).ok_or_else(stale)?;
                crate::game::minecarts::forget_unreadable_display(&mut cart, |t| {
                    self.state_from_nbt(t).is_some()
                });
                if !self.store_cart_back(i, cart) {
                    return Err(stale());
                }
                set_key(&mut tag, "UUID", Tag::IntArray(uuid.to_vec()));
            }
            Target::Painting(id) => {
                // Painting.readAdditionalSaveData over HangingEntity's: the
                // facing, the block it hangs on and a known variant
                // (`VariantUtils.readVariant(..).ifPresent`: an unknown or
                // unreadable one keeps the painting's).
                let uuid = self.uuid_for(id);
                let (pos, facing) =
                    entity_nbt::painting_placement_from_nbt(&tag).ok_or_else(stale)?;
                let known = entity_nbt::painting_from_nbt(&tag)
                    .and_then(|(_, _, v)| self.painting_variants.get(&v?).cloned());
                let p = self
                    .paintings
                    .iter_mut()
                    .find(|p| p.id == id)
                    .ok_or_else(stale)?;
                (p.pos, p.facing) = (pos, facing);
                if let Some(v) = known {
                    (p.variant, p.width, p.height) = (v.id, v.width, v.height);
                }
                set_key(&mut tag, "UUID", Tag::IntArray(uuid.to_vec()));
            }
            Target::Projectile(id) => {
                let uuid = self.uuid_for(id);
                let index = self
                    .creatures
                    .projectiles
                    .iter()
                    .position(|p| p.id == id)
                    .ok_or_else(stale)?;
                if let Some(spectral) = self.creatures.projectiles[index]
                    .arrow
                    .as_ref()
                    .map(|a| a.spectral)
                {
                    self.load_resident_arrow(index, spectral, &tag)
                        .ok_or_else(stale)?;
                    self.creatures.projectiles.update(index, |p| {
                        p.fire_ticks = entity_nbt::fire_ticks_from_nbt(&tag);
                        p.has_visual_fire = entity_nbt::flag(&tag, "HasVisualFire");
                        if let Some(a) = p.arrow.as_mut() {
                            a.fire_ticks = p.fire_ticks;
                        }
                    });
                } else {
                    let thrown_item = self.creatures.projectiles[index]
                        .kind
                        .is_throwable_item()
                        .then(|| entity_nbt::decode_throwable_item_field(&tag, self.res.items()));
                    self.creatures.projectiles.update(index, |p| {
                        if let Some(pos) = entity_nbt::position(&tag) {
                            p.position = pos;
                            p.previous = pos;
                        }
                        p.velocity = entity_nbt::motion(&tag);
                        let [yaw, pitch] = entity_nbt::rotation(&tag);
                        p.yaw = yaw;
                        p.pitch = pitch;
                        p.no_gravity = entity_nbt::flag(&tag, "NoGravity");
                        p.ground.on_ground = entity_nbt::flag(&tag, "OnGround");
                        p.fire_ticks = entity_nbt::fire_ticks_from_nbt(&tag);
                        p.has_visual_fire = entity_nbt::flag(&tag, "HasVisualFire");
                        if let Some(item) = thrown_item {
                            p.set_thrown_item(item);
                        }
                    });
                }
                set_key(&mut tag, "UUID", Tag::IntArray(uuid.to_vec()));
            }
            Target::Bolt(id) => {
                Self::normalize_loaded_mob_tags(&mut tag);
                let i = self
                    .bolts
                    .iter()
                    .position(|b| b.id == id)
                    .ok_or_else(stale)?;
                self.bolts.update(i, |b| b.apply_nbt(&tag));
                self.join_saved_team(id, &tag);
                let mut kept = Compound::new();
                if let Some(tags @ Tag::List(_)) = tag.get("Tags") {
                    kept.insert("Tags".into(), tags.clone());
                }
                if let Some(data @ Tag::Compound(d)) = tag.get("data")
                    && !d.is_empty()
                {
                    kept.insert("data".into(), data.clone());
                }
                tag = Tag::Compound(kept);
            }
            _ => return Err(stale()),
        }
        if let Target::Mob(id)
        | Target::Item(id)
        | Target::Stand(id)
        | Target::Frame(id)
        | Target::Painting(id)
        | Target::Boat(id)
        | Target::Cart(id)
        | Target::Tnt(id)
        | Target::Orb(id)
        | Target::Cushion(id)
        | Target::Trident(id)
        | Target::Rocket(id)
        | Target::Other(id)
        | Target::Fangs(id)
        | Target::Bolt(id)
        | Target::Projectile(id) = t
        {
            if let Some(kept) = kept_passengers
                && let Tag::Compound(c) = &mut tag
            {
                match kept {
                    Some(passengers) => {
                        c.insert("Passengers".into(), passengers);
                    }
                    None => {
                        c.remove(&"Passengers".into());
                    }
                }
            }
            entity_nbt::sanitize_custom_name(&mut tag);
            self.persist.raw.insert(id, tag);
            if let (Some(from), Some(to)) = (before, self.target_position(t)) {
                self.entity_section_moved(id, Some(from), to);
            }
        }
        Ok(())
    }
    pub(in crate::game::server_side) fn entity_tag_in_level(
        &mut self,
        t: Target,
        include_id: bool,
    ) -> Option<Tag> {
        let player_uuid = self.saved_player_uuid();
        let mut tag = match t {
            // `FishingHook.shouldBeSaved` is false, and it keeps no compound here.
            // Neither is saved: `FishingHook.shouldBeSaved` and
            // `EnderDragonPart.shouldBeSaved` are false.
            Target::Hook(_) | Target::DragonPart(_) => return None,
            // Stage 3's save owns the general record; none is saved here.
            Target::General(_) => return None,
            Target::Guest(_) => self.for_player(t, |g| g.player_nbt()).flatten()?,
            Target::Player => self.player_nbt()?,
            Target::Mob(id) => {
                // `saveWithoutId` writes the entity as saved, riders under
                // `Passengers`: `/data get` reads the same tree persistence does (#1519).
                let i = self.mobs.iter().position(|m| m.id == id)?;
                let mut tag = if include_id {
                    self.mob_tree_nbt(i)
                } else {
                    self.mob_tree_tag_without_id(i)
                };
                let session = self
                    .lan_trade_conn(id)
                    .and_then(|conn| self.lan_trade_session(conn))
                    .or_else(|| {
                        self.station
                            .as_ref()
                            .and_then(|s| s.merchant.as_ref())
                            .filter(|m| m.mob == Some(id))
                            .map(|m| &*m.session)
                    });
                if let (Some(session), Tag::Compound(data)) = (session, &mut tag) {
                    data.insert(
                        "Offers".into(),
                        entity_nbt::offers_to_nbt(&session.offers, self.res.items()),
                    );
                    data.insert("Xp".into(), Tag::Int(session.xp));
                }
                tag
            }
            Target::Item(id) => {
                let uuid = self.uuid_for(id);
                let e = self.dropped.iter().find(|e| e.id == id)?;
                let mut tag = entity_nbt::item_entity_to_nbt(
                    e,
                    uuid,
                    self.persist.raw.get(&id),
                    self.items(),
                );
                self.save_portal_cooldown(id, &mut tag);
                tag
            }
            Target::Stand(id) => {
                let uuid = self.uuid_for(id);
                let a = self.armor_stands.iter().find(|a| a.id == id)?;
                let mut tag = entity_nbt::armor_stand_tag(
                    a,
                    uuid,
                    self.persist.raw.get(&id),
                    self.res.items(),
                    include_id,
                );
                self.save_portal_cooldown(id, &mut tag);
                tag
            }
            Target::Frame(id) => {
                let uuid = self.uuid_for(id);
                let f = self.item_frames.iter().find(|f| f.id == id)?;
                entity_nbt::item_frame_to_nbt(f, uuid, self.persist.raw.get(&id), self.res.items())
            }
            Target::Painting(id) => {
                let uuid = self.uuid_for(id);
                let p = self.paintings.iter().find(|p| p.id == id)?;
                entity_nbt::painting_to_nbt(p, uuid, self.persist.raw.get(&id))
            }
            Target::Boat(id) => {
                let uuid = self.uuid_for(id);
                let i = self.boats.iter().position(|b| b.id == id)?;
                let b = &self.boats[i];
                let mut tag = b.to_nbt(uuid, self.persist.raw.get(&id), self.res.items());
                // Entity.saveWithoutId: the actual runtime passenger tree.
                self.put_passengers(&mut tag, id);
                // AbstractBoat.addAdditionalSaveData: the live or pending leash.
                self.write_boat_leash(&mut tag, i);
                self.save_portal_cooldown(id, &mut tag);
                tag
            }
            Target::Cart(id) => {
                let uuid = self.uuid_for(id);
                let c = self.carts.iter().find(|c| c.id == id)?;
                let mut tag = c.to_nbt(uuid, self.persist.raw.get(&id), self.res.items());
                self.put_passengers(&mut tag, id);
                self.save_portal_cooldown(id, &mut tag);
                tag
            }
            Target::Fangs(id) => {
                let uuid = self.uuid_for(id);
                let f = self.fangs.list.iter().find(|f| f.id == id)?;
                self.fangs_to_nbt(f, uuid)
            }
            Target::Other(id) => {
                let uuid = self.uuid_for(id);
                let o = self.side.others.0.iter().find(|o| o.id == id)?.clone();
                self.other_tag(&o, uuid, include_id)
            }
            Target::Orb(id) => {
                let uuid = self.uuid_for(id);
                let tag = entity_nbt::orb_to_nbt(self.orbs.iter().find(|o| o.id == id)?, uuid);
                // Entity.saveWithoutId: the live fire, air and portal cooldown.
                let mut tag = crate::game::persist::with_raw_over(
                    tag,
                    self.persist.raw.get(&id),
                    crate::game::persist::ORB_LIVE,
                );
                self.save_portal_cooldown(id, &mut tag);
                tag
            }
            Target::Projectile(id) => {
                let uuid = self.uuid_for(id);
                // A homing bullet's `Target` is the entity's, not the raw
                // compound's, as `save_projectile` writes it.
                let who = self
                    .creatures
                    .projectiles
                    .iter()
                    .find(|p| p.id == id)
                    .and_then(|p| match &p.kind {
                        sunrise_client::projectiles::ProjectileKind::ShulkerBullet {
                            target: Some(who),
                            ..
                        } => Some(*who),
                        _ => None,
                    });
                let target = who.and_then(|who| self.save_who_uuid(who));
                let p = self.creatures.projectiles.iter().find(|p| p.id == id)?;
                let owner = if p.owner == Some(sunrise_client::mobs::Who::Player) {
                    player_uuid
                } else {
                    None
                };
                let in_block = p
                    .arrow
                    .as_ref()
                    .and_then(|a| a.last_state)
                    .map(|s| self.state_to_nbt(s));
                let raw = self.persist.raw.get(&id);
                // Projectile.addAdditionalSaveData stores the retained EntityReference,
                // including an owner that is not currently resolved to a live entity.
                let mut tag = entity_nbt::arrow_to_nbt(
                    p,
                    uuid,
                    p.owner_uuid.or(owner),
                    in_block,
                    raw,
                    self.res.items(),
                )
                .or_else(|| entity_nbt::fireball_to_nbt(p, uuid, owner, raw, self.res.items()))
                .or_else(|| entity_nbt::throwable_to_nbt(p, uuid, owner, raw, self.res.items()))
                .or_else(|| entity_nbt::spit_or_bullet_to_nbt(p, uuid, owner, target, raw))?;
                self.save_projectile_portal_cooldown(p, &mut tag);
                tag
            }
            Target::Crystal(id) => {
                let c = self.crystals.iter().find(|c| c.id == id)?;
                self.crystal_nbt(c)
            }
            Target::Dragon(id) => {
                let d = self.dragon.as_ref().filter(|d| d.id == id)?;
                let raw = self.persist_anon("minecraft:ender_dragon", d.position);
                let uuid = self.persist.uuids.get(&id).copied();
                let tag = d.to_nbt(uuid.unwrap_or_else(random_uuid), raw, self.res.items());
                crate::game::persist::with_raw(tag, self.persist.raw.get(&id))
            }
            Target::Tnt(id) => {
                let i = self.tnts.iter().position(|t| t.id == id)?;
                self.tnt_nbt(i)
            }
            Target::Cushion(id) => {
                let uuid = self.uuid_for(id);
                let c = self.cushions.iter().find(|c| c.id == id)?;
                entity_nbt::cushion_to_nbt(c, uuid, self.persist.raw.get(&id))
            }
            Target::Falling(id) => {
                let f = self.falling.iter().find(|f| f.id == id)?.clone();
                let (uuid, raw) = match &f.saved {
                    Some(saved) => (saved.0, Some(&saved.1)),
                    None => (self.uuid_for(id), None),
                };
                let tag = entity_nbt::falling_block_to_nbt(
                    self.state_to_nbt(f.state),
                    f.position,
                    f.velocity,
                    f.age,
                    f.drop_item,
                    f.damage,
                    f.fall_distance,
                    f.ground.on_ground,
                    f.rotation,
                    uuid,
                );
                let mut tag = crate::game::persist::with_raw(tag, raw);
                if let Tag::Compound(fields) = &mut tag {
                    // Its simulated fire, not a saved copy's (`with_raw` keeps those).
                    fields.insert("Fire".into(), Tag::Short(f.fire_ticks as i16));
                }
                tag
            }
            Target::Rocket(id) => {
                let uuid = self.uuid_for(id);
                let r = self.rockets.iter().find(|r| r.id == id)?;
                r.to_nbt(
                    uuid,
                    player_uuid,
                    self.persist.raw.get(&id),
                    self.res.items(),
                )
            }
            Target::Cloud(id) => {
                let uuid = self.uuid_for(id);
                let c = self.breath_clouds.iter().find(|c| c.id == id)?;
                let [x, y, z] = c.at;
                let [yaw, pitch] = self
                    .persist
                    .raw
                    .get(&id)
                    .map_or([0.0; 2], entity_nbt::loaded_rotation);
                // Read the live cloud, including its current radius and age;
                // the raw compound alone does not contain generated clouds.
                let mut tag = self.other_to_nbt(
                    &super::OtherEntity {
                        size_answer: None,
                        attachment_lifetime: Default::default(),
                        join: Default::default(),
                        id,
                        kind: "minecraft:area_effect_cloud".into(),
                        item_transformation: None,
                        interaction: None,
                        knot_check: None,
                        eye: None,
                        position: sunrise_core::geometry::Vec3::new(x, y, z),
                        yaw,
                        pitch,
                    },
                    uuid,
                );
                // Entity.saveWithoutId's common fields, also for a cloud never saved.
                crate::game::persist::write_static_entity_fields(&mut tag);
                set_key(
                    &mut tag,
                    "Motion",
                    Tag::List(
                        c.motion
                            .into_iter()
                            .map(|v| Tag::Double(if v == 0.0 { 0 } else { v.to_bits() }))
                            .collect(),
                    ),
                );
                set_key(
                    &mut tag,
                    "fall_distance",
                    Tag::Double(if c.fall_distance == 0.0 {
                        0
                    } else {
                        c.fall_distance.to_bits()
                    }),
                );
                set_key(&mut tag, "Fire", Tag::Short(c.fire_ticks as i16));
                set_key(&mut tag, "Age", Tag::Int(c.age));
                set_key(&mut tag, "Duration", Tag::Int(c.duration));
                set_key(&mut tag, "Radius", Tag::Float(c.radius.to_bits()));
                let wait = c.potion.as_ref().map_or(20, |p| p.settings.wait_time);
                set_key(&mut tag, "WaitTime", Tag::Int(wait));
                if let Some(p) = &c.potion {
                    set_key(
                        &mut tag,
                        "ReapplicationDelay",
                        Tag::Int(p.settings.reapplication_delay),
                    );
                    set_key(
                        &mut tag,
                        "DurationOnUse",
                        Tag::Int(p.settings.duration_on_use),
                    );
                    set_key(
                        &mut tag,
                        "RadiusOnUse",
                        Tag::Float(p.settings.radius_on_use.to_bits()),
                    );
                    set_key(
                        &mut tag,
                        "RadiusPerTick",
                        Tag::Float(p.radius_per_tick.to_bits()),
                    );
                    if p.settings.duration_scale != 1.0 {
                        set_key(
                            &mut tag,
                            "potion_duration_scale",
                            Tag::Float(p.settings.duration_scale.to_bits()),
                        );
                    } else if let Tag::Compound(c) = &mut tag {
                        c.remove(&"potion_duration_scale".into());
                    }
                }
                tag
            }
            Target::Trident(id) => {
                let uuid = self.uuid_for(id);
                let t = self.tridents.iter().find(|t| t.id == id)?;
                t.to_nbt(
                    uuid,
                    self.pack.assets.registry,
                    self.persist.raw.get(&id),
                    self.res.items(),
                )
            }
            Target::Spawner(id) => {
                let s = self.ominous_spawners.iter().find(|s| s.id == id)?;
                let [x, y, z] = s.position;
                entity_nbt::ominous_item_spawner_to_nbt(
                    &s.item,
                    sunrise_core::geometry::Vec3::new(x, y, z),
                    s.spawn_item_after,
                    s.fire_ticks,
                    s.uuid,
                    s.raw.as_deref(),
                    self.res.items(),
                )
            }
            Target::Bolt(id) => {
                let uuid = self.uuid_for(id);
                let b = self.bolts.iter().find(|b| b.id == id)?;
                self.bolt_tag(b, uuid, include_id)
            }
            // LeashFenceKnotEntity saves nothing of its own: the common
            // entity fields, as an explicitly summoned knot's.
            Target::Knot(id) => {
                let fence = self.natural_knot(id)?;
                let uuid = *self.persist.uuids.get(&id)?;
                let knot = super::OtherEntity {
                    size_answer: None,
                    attachment_lifetime: Default::default(),
                    // A serialization view, not a second resident entity.
                    join: sunrise_client::join_seq::JoinSeq::UNJOINED,
                    id,
                    kind: "minecraft:leash_knot".into(),
                    position: sunrise_client::mobs::leash::knot_state(fence).position,
                    yaw: 0.0,
                    pitch: 0.0,
                    item_transformation: None,
                    interaction: None,
                    knot_check: None,
                    eye: None,
                };
                let mut tag = self.other_to_nbt(&knot, uuid);
                // Entity.saveWithoutId writes `remainingFireTicks` always; the
                // knot's never changes (BlockAttachedEntity.tick has no
                // baseTick).
                set_key(&mut tag, "Fire", Tag::Short(0));
                tag
            }
        };
        if !include_id
            && !matches!(t, Target::Mob(_) | Target::Stand(_) | Target::Other(_))
            && let Tag::Compound(c) = &mut tag
        {
            c.remove(&"id".into());
        }
        Some(tag)
    }
    /// The block entity part of `BlockInput.place`: load the block entity
    /// from the argument's tag (read; `load_block_entity_nbt` reads SNBT); true when its saved data changed.
    pub(in crate::game) fn load_block_entity_tag(
        &mut self,
        pos: [i32; 3],
        mut tag: sunrise_core::nbt::Tag,
    ) -> bool {
        use sunrise_core::nbt::Tag;
        if self.pot_data(pos).is_some() {
            return self.set_pot_data(pos, &tag).unwrap_or(false);
        }
        if let Some(changed) = self.load_comparator_output(pos, &tag) {
            let changed = changed.unwrap_or(false);
            if changed {
                self.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
            }
            return changed;
        }
        let key = (pos[0], pos[1], pos[2]);
        let was_auto = self.command_block_auto(key);
        let Some(e) = self.world.block_entities.get(&key) else {
            return false;
        };
        let name = self.data_block_name(pos);
        let before =
            sunrise_client::save::block_entity_encode::encode(key, e, &name, self.res.items());
        if let Tag::Compound(c) = &mut tag {
            for k in ["id", "x", "y", "z"] {
                if let Some(v) = before.get(k) {
                    c.insert(k.into(), v.clone());
                }
            }
        }
        let Some((_, mut new)) = self.decode_block_entity_at(key, &tag) else {
            return false;
        };
        let rolled = self.take_loot_table(key, &tag, &mut new);
        let after =
            sunrise_client::save::block_entity_encode::encode(key, &new, &name, self.res.items());
        // The lock is outside the model: a tag that only changes it still
        // changes the block entity.
        let lock_changed = self.world.block_lock(key) != tag.get("lock");
        // So is the custom name.
        let name_changed = self.load_block_name(key, &new, &tag);
        let cooldown_changed = if name == "minecraft:hopper" {
            let state = super::super::machines::HopperState::load(&tag);
            let changed = self.side.hoppers.get(&key).map_or(-1, |s| s.cooldown) != state.cooldown;
            self.side.hoppers.insert(key, state);
            changed
        } else {
            false
        };
        if after == before && !rolled && !lock_changed && !name_changed && !cooldown_changed {
            // BlockInput.place calls loadWithComponents even when its before/after
            // saves are equal. DataCommands instead rejects a no-op before setData.
            self.notify_block_entity_load_additional(key);
            return false;
        }
        self.world.block_entities.insert(key, new);
        self.world.set_block_lock(key, &tag);
        self.notify_block_entity_load_additional(key);
        // Tag-only /setblock or /fill edits must still notify clients of spawner display changes.
        if matches!(
            self.world.block_entities.get(&key),
            Some(BlockEntity::Spawner(_) | BlockEntity::TrialSpawner(_))
        ) {
            self.mark_block_entity_updated(key);
        }
        self.command_block_loaded(key, was_auto);
        self.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
        true
    }
    /// The acting player's own UUID: a guest's turn (the window's player on
    /// the server thread) keeps the world owner's in `level`.
    pub(in crate::game::server_side) fn saved_player_uuid(&self) -> Option<[i32; 4]> {
        self.side
            .lan_acting_uuid
            .map(sunrise_client::lan::playerdata::uuid_ints)
            .or(self.level.player_uuid)
    }
    pub(in crate::game) fn set_data_of(&mut self, r: &DataRef, tag: Tag) -> R<()> {
        match r {
            DataRef::Entity(t) => self.apply_entity_nbt(*t, tag),
            DataRef::Block(p) => {
                // This view is read-only: never decode a live piston into a
                // second, detached entry in world.block_entities.
                if self.moving_piston_data(*p).is_some() {
                    return Err(err("commands.data.block.invalid", &[]));
                }
                if self.set_pot_data(*p, &tag).is_some() {
                    return Ok(());
                }
                let key = (p[0], p[1], p[2]);
                if let Some(changed) = self.load_comparator_output(*p, &tag) {
                    changed.ok_or_else(|| err("commands.data.block.invalid", &[]))?;
                    self.mark_block_entity_updated(key);
                    return Ok(());
                }
                if let Some((_, mut new)) = self.decode_block_entity_at(key, &tag) {
                    let was_auto = self.command_block_auto(key);
                    if self.data_block_name(*p) == "minecraft:hopper" {
                        self.side
                            .hoppers
                            .insert(key, super::super::machines::HopperState::load(&tag));
                    }
                    self.take_loot_table(key, &tag, &mut new);
                    self.load_block_name(key, &new, &tag);
                    if let (Some(BlockEntity::Mod(current)), BlockEntity::Mod(loaded)) =
                        (self.world.block_entities.get_mut(&key), &mut new)
                        && current.id() == loaded.id()
                    {
                        // BlockDataAccessor.setData loads the existing entity.
                        // Replace its saved fields, preserving runtime data and
                        // identity; DerefMut still invalidates content caches.
                        current.nbt = std::mem::take(&mut loaded.nbt);
                        current.items = std::mem::take(&mut loaded.items);
                    } else {
                        self.world.block_entities.insert(key, new);
                    }
                    self.world.set_block_lock(key, &tag);
                    self.notify_block_entity_load_additional(key);
                    self.command_block_loaded(key, was_auto);
                    // BlockDataAccessor.setData: setChanged and sendBlockUpdated(3),
                    // even when the block state itself stays the same.
                    self.mark_block_entity_updated(key);
                    let state = self.world.block(p[0], p[1], p[2]);
                    // Redraw what shows the block entity (signs, banners).
                    let _ = self.world.set(p[0], p[1], p[2], state);
                }
                Ok(())
            }
            DataRef::Storage(id) => {
                self.score.storage.set(id, tag);
                Ok(())
            }
        }
    }
    /// Spawn an entity from its compound (`EntityType.loadEntityRecursive`);
    /// returns what was created.
    pub(in crate::game) fn spawn_entity_from_nbt(&mut self, tag: Tag) -> Option<Target> {
        let key = {
            let p = entity_nbt::position(&tag)?;
            ((p.x.floor() as i32) >> 4, (p.z.floor() as i32) >> 4)
        };
        // Only this stack spawns now: the entities of chunks that finished
        // loading wait for the level's entity tick, as in vanilla
        // (PersistentEntitySectionManager.processPendingLoads), and must not
        // take the ids counted as this stack's.
        let mut waiting = std::mem::take(&mut self.world.loaded_entities);
        let before = self.side.next_entity;
        let mut tag = tag;
        crate::game::persist::mark_fresh(&mut tag);
        self.world.queue_entity_nbt(key, vec![tag]);
        self.spawn_loaded_entities();
        waiting.append(&mut self.world.loaded_entities);
        self.world.loaded_entities = waiting;
        if self.side.next_entity == before {
            return None;
        }
        // The root of the stack is the first entity spawned.
        let id = before + 1;
        if self
            .resident_entities
            .general
            .rows
            .iter()
            .any(|row| row.id == id)
        {
            Some(Target::General(id))
        } else if self.mobs.iter().any(|m| m.id == id) {
            Some(Target::Mob(id))
        } else if self.dropped.iter().any(|e| e.id == id) {
            Some(Target::Item(id))
        } else if self.armor_stands.iter().any(|a| a.id == id) {
            Some(Target::Stand(id))
        } else if self.item_frames.iter().any(|f| f.id == id) {
            Some(Target::Frame(id))
        } else if self.paintings.iter().any(|p| p.id == id) {
            Some(Target::Painting(id))
        } else if self.boats.iter().any(|b| b.id == id) {
            Some(Target::Boat(id))
        } else if self.carts.iter().any(|c| c.id == id) {
            Some(Target::Cart(id))
        } else if self.side.others.0.iter().any(|o| o.id == id) {
            Some(Target::Other(id))
        } else if self.tnts.iter().any(|t| t.id == id) {
            Some(Target::Tnt(id))
        } else if self.crystals.iter().any(|c| c.id == id) {
            Some(Target::Crystal(id))
        } else if self.cushions.iter().any(|c| c.id == id) {
            Some(Target::Cushion(id))
        } else if self.falling.iter().any(|f| f.id == id) {
            Some(Target::Falling(id))
        } else if self.rockets.iter().any(|r| r.id == id) {
            Some(Target::Rocket(id))
        } else if self.tridents.iter().any(|t| t.id == id) {
            Some(Target::Trident(id))
        } else if self.ominous_spawners.iter().any(|s| s.id == id) {
            Some(Target::Spawner(id))
        } else if self.bolts.iter().any(|b| b.id == id) {
            Some(Target::Bolt(id))
        } else if self.creatures.projectiles.iter().any(|p| p.id == id) {
            Some(Target::Projectile(id))
        } else if self.orbs.iter().any(|o| o.id == id) {
            Some(Target::Orb(id))
        } else if self.fangs.list.iter().any(|f| f.id == id) {
            Some(Target::Fangs(id))
        } else if self.breath_clouds.iter().any(|c| c.id == id) {
            Some(Target::Cloud(id))
        } else if self.dragon.as_ref().is_some_and(|d| d.id == id) {
            Some(Target::Dragon(id))
        } else {
            None
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    use super::*;
    use crate::game::native_entity_refs::tests::with_game;
    use sunrise_core::geometry::Vec3;

    /// `EntityDataAccessor.setData` keeps the entity's id and UUID: a
    /// `/data merge` that carries a different UUID does not take, and a
    /// display's load marks its start delay forced
    /// (`Display.setTransformationInterpolationDelay`).
    #[test]
    #[ignore = "daily"]
    fn merging_display_nbt_keeps_the_uuid_and_forces_the_start_delay() {
        with_game(|game| {
            let mut raw = Compound::new();
            raw.insert(
                "Pos".into(),
                Tag::List(
                    [0.5f64, 64.0, 0.5]
                        .map(|v| Tag::Double(v.to_bits()))
                        .to_vec(),
                ),
            );
            game.persist.raw.insert(12, Tag::Compound(raw));
            game.persist.uuids.insert(12, [1, 2, 3, 4]);
            game.side.others.0.push(super::super::OtherEntity {
                size_answer: None,
                attachment_lifetime: Default::default(),
                join: Default::default(),
                item_transformation: None,
                knot_check: None,
                eye: None,
                interaction: None,
                id: 12,
                kind: "minecraft:block_display".into(),
                position: Vec3::new(0.5, 64.0, 0.5),
                yaw: 0.0,
                pitch: 0.0,
            });
            let mut merge = Compound::new();
            merge.insert(
                "Pos".into(),
                Tag::List(
                    [0.5f64, 64.0, 0.5]
                        .map(|v| Tag::Double(v.to_bits()))
                        .to_vec(),
                ),
            );
            merge.insert("UUID".into(), Tag::IntArray(vec![9, 9, 9, 9]));
            merge.insert("start_interpolation".into(), Tag::Int(0));
            game.apply_entity_nbt(Target::Other(12), Tag::Compound(merge))
                .unwrap();
            assert_eq!(game.uuid_for(12), [1, 2, 3, 4]);
            assert_eq!(game.persist.forced_meta.get(&12), Some(&[1 << 8, 0, 0, 0]));
            game.spend_forced_meta();
            let ctx = sunrise_command::Context {
                root: 0,
                nodes: vec![],
                args: vec![sunrise_command::ParsedArg {
                    name: "nbt".into(),
                    value: sunrise_command::Arg::Str("{start_interpolation:0}".into()),
                    range: (0, 23),
                }],
                command: None,
                executor: None,
                range: (0, 23),
                child: None,
                literals: vec![],
            };
            for _ in 0..2 {
                assert_eq!(
                    game.cmd_data_in("data.merge", &ctx, DataRef::Entity(Target::Other(12)))
                        .unwrap(),
                    1
                );
                assert_eq!(game.persist.forced_meta.get(&12), Some(&[1 << 8, 0, 0, 0]));
                game.spend_forced_meta();
            }
        });
    }
}

#[cfg(test)]
#[path = "data/eye_item_tests_server.rs"]
pub(in crate::game) mod eye_item_tests;

#[cfg(test)]
#[path = "data/saved_tag_tests_server.rs"]
pub(in crate::game) mod saved_tag_tests;

#[cfg(test)]
#[path = "data/fire_tests_server.rs"]
pub(in crate::game) mod fire_tests;

#[cfg(test)]
pub(in crate::game) mod modern_literal_tests {
    use super::*;
    use crate::game::commands::execute::ExecState;
    use crate::game::{Options, Progress, Settings, load_resources};

    #[test]
    #[ignore = "daily"]
    fn admitted_modern_literals_reach_actual_storage_as_typed_tags() {
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
        let run = |g: &mut crate::game::ServerGame<'_>, line: &str| {
            g.side.exec.0 = ExecState::default();
            let source = g.player_source();
            g.execute_line(line, source)
        };
        let cases = [
            ("hex", "0x10", "16", Tag::Int(16)),
            ("underscore", "1_000", "1000", Tag::Int(1000)),
            ("unicode4", r#""\u0041""#, r#""A""#, Tag::String("A".into())),
            ("hex2", r#""\x41""#, r#""A""#, Tag::String("A".into())),
            (
                "unicode8",
                r#""\U00000041""#,
                r#""A""#,
                Tag::String("A".into()),
            ),
            (
                "trailing",
                "{n:3,}",
                "{n:3}",
                Tag::compound([("n", Tag::Int(3))]),
            ),
            ("plain", "plain", r#""plain""#, Tag::String("plain".into())),
        ];
        let clock = game.world.game_time;
        let mut differences = Vec::new();
        for (name, modern, plain, expected) in &cases {
            for route in ["merge", "modify"] {
                let target = format!("h189:{route}_{name}");
                let reference = format!("h189:ref_{route}_{name}");
                let seed = ["data merge storage ", &target, r#" {v:"seed"}"#].concat();
                assert_eq!(run(&mut game, &seed).unwrap(), 1);
                assert_eq!(
                    game.score.storage.get(&target).get("v"),
                    Some(&Tag::String("seed".into()))
                );
                let canonical = ["data merge storage ", &reference, " {v:", plain, "}"].concat();
                assert_eq!(run(&mut game, &canonical).unwrap(), 1);
                assert_eq!(game.score.storage.get(&reference).get("v"), Some(expected));
                let command = if route == "merge" {
                    ["data merge storage ", &target, " {v:", modern, "}"].concat()
                } else {
                    ["data modify storage ", &target, " v set value ", modern].concat()
                };
                let dispatcher = sunrise_client::command::vanilla::game_dispatcher();
                assert!(
                    dispatcher
                        .parse(&command, &game.ui.registries)
                        .executable()
                        .is_ok(),
                    "admission: {route}/{name}"
                );
                let result = run(&mut game, &command);
                let actual = game.score.storage.get(&target).get("v").cloned();
                if result.as_ref().ok() != Some(&1) || actual.as_ref() != Some(expected) {
                    differences.push(format!("{route}/{name}: result={result:?}, stored={actual:?}, expected={expected:?}"));
                }
                // Copy actual canonical stored data back: equality is a real
                // no-change failure, independent of a formatter or parser stub.
                let copy = [
                    "data modify storage ",
                    &target,
                    " v set from storage ",
                    &reference,
                    " v",
                ]
                .concat();
                if run(&mut game, &copy).is_ok() {
                    differences.push(format!(
                        "{route}/{name}: canonical copy changed the stored Tag"
                    ));
                }
                assert_eq!(game.score.storage.get(&target).get("v"), Some(expected));
            }
        }
        assert_eq!(game.world.game_time, clock);
        assert!(differences.is_empty(), "{}", differences.join("\n"));
    }
}

#[cfg(test)]
pub(in crate::game) mod falling_live_adoption_tests {
    use crate::game::commands::Target;
    use crate::game::commands::data::falling_live_adoption_tests::with_game;
    use sunrise_core::nbt::Tag;
    use sunrise_core::{geometry::Vec3, position::BlockPos};

    #[test]
    #[ignore = "daily"]
    fn commands_mutate_the_resident_falling_model_without_rejoining() {
        with_game(|game| {
            let uuid = "000003d4-0000-0000-0000-000000000001";
            assert!(game.run_command("summon falling_block 4.5 200 5.5 {UUID:[I;980,0,0,1],BlockState:\"minecraft:gravel\",Time:5,DropItem:1b,NoGravity:1b,Motion:[0d,0d,0d],Tags:[keep],Silent:1b}"));
            assert!(game.run_command(&format!(
                "execute as {uuid} if entity @s[type=falling_block]"
            )));
            let index = game
                .falling
                .iter()
                .position(|f| game.persist.uuid_of(f.id) == Some([980, 0, 0, 1]))
                .unwrap();
            game.falling.update(index, |falling| {
                falling.start_pos = BlockPos::new(9, 201, 11);
                falling.stuck = Vec3::new(0.2, 0.3, 0.4);
                falling.portal.cooldown = 17;
                falling.portal.process = Some(sunrise_client::nether_portal::Process {
                    entry: BlockPos::new(8, 199, 8),
                    time: 7,
                    inside: true,
                });
            });
            let before = game.falling[index].clone();
            assert!(before.join.is_joined());
            assert_eq!((before.age, before.drop_item), (5, true));
            let next_id = game.side.next_entity;
            let clock = game.world.game_time;
            assert!(!game.run_command(&format!("data merge entity {uuid} {{Time:5}}")));
            assert!(
                game.run_command(&format!("data merge entity {uuid} {{Time:12}}")),
                "valid Time update must reach model"
            );
            assert_eq!(game.falling[index].age, 12);
            assert!(game.run_command(&format!("data merge entity {uuid} {{DropItem:0b}}")));
            assert!(!game.falling[index].drop_item);
            let f = &game.falling[index];
            assert_eq!(
                (f.id, f.join, f.start_pos, f.stuck),
                (before.id, before.join, before.start_pos, before.stuck)
            );
            assert_eq!(
                (
                    f.position,
                    f.previous,
                    f.velocity,
                    f.rotation,
                    f.state,
                    f.no_gravity,
                    f.damage,
                    f.fall_distance
                ),
                (
                    before.position,
                    before.position,
                    before.velocity,
                    before.rotation,
                    before.state,
                    before.no_gravity,
                    before.damage,
                    before.fall_distance
                )
            );
            assert_eq!(f.portal.cooldown, before.portal.cooldown);
            let (a, b) = (f.portal.process.unwrap(), before.portal.process.unwrap());
            assert_eq!((a.entry, a.time, a.inside), (b.entry, b.time, b.inside));
            assert_eq!(f.saved.as_ref().unwrap().0, [980, 0, 0, 1]);
            assert_eq!(game.persist.uuid_of(f.id), Some([980, 0, 0, 1]));
            let saved = game.command_target_nbt(Target::Falling(f.id)).unwrap();
            assert_eq!(saved.get("Time"), Some(&Tag::Int(12)));
            assert_eq!(saved.get("DropItem"), Some(&Tag::Byte(0)));
            assert_eq!(
                saved.get("BlockState"),
                Some(&Tag::String("minecraft:gravel".into()))
            );
            assert_eq!(
                saved.get("Tags"),
                Some(&Tag::List(vec![Tag::String("keep".into())]))
            );
            assert_eq!(saved.get("Silent"), Some(&Tag::Byte(1)));
            assert_eq!(game.falling.len(), 1);
            assert_eq!(game.side.next_entity, next_id);
            assert_eq!(game.world.game_time, clock);
        });
    }
}
