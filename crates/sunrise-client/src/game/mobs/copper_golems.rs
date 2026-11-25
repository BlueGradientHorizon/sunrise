//! Copper golems on the game side: what their brain (`mobs::transport_items`)
//! does to chests (opener counts, the lid block events, slots), the openers
//! recheck, turning into statues, and the hand-over, waxing, scraping and
//! shearing interactions.
#[allow(unused_imports)]
use super::Game;
#[allow(unused_imports)]
use super::level_sounds::LevelSound;
#[allow(unused_imports)]
use mod_api::SoundSource;
#[allow(unused_imports)]
use sunrise_client::{
    containers::BlockEntity,
    items::ItemStack,
    mobs::{
        Mob, Species,
        transport_items::{held, set_held},
    },
};
#[allow(unused_imports)]
use sunrise_core::random::RandomSource;
#[allow(unused_imports)]
use sunrise_world::chunk::BlockState;

#[allow(unused_imports)]
use sunrise_core::nbt::{Compound, Tag};

/// `ContainerOpenersCounter.CHECK_TICK_DELAY`.
pub(in crate::game) const RECHECK_TICKS: i64 = 5;

impl Game<'_> {
    /// `ContainerOpenersCounter.recheckOpeners` every five ticks: the golems
    /// whose opened chest is this one, within its range plus 4; when none
    /// is left the chest closes (a double chest the golem left open).
    pub(in crate::game) fn recheck_golem_openers(&mut self) {
        if self.world.game_time % RECHECK_TICKS != 0 {
            return;
        }
        let open: Vec<(i32, i32, i32)> = self.world.chest_openers.keys().copied().collect();
        for key in open {
            let p = [key.0, key.1, key.2];
            let other = self.chest_other_half(key).map(|o| [o.0, o.1, o.2]);
            let range = self.world.chest_openers.get(&key).map_or(0.0, |o| o.1) + 4.0;
            let area = sunrise_core::geometry::Aabb::new(
                sunrise_core::geometry::Vec3::new(
                    f64::from(p[0]) - range,
                    f64::from(p[1]) - range,
                    f64::from(p[2]) - range,
                ),
                sunrise_core::geometry::Vec3::new(
                    f64::from(p[0]) + 1.0 + range,
                    f64::from(p[1]) + 1.0 + range,
                    f64::from(p[2]) + 1.0 + range,
                ),
            );
            let count = self
                .mobs
                .iter()
                .filter(|m| {
                    m.species() == Species::CopperGolem
                        && !m.removed
                        && m.extra
                            .opened_chest
                            .is_some_and(|o| o == p || Some(o) == other)
                        && m.bounds().intersects(area)
                })
                .count() as i32;
            let before = self.world.chest_openers.get(&key).map_or(0, |o| o.0);
            if count == before {
                continue;
            }
            // `recheckOpeners`: the game event has no source.
            if count == 0 {
                self.world.chest_openers.remove(&key);
                self.golem_chest_sound(p, false);
                self.post_block_game_event("minecraft:container_close", p, None, None);
            } else if let Some(o) = self.world.chest_openers.get_mut(&key) {
                o.0 = count;
            }
            self.block_entity_event(p, 1, count);
            self.lan_block_event(p, 1, count);
        }
    }
    /// `ChestBlockEntity.playSound` for a chest the recheck closed.
    pub(in crate::game::client_side) fn golem_chest_sound(&mut self, p: [i32; 3], open: bool) {
        let name = self
            .res
            .facts()
            .get(self.world.block(p[0], p[1], p[2]))
            .name
            .clone();
        let base = name
            .trim_start_matches("minecraft:")
            .trim_start_matches("waxed_");
        let kind = if !base.ends_with("copper_chest") {
            "chest"
        } else if base.starts_with("weathered") {
            "copper_chest_weathered"
        } else if base.starts_with("oxidized") {
            "copper_chest_oxidized"
        } else {
            "copper_chest"
        };
        let side = if open { "open" } else { "close" };
        let at = [
            f64::from(p[0]) + 0.5,
            f64::from(p[1]) + 0.5,
            f64::from(p[2]) + 0.5,
        ];
        let pitch = self.world.random.next_float() * 0.1 + 0.9;
        self.server_play_sound(&format!("minecraft:block.{kind}.{side}"), at, 0.5, pitch);
    }
    /// `CopperGolem.turnToStatue`, which its weathering asked for.
    pub(in crate::game) fn golem_become_statue(&mut self, id: u32) {
        let Some(i) = self.mobs.iter().position(|m| m.id == id && !m.removed) else {
            return;
        };
        let statue_at = self.with_entity_held_out(id, |game| {
            let mut m = game.mobs[i].clone();
            let became_statue = game.golem_turn_to_statue(&mut m);
            let position = m.position.components();
            if let Some(i) = game.mobs.iter().position(|x| x.id == id && !x.removed) {
                game.mobs.update(i, |owner| {
                    sunrise_client::join_seq::store_back(owner, m);
                });
            }
            became_statue.then_some(position)
        });
        if let Some(at) = statue_at {
            self.remove_mob_now(id, false);
            self.server_play_sound("minecraft:entity.copper_golem_become_statue", at, 1.0, 1.0);
            if self
                .mob_object(id)
                .is_some_and(|mob| mob.extra.leash.is_some())
            {
                let drop = self
                    .ui
                    .rules
                    .bool_at(sunrise_client::gamerules::vanilla::ENTITY_DROPS);
                self.break_leash(id, drop, false);
            }
        }
    }
    pub(in crate::game::client_side) fn golem_turn_to_statue(&mut self, m: &mut Mob) -> bool {
        let pos = [
            m.position.x.floor() as i32,
            m.position.y.floor() as i32,
            m.position.z.floor() as i32,
        ];
        // The pose comes from the golem's own random (`this.random.nextInt(0, 4)`).
        let pose = ["standing", "sitting", "running", "star"]
            [m.random.next_int_bound(4).unwrap_or(0) as usize];
        // Direction.fromYRot.
        let facing = ["south", "west", "north", "east"]
            [((f64::from(m.yaw) / 90.0 + 0.5).floor() as i32 & 3) as usize];
        let state = BlockState {
            name: "minecraft:oxidized_copper_golem_statue".into(),
            properties: [
                ("facing".to_string(), facing.to_string()),
                ("copper_golem_pose".to_string(), pose.to_string()),
                ("waterlogged".to_string(), "false".to_string()),
            ]
            .into(),
        };
        let registry = self.pack.assets.registry;
        let Some(id) = registry.id(&state) else {
            return false;
        };
        if self.level_set(pos, id).is_err() {
            return false;
        }
        // `createStatue`: the golem's custom name goes into the statue's components.
        let mut components = Compound::new();
        if let Some(name) = self.golem_name_tag(m) {
            components.insert("minecraft:custom_name".into(), name);
        }
        self.world.block_entities.insert(
            (pos[0], pos[1], pos[2]),
            BlockEntity::CopperGolemStatue(components),
        );
        self.golem_drop_equipment(m);
        m.extra.opened_chest = None;
        true
    }
    /// The golem's `CustomName` as a component: the styled name it was loaded
    /// or summoned with while it still matches the plain name, else the plain one.
    pub(in crate::game::client_side) fn golem_name_tag(&self, m: &Mob) -> Option<Tag> {
        let plain = m.custom_name.as_ref()?;
        let saved = self
            .persist
            .raw
            .get(&m.id)
            .and_then(|raw| raw.get("CustomName"))
            .filter(|name| match name {
                Tag::String(s) => s.to_string_lossy() == *plain,
                Tag::Compound(_) => matches!(
                    name.get("text"),
                    Some(Tag::String(s)) if s.to_string_lossy() == *plain
                ),
                _ => false,
            });
        Some(
            saved
                .cloned()
                .unwrap_or_else(|| Tag::String(plain.as_str().into())),
        )
    }
    /// `Entity.setCustomName` of golem `id` with the component `name`: its plain
    /// and styled forms, and the saved compound a later save writes it from.
    pub(in crate::game::client_side) fn name_golem(&mut self, id: u32, name: Tag) {
        let plain = match &name {
            Tag::String(s) => Some(s.to_string_lossy()),
            Tag::Compound(_) => match name.get("text") {
                Some(Tag::String(s)) => Some(s.to_string_lossy()),
                _ => None,
            },
            _ => None,
        };
        if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) {
            m.custom_name = plain;
            m.custom_name_text = Some(sunrise_client::entity_nbt::name_text(&name));
        }
        if let Tag::Compound(raw) = self
            .persist
            .raw
            .entry(id)
            .or_insert_with(|| Tag::Compound(Default::default()))
        {
            raw.insert("CustomName".into(), name);
        }
    }
    /// `golem_preserved_equipment`, each `spawnAtLocation` at the golem's feet
    /// (a golem turning into a statue).
    pub(in crate::game) fn golem_drop_equipment(&mut self, m: &mut Mob) {
        let at = m.position;
        for stack in crate::game::golem_preserved_equipment(m) {
            self.spawn_at_location(at, 0.0, stack);
        }
    }
    // ---- interactions -----------------------------------------------------
    /// `WeatheringCopperGolemStatueBlock.useItemOn`: an axe on an unoxidized
    /// statue turns it back into a golem (`CopperGolemStatueBlockEntity.removeStatue`),
    /// wearing the axe. Weathered statues are scraped by the axe's own
    /// transform instead.
    pub(in crate::game) fn use_golem_statue(
        &mut self,
        pos: [i32; 3],
        held: Option<&ItemStack>,
    ) -> bool {
        if !held.is_some_and(|s| s.item.ends_with("_axe")) {
            return false;
        }
        let registry = self.pack.assets.registry;
        let Some(state) = registry.state(self.world.block(pos[0], pos[1], pos[2])) else {
            return false;
        };
        if state.name != "minecraft:copper_golem_statue" {
            return false;
        }
        // `Direction.toYRot` of the statue's facing.
        let yaw = match state.properties.get("facing").map(String::as_str) {
            Some("west") => 90.0,
            Some("north") => 180.0,
            Some("east") => 270.0,
            _ => 0.0,
        };
        let waterlogged = state
            .properties
            .get("waterlogged")
            .is_some_and(|w| w == "true");
        // initCopperGolem: at the block's feet, facing as the statue does.
        let at = [
            f64::from(pos[0]) + 0.5,
            f64::from(pos[1]),
            f64::from(pos[2]) + 0.5,
        ];
        // `removeStatue`: the golem takes the custom name the statue keeps.
        let name = match self.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
            Some(BlockEntity::CopperGolemStatue(components)) => {
                components.get(&"minecraft:custom_name".into()).cloned()
            }
            _ => None,
        };
        let Some(id) = self.spawn_mob_raw("minecraft:copper_golem", at, true) else {
            return false;
        };
        if let Some(name) = name {
            self.name_golem(id, name);
        }
        if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) {
            m.yaw = yaw;
            m.previous_yaw = yaw;
            m.body_yaw = yaw;
            m.previous_body_yaw = yaw;
            m.head_yaw = yaw;
            m.previous_head_yaw = yaw;
        }
        self.damage_held(1);
        self.server_play_sound("minecraft:entity.copper_golem.spawn", at, 1.0, 1.0);
        // Level.removeBlock(pos, false): air, or the water the statue stood in.
        let replacement = registry.default_state(if waterlogged {
            "minecraft:water"
        } else {
            "minecraft:air"
        });
        if let Some(replacement) = replacement {
            let _ = self.level_set(pos, replacement);
        }
        self.world.block_entities.remove(&(pos[0], pos[1], pos[2]));
        true
    }
    /// `readyForShearing`: the antenna holds a flower shears take.
    pub(in crate::game) fn golem_shearable(&self, index: usize) -> bool {
        self.mobs[index].saddle_item().is_some_and(|a| {
            self.res
                .items()
                .item_tags
                .get("minecraft:shearable_from_copper_golem")
                .is_some_and(|t| t.contains(&*a.item))
        })
    }
}

/// `Mob.dropPreservedEquipment`: the guaranteed drops in `EquipmentSlot`
/// order, the carried stack (main hand) then the antenna flower (saddle),
/// taken off the golem.
pub(in crate::game) fn golem_preserved_equipment(m: &mut Mob) -> Vec<ItemStack> {
    let mut stacks = Vec::new();
    if let Some(stack) = held(m).cloned() {
        set_held(m, None);
        stacks.push(stack);
    }
    if let Some(antenna) = m.saddle_item() {
        m.set_saddle(None);
        stacks.push(antenna);
    }
    stacks
}

#[cfg(test)]
#[path = "copper_golems/lifetime_tests.rs"]
pub(in crate::game) mod lifetime_tests;

#[cfg(test)]
#[path = "tests/golem_gift_tests.rs"]
pub(in crate::game) mod golem_gift_tests;
