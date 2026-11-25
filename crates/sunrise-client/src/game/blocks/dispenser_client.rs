#[allow(unused_imports)]
pub(in crate::game::client_side) use super::api_values::{
    TryIntoApi, TryIntoEnginePreservingCount,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::level_sounds::LevelSound;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Dim, Game, item_abilities::ability};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispenser::DispenseScope;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispenser::DispenseSource;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispenser::SUCCESS;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispenser::Target;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispenser::optional_event;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispenser::set_success;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::dispenser::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::SoundSource;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::ops::ControlFlow;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    Result,
    block_entities::BlockEntityIdentity,
    collisions::SceneCollisions,
    containers::BlockEntity,
    items::{ItemStack, Items},
    mobs::{Species, equip::equippable, sulfur_cube},
    particles::{ParticleOptions, ParticleType},
    projectiles::{Projectile, ProjectileKind},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::{
    geometry::{Aabb, Direction, Vec3},
    position::BlockPos,
    random::RandomSource,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_gameplay::dispense_rules::{
    CLICK, Dispensed, FAIL, Pos, Slot, body_equipment_species, equip_sound, mob_set_slot,
    mob_slot_item, offset, tipped_effect,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_gameplay::dispense_rules::{
    LAUNCH, block_box, data_value, mob_can_dispenser_equip, mob_can_use_slot, overlaps,
    section_order, shrink, step, y_rot,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_world::{
    fluid::FluidKind,
    level::{PlaceContext, PlacementSource},
    registry::BlockStateId,
};

impl Game<'_> {
    /// The brush behaviour: the first armadillo in front that `brushOffScute`s.
    pub(in crate::game) fn dispense_brush(
        &mut self,
        pos: Pos,
        facing: Direction,
        stack: ItemStack,
    ) -> Dispensed {
        let target = offset(pos, facing);
        let armadillos: Vec<usize> = self
            .targets_at(target)
            .into_iter()
            .filter_map(|t| match t {
                Target::Mob(i) if self.mobs[i].species() == Species::Armadillo => Some(i),
                _ => None,
            })
            .collect();
        for i in armadillos {
            if self.mobs[i].is_baby() {
                continue;
            }
            let at = self.mobs[i].position;
            let drops = self.sequence_loot(
                "minecraft:brush/armadillo",
                "minecraft:brush/armadillo",
                Some(&stack),
            );
            for d in drops {
                self.spawn_at_location(at, 0.0, d);
            }
            self.audio.0.play(
                "minecraft:entity.armadillo.brush",
                Some([at.x, at.y, at.z]),
                1.0,
                1.0,
            );
            let who = sunrise_client::mobs::Who::Mob(self.mobs[i].id);
            let ctx = self.who_context(Some(who), None);
            self.post_game_event("minecraft:entity_interact", [at.x, at.y, at.z], ctx);
            return (self.hurt_and_break(stack, 16), optional_event("brush"));
        }
        set_success("brush", false);
        (Some(stack), FAIL)
    }
    /// `EquipmentDispenseItemBehavior.dispenseEquipment`: the first living
    /// entity in front that `canEquipWithDispenser` gets one of the item.
    pub(in crate::game) fn dispense_equipment(
        &mut self,
        pos: Pos,
        facing: Direction,
        stack: &mut ItemStack,
    ) -> bool {
        // The stack's own `equippable`: its slot and allowed entities, and `dispensable`.
        let Some(equippable) = equippable(self.items(), stack) else {
            return false;
        };
        let default = (self.items().get(&stack.item)).is_some_and(|info| info.equip_dispensable);
        let dispensable = sunrise_client::item_sync::dispensable(stack, default);
        let Some(slot) = Slot::from_name(equippable.slot).filter(|_| dispensable) else {
            return false;
        };
        // Equippable.equipSound defaults to the generic armour sound.
        let sound = (self.items().get(&stack.item))
            .and_then(|info| info.equip_sound.clone())
            .or_else(|| Some("minecraft:item.armor.equip_generic".into()));
        let front = offset(pos, facing);
        let one = ItemStack {
            count: 1,
            ..stack.clone()
        };
        // `Equippable.canBeEquippedBy`, then the item's parts (`canEquip`) with the stack converted
        // once, for the entity in front.
        let targets = self.targets_at(front);
        let prepared = self.equip_stack(stack);
        let allows = |entity: u32, id: &str| {
            let data = equippable.allows(id);
            self.can_equip(&prepared, equippable.slot, entity, id, data)
        };
        // LivingEntity.canEquipWithDispenser: alive, the slot usable, the
        // item's allowed entities, the slot empty and the dispenser rule.
        let target = targets.into_iter().find(|t| match *t {
            // Player.canUseSlot: every slot; the armour and the off hand are
            // the ones an equippable item can reach.
            Target::Player => {
                allows(self.own_entity_id() as u32, "minecraft:player")
                    && match slot {
                        Slot::OffHand => self.offhand.is_none(),
                        _ => slot.armor_index().is_some_and(|i| self.armor[i].is_none()),
                    }
            }
            Target::Guest(conn) => self
                .lan
                .as_ref()
                .and_then(|h| h.hub.guest(conn))
                .is_some_and(|g| {
                    allows(g.entity_id as u32, "minecraft:player")
                        && match slot {
                            Slot::OffHand => g.offhand.is_none(),
                            _ => slot.armor_index().is_some_and(|i| g.armor[i].is_none()),
                        }
                }),
            // ArmorStand.canUseSlot: not a locked slot, and a hand needs arms.
            Target::Stand(i) => slot.stand_index().is_some_and(|s| {
                let stand = &self.armor_stands[i];
                allows(stand.id, "minecraft:armor_stand")
                    && !stand.is_disabled(s)
                    && stand.equipment[s].is_none()
            }),
            Target::Mob(i) => {
                let m = &self.mobs[i];
                m.alive()
                    && mob_can_use_slot(m, slot)
                    && allows(m.id, &m.kind.id)
                    && mob_slot_item(m, slot).is_none()
                    && mob_can_dispenser_equip(m, slot)
            }
        });
        let Some(target) = target else {
            return false;
        };
        stack.count -= 1;
        // setItemSlot: onEquipItem plays the equip sound and the equip event.
        let (at, sound) = match target {
            Target::Guest(conn) => {
                // The guest's own turn: their state in place, so the slot, the
                // sound, the game event and the equipment sync run as for the host.
                let done = self
                    .for_player(super::commands::Target::Guest(conn), |g| {
                        g.put_on_player(slot, &one)
                    })
                    .is_some();
                if !done {
                    stack.count += 1;
                    return false;
                }
                self.native_block_dispense_armor(
                    self.dim.id(),
                    [pos.0, pos.1, pos.2],
                    &stack.item,
                    "player",
                );
                return true;
            }
            Target::Player => {
                self.put_on_player(slot, &one);
                self.native_block_dispense_armor(
                    self.dim.id(),
                    [pos.0, pos.1, pos.2],
                    &stack.item,
                    "player",
                );
                return true;
            }
            Target::Stand(i) => {
                let stand = &mut self.armor_stands[i];
                if let Some(s) = slot.stand_index() {
                    stand.equipment[s] = Some(one);
                }
                (Some(stand.position), sound)
            }
            Target::Mob(i) => {
                let m = &mut self.mobs[i];
                mob_set_slot(m, slot, one);
                // setGuaranteedDrop / setPersistenceRequired.
                m.persistent = true;
                let sound = if m.silent {
                    None
                } else {
                    equip_sound(m, slot, sound)
                };
                (Some(m.position), sound)
            }
        };
        if let Some(at) = at {
            if let Some(sound) = sound {
                self.audio
                    .0
                    .play(&sound, Some([at.x, at.y, at.z]), 1.0, 1.0);
            }
            let who = match target {
                Target::Mob(i) => Some(sunrise_client::mobs::Who::Mob(self.mobs[i].id)),
                Target::Player | Target::Guest(_) | Target::Stand(_) => None,
            };
            let ctx = self.who_context(who, None);
            self.post_game_event("minecraft:equip", [at.x, at.y, at.z], ctx);
        }
        self.native_block_dispense_armor(
            self.dim.id(),
            [pos.0, pos.1, pos.2],
            &stack.item,
            match target {
                Target::Stand(_) => "armor_stand",
                Target::Mob(_) => "mob",
                Target::Player | Target::Guest(_) => "player",
            },
        );
        true
    }
    pub(in crate::game) fn dispense_glowstone(
        &mut self,
        pos: Pos,
        facing: Direction,
        stack: ItemStack,
    ) -> Result<Dispensed> {
        let target = offset(pos, facing);
        let state = self.block_at(target);
        if &*self.dispenser_block_name(target) == "minecraft:respawn_anchor" {
            let charge = self
                .prop(state, "charges")
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(0);
            if charge != 4 {
                if let Some(next) =
                    self.with_props(state, &[("charges", &(charge + 1).to_string())])
                {
                    self.level_set([target.0, target.1, target.2], next)?;
                    // RespawnAnchorBlock.charge: Context.of(null, newState).
                    self.post_block_game_event(
                        "minecraft:block_change",
                        [target.0, target.1, target.2],
                        None,
                        Some(next),
                    );
                }
                self.audio.0.play(
                    "minecraft:block.respawn_anchor.charge",
                    Some(Self::center(target)),
                    1.0,
                    1.0,
                );
                return Ok((shrink(stack), CLICK));
            }
            return Ok((Some(stack), FAIL));
        }
        Ok((self.default_execute(pos, facing, stack), CLICK))
    }
    pub(in crate::game) fn dispense_honeycomb(
        &mut self,
        pos: Pos,
        facing: Direction,
        stack: ItemStack,
    ) -> Result<Dispensed> {
        let target = offset(pos, facing);
        let state = self.block_at(target);
        // HoneycombItem.getWaxed, as the player's honeycomb waxes.
        let Some(waxed) = self.honeycomb_waxed(state) else {
            return Ok((
                self.default_execute(pos, facing, stack),
                optional_event("honeycomb"),
            ));
        };
        set_success("honeycomb", true);
        self.level_set([target.0, target.1, target.2], waxed)?;
        self.level_events
            .push(sunrise_world::level::LevelEvent::LevelEvent {
                id: 3003,
                pos: Self::pos_of(target),
                data: 0,
                source: None,
            });
        self.process_level_events()?;
        self.audio.0.play(
            "minecraft:item.honeycomb.wax_on",
            Some(Self::center(target)),
            1.0,
            1.0,
        );
        Ok((shrink(stack), CLICK))
    }
    pub(in crate::game) fn dispense_potion(
        &mut self,
        pos: Pos,
        facing: Direction,
        stack: ItemStack,
    ) -> Result<Dispensed> {
        let target = offset(pos, facing);
        let water = stack.potion() == Some("minecraft:water");
        let name = self.dispenser_block_name(target);
        if !water || !self.in_block_tag("minecraft:convertible_to_mud", &name) {
            // The inner default behaviour dispenses, then the potion's own
            // events play on top.
            return Ok((self.dispense_default_in_level(pos, facing, stack), CLICK));
        }
        // The server picks the splash positions with the level random.
        for _ in 0..5 {
            let r = &mut self.world.random;
            let p = [
                f64::from(pos.0) + r.next_double(),
                f64::from(pos.1 + 1),
                f64::from(pos.2) + r.next_double(),
            ];
            self.particles
                .0
                .add(&ParticleOptions::simple(ParticleType::Splash), p, [0.0; 3]);
        }
        self.audio.0.play(
            "minecraft:item.bottle.empty",
            Some(Self::center(pos)),
            1.0,
            1.0,
        );
        self.post_block_game_event("minecraft:fluid_place", [pos.0, pos.1, pos.2], None, None);
        if let Some(mud) = self.pack.assets.registry.default_state("minecraft:mud") {
            self.level_set([target.0, target.1, target.2], mud)?;
        }
        let bottle = ItemStack::new("minecraft:glass_bottle", 1);
        Ok((
            self.consume_with_remainder(pos, facing, stack, bottle),
            CLICK,
        ))
    }
    pub(in crate::game) fn dispense_tnt(
        &mut self,
        pos: Pos,
        facing: Direction,
        stack: ItemStack,
    ) -> Dispensed {
        if !self
            .ui
            .rules
            .bool_at(sunrise_client::gamerules::vanilla::TNT_EXPLODES)
        {
            set_success("tnt", false);
            return (Some(stack), FAIL);
        }
        let front = offset(pos, facing);
        if self.sulfur_cube_swallow(front, &stack) {
            return (shrink(stack), optional_event("tnt"));
        }
        let target = Self::pos_of(front);
        if !self.tnt_prime_allowed(target) {
            set_success("tnt", false);
            return (Some(stack), FAIL);
        }
        set_success("tnt", true);
        // new PrimedTnt(level, x + 0.5, y, z + 0.5, null), its sound and ENTITY_PLACE.
        let rot = self.world.random.next_double() * f64::from(std::f32::consts::TAU);
        self.add_primed_tnt(target, rot, 80, None, None);
        self.audio.0.play(
            "minecraft:entity.tnt.primed",
            Some([
                f64::from(target.x) + 0.5,
                f64::from(target.y),
                f64::from(target.z) + 0.5,
            ]),
            1.0,
            1.0,
        );
        self.post_block_game_event(
            "minecraft:entity_place",
            [front.0, front.1, front.2],
            None,
            None,
        );
        (shrink(stack), CLICK)
    }
    /// The dispenser level events: 1000 click, 1001 fail, 1002 launch, 1004
    /// firework shoot, 1018 blaze shoot and 2000 smoke shot out of the front.
    pub(in crate::game) fn dispenser_event(&mut self, id: i32, pos: Pos, data: i32) {
        super::dispense_fixture::record(|| format!("L {id} {},{},{} {data}", pos.0, pos.1, pos.2));
        let c = Self::center(pos);
        match id {
            1000 => self
                .audio
                .0
                .play("minecraft:block.dispenser.dispense", Some(c), 1.0, 1.0),
            1001 => self
                .audio
                .0
                .play("minecraft:block.dispenser.fail", Some(c), 1.0, 1.2),
            1002 => self
                .audio
                .0
                .play("minecraft:block.dispenser.launch", Some(c), 1.0, 1.2),
            1004 => self
                .audio
                .0
                .play("minecraft:entity.firework_rocket.shoot", Some(c), 1.0, 1.2),
            // LevelEventHandler runs on the client: its pitch draws come from
            // the client level's random, never the server stream.
            1018 => {
                let r = self.particles.0.random();
                let pitch = (r.next_float() - r.next_float()) * 0.2 + 1.0;
                self.audio
                    .0
                    .play("minecraft:entity.blaze.shoot", Some(c), 2.0, pitch);
            }
            1051 => {
                let pitch = 0.4 / (self.particles.0.random().next_float() * 0.4 + 0.8);
                self.audio
                    .0
                    .play("minecraft:entity.wind_charge.throw", Some(c), 0.5, pitch);
            }
            1049 => self
                .audio
                .0
                .play("minecraft:block.crafter.craft", Some(c), 1.0, 1.0),
            1050 => self
                .audio
                .0
                .play("minecraft:block.crafter.fail", Some(c), 1.0, 1.0),
            // LevelEventHandler.shootParticles: smoke for dispensers, white
            // smoke out of a crafter's front.
            2000 | 2010 => {
                let particle = if id == 2010 {
                    ParticleType::WhiteSmoke
                } else {
                    ParticleType::Smoke
                };
                let s = match data {
                    0 => [0, -1, 0],
                    1 => [0, 1, 0],
                    2 => [0, 0, -1],
                    3 => [0, 0, 1],
                    4 => [-1, 0, 0],
                    _ => [1, 0, 0],
                };
                for _ in 0..10 {
                    let r = self.particles.0.random();
                    let pow = r.next_double() * 0.2 + 0.01;
                    let (sx, sy, sz) = (f64::from(s[0]), f64::from(s[1]), f64::from(s[2]));
                    let p = [
                        f64::from(pos.0)
                            + sx * 0.6
                            + 0.5
                            + sx * 0.01
                            + (r.next_double() - 0.5) * sz * 0.5,
                        f64::from(pos.1)
                            + sy * 0.6
                            + 0.5
                            + sy * 0.01
                            + (r.next_double() - 0.5) * sy * 0.5,
                        f64::from(pos.2)
                            + sz * 0.6
                            + 0.5
                            + sz * 0.01
                            + (r.next_double() - 0.5) * sx * 0.5,
                    ];
                    let v = [
                        sx * pow + r.next_gaussian() * 0.01,
                        sy * pow + r.next_gaussian() * 0.01,
                        sz * pow + r.next_gaussian() * 0.01,
                    ];
                    self.particles
                        .0
                        .add(&ParticleOptions::simple(particle), p, v);
                }
            }
            _ => {}
        }
    }
    pub(in crate::game) fn shear_beehive(&mut self, target: Pos) -> Result<bool> {
        let state = self.block_at(target);
        let name = self.dispenser_block_name(target);
        let level = self
            .prop(state, "honey_level")
            .and_then(|v| v.parse::<i32>().ok());
        if !self.in_block_tag("minecraft:beehives", &name) || level.is_none_or(|l| l < 5) {
            return Ok(false);
        }
        self.audio.0.play(
            "minecraft:block.beehive.shear",
            Some(Self::center(target)),
            1.0,
            1.0,
        );
        // BeehiveBlock.dropHoneycomb (harvest/beehive), then
        // releaseBeesAndResetHoneyLevel and the shear event.
        let p = [target.0, target.1, target.2];
        for stack in self.sequence_loot(
            "minecraft:harvest/beehive",
            "minecraft:harvest/beehive",
            None,
        ) {
            self.pop_resource(p, stack);
        }
        if let Some(reset) = self.with_props(state, &[("honey_level", "0")]) {
            self.level_set(p, reset)?;
        }
        self.dispenser_empty_hive(target, state);
        self.post_block_game_event("minecraft:shear", p, None, None);
        Ok(true)
    }
    /// `ShearsDispenseItemBehavior.tryShearEntity`: the first live, ready
    /// `Shearable` in the block is sheared (an entity part's `IShearable` answer comes first).
    pub(in crate::game) fn shear_entity(&mut self, target: Pos) -> Result<bool> {
        let shears = ItemStack::new("minecraft:shears", 1);
        for t in self.targets_at(target) {
            // Entity.shearOffAllLeashConnections comes first, for any entity.
            if self.shear_off_leashes(t) {
                return Ok(true);
            }
            let Target::Mob(i) = t else { continue };
            let part = self.part_shearable(i, None, &shears);
            let m = &self.mobs[i];
            if !m.alive() || part.is_some_and(|(_, shearable)| !shearable) {
                continue;
            }
            let (at, entity_id, kind) = (m.position, m.id, m.kind.id.clone());
            let sound_at = Some([at.x, at.y, at.z]);
            // shear(level, BLOCKS, tool): level.playSound(null, this, sound, BLOCKS, 1, 1).
            let shear_sound = |name: &'static str| {
                LevelSound::new(name, [at.x, at.y, at.z], 1.0, 1.0).in_source(SoundSource::Blocks)
            };
            match m.species() {
                _ if let Some((part, _)) = part => {
                    self.part_shear(i, part, None, &shears);
                }
                Species::SulfurCube if sulfur_cube::has_body(m) => {
                    // SulfurCube.shear: the swallowed block comes out at the
                    // passenger attachment, and nothing is swallowed for 5 s.
                    let height = f64::from(m.height());
                    let id = m.id;
                    if let Some(body) = self.mobs[i].take_body_equipment() {
                        self.spawn_at_location(at, height, body);
                    }
                    // setItemSlot(BODY, EMPTY): an unequip.
                    let ctx = self.who_context(Some(sunrise_client::mobs::Who::Mob(id)), None);
                    self.post_game_event("minecraft:unequip", [at.x, at.y, at.z], ctx);
                    self.mobs[i].extra.ints[3] = sulfur_cube::PICKUP_TIMER;
                    self.audio
                        .0
                        .play("minecraft:entity.sulfur_cube.eject", sound_at, 1.0, 1.0);
                }
                Species::Sheep if !m.extra.sheared && !m.is_baby() => {
                    let color =
                        sunrise_client::mobs::render::DYE_NAMES[m.extra.color as usize & 15];
                    self.play_bound_sound(
                        shear_sound("minecraft:entity.sheep.shear"),
                        entity_id,
                        None,
                    );
                    let drops = self.sequence_loot(
                        "minecraft:shearing/sheep",
                        &format!("minecraft:shearing/sheep/{color}"),
                        Some(&shears),
                    );
                    for stack in drops {
                        for _ in 0..stack.count {
                            let one = ItemStack {
                                count: 1,
                                ..stack.clone()
                            };
                            self.spawn_at_location(at, 1.0, one);
                            // Sheep.shear nudges each item with the sheep's own random.
                            let r = &mut self.mobs[i].random;
                            let push = [
                                f64::from((r.next_float() - r.next_float()) * 0.1),
                                f64::from(r.next_float() * 0.05),
                                f64::from((r.next_float() - r.next_float()) * 0.1),
                            ];
                            if let Some(item) = self.just_spawned_item() {
                                item.velocity = Vec3::new(
                                    item.velocity.x + push[0],
                                    item.velocity.y + push[1],
                                    item.velocity.z + push[2],
                                );
                            }
                        }
                    }
                    self.mobs[i].extra.sheared = true;
                }
                Species::Bogged if !m.extra.sheared => {
                    let height = f64::from(m.height());
                    self.play_bound_sound(
                        shear_sound("minecraft:entity.bogged.shear"),
                        entity_id,
                        None,
                    );
                    let drops = self.sequence_loot(
                        "minecraft:shearing/bogged",
                        "minecraft:shearing/bogged",
                        Some(&shears),
                    );
                    for stack in drops {
                        self.spawn_at_location(at, height, stack);
                    }
                    self.mobs[i].extra.sheared = true;
                }
                Species::SnowGolem if !m.extra.sheared => {
                    let eye = m.eye_height();
                    self.play_bound_sound(
                        shear_sound("minecraft:entity.snow_golem.shear"),
                        entity_id,
                        None,
                    );
                    self.mobs[i].extra.sheared = true;
                    let drops = self.sequence_loot(
                        "minecraft:shearing/snow_golem",
                        "minecraft:shearing/snow_golem",
                        Some(&shears),
                    );
                    for stack in drops {
                        self.spawn_at_location(at, eye, stack);
                    }
                }
                // CopperGolem.shear: the antenna's flower drops 1.5 up.
                Species::CopperGolem if self.golem_shearable(i) => {
                    self.play_bound_sound(
                        shear_sound("minecraft:entity.copper_golem.shear"),
                        entity_id,
                        None,
                    );
                    let antenna = self.mobs[i].saddle_item();
                    let id = self.mobs[i].id;
                    // setItemSlot(EQUIPMENT_SLOT_ANTENNA, EMPTY): an unequip.
                    self.mobs[i].set_saddle(None);
                    let ctx = self.who_context(Some(sunrise_client::mobs::Who::Mob(id)), None);
                    self.post_game_event("minecraft:unequip", [at.x, at.y, at.z], ctx);
                    if let Some(antenna) = antenna {
                        self.spawn_at_location(at, 1.5, antenna);
                    }
                }
                Species::Mooshroom if !m.is_baby() => {
                    self.shear_mooshroom(i, None, &shears);
                }
                _ => continue,
            }
            self.native_block_shear_entity(
                self.dim.id(),
                [target.0, target.1, target.2],
                entity_id,
                &kind,
            );
            self.post_block_game_event(
                "minecraft:shear",
                [target.0, target.1, target.2],
                None,
                None,
            );
            return Ok(true);
        }
        Ok(false)
    }
    /// `Entity.shearOffAllLeashConnections(null)`: the entity's own lead and
    /// every lead held by it (within a 32 block box around its centre) drop;
    /// a shear game event at the entity and a snip at its block follow.
    pub(in crate::game::client_side) fn shear_off_leashes(&mut self, t: Target) -> bool {
        use sunrise_client::mobs::{Who, leash::Holder};
        let (holder, centre, own) = match t {
            Target::Mob(i) => {
                let m = &self.mobs[i];
                let b = m.bounds();
                (Holder::Mob(m.id), b.center(), Some(i))
            }
            Target::Player => {
                let Some(v) = self.player_view() else {
                    return false;
                };
                (Holder::Player, v.bounds.center(), None)
            }
            Target::Guest(_) | Target::Stand(_) => return false,
        };
        let area = Aabb::new(centre, centre).inflate(Vec3::new(16.0, 16.0, 16.0));
        let mut leashed: Vec<(u32, usize)> = self
            .mobs
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                !m.removed && m.extra.leash == Some(holder) && overlaps(m.bounds(), area)
            })
            .map(|(j, m)| (m.id, j))
            .collect();
        leashed.sort_unstable();
        let own_leashed = own.is_some_and(|i| self.mobs[i].extra.leash.is_some());
        if leashed.is_empty() && !own_leashed {
            return false;
        }
        // Leashable.dropLeash: a lead item at the leashed entity's feet.
        let drop = |game: &mut Self, j: usize| {
            if sunrise_client::mobs::leash::remove(&mut game.mobs[j]).is_some() {
                let at = game.mobs[j].position;
                game.spawn_at_location(at, 0.0, ItemStack::new("minecraft:lead", 1));
            }
        };
        if let Some(i) = own.filter(|_| own_leashed) {
            drop(self, i);
        }
        for (_, j) in leashed {
            drop(self, j);
        }
        let (at, who) = match t {
            Target::Mob(i) => {
                let m = &self.mobs[i];
                (m.position, Who::Mob(m.id))
            }
            _ => (
                self.player.as_ref().map_or(Vec3::ZERO, |p| p.position),
                Who::Player,
            ),
        };
        let ctx = self.who_context(Some(who), None);
        self.post_game_event("minecraft:shear", [at.x, at.y, at.z], ctx);
        let block = [at.x.floor() + 0.5, at.y.floor() + 0.5, at.z.floor() + 0.5];
        self.audio
            .0
            .play("minecraft:item.shears.snip", Some(block), 1.0, 1.0);
        true
    }
    /// `SulfurCubeBlockDispenseItemBehavior.dispenseBlock`: the first sulfur
    /// cube in the block that `equipItem`s the stack swallows one.
    pub(in crate::game) fn sulfur_cube_swallow(&mut self, target: Pos, stack: &ItemStack) -> bool {
        let cubes: Vec<usize> = self
            .targets_at(target)
            .into_iter()
            .filter_map(|t| match t {
                Target::Mob(i) if self.mobs[i].species() == Species::SulfurCube => Some(i),
                _ => None,
            })
            .collect();
        for i in cubes {
            let m = &self.mobs[i];
            if m.is_baby() || m.extra.body_id() == Some(&*stack.item) {
                continue;
            }
            let (at, height) = (m.position, f64::from(m.height()));
            // The swallowed block comes out on top (the passenger attachment).
            if let Some(old) = self.mobs[i].take_body_equipment() {
                self.spawn_at_location(at, height, old);
            }
            let m = &mut self.mobs[i];
            m.extra.set_body_stack(Some(ItemStack {
                count: 1,
                ..stack.clone()
            }));
            m.persistent = true;
            let (at, id) = (m.position, m.id);
            // setItemSlot: a body item without `equippable` posts UNEQUIP.
            let ctx = self.who_context(Some(sunrise_client::mobs::Who::Mob(id)), None);
            self.post_game_event("minecraft:unequip", [at.x, at.y, at.z], ctx);
            self.audio.0.play(
                "minecraft:entity.sulfur_cube.absorb",
                Some([at.x, at.y, at.z]),
                1.0,
                1.0,
            );
            return true;
        }
        false
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use super::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::dispenser::tests::Fits;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::dispenser::tests::Front;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::dispenser::tests::Heard;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::dispenser::tests::Item;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::dispenser::tests::Probe;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::dispenser::tests::Tailor;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::dispenser::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use std::sync::{Arc, atomic::AtomicI32, mpsc};
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_client::lan::{
        Joined, NetConfig, PlayerSender, guest::Guest, hub::HubConfig,
    };
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_client::mobs::Mob;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use sunrise_protocol::packets::configuration::ClientInformation;

    /// `DispenserBlock.getDispenseMethod`: the block the dispenser faces
    /// answers first and wins for any item; with no answer from it the item's
    /// part decides; with neither, vanilla drops one item as before.
    #[test]
    #[ignore = "daily"]
    fn the_faced_block_answers_before_the_item_and_vanilla_stands_when_no_part_listens() {
        use std::sync::Arc;
        use sunrise_client::chunk::Chunk;

        let heard = Heard::default();
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "dispense_probe",
            Box::new(Probe(heard.clone())),
        )]
        .into();
        super::super::api_values::install_parts(&mods).unwrap();
        mod_platform::testing::with_mods(mods, || {
            super::super::native_entity_refs::tests::with_game(|game| {
                game.options.dedicated = Some(Vec::new());
                game.world
                    .chunks
                    .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
                let stone = game
                    .pack
                    .assets
                    .registry
                    .default_state("minecraft:stone")
                    .unwrap();
                let air = game
                    .pack
                    .assets
                    .registry
                    .default_state("minecraft:air")
                    .unwrap();
                // The dispenser at (5, 64, 5) faces stone.
                game.world.set(6, 64, 5, stone).unwrap();
                game.audio.0.capture = Some(Vec::new());
                let dispense = |game: &mut Game<'_>, item: &str| {
                    game.dispense_item((5, 64, 5), Direction::East, ItemStack::new(item, 1))
                        .unwrap()
                        .continue_value()
                        .unwrap()
                };

                // 1. The faced block answers for an item with no part of its own: it wins, and
                //    the slot is emptied with the event it chose.
                assert_eq!(dispense(game, "minecraft:dirt"), None);
                assert_eq!(heard.take(), [("block", [5, 64, 5], stone.0)]);
                assert_eq!(
                    game.audio.0.capture.as_ref().unwrap().last().unwrap().event,
                    "minecraft:block.dispenser.fail"
                );
                assert!(
                    game.dropped.is_empty(),
                    "the block's answer owns the dispense"
                );
                game.audio.0.capture.as_mut().unwrap().clear();

                // 2. The faced block answers for an item that has its own part: its answer wins
                //    and the item's part is never asked.
                assert_eq!(dispense(game, "minecraft:cobblestone"), None);
                assert_eq!(heard.take(), [("block", [5, 64, 5], stone.0)]);
                assert_eq!(
                    game.audio.0.capture.as_ref().unwrap().last().unwrap().event,
                    "minecraft:block.dispenser.fail"
                );
                assert!(game.dropped.is_empty());
                game.audio.0.capture.as_mut().unwrap().clear();

                // 3. No block answer: the item's part decides.
                assert_eq!(dispense(game, "minecraft:stone"), None);
                assert_eq!(
                    heard.take(),
                    [("block", [5, 64, 5], stone.0), ("item", [5, 64, 5], 0)]
                );
                assert_eq!(
                    game.audio.0.capture.as_ref().unwrap().last().unwrap().event,
                    "minecraft:block.dispenser.dispense"
                );
                assert!(game.dropped.is_empty());
                game.audio.0.capture.as_mut().unwrap().clear();

                // 4. Neither the block in front nor the item has a part: vanilla drops one.
                game.world.set(6, 64, 5, air).unwrap();
                assert_eq!(dispense(game, "minecraft:stick"), None);
                assert!(heard.take().is_empty());
                assert_eq!(game.dropped.len(), 1);
                assert_eq!(game.dropped[0].stack, ItemStack::new("minecraft:stick", 1));
                assert_eq!(
                    game.audio.0.capture.as_ref().unwrap().last().unwrap().event,
                    "minecraft:block.dispenser.dispense"
                );
            });
        });
    }
}
