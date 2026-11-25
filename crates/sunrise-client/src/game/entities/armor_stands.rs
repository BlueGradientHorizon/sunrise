//! Armor stands: `ArmorStandItem.useOn` placement (snapped to 45 degrees),
//! falling, `interactAt` equipment swaps by clicked height, the two-hit break
//! of `hurtServer` (creative breaks at once), and `ArmorStandRenderer` with
//! the default pose, the hit wobble and humanoid armor layers.
#[allow(unused_imports)]
use super::entity::Entity;
#[allow(unused_imports)]
use super::{Game, InteractTarget};
#[allow(unused_imports)]
use std::{collections::HashMap, sync::Arc};
#[allow(unused_imports)]
use sunrise_assets::resource_source::ResourceSource;
#[allow(unused_imports)]
use sunrise_client::{
    Result,
    cit::{Hand, Kind, Rule},
    entities::ArmorStand,
    items::ItemStack,
    join_seq::JoinSeq,
    mesher::Vertex,
    mobs::{Mob, Part, Species, Who, anim::Poses},
    trims::{TrimOverride, TrimRegistry, TrimTexture},
};
#[allow(unused_imports)]
use sunrise_core::{
    geometry::{Aabb, Vec3},
    random::RandomSource,
};
pub(in crate::game) const TYPE_ID: &str = "minecraft:armor_stand";
/// Build the living-drops input at the loot-source boundary, before equipment pops.
pub(in crate::game::server_side) fn armor_stand_dying_drops<'a>(
    stand: &ArmorStand,
    mob_drops: bool,
    dying: super::creatures::DyingDrops<'a>,
    roll: impl FnOnce(&str) -> Vec<sunrise_world::loot::LootStack>,
) -> (super::creatures::DyingDrops<'a>, Vec<(ItemStack, [f64; 3])>) {
    let stacks = if stand.small || !mob_drops {
        Vec::new()
    } else {
        roll("minecraft:entities/armor_stand")
            .iter()
            .filter(|stack| !stack.is_empty())
            .map(|stack| {
                (
                    super::creatures::from_loot(stack),
                    [stand.position.x, stand.position.y, stand.position.z],
                )
            })
            .collect()
    };
    (dying, stacks)
}

#[cfg(test)]
pub(in crate::game) mod death_loot_tests {
    use super::*;

    #[test]
    fn loot_source_stacks_reach_dying_drops_and_obey_gates() {
        for (small, mob_drops) in [(false, true), (true, true), (false, false), (true, false)] {
            let mut stand = ArmorStand::new(42, Vec3::new(1.5, 64.0, -2.5), 0.0);
            stand.small = small;
            let dying = super::super::creatures::DyingDrops {
                entity: stand.id,
                kind: "minecraft:armor_stand",
                player: None,
                source: None,
                recently_hit: false,
            };
            let mut called = false;
            let (dying, stacks) = armor_stand_dying_drops(&stand, mob_drops, dying, |table| {
                assert_eq!(table, "minecraft:entities/armor_stand");
                called = true;
                // A fake table source; no prepared vanilla resources or Game required.
                let stack = sunrise_world::loot::LootStack {
                    item: "minecraft:stick".into(),
                    count: 3,
                    damage: 0,
                    enchantments: Vec::new(),
                    potion: None,
                    map: false,
                    exploration: None,
                    ominous_amplifier: None,
                    stew_effects: Vec::new(),
                    names: Vec::new(),
                    components: Vec::new(),
                };
                let mut empty = stack.clone();
                empty.count = 0;
                vec![stack, empty]
            });
            assert_eq!(dying.entity, 42);
            assert_eq!(dying.kind, "minecraft:armor_stand");
            assert_eq!(called, !small && mob_drops);
            if !small && mob_drops {
                assert_eq!(stacks.len(), 1);
                assert_eq!(stacks[0].0.item.as_ref(), "minecraft:stick");
                assert_eq!(stacks[0].0.count, 3);
                assert_eq!(stacks[0].1, [1.5, 64.0, -2.5]);
            } else {
                assert!(stacks.is_empty());
            }
        }
    }
}

/// Velocity `AbstractMinecart.push` adds: the cart, and a quarter on the stand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::game) struct CartShove {
    pub(in crate::game) cart_x: f64,
    pub(in crate::game) cart_z: f64,
    pub(in crate::game) stand_x: f64,
    pub(in crate::game) stand_z: f64,
}
/// `ArmorStand.pushEntities` for one cart. `None` when the cart is not the
/// plain rideable kind, the boxes miss, `distanceToSqr` is past 0.2, the
/// stand is a passenger of the cart, or the horizontal gap is under
/// `1.0E-4F` (where `AbstractMinecart.push` does nothing).
pub(in crate::game::server_side) fn armor_stand_cart_shove(
    stand_pos: Vec3,
    stand_box: Aabb,
    cart_pos: Vec3,
    cart_box: Aabb,
    kind: super::minecarts::CartKind,
    cart_carries_stand: bool,
) -> Option<CartShove> {
    if kind != super::minecarts::CartKind::Rideable
        || cart_carries_stand
        || !stand_box.intersects(cart_box)
    {
        return None;
    }
    let (dx, dy, dz) = (
        stand_pos.x - cart_pos.x,
        stand_pos.y - cart_pos.y,
        stand_pos.z - cart_pos.z,
    );
    if dx * dx + dy * dy + dz * dz > 0.2 {
        return None;
    }
    let (xa, za) = super::minecarts::cart_push_from(stand_pos, cart_pos)?;
    Some(CartShove {
        cart_x: -xa,
        cart_z: -za,
        stand_x: xa / 4.0,
        stand_z: za / 4.0,
    })
}
/// `EquipmentLayerRenderer.getColorForLayer`: a dyeable layer takes the stack's
/// `minecraft:dyed_color`, else its optional undyed colour, else zero to skip it.
/// Nondyeable layers stay white. The result uses the renderer's byte order (R and B swapped).
pub(in crate::game) fn layer_color(
    dyeable: bool,
    undyed: Option<u32>,
    stack: Option<&ItemStack>,
) -> u32 {
    let dyed = stack
        .and_then(|s| s.data())
        .and_then(|d| d.dyed_color)
        .map(|c| c as u32 & 0xFF_FFFF);
    if !dyeable {
        return 0xFFFF_FFFF;
    }
    let Some(c) = dyed.or(undyed) else {
        return 0;
    };
    0xFF00_0000 | (c & 0xFF) << 16 | (c & 0xFF00) | (c >> 16) & 0xFF
}

/// Armor stand models and the humanoid layers of every equipment asset.
#[derive(Default)]
pub(in crate::game) struct ArmorStandAssets {
    /// Shared (`Arc`) so the GPU path (`mobs::skin`) keeps one stored copy of each.
    pub main: Option<Arc<Part>>,
    pub small: Option<Arc<Part>>,
    /// (slot model suffix, small) -> model.
    pub layers: HashMap<(&'static str, bool), Arc<Part>>,
    /// The player's armour models by slot (helmet, chestplate, leggings, boots).
    pub player: HashMap<&'static str, Part>,
    /// Asset id -> layer kind -> ordered textures with dyeability and optional undyed colour.
    pub equipment: super::equipment_piece::EquipmentAssets,
    pub extensions: super::client_item_extensions::ArmorExtensions,
    /// asset id -> `trim_overrides`.
    pub trim_overrides: HashMap<String, Vec<TrimOverride>>,
    pub trims: TrimRegistry,
    /// The resources the trim textures are made from.
    pub root: ResourceSource,
}
#[cfg(test)]
impl ArmorStandAssets {
    pub(in crate::game) fn clone_for_tests(&self) -> Self {
        Self {
            main: self.main.clone(),
            small: self.small.clone(),
            layers: self.layers.clone(),
            player: self.player.clone(),
            equipment: self.equipment.clone(),
            extensions: Default::default(),
            trim_overrides: self.trim_overrides.clone(),
            trims: self.trims.clone(),
            root: self.root.clone(),
        }
    }
}
impl ArmorStandAssets {
    pub fn load(
        root: &ResourceSource,
        document: Option<&sunrise_client::mobs::EntityModelDocument>,
    ) -> Self {
        let load = |key: &str| document.and_then(|document| document.model(key));
        let mut layers = HashMap::new();
        let mut player = HashMap::new();
        for slot in ["helmet", "chestplate", "leggings", "boots"] {
            if let Some(part) = load(&format!("minecraft:player#{slot}")) {
                player.insert(slot, part);
            }
            for (small, model) in [(false, "armor_stand"), (true, "armor_stand_small")] {
                if let Some(part) = load(&format!("minecraft:{model}#{slot}")) {
                    layers.insert((slot, small), Arc::new(part));
                }
            }
        }
        for (key, name) in [
            ("minecraft:elytra#main", "elytra"),
            ("minecraft:elytra_baby#main", "elytra_baby"),
        ] {
            if let Some(part) = load(key) {
                player.insert(name, part);
            }
        }
        let mut equipment = HashMap::new();
        let mut trim_overrides = HashMap::new();
        // EquipmentAssetManager reads resolved resources in every namespace.
        for namespace in root.namespaces("assets") {
            let prefix = format!("assets/{namespace}/equipment/");
            for path in root.list(&prefix) {
                let Some(name) = path
                    .strip_prefix(&prefix)
                    .and_then(|path| path.strip_suffix(".json"))
                else {
                    continue;
                };
                let Ok(v) = root
                    .read(&path)
                    .map(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes))
                else {
                    continue;
                };
                let Ok(v) = v else { continue };
                let id = format!("{namespace}:{name}");
                let mut kinds = HashMap::new();
                for kind in ["humanoid", "humanoid_leggings", "humanoid_baby", "wings"] {
                    // Every layer of the kind: leather's overlay is layer 1.
                    let layers: Vec<super::equipment_piece::EquipmentAssetLayer> = v["layers"]
                        [kind]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|layer| {
                            let tex = layer["texture"].as_str().unwrap_or(&id);
                            let (tex_namespace, tex) =
                                tex.split_once(':').unwrap_or(("minecraft", tex));
                            let sprite = match tex_namespace {
                                "minecraft" => format!("entity/equipment/{kind}/{tex}"),
                                other => format!("{other}:entity/equipment/{kind}/{tex}"),
                            };
                            let undyed = layer["dyeable"]["color_when_undyed"]
                                .as_i64()
                                .map(|c| c as u32 & 0xFF_FFFF);
                            super::equipment_piece::EquipmentAssetLayer {
                                texture: sprite,
                                dyeable: layer
                                    .get("dyeable")
                                    .is_some_and(serde_json::Value::is_object),
                                color_when_undyed: undyed,
                                use_player_texture: layer["use_player_texture"]
                                    .as_bool()
                                    .unwrap_or(false),
                            }
                        })
                        .collect();
                    if !layers.is_empty() {
                        kinds.insert(kind.to_owned(), layers);
                    }
                }
                trim_overrides.insert(id.clone(), TrimOverride::list(&v));
                equipment.insert(id, kinds);
            }
        }
        Self {
            main: load("minecraft:armor_stand#main").map(Arc::new),
            small: load("minecraft:armor_stand_small#main").map(Arc::new),
            layers,
            player,
            equipment,
            extensions: Default::default(),
            trim_overrides,
            trims: TrimRegistry::load(root),
            root: root.to_owned(),
        }
    }
}
impl Game<'_> {
    /// `ArmorStandItem.useOn`.
    pub(in crate::game::server_side) fn place_armor_stand(
        &mut self,
        hit: &super::Hit,
        stack: &ItemStack,
    ) -> bool {
        if &*stack.item != "minecraft:armor_stand" {
            return false;
        }
        let Some((pos, mut stand)) = self.armor_stand_spawn(hit, stack) else {
            return false;
        };
        stand.id = self.server.next_entity + 1;
        // EntityType.create(.., tryMoveDown, movedUp): the new stand falls onto what is under it,
        // up to two blocks (a bottom slab, the block under replaced grass).
        let drop = self.drop_offset_from(0.5, 1.975, (pos[0], pos[1], pos[2]), true);
        stand.position.y = f64::from(pos[1]) + drop;
        stand.previous = stand.position;
        // `EntityType.createDefaultStackConfig`: the item's `custom_data`, then its
        // `entity_data` merged over the stand; `ArmorStandItem.useOn` then snaps it
        // to the placement yaw with pitch 0.
        let own =
            sunrise_client::entity_nbt::armor_stand_to_nbt(&stand, [0; 4], None, self.res.items());
        let mut raw = sunrise_client::block_entity_item::stack_configured_entity(
            stack,
            "minecraft:armor_stand",
            &own,
        );
        if let Some(mut loaded) = sunrise_client::entity_nbt::armor_stand_from_nbt(&raw, stand.id) {
            loaded.first_tick = stand.first_tick;
            loaded.yaw = stand.yaw;
            loaded.previous_yaw = stand.previous_yaw;
            loaded.pitch = 0.0;
            loaded.previous = loaded.position;
            stand = loaded;
        }
        if let sunrise_core::nbt::Tag::Compound(map) = &mut raw {
            map.remove(&"UUID".into());
        }
        self.persist.raw.insert(stand.id, raw);
        let foot = stand.position;
        // EntityType.create's spawn yaw, `level.getRandom().nextFloat() * 360`, before
        // ArmorStandItem.useOn turns the stand to face the player.
        self.world.random.next_float();
        self.server.next_entity += 1;
        let (_, uuid) = sunrise_client::mobs::living_random();
        self.persist.uuids.insert(stand.id, uuid);
        self.server_play_sound(
            "minecraft:entity.armor_stand.place",
            [foot.x, foot.y, foot.z],
            0.75,
            0.8,
        );
        // ArmorStandItem.useOn: the stand's ENTITY_PLACE, by the player.
        let ctx = self.player_context(None);
        self.post_game_event("minecraft:entity_place", [foot.x, foot.y, foot.z], ctx);
        self.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
        self.armor_stands.push(stand);
        if !self.creative {
            self.consume_held();
        }
        // ItemStack.useOn counts the use.
        self.award_stat("used", &stack.item, 1);
        // ArmorStandItem.useOn returns SUCCESS: the hand swings.
        self.swing_arm();
        true
    }
    pub(in crate::game::server_side) fn equipment_slot(&self, stack: &ItemStack) -> usize {
        match self
            .res
            .items()
            .get(&stack.item)
            .and_then(|i| i.equip_slot.as_deref())
        {
            Some("feet") => 2,
            Some("legs") => 3,
            Some("chest") => 4,
            Some("head") => 5,
            Some("offhand") => 1,
            _ => 0,
        }
    }
    /// `ArmorStand.interactAt` for the stand under the crosshair, or for the
    /// stand a guest's interact packet named (its client's click point).
    pub(in crate::game) fn interact_armor_stand(&mut self, target: Option<InteractTarget>) -> bool {
        let Some((mut i, d, point)) = (match target {
            // interactAt gets the packet's location, measured from the stand's
            // position, so getClickedSlot reads the height the client clicked.
            Some(t) => Self::named_entity(&self.armor_stands, t.id, |a| a.id).map(|i| {
                let p = self.armor_stands[i].position;
                (
                    i,
                    0.0,
                    [
                        p.x + t.location[0],
                        p.y + t.location[1],
                        p.z + t.location[2],
                    ],
                )
            }),
            None => self.armor_stand_hit(),
        }) else {
            return false;
        };
        // Only a client's own pick is stopped by a block: the range check passed.
        if target.is_none() && !self.before_block(d) {
            return false;
        }
        let held = self.held().cloned();
        if let Some(stack) = held.as_ref().filter(|h| &*h.item == "minecraft:name_tag") {
            let stand = &mut self.armor_stands[i];
            let alive = stand.health > 0.0;
            if !stand.name_with_tag(stack) {
                return false;
            }
            let id = stand.id;
            let at = [stand.position.x, stand.position.y, stand.position.z];
            let name = stand.custom_name.as_ref().map(|name| name.plain());
            if alive {
                // `Player.interactOn` gives a creative player the count back.
                self.consume_held();
                if let (Some(player), Some(player_name), Some(name)) = (
                    self.server.lan_acting_uuid,
                    self.server.lan_acting_name.clone(),
                    name,
                ) {
                    self.native_player_name_entity(player, &player_name, id, &name);
                }
            }
            // `Player.interactOn`: the tag consumed the use, so the stand
            // posts `gameEvent(GameEvent.ENTITY_INTERACT)` from the player,
            // alive or not (`NameTagItem` succeeds either way).
            let ctx = self.who_context(Some(Who::Player), None);
            self.post_game_event("minecraft:entity_interact", at, ctx);
            self.swing_arm();
            return true;
        }
        let a = &self.armor_stands[i];
        let slot = match &held {
            Some(h) => {
                let slot = self.equipment_slot(h);
                // ArmorStand.interactAt: a disabled slot, or a hand without
                // arms, is FAIL.
                if a.is_disabled(slot) {
                    return true;
                }
                slot
            }
            None => {
                // getClickedSlot by the height of the click.
                let y = (point[1] - a.position.y) / if a.small { 0.5 } else { 1.0 };
                let has = |s: usize| a.equipment[s].is_some();
                let small = a.small;
                if y >= 0.1 && y < 0.1 + if small { 0.8 } else { 0.45 } && has(2) {
                    2
                } else if y >= 0.9 + if small { 0.3 } else { 0.0 }
                    && y < 0.9 + if small { 1.0 } else { 0.7 }
                    && has(4)
                {
                    4
                } else if y >= 0.4 && y < 0.4 + if small { 1.0 } else { 0.8 } && has(3) {
                    3
                } else if y >= 1.6 && has(5) {
                    5
                } else if !has(0) && has(1) {
                    1
                } else {
                    0
                }
            }
        };
        // An empty hand takes from the main hand when the clicked slot is
        // disabled (`isDisabled(clickedSlot) ? slot : clickedSlot`).
        let slot = if held.is_none() && a.is_disabled(slot) {
            0
        } else {
            slot
        };
        // ArmorStand.swapItem: taking from a filled slot or putting into an
        // empty one can be disabled; the click then passes.
        if a.swap_locked(slot) {
            return false;
        }
        let current = self.armor_stands[i].equipment[slot].clone();
        if held.is_none() && current.is_none() {
            return false;
        }
        // A server's mods hear the swap before it happens, and may stop it.
        // A stack of several on an occupied slot goes nowhere (below).
        let moves = held
            .as_ref()
            .is_none_or(|h| h.count <= 1 || current.is_none());
        if moves
            && target.is_some()
            && let (Some(player), Some(name)) = (
                self.server.lan_acting_uuid,
                self.server.lan_acting_name.clone(),
            )
        {
            let stand = self.armor_stands[i].id as i32;
            let slot_name = ["mainhand", "offhand", "feet", "legs", "chest", "head"][slot];
            let mut cancel = false;
            self.native_armor_stand_manipulate(player, &name, stand, slot_name, &mut cancel);
            if cancel {
                return true;
            }
            // The listener had the game in its hands: find the stand again by
            // its id and check the click's facts. A change spends the click.
            let Some(now) = Self::named_entity(&self.armor_stands, stand, |a| a.id) else {
                return true;
            };
            let stand = &self.armor_stands[now];
            if !Self::same_item_and_count(stand.equipment[slot].as_ref(), current.as_ref())
                || !Self::same_item_and_count(self.held(), held.as_ref())
                || stand.swap_locked(slot)
            {
                return true;
            }
            i = now;
        }
        // ArmorStand.swapItem.
        match held {
            // A player with infinite materials puts a copy of one on an empty
            // slot and keeps the stack.
            Some(h) if self.creative && current.is_none() => {
                self.armor_stands[i].equipment[slot] = Some(ItemStack { count: 1, ..h });
            }
            Some(h) if h.count > 1 => {
                if current.is_some() {
                    return true;
                }
                self.armor_stands[i].equipment[slot] = Some(ItemStack {
                    count: 1,
                    ..h.clone()
                });
                *self.held_mut() = Some(ItemStack {
                    count: h.count - 1,
                    ..h
                });
            }
            h => {
                self.armor_stands[i].equipment[slot] = h;
                *self.held_mut() = current;
            }
        }
        let p = self.armor_stands[i].position;
        self.world
            .modified
            .insert(((p.x.floor() as i32) >> 4, (p.z.floor() as i32) >> 4));
        self.swing_arm();
        true
    }
    /// `ArmorStand.hurtServer` from a player attack.
    pub(in crate::game::server_side) fn attack_armor_stand(&mut self) -> bool {
        let Some((i, d, _)) = self.armor_stand_hit() else {
            return false;
        };
        if !self.before_block(d) || self.mob_hit().is_some_and(|m| m.1 < d) {
            return false;
        }
        let ((_, scale, crit, magic), admitted) = self.melee_damage_with_admission(
            "minecraft:armor_stand",
            true,
            Some(self.armor_stands[i].id),
        );
        let sweep = self.sweep_before_hit("minecraft:armor_stand", crit);
        let hurt = self.hit_armor_stand(i, |g, area, id| {
            let knockback = if admitted {
                g.extra_knockback("minecraft:armor_stand", scale).0
            } else {
                0.0
            };
            g.attack_effects(
                area,
                sweep,
                Some(id as i32),
                crit,
                super::mace::Spared::Stand(id),
                magic.then_some(id as i32),
            );
            g.special_attack_slowdown(knockback);
            if admitted {
                g.remember_player_target(Some(id));
            }
        });
        // Player.onAttack: the hit starts the attack strength again, hurt or
        // not (skipAttackInteraction only asks mayInteract).
        self.server.attack_ticks = 0;
        if hurt {
            if admitted {
                self.special_attack_exhaustion();
            }
            self.swing_arm();
        }
        hurt
    }
    /// A player's attack on the stand `i` once it is picked: the stand's own
    /// hurt, `before_weapon` (the attack's sounds and particles, given the
    /// stand's box and id) and `Player.itemAttackInteraction`, which treats
    /// the stand as a living target: the held weapon's hit and post-attack
    /// effects.
    pub(in crate::game::server_side) fn hit_armor_stand(
        &mut self,
        i: usize,
        before_weapon: impl FnOnce(&mut Self, Aabb, u32),
    ) -> bool {
        // The stand's box, id and place, before the hit may remove it.
        let (area, id) = (self.armor_stands[i].bounds(), self.armor_stands[i].id);
        let (center, on_ground) = (
            self.armor_stands[i].position,
            self.armor_stands[i].ground.on_ground,
        );
        let hurt = self.punch_armor_stand(i);
        if hurt {
            before_weapon(self, area, id);
            self.weapon_hit_stand(id, center, on_ground);
        }
        hurt
    }
    /// `ArmorStand.hurtServer` for a player's attack (also a sweep's): a
    /// creative player breaks it, otherwise the first hit wobbles it and a
    /// second within five ticks breaks it.
    pub(in crate::game::server_side) fn punch_armor_stand(&mut self, i: usize) -> bool {
        // `Entity.isInvulnerableTo` before ArmorStand's own branches (#916):
        // the saved flag or window refuses the hit — past only for
        // #bypasses_invulnerability damage and a creative player's hit
        // (`DamageSource.isCreativePlayer`).
        let id = self.armor_stands[i].id;
        let attacker = self.own_entity_id();
        let vanilla = self.saved_refuses_damage(id, "minecraft:player_attack", self.creative);
        let source = sunrise_client::health::HurtSource {
            damage_type: "minecraft:player_attack",
            causing: attacker,
            direct: attacker,
            position: None,
        };
        if super::entity::damage_level_of!(self, self.combat.0.attack(), no_push, None)
            .invulnerability_check(
                sunrise_client::health::Hurting::ArmorStand(id as i32, source),
                vanilla,
            )
        {
            return false;
        }

        self.break_armor_stand_by_hit(
            i,
            self.creative,
            false,
            Some(super::commands::Target::Player),
            "minecraft:player_attack",
            attacker as u32,
        )
    }
    /// `Entity.gameEvent(event, attributedTo)` for a stand at `at`: the
    /// causing player (a LAN guest too) or mob, else the stand itself (which
    /// no listener tells apart from none).
    pub(in crate::game) fn stand_game_event(
        &mut self,
        event: &'static str,
        at: [f64; 3],
        by: Option<super::commands::Target>,
    ) {
        self.entity_event_by(event, at, by);
    }
    /// Run stand turns that precede the level's first shulker. The mob tick
    /// interleaves the remaining turns with shulkers before applying each lid push.
    pub(in crate::game::server_side) fn begin_armor_stand_ticks(&mut self) -> Result<()> {
        self.server.ordered.armor_stands.extend(
            self.armor_stands
                .iter()
                .filter(|stand| stand.join < self.server.ordered.born)
                .map(|stand| stand.join),
        );
        if self.server.ordered.armor_stands.is_empty() {
            return Ok(());
        }
        self.server
            .ordered
            .armor_stands
            .make_contiguous()
            .sort_unstable();
        let first_shulker = self
            .mobs
            .iter()
            .filter(|mob| !mob.removed && mob.species() == Species::Shulker)
            .map(|mob| mob.join)
            .min();
        self.tick_armor_stands_before(first_shulker.unwrap_or(JoinSeq::MAX))
    }

    pub(in crate::game::server_side) fn tick_armor_stands_before(
        &mut self,
        cut: JoinSeq,
    ) -> Result<()> {
        if self
            .server
            .ordered
            .armor_stands
            .front()
            .is_none_or(|&join| join >= cut)
        {
            return Ok(());
        }
        let mut pending = std::mem::take(&mut self.server.ordered.armor_stands);
        let count = pending
            .make_contiguous()
            .partition_point(|&join| join < cut);
        let result = self.tick_armor_stand_turns(pending.drain(..count));
        self.server.ordered.armor_stands = pending;
        result
    }

    #[cfg(test)]
    pub(in crate::game) fn tick_armor_stands(&mut self) -> Result<()> {
        let joins: Vec<_> = self.armor_stands.iter().map(|stand| stand.join).collect();
        self.tick_armor_stand_turns(joins)
    }

    pub(in crate::game::server_side) fn tick_armor_stand_turns(
        &mut self,
        joins: impl IntoIterator<Item = JoinSeq>,
    ) -> Result<()> {
        let mut dead = Vec::new();
        let mut drowned = Vec::new();
        // ServerLevel.tick: only entities in entity-ticking chunks tick.
        let ticking = self.entity_ticking_area();
        let server = self.remote.is_none();
        let stand_breathes_under_water = self.items().entity_type_tags.has_flag(
            TYPE_ID,
            sunrise_client::items::EntityTypeTags::CAN_BREATHE_UNDER_WATER,
        );
        // EntityGetter.getEntityCollisions: the boxes that stop a stand's move
        // (a living shulker, a boat, a happy ghast held still), gathered when
        // the first stand travels. A server's stand rides nothing (`vehicle`
        // is set on a server's client only), but mobs can ride it (`/ride`):
        // `isPassengerOfSameVehicle` leaves those riders out.
        let mut obstacles: Option<sunrise_client::entities::ObstacleGrid> = None;
        let mut nearby = Vec::new();
        for join in joins {
            let Some(i) = self.armor_stands.locate(join) else {
                continue;
            };
            let at = self.armor_stands[i].position;
            if !ticking(at.x.floor() as i32, at.z.floor() as i32) {
                self.armor_stands[i].previous = at;
                self.armor_stands[i].previous_yaw = self.armor_stands[i].yaw;
                continue;
            }
            let target_dead = self.armor_stands[i]
                .last_hurt_mob
                .is_some_and(|(id, _)| !self.last_hurt_target_alive(id));
            let stand = &mut self.armor_stands[i];
            stand.tick_count = stand.tick_count.wrapping_add(1);
            stand.first_tick = false;
            if target_dead {
                stand.last_hurt_mob = None;
            }
            // Entity.baseTick: the saved invulnerability window counts down
            // (#916).
            self.tick_saved_invulnerable_time(self.armor_stands[i].id);
            // LivingEntity.baseTick -> tickDeath: a stand at 0 health or less
            // is dead, and goes at a death time of 20 with no drops.
            let mut died_of_age = false;
            let a = &mut self.armor_stands[i];
            if a.health <= 0.0 {
                a.death_time = a.death_time.wrapping_add(1);
                if a.death_time >= 20 {
                    dead.push((a.id, None));
                    died_of_age = true;
                }
            }
            self.armor_stand_location_effects(i, false);
            // ArmorStand.travel: a marker or a NoGravity stand does not move.
            let travels = !self.armor_stands[i].marker && !self.armor_stands[i].no_gravity;
            if travels && obstacles.is_none() {
                obstacles = Some(self.stand_entity_collisions());
            }
            let a = &mut self.armor_stands[i];
            a.previous = a.position;
            a.previous_yaw = a.yaw;
            // aiStep's freezing step (removeFrost) makes the speed instance.
            a.speed_instance = true;
            // LivingEntity.aiStep -> EntityEquipment.tick, before movement.
            let a = &mut self.armor_stands[i];
            super::inventory_tick::equipment_inventory_tick(
                &self.world,
                self.res.facts(),
                self.dim.id(),
                &mut a.equipment,
            );
            // Entity.baseTick's updateFluidInteraction; a marker takes none.
            let (water_height, lava_height, eye_in_water) = if a.marker {
                (0.0, 0.0, false)
            } else {
                let (bounds, at, eye) = (a.bounds(), a.position, a.fluid_eye_y());
                sunrise_client::entities::update_entity_fluids(
                    bounds,
                    at,
                    eye,
                    &mut a.velocity,
                    &self.world,
                    &self.res.motion().fluids,
                    &self.loop_context,
                )
            };
            let in_water = water_height > 0.0;
            // LivingEntity.baseTick: shared air logic, then hurt time; aiStep clears fall flying.
            let stand_air = if server && a.health > 0.0 {
                let in_bubble_column = eye_in_water && {
                    let eye = a.fluid_eye_y();
                    let eye_block = sunrise_core::position::BlockPos::containing(Vec3::new(
                        a.position.x,
                        eye,
                        a.position.z,
                    ));
                    &*self
                        .res
                        .facts()
                        .get(self.world.block(eye_block.x, eye_block.y, eye_block.z))
                        .name
                        == "minecraft:bubble_column"
                };
                let drowns = eye_in_water && !in_bubble_column && !stand_breathes_under_water;
                let vanilla = sunrise_client::breathing::Breath {
                    can_breathe: !drowns,
                    consume: 1,
                    refill: if drowns { 0 } else { 4 },
                };
                Some((
                    a.id,
                    sunrise_client::breathing::AirStep::generic(a.air, 300, vanilla),
                ))
            } else {
                None
            };
            let i = if let Some((stand_id, step)) = stand_air {
                let answer = if sunrise_client::breathing::heard() {
                    self.ask_stand_breath(stand_id, step)
                } else {
                    Some(step.vanilla)
                };
                let Some(i) = self.armor_stands.locate(join) else {
                    continue;
                };
                let a = &mut self.armor_stands[i];
                if let Some(breath) = answer
                    && a.air == step.air
                    && let Some(drowning) =
                        step.finish(&self.loop_context, a.id, TYPE_ID, &mut a.air, breath)
                {
                    // Stand refuses drown damage; queue entity event 67 / bubble particles.
                    drowned.push((a.id, drowning.bubbles));
                }
                i
            } else {
                i
            };
            let a = &mut self.armor_stands[i];
            if a.hurt_time > 0 {
                a.hurt_time -= 1;
            }
            a.fall_flying = false;
            let medium =
                sunrise_client::entities::StandMedium::from_heights(water_height, lava_height);
            // Entity.collide asks for the entities meeting the box stretched
            // by this tick's movement (the fluid push is already in it).
            nearby.clear();
            if let Some(grid) = obstacles.as_mut().filter(|_| travels) {
                let area = a.bounds().expand_towards(a.velocity);
                let area = area.inflate(Vec3::new(1.0e-7, 1.0e-7, 1.0e-7));
                grid.near_except(area, Some(a.id), &mut nearby);
            }
            let (world, registry, facts, collision) = (
                &self.world,
                self.pack.assets.registry,
                self.res.facts(),
                self.collision,
            );
            let scene = sunrise_client::collisions::SceneCollisions {
                world,
                binding: collision,
                subject: None,
            };
            let side = game_side!(self);
            let step = a.tick_move(
                &scene,
                facts,
                Some(&self.res.motion().blocks),
                &self.res.motion().climbing,
                in_water,
                Some(self.border.geometry()),
                &nearby,
                &self.loop_context,
                &side,
            );
            if let Some(step) = step {
                // LivingEntity.aiStep: `travel` settles before applyEffectsFromBlocks.
                a.tick_settle(medium, step);
            }
            let section_step = (a.id, a.previous, a.position);
            let a = &mut self.armor_stands[i];
            if in_water {
                a.enchantment_fall_distance = 0.0;
            } else {
                // Entity.checkFallDamage narrows each movement before adding to the double.
                a.enchantment_fall_distance +=
                    f64::from((a.previous.y - a.position.y).max(0.0) as f32);
            }
            let landed = a.ground.on_ground && a.enchantment_fall_distance > 0.0;
            if a.ground.on_ground {
                a.enchantment_fall_distance = 0.0;
            }
            // LivingEntity.checkFallDamage: onChangedBlock before the block's
            // fallOn, then aiStep's applyEffectsFromBlocks (stepOn, then the
            // blocks it is inside).
            if landed {
                self.armor_stand_location_effects(i, true);
            }
            self.armor_stand_turtle_eggs(i, landed)?;
            if landed {
                let stand = &self.armor_stands[i];
                let (position, bounds) = (stand.position, stand.bounds());
                self.post_landing_event(position, bounds, true);
            }
            if step.is_some() && !self.armor_stands[i].no_physics(None) {
                let stand = &self.armor_stands[i];
                let (id, footprint, ground, mut velocity) = (
                    stand.id,
                    (stand.position, stand.bounds()),
                    stand.ground,
                    stand.velocity,
                );
                let writes = self.body_step_on((id, TYPE_ID), footprint, &ground, &mut velocity);
                let stand = &mut self.armor_stands[i];
                // `ArmorStand.hurtServer` does not take generic damage; the stand
                // has no stuck multiplier or active-effect list in its travel row.
                super::entity::apply_contact_writes(
                    writes,
                    &mut velocity,
                    Some(&mut stand.enchantment_fall_distance),
                    None,
                    Some(&mut stand.fire_ticks),
                );
                stand.velocity = velocity;
                // BubbleColumnBlock.entityInside runs in the sweep after Block.stepOn.
                let mut tops = Vec::new();
                stand.push_by_bubble_column_over(
                    &self.world,
                    registry,
                    self.res.facts(),
                    collision,
                    |pos| tops.push(pos),
                );
                for pos in tops {
                    self.bubble_column_particles(pos);
                }
            }
            self.armor_stands[i].apply_honey_slide(&self.world, self.res.facts());
            let a = &self.armor_stands[i];
            if !a.marker && !a.no_gravity {
                self.entity_powder_snow_particles(
                    a.previous,
                    a.position,
                    a.bounds(),
                    a.velocity,
                    true,
                );
            }
            // LivingEntity.aiStep calls pushEntities after travel. The stand's
            // own doPush is empty; only this shove runs.
            self.push_rideable_minecarts(i);
            let id = self.armor_stands[i].id;
            let prev_fire = self.armor_stands[i].fire_ticks;
            let saved = ["minecraft:on_fire", "minecraft:in_fire"]
                .map(|damage| self.saved_refuses_damage(id, damage, false));
            let tags = world_items!(self);
            let registry = self.pack.assets.registry;
            let block_registries = &self.pack.block_registries;
            let hooks = &self.damage_hooks;
            if let Some(damage_type) = self.armor_stands[i].environment_tick_with_hook(
                &mut self.world,
                self.res.facts(),
                registry,
                |world, damage| {
                    let vanilla = if damage == "minecraft:on_fire" {
                        saved[0]
                    } else {
                        saved[1]
                    };
                    let source = sunrise_client::health::HurtSource {
                        damage_type: damage,
                        causing: 0,
                        direct: 0,
                        position: None,
                    };
                    sunrise_client::health::DamageLevel::of(
                        world,
                        registry,
                        Some(tags),
                        Some(block_registries),
                        hooks,
                    )
                    .no_push()
                    .invulnerability_check(
                        sunrise_client::health::Hurting::ArmorStand(id as i32, source),
                        vanilla,
                    )
                },
            ) && !died_of_age
            {
                dead.push((id, Some(damage_type)));
            }
            if self.armor_stands[i].fire_ticks > 0
                && self.armor_stands[i].fire_ticks >= prev_fire
                && let Some(sunrise_core::nbt::Tag::Compound(map)) = self.persist.raw.get_mut(&id)
            {
                map.remove(&"TicksFrozen".into());
            }
            let (id, from, to) = section_step;
            self.entity_section_step(id, [from.x, from.y, from.z], [to.x, to.y, to.z]);
            // The end of LivingEntity.tick.
            let a = &mut self.armor_stands[i];
            if a.impulse_grace > 0 {
                a.impulse_grace -= 1;
            }
        }
        for (id, bubbles) in drowned {
            let Some((at, v)) = self.armor_stands.iter().find(|a| a.id == id).map(|a| {
                (
                    [a.position.x, a.position.y, a.position.z],
                    [a.velocity.x, a.velocity.y, a.velocity.z],
                )
            }) else {
                continue;
            };
            if self.drown_bubbles(at, v, bubbles) {
                self.lan_entity_event(id as i32, super::combat::DROWN_EVENT, at);
            }
        }
        for (id, damage_type) in dead.into_iter().rev() {
            match damage_type {
                Some(damage_type) => {
                    if let Some(i) = self.armor_stands.iter().position(|a| a.id == id) {
                        self.break_armor_stand_by_anything(i, damage_type, 0);
                    }
                }
                None => {
                    // Entity event 60 (the poof), then remove(KILLED).
                    if let Some(a) = self.armor_stands.iter().find(|a| a.id == id) {
                        let at = a.position.components();
                        self.remove_armor_stand_after(id, |game| {
                            game.lan_entity_event(id as i32, 60, at);
                            let (x, z) = (at[0].floor() as i32, at[2].floor() as i32);
                            game.world.modified.insert((x >> 4, z >> 4));
                        });
                    }
                }
            }
        }
        Ok(())
    }
    /// `Entity.move(mover, delta)` for a stand with physics (not a marker or
    /// `NoGravity`). Mob lids act on the shulker's turn; block-entity lids act
    /// after the entity ticks.
    /// `grid` holds the stands' obstacles, built once per lid at its first stand.
    pub(in crate::game) fn shoves_armor_stand(
        &mut self,
        i: usize,
        mover: sunrise_world::entity_move::MoverType,
        delta: Vec3,
        grid: &mut Option<sunrise_client::entities::ObstacleGrid>,
    ) {
        let grid = grid.get_or_insert_with(|| self.stand_entity_collisions());
        let mut nearby = Vec::new();
        let a = &mut self.armor_stands[i];
        // Entity.collide: the entities meeting the box stretched by the push.
        let reach = a.bounds().expand_towards(delta);
        let reach = reach.inflate(Vec3::new(1.0e-7, 1.0e-7, 1.0e-7));
        grid.near_except(reach, Some(a.id), &mut nearby);
        let scene = sunrise_client::collisions::SceneCollisions {
            world: &self.world,
            binding: self.collision,
            subject: None,
        };
        a.shove(
            scene,
            delta,
            Some(self.border.geometry()),
            &nearby,
            !self.options.execution_role.is_client() && self.remote.is_none(),
            mover,
            &self.loop_context,
        );
    }
    /// `Ravager.roar` with `mob_griefing` on: every armor stand in `area` (the
    /// ravager's box grown by 4) takes `strongKnockback` from the ravager at
    /// `from`. Its `mobAttack` leaves the stand whole (`ArmorStand.hurtServer`:
    /// not a damage type that breaks one).
    pub(in crate::game::server_side) fn roar_pushes_armor_stands(
        &mut self,
        area: Aabb,
        from: [f64; 3],
    ) {
        for a in self.armor_stands.iter_mut() {
            let (width, height) = stand_box(a);
            let half = f64::from(width) / 2.0;
            let p = a.position;
            let bounds = Aabb::new(
                Vec3::new(p.x - half, p.y, p.z - half),
                Vec3::new(p.x + half, p.y + f64::from(height), p.z + half),
            );
            if a.health <= 0.0 || !bounds.intersects(area) {
                continue;
            }
            let (xd, zd) = (p.x - from[0], p.z - from[2]);
            let dd = (xd * xd + zd * zd).max(0.001);
            // Entity.push.
            a.velocity = a
                .velocity
                .add_vector(Vec3::new(xd / dd * 4.0, 0.2, zd / dd * 4.0));
        }
    }
    /// `ArmorStand.pushEntities`: a plain minecart (`RIDABLE_MINECARTS`,
    /// `AbstractMinecart.isRideable`) whose box meets the stand's is shoved
    /// when `distanceToSqr` is at most 0.2. `AbstractMinecart.push` of a
    /// non-cart adds the push to the cart and a quarter of it to the stand
    /// (`Entity.push(double)` does not consult `isPushable`). A stand with no
    /// physics (`Marker` or `NoGravity`, `ArmorStand.hasPhysics` / `noPhysics`)
    /// or one riding that cart does not shove it.
    pub(in crate::game::server_side) fn push_rideable_minecarts(&mut self, i: usize) {
        let stand = &self.armor_stands[i];
        if stand.marker || stand.no_gravity {
            return;
        }
        let stand_pos = stand.position;
        let stand_box = stand.bounds();
        let vehicle = stand.vehicle;
        let mut applied = Vec::new();
        for (j, cart) in self.carts.iter().enumerate() {
            let Some(shove) = armor_stand_cart_shove(
                stand_pos,
                stand_box,
                cart.position,
                cart.bounds(),
                cart.kind,
                vehicle == Some(cart.id),
            ) else {
                continue;
            };
            applied.push((j, shove));
        }
        let mut stand_x = 0.0;
        let mut stand_z = 0.0;
        for (j, shove) in applied {
            self.carts.update(j, |cart| {
                cart.velocity.x += shove.cart_x;
                cart.velocity.z += shove.cart_z;
            });
            stand_x += shove.stand_x;
            stand_z += shove.stand_z;
        }
        if stand_x != 0.0 || stand_z != 0.0 {
            let stand = &mut self.armor_stands[i];
            stand.velocity.x += stand_x;
            stand.velocity.z += stand_z;
        }
    }
    /// An armor stand is a `LivingEntity`, so the egg under it
    /// (`getOnPosLegacy`) cracks like under a mob: `TurtleEggBlock.fallOn` one
    /// landing in 3 (`Entity.move`'s `checkFallDamage`), then `stepOn` one tick
    /// in 100 while it is on the ground (`LivingEntity.aiStep`'s
    /// `applyEffectsFromBlocks`, after `travel`, so a marker that has landed
    /// still steps). `canDestroyEgg` wants mob griefing before the level's roll.
    pub(in crate::game::server_side) fn armor_stand_turtle_eggs(
        &mut self,
        i: usize,
        landed: bool,
    ) -> Result<()> {
        let stand = &self.armor_stands[i];
        if !stand.ground.on_ground
            || self.remote.is_some()
            || self.options.execution_role.is_client()
        {
            return Ok(());
        }
        let p = stand.on_pos_legacy(|p| {
            let name = &self.res.facts().get(self.world.block(p.x, p.y, p.z)).name;
            let tags = &self.res.items().block_tags;
            [
                "minecraft:fences",
                "minecraft:walls",
                "minecraft:fence_gates",
            ]
            .iter()
            .any(|t| tags.get(*t).is_some_and(|s| s.contains(&**name)))
        });
        for chance in [landed.then_some(3), Some(100)].into_iter().flatten() {
            let egg = self
                .pack
                .assets
                .registry
                .state(self.world.block(p.x, p.y, p.z))
                .is_some_and(|s| s.name == "minecraft:turtle_egg");
            if egg
                && self
                    .ui
                    .rules
                    .bool_at(sunrise_client::gamerules::vanilla::MOB_GRIEFING)
                && self.world.random.next_int_bound(chance).unwrap_or(1) == 0
            {
                self.crack_turtle_egg([p.x, p.y, p.z])?;
            }
        }
        Ok(())
    }
    /// `ArmorStand.brokenByAnything` rolls the stand's entity loot table before
    /// the equipment. The resulting list crosses the existing living-drops hook;
    /// the stand item and equipment pop outside it, as in vanilla.
    pub(in crate::game) fn armor_stand_death_drops(
        &mut self,
        stand: &ArmorStand,
        damage_type: &str,
        causing: u32,
    ) {
        let source = (!mod_api::__private::mods().is_empty())
            .then(|| super::creatures::death_hurt_source(damage_type, causing));
        let dying = super::creatures::DyingDrops {
            entity: stand.id,
            kind: "minecraft:armor_stand",
            player: None,
            source,
            recently_hit: false,
        };
        let (dying, stacks) = armor_stand_dying_drops(
            stand,
            self.ui
                .rules
                .bool_at(sunrise_client::gamerules::vanilla::MOB_DROPS),
            dying,
            |table| {
                let Some(loot) = self.server_loot.0.get() else {
                    return Vec::new();
                };
                if !loot.has_table(table) {
                    return Vec::new();
                }
                let params = sunrise_world::loot::EntityLootParams {
                    this: sunrise_world::loot::LootEntity {
                        on_fire: stand.is_on_fire(),
                        baby: false,
                        ..sunrise_world::loot::LootEntity::new("minecraft:armor_stand")
                    },
                    ..Default::default()
                };
                match loot.random_sequence(table) {
                    Some(sequence) => loot.roll_entity(
                        table,
                        self.server.rand_sequences.get(sequence, self.level.seed),
                        &params,
                    ),
                    None => loot.roll_entity(table, &mut self.world.random, &params),
                }
            },
        );
        self.drop_death_items(
            &dying,
            stacks
                .into_iter()
                .map(|(stack, at)| (stack, at, 0))
                .collect(),
            10,
            |_| None,
        );
    }
    pub(in crate::game) fn remove_armor_stand_after(
        &mut self,
        id: u32,
        effects: impl FnOnce(&mut Self),
    ) -> bool {
        self.with_vehicle_removal_effects(super::commands::Target::Stand(id), effects, |game| {
            if let Some(i) = game.armor_stands.iter().position(|stand| stand.id == id) {
                game.armor_stands.remove(i);
            }
        })
        .is_some()
    }

    /// `ArmorStand.hurtServer` for a command's damage of `damage_type` from the
    /// causing entity `causing`, up to its sources that act and still return
    /// false: a blast breaks the stand (`brokenByAnything`, `kill`), fire that
    /// ignites stands lights it (or burns a burning one 0.15), and burning
    /// takes 4 health, all returning false. A source in
    /// `#can_break_armor_stand` or `#always_kills_armor_stands` hurts the
    /// stand as a player's hit does and returns true; every other source is
    /// refused (false), as `/damage` fails for it in vanilla.
    pub(in crate::game) fn command_damage_stand(
        &mut self,
        id: u32,
        damage_type: &str,
        causing: Option<super::commands::Target>,
    ) -> bool {
        let direct = causing
            .and_then(|target| self.network_entity_id(target))
            .unwrap_or(0) as u32;
        self.hurt_armor_stand(id, damage_type, causing, direct, None)
    }
    /// `ArmorStand.hurtServer`: shared damage entry for commands and projectiles.
    pub(in crate::game) fn hurt_armor_stand(
        &mut self,
        id: u32,
        damage_type: &str,
        causing: Option<super::commands::Target>,
        direct: u32,
        position: Option<[f64; 3]>,
    ) -> bool {
        use super::commands::Target;
        if !self.armor_stands.iter().any(|a| a.id == id) {
            return false;
        }
        let Some(kind) = self.health_resources.damage.id(damage_type) else {
            return false;
        };
        let tagged = |game: &Self, tag: &str| game.health_resources.damage.in_tag(kind, tag);
        if !self
            .ui
            .rules
            .bool_at(sunrise_client::gamerules::vanilla::MOB_GRIEFING)
            && matches!(causing, Some(Target::Mob(_)))
        {
            return false;
        }
        if tagged(self, "minecraft:bypasses_invulnerability") {
            self.kill_target(Target::Stand(id));
            return false;
        }
        // Entity.isInvulnerableToBase: the flag, unless a creative player hits.
        let creative = match causing {
            Some(Target::Player) => self.creative,
            Some(Target::Guest(conn)) => self.command_guest(conn).is_some_and(|g| g.creative()),
            _ => false,
        };
        let causing_id = causing
            .and_then(|target| self.network_entity_id(target))
            .unwrap_or(0) as u32;
        let invulnerable = self.saved_refuses_damage(id, damage_type, creative);
        let source = sunrise_client::health::HurtSource {
            damage_type,
            causing: causing_id as i32,
            direct: direct as i32,
            position,
        };
        if super::entity::damage_level_of!(self, self.combat.0.attack(), no_push, None)
            .invulnerability_check(
                sunrise_client::health::Hurting::ArmorStand(id as i32, source),
                invulnerable,
            )
        {
            return false;
        }
        // Re-locate after invulnerability hooks may have removed or moved the stand.
        let Some(i) = self.armor_stands.iter().position(|a| a.id == id) else {
            return false;
        };
        let a = &self.armor_stands[i];
        if (a.mod_fire_immune && tagged(self, "minecraft:is_fire"))
            || a.invisible
            || a.marker
            || self.stand_immune_to_damage(i, damage_type, causing)
        {
            return false;
        }
        // ArmorStand.hurtServer attributes its game events to the source entity.
        let by = match causing {
            Some(Target::Player | Target::Guest(_) | Target::Mob(_)) => causing,
            _ => None,
        };
        let (explosion, ignites, burns) = (
            tagged(self, "minecraft:is_explosion"),
            tagged(self, "minecraft:ignites_armor_stands"),
            tagged(self, "minecraft:burns_armor_stands"),
        );
        let (breaks, always_kills) = (
            tagged(self, "minecraft:can_break_armor_stand"),
            tagged(self, "minecraft:always_kills_armor_stands"),
        );
        if explosion {
            self.break_armor_stand_by_anything(i, damage_type, causing_id);
            return false;
        }
        if ignites {
            if self.armor_stands[i].mod_fire_immune {
                return false;
            }
            if self.armor_stands[i].is_on_fire() {
                self.cause_stand_damage(i, 0.15, damage_type, causing_id);
            } else {
                // igniteForSeconds(5.0f): at least 100 ticks, and clearFreeze.
                let a = &mut self.armor_stands[i];
                a.fire_ticks = a.fire_ticks.max(100);
                if let Some(sunrise_core::nbt::Tag::Compound(map)) = self.persist.raw.get_mut(&id) {
                    map.remove(&"TicksFrozen".into());
                }
            }
            return false;
        }
        if burns && self.armor_stands[i].health > 0.5 {
            self.cause_stand_damage(i, 4.0, damage_type, causing_id);
            return false;
        }
        if !breaks && !always_kills {
            return false;
        }
        // A causing player who may not build (adventure, spectator) cannot.
        let may_build = match causing {
            Some(Target::Player) => self.may_build(),
            Some(Target::Guest(conn)) => self
                .command_guest(conn)
                .is_none_or(|g| g.abilities.may_build),
            _ => true,
        };
        if !may_build {
            return false;
        }
        self.break_armor_stand_by_hit(i, creative, always_kills, by, damage_type, causing_id)
    }
    /// `EnchantmentHelper.isImmuneToDamage` for the stand at `i`: whether an
    /// enchantment on an active equipment slot has a `minecraft:damage_immunity`
    /// effect whose requirements hold for `damage_type` and `causing`.
    pub(in crate::game::server_side) fn stand_immune_to_damage(
        &mut self,
        i: usize,
        damage_type: &str,
        causing: Option<super::commands::Target>,
    ) -> bool {
        use super::commands::Target;
        let mut checks = Vec::new();
        let equipment = &self.armor_stands[i].equipment;
        for (stack, slot) in equipment.iter().zip(ArmorStand::SLOTS) {
            let Some(stack) = stack else { continue };
            let armor = matches!(slot, "head" | "chest" | "legs" | "feet");
            let hand = matches!(slot, "mainhand" | "offhand");
            for (id, level) in stack.enchantments() {
                let Some(def) = self.res.stations().enchantments.get(id) else {
                    continue;
                };
                if !def.slots.iter().any(|s| {
                    s == slot || s == "any" || (s == "armor" && armor) || (s == "hand" && hand)
                }) {
                    continue;
                }
                let immunity = &def.effects["minecraft:damage_immunity"];
                for entry in immunity.as_array().into_iter().flatten() {
                    checks.push((entry.get("requirements").cloned(), *level));
                }
            }
        }
        if checks.is_empty() {
            return false;
        }
        let attacker = match causing {
            Some(Target::Player | Target::Guest(_)) => Some("minecraft:player".to_owned()),
            Some(Target::Mob(m_id)) => self
                .mobs
                .iter()
                .find(|m| m.id == m_id)
                .map(|m| m.kind.id.clone()),
            Some(Target::Stand(_)) => Some("minecraft:armor_stand".to_owned()),
            _ => None,
        };
        let canonical = if damage_type.contains(':') {
            damage_type.to_owned()
        } else {
            format!("minecraft:{damage_type}")
        };
        let ctx = sunrise_client::enchant::EffectContext {
            items: Some(self.res.items()),
            damage_type: Some(&canonical),
            is_direct: attacker.is_some(),
            this: Some("minecraft:armor_stand"),
            attacker: attacker.as_deref(),
            direct_attacker: attacker.as_deref(),
            ..Default::default()
        };
        let ench = &self.res.stations().enchantments;
        let rng = &mut self.world.random;
        for (req, level) in checks {
            if req.is_none_or(|r| ench.condition(&r, level, &ctx, rng)) {
                return true;
            }
        }
        false
    }
    /// `ArmorStand.causeDamage`: the health less `amount`, broken by anything
    /// at 0.5 or less.
    pub(in crate::game::server_side) fn cause_stand_damage(
        &mut self,
        i: usize,
        amount: f32,
        damage_type: &str,
        causing: u32,
    ) {
        let health = self.armor_stands[i].health - amount;
        if health <= 0.5 {
            self.break_armor_stand_by_anything(i, damage_type, causing);
        } else {
            let at = self.armor_stands[i].position;
            self.armor_stands[i].health = health;
            self.stand_game_event(
                "minecraft:entity_damage",
                [at.x, at.y, at.z],
                self.level_entity(causing as i32),
            );
        }
    }
    /// `ArmorStand.brokenByAnything`, then `kill`.
    pub(in crate::game) fn break_armor_stand_by_anything(
        &mut self,
        i: usize,
        damage_type: &str,
        causing: u32,
    ) -> bool {
        let a = self.armor_stands[i].clone();
        let id = a.id;
        let at = a.position.components();
        let removed = self.remove_armor_stand_after(id, |game| {
            game.server_play_sound("minecraft:entity.armor_stand.break", at, 1.0, 1.0);
            game.armor_stand_death_drops(&a, damage_type, causing);
            let (pos, drops) = broken_armor_stand_equipment(a);
            for stack in drops {
                game.pop_resource(pos, stack);
            }
            game.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
        });
        if removed {
            let source = self
                .level_entity(causing as i32)
                .or(Some(super::commands::Target::Stand(id)));
            self.entity_killed_event_by(at, source);
        }
        removed
    }
    /// The end of `ArmorStand.hurtServer` for `#can_break_armor_stand` or `#always_kills_armor_stands`.
    pub(in crate::game) fn break_armor_stand_by_hit(
        &mut self,
        i: usize,
        creative: bool,
        always_kills: bool,
        by: Option<super::commands::Target>,
        damage_type: &str,
        causing_id: u32,
    ) -> bool {
        let time = self.world.game_time;
        let at = self.armor_stands[i].position;
        let at = [at.x, at.y, at.z];
        if creative {
            let id = self.armor_stands[i].id;
            let (width, height) = stand_box(&self.armor_stands[i]);
            if !self.remove_armor_stand_after(id, |game| {
                game.server_play_sound("minecraft:entity.armor_stand.break", at, 1.0, 1.0);
                game.show_breaking_particles(at, f64::from(width), f64::from(height));
            }) {
                return false;
            }
            self.stand_game_event("minecraft:entity_die", at, by);
            return true;
        }
        if time - self.armor_stands[i].last_hit > 5 && !always_kills {
            self.server_play_sound("minecraft:entity.armor_stand.hit", at, 0.3, 1.0);
            self.stand_game_event("minecraft:entity_damage", at, by);
            self.armor_stands[i].last_hit = time;
            return true;
        }
        let a = self.armor_stands[i].clone();
        let removed = self.remove_armor_stand_after(a.id, |game| {
            let (width, height) = stand_box(&a);
            let center = [at[0], at[1] + if a.small { 0.4875 } else { 0.9875 }, at[2]];
            let mut stand = ItemStack::new("minecraft:armor_stand", 1);
            if let Some(name) = &a.custom_name {
                stand
                    .data_mut()
                    .set_loot_name("minecraft:custom_name", &name.component());
            }
            game.spawn_item(stand, center, [0.0; 3], 10);
            game.server_play_sound("minecraft:entity.armor_stand.break", at, 1.0, 1.0);
            game.armor_stand_death_drops(&a, damage_type, causing_id);
            for stack in a.equipment.into_iter().flatten() {
                let r = &mut game.world.random;
                let v = [
                    f64::from(r.next_float()) * 0.2 - 0.1,
                    0.2,
                    f64::from(r.next_float()) * 0.2 - 0.1,
                ];
                game.spawn_item(stack, center, v, 10);
            }
            game.show_breaking_particles(at, f64::from(width), f64::from(height));
        });
        if removed {
            self.stand_game_event("minecraft:entity_die", at, by);
        }
        removed
    }
    /// `ArmorStand.showBreakingParticles`: ten oak plank crumbs sent via `server_level_particles`.
    pub(in crate::game::server_side) fn show_breaking_particles(
        &mut self,
        at: [f64; 3],
        width: f64,
        height: f64,
    ) {
        let Some(state) = self
            .pack
            .assets
            .registry
            .default_state("minecraft:oak_planks")
        else {
            return;
        };
        let centre = [at[0], at[1] + height * (2.0 / 3.0), at[2]];
        let spread = [width / 4.0, height / 4.0, width / 4.0];
        self.server_level_particles(
            &crate::lan::server_effects::ServerParticle::block(
                sunrise_client::particles::ParticleType::Block,
                state,
            ),
            centre,
            spread,
            0.05,
            10,
        );
    }
}
/// The stand's slot (`ArmorStand::SLOTS` order) for an item whose
/// `equippable` slot is `equip` (`SmithingScreen.updateArmorStandPreview`):
/// worn in a head, chest, legs or feet slot, else (no `equippable`, or any
/// other slot: main hand, off hand, body, saddle) held in the left hand.
pub(in crate::game) fn preview_slot(equip: Option<&str>) -> usize {
    match equip {
        Some("feet") => 2,
        Some("legs") => 3,
        Some("chest") => 4,
        Some("head") => 5,
        _ => 1,
    }
}
/// The `Pose` rotations on the parts of an armor stand model or its armor
/// (`ArmorStandArmorModel.setupAnim`); the body also turns the sticks.
pub(in crate::game) fn apply_pose(poses: &mut Poses, pose: &[[f32; 3]; 6]) {
    let parts: [&[&str]; 6] = [
        &["head"],
        &[
            "body",
            "right_body_stick",
            "left_body_stick",
            "shoulder_stick",
        ],
        &["left_arm"],
        &["right_arm"],
        &["left_leg"],
        &["right_leg"],
    ];
    for (names, rotation) in parts.into_iter().zip(pose) {
        for name in names {
            if poses.has(name) {
                let part = poses.get(name);
                part[3] = rotation[0].to_radians();
                part[4] = rotation[1].to_radians();
                part[5] = rotation[2].to_radians();
            }
        }
    }
}
pub(in crate::game) fn overlaps(a: Aabb, b: Aabb) -> bool {
    let (a0, a1, b0, b1) = (
        a.min_position(),
        a.max_position(),
        b.min_position(),
        b.max_position(),
    );
    a0.x < b1.x && b0.x < a1.x && a0.y < b1.y && b0.y < a1.y && a0.z < b1.z && b0.z < a1.z
}
/// What `Equippable.swapWithEquipmentSlot` leaves once it has passed its
/// checks: the piece worn in the slot, the hand, and the old piece bound for
/// the inventory.
#[derive(Debug, PartialEq)]
pub(in crate::game) struct ArmorSwap {
    pub(in crate::game) worn: ItemStack,
    pub(in crate::game) hand: Option<ItemStack>,
    pub(in crate::game) to_inventory: Option<ItemStack>,
}
/// `Equippable.swapWithEquipmentSlot` after its checks. A single item swaps
/// with the worn piece: the hand gets the worn piece (`copyAndClear`), or
/// keeps the item in creative when nothing was worn; creative wears a copy
/// (`copy` instead of `copyAndClear`). From a larger stack one item is worn
/// (`consumeAndReturn`, which takes none in creative) and the worn piece goes
/// to the inventory.
pub(in crate::game) fn swap_with_equipment_slot(
    stack: &ItemStack,
    current: Option<ItemStack>,
    creative: bool,
) -> ArmorSwap {
    let one = ItemStack {
        count: 1,
        ..stack.clone()
    };
    if stack.count <= 1 {
        let hand = match current {
            Some(old) => Some(old),
            None if creative => Some(stack.clone()),
            None => None,
        };
        return ArmorSwap {
            worn: one,
            hand,
            to_inventory: None,
        };
    }
    let mut held = stack.clone();
    if !creative {
        held.count -= 1;
    }
    ArmorSwap {
        worn: one,
        hand: Some(held),
        to_inventory: current,
    }
}
impl Game<'_> {
    /// `Equippable.swapWithEquipmentSlot`: right-clicking armor swaps it into
    /// its slot (the worn piece comes back to the hand). It runs on a client
    /// too (`Item.use`), as the prediction the server's slot packets confirm.
    pub(in crate::game) fn equip_from_hand(&mut self, stack: &ItemStack) -> bool {
        let Some(info) = self.items().get(&stack.item) else {
            return false;
        };
        let components = self
            .pack
            .item_renderer
            .models
            .components(&sunrise_client::item_render::stack_view(stack));
        let Some(equippable) = components.get("minecraft:equippable") else {
            return false;
        };
        let actual = stack
            .data()
            .and_then(|data| data.other("minecraft:equippable"));
        // Only adopt the default-allowed shape here. Restricted or otherwise
        // shaped overrides keep their existing admission until supported.
        let supported = actual.is_none_or(|tag| {
            matches!(tag, sunrise_core::nbt::Tag::Compound(map)
                if map.keys().all(|key| matches!(key.to_string_lossy().as_str(), "slot" | "swappable")))
                && matches!(tag.get("slot"), Some(sunrise_core::nbt::Tag::String(_)))
                && tag.get("swappable").is_none_or(|flag| {
                    sunrise_client::item_data::component_bool(flag).is_some()
                })
        });
        let slot = if supported {
            equippable.get("slot").and_then(serde_json::Value::as_str)
        } else {
            info.equip_slot.as_deref()
        };
        let (index, slot) = match slot {
            Some("head") => (0, "head"),
            Some("chest") => (1, "chest"),
            Some("legs") => (2, "legs"),
            Some("feet") => (3, "feet"),
            _ => return false,
        };
        // The component codec replaces the whole value: an override which
        // omits swappable uses true, rather than its item's prototype flag.
        let swappable = if !supported {
            info.swappable
        } else if let Some(actual) = actual {
            actual
                .get("swappable")
                .and_then(sunrise_client::item_data::component_bool)
                .unwrap_or(true)
        } else {
            equippable
                .get("swappable")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true)
        };
        if !swappable {
            return false;
        }
        let current = self.armor[index].clone();
        // PREVENT_ARMOR_CHANGE (Curse of Binding) outside creative, or the same item.
        if current.as_ref().is_some_and(|c| {
            (world_stations!(self).enchantments.prevents_armor_change(c) && !self.creative)
                || c.same_item_same_components(stack)
        }) {
            return true;
        }
        self.award_stat("used", &stack.item, 1);
        let swap = swap_with_equipment_slot(stack, current, self.creative);
        self.armor[index] = Some(swap.worn.clone());
        self.player_equipped(slot, Some(&swap.worn));
        *self.held_mut() = swap.hand;
        if let Some(old) = swap.to_inventory
            && let Some(rest) = self.add_stack_to_inventory(old)
            && !self.creative
            && let Some(eye) = self.eye_position()
        {
            self.throw_from_eyes(rest, eye, false, false);
        }
        self.apply_equipment();
        self.swing_arm();
        true
    }
}
/// `ArmorStand.getBbWidth` and `getBbHeight`: a marker has no box, a small stand is half size.
pub(in crate::game) fn stand_box(a: &ArmorStand) -> (f32, f32) {
    match (a.marker, a.small) {
        (true, _) => (0.0, 0.0),
        (false, true) => (0.25, 0.9875),
        (false, false) => (0.5, 1.975),
    }
}
/// `AbstractSkullBlock.getType` of a skull item.
pub(in crate::game) fn skull_kind(item: &str) -> Option<&'static str> {
    Some(match item {
        "minecraft:skeleton_skull" => "skeleton",
        "minecraft:wither_skeleton_skull" => "wither_skeleton",
        "minecraft:zombie_head" => "zombie",
        "minecraft:creeper_head" => "creeper",
        "minecraft:dragon_head" => "dragon",
        "minecraft:piglin_head" => "piglin",
        "minecraft:player_head" => "player",
        _ => return None,
    })
}

/// `ArmorStand.ignoreExplosion` (an invisible stand), and `hurtServer`
/// refusing an invisible or marker stand: such a stand outlasts any blast.
pub(in crate::game::server_side) fn spared_by_blast(a: &ArmorStand) -> bool {
    a.invisible || a.marker
}

/// `ArmorStand.brokenByAnything`: the equipment, less what curse of vanishing
/// keeps (`PREVENT_EQUIPMENT_DROP`), popped at `blockPosition().above()`.
pub(in crate::game) fn broken_armor_stand_equipment(a: ArmorStand) -> ([i32; 3], Vec<ItemStack>) {
    let pos = [
        a.position.x.floor() as i32,
        a.position.y.floor() as i32 + 1,
        a.position.z.floor() as i32,
    ];
    let drops = a
        .equipment
        .into_iter()
        .flatten()
        .filter(|stack| stack.enchantment_level("minecraft:vanishing_curse") == 0)
        .collect();
    (pos, drops)
}
#[cfg(test)]
pub(in crate::game) mod swap_tests {
    use super::*;
    fn stack(item: &str, count: u32) -> ItemStack {
        ItemStack::new(item, count)
    }
    #[test]
    fn disabled_slots_lock_by_filter_flag() {
        let mut a = ArmorStand::new(1, sunrise_core::geometry::Vec3::ZERO, 0.0);
        // Hands need arms.
        assert!(a.is_disabled(0) && a.is_disabled(1));
        a.show_arms = true;
        assert!(!a.is_disabled(0) && !a.is_disabled(1));
        // Bit 4 locks the head (equipment index 5), bit 5 the off hand (1).
        a.disabled_slots = (1 << 4) | (1 << 5);
        assert!(a.is_disabled(5) && a.is_disabled(1));
        assert!(!a.is_disabled(4) && !a.is_disabled(0));
        // Taking (+8) from a filled slot and putting (+16) into an empty one.
        a.disabled_slots = (1 << (3 + 8)) | (1 << (1 + 16));
        assert!(!a.swap_locked(4), "an empty chest slot is not take-locked");
        a.equipment[4] = Some(stack("minecraft:iron_chestplate", 1));
        assert!(a.swap_locked(4));
        assert!(a.swap_locked(2), "an empty feet slot is put-locked");
        a.equipment[2] = Some(stack("minecraft:iron_boots", 1));
        assert!(!a.swap_locked(2));
    }
    #[test]
    fn creative_swap_gives_the_worn_piece_to_the_hand() {
        let swap = swap_with_equipment_slot(
            &stack("minecraft:diamond_chestplate", 1),
            Some(stack("minecraft:iron_chestplate", 1)),
            true,
        );
        assert_eq!(swap.worn, stack("minecraft:diamond_chestplate", 1));
        assert_eq!(swap.hand, Some(stack("minecraft:iron_chestplate", 1)));
        assert_eq!(swap.to_inventory, None);
    }
    #[test]
    fn creative_equip_with_nothing_worn_keeps_the_hand() {
        let swap = swap_with_equipment_slot(&stack("minecraft:diamond_helmet", 1), None, true);
        assert_eq!(swap.worn, stack("minecraft:diamond_helmet", 1));
        assert_eq!(swap.hand, Some(stack("minecraft:diamond_helmet", 1)));
    }
    #[test]
    fn survival_swap_gives_the_worn_piece_or_empties_the_hand() {
        let worn = Some(stack("minecraft:iron_boots", 1));
        let swap = swap_with_equipment_slot(&stack("minecraft:diamond_boots", 1), worn, false);
        assert_eq!(swap.hand, Some(stack("minecraft:iron_boots", 1)));
        let swap = swap_with_equipment_slot(&stack("minecraft:diamond_boots", 1), None, false);
        assert_eq!(swap.hand, None);
    }
    #[test]
    fn a_larger_stack_wears_one_and_sends_the_old_piece_to_the_inventory() {
        // Armour stacks past one only with a max_stack_size component.
        let worn = Some(stack("minecraft:leather_helmet", 1));
        let swap =
            swap_with_equipment_slot(&stack("minecraft:golden_helmet", 3), worn.clone(), false);
        assert_eq!(swap.worn, stack("minecraft:golden_helmet", 1));
        assert_eq!(swap.hand, Some(stack("minecraft:golden_helmet", 2)));
        assert_eq!(swap.to_inventory, worn);
        let swap =
            swap_with_equipment_slot(&stack("minecraft:golden_helmet", 3), worn.clone(), true);
        assert_eq!(swap.hand, Some(stack("minecraft:golden_helmet", 3)));
        assert_eq!(swap.to_inventory, worn);
    }
}
#[cfg(test)]
pub(in crate::game) mod burn_drop_tests {
    use super::*;
    #[test]
    fn only_visible_non_marker_stands_are_broken_by_blasts() {
        let mut stand = ArmorStand::new(1, Vec3::new(0.5, 64.0, 0.5), 0.0);
        assert!(!spared_by_blast(&stand));
        stand.invisible = true;
        assert!(spared_by_blast(&stand));
        stand.invisible = false;
        stand.marker = true;
        assert!(spared_by_blast(&stand));
    }
    #[test]
    fn burnt_equipment_drops_above_the_block_and_vanishing_items_do_not_drop() {
        let mut stand = ArmorStand::new(1, Vec3::new(-0.1, 4.8, 0.0), 0.0);
        let kept = ItemStack::new("minecraft:diamond_boots", 1);
        let mut vanished = ItemStack::new("minecraft:diamond_helmet", 1);
        vanished
            .data_mut()
            .enchantments
            .push(("minecraft:vanishing_curse".into(), 1));
        stand.equipment[0] = Some(kept.clone());
        stand.equipment[1] = Some(vanished);
        let (pos, drops) = broken_armor_stand_equipment(stand);
        assert_eq!(pos, [-1, 5, 0]);
        assert_eq!(drops, vec![kept]);
    }
    #[test]
    fn a_broken_stand_pops_body_and_saddle_after_the_other_six_slots() {
        use sunrise_items::equipment::{BODY, HEAD, MAINHAND, SADDLE};
        let mut stand = ArmorStand::new(1, Vec3::new(0.5, 4.0, 0.5), 0.0);
        let stacks: Vec<_> = [SADDLE, BODY, HEAD, MAINHAND]
            .into_iter()
            .map(|slot| {
                let mut stack = ItemStack::new("minecraft:stone", 1 + slot as u32);
                stack.damage = slot as u32;
                stand.equipment[slot] = Some(stack.clone());
                (slot, stack)
            })
            .collect();
        let (_, drops) = broken_armor_stand_equipment(stand);
        let order = |slots: [usize; 4]| {
            slots
                .map(|s| {
                    stacks
                        .iter()
                        .find(|(slot, _)| *slot == s)
                        .unwrap()
                        .1
                        .clone()
                })
                .to_vec()
        };
        assert_eq!(drops, order([MAINHAND, HEAD, BODY, SADDLE]));
    }
}
#[cfg(test)]
pub(in crate::game) mod push_tests {
    use super::super::minecarts::{CartKind, Minecart};
    use super::*;

    fn boxes(kind: CartKind, cart_at: Vec3) -> (Aabb, Aabb) {
        let stand = ArmorStand::new(1, Vec3::ZERO, 0.0);
        let cart = Minecart::new(2, kind, cart_at);
        (stand.bounds(), cart.bounds())
    }

    #[test]
    fn a_rideable_minecart_inside_the_0_2_gate_is_shoved() {
        // 0.25² is inside the 0.2 gate; 0.5² is not. The function's Some/None is the check.
        let near = Vec3::new(0.25, 0.0, 0.0);
        let far = Vec3::new(0.5, 0.0, 0.0);
        let (stand_box, cart_box) = boxes(CartKind::Rideable, near);
        let shove = armor_stand_cart_shove(
            Vec3::ZERO,
            stand_box,
            near,
            cart_box,
            CartKind::Rideable,
            false,
        )
        .expect("a rideable cart 0.25 away is shoved");
        // AbstractMinecart.push: gap 0.25, length 0.25, pow capped at 1, then 0.1F * 0.5.
        // The cart moves east, away from the stand; the stand takes a quarter west.
        let scale = f64::from(0.1_f32) * 0.5;
        assert_eq!(shove.cart_x, scale);
        assert_eq!(shove.cart_z, 0.0);
        assert_eq!(shove.stand_x, -scale / 4.0);
        assert_eq!(shove.stand_z, 0.0);

        let (stand_box, cart_box) = boxes(CartKind::Rideable, far);
        assert!(
            armor_stand_cart_shove(
                Vec3::ZERO,
                stand_box,
                far,
                cart_box,
                CartKind::Rideable,
                false
            )
            .is_none(),
            "past 0.2 the cart stays"
        );
        for kind in [
            CartKind::Chest,
            CartKind::Hopper,
            CartKind::Furnace,
            CartKind::Tnt,
            CartKind::CommandBlock,
            CartKind::Spawner,
        ] {
            let (stand_box, cart_box) = boxes(kind, near);
            assert!(
                armor_stand_cart_shove(Vec3::ZERO, stand_box, near, cart_box, kind, false)
                    .is_none(),
                "{} is not a rideable minecart",
                kind.id()
            );
        }
        let (stand_box, cart_box) = boxes(CartKind::Rideable, near);
        assert!(
            armor_stand_cart_shove(
                Vec3::ZERO,
                stand_box,
                near,
                cart_box,
                CartKind::Rideable,
                true
            )
            .is_none(),
            "a cart does not shove the stand riding it"
        );
    }
}
#[cfg(test)]
pub(in crate::game) mod layer_color_tests {
    use super::*;
    #[test]
    fn the_smithing_preview_wears_armour_and_holds_the_rest_in_its_off_hand() {
        assert_eq!(preview_slot(Some("head")), 5);
        assert_eq!(preview_slot(Some("chest")), 4);
        assert_eq!(preview_slot(Some("legs")), 3);
        assert_eq!(preview_slot(Some("feet")), 2);
        // A netherite sword has no equippable slot; a saddle or a
        // main-hand item falls to the same default branch.
        for other in [
            None,
            Some("mainhand"),
            Some("offhand"),
            Some("body"),
            Some("saddle"),
        ] {
            assert_eq!(preview_slot(other), 1);
        }
    }

    #[test]
    fn a_dyed_stack_overrides_the_undyed_leather_colour() {
        let mut dyed = ItemStack::new("minecraft:leather_helmet", 1);
        dyed.data_mut().dyed_color = Some(0x1E_90FF);
        // Dyed blue, R and B swapped into the renderer's order, opaque.
        assert_eq!(layer_color(true, Some(0xA06540), Some(&dyed)), 0xFFFF_901E);
        // An undyed stack keeps the asset's brown.
        let plain = ItemStack::new("minecraft:leather_helmet", 1);
        assert_eq!(layer_color(true, Some(0xA06540), Some(&plain)), 0xFF40_65A0);
        // No dye and no undyed colour: white.
        assert_eq!(layer_color(false, None, Some(&plain)), 0xFFFF_FFFF);
        // A layer that isn't dyeable stays white even on a dyed stack.
        assert_eq!(layer_color(false, None, Some(&dyed)), 0xFFFF_FFFF);
    }
}

#[cfg(test)]
pub(in crate::game) mod pumpkin_equippable_use_tests {
    use super::*;
    use std::sync::{atomic::AtomicI32, mpsc};
    use sunrise_client::lan::{Joined, NetConfig, PlayerSender, guest::Guest, hub::HubConfig};
    use sunrise_core::position::ChunkPos;
    use sunrise_protocol::packets::{
        configuration::ClientInformation,
        play::{Serverbound, UseItem},
    };

    #[test]
    #[ignore = "daily"]
    fn h207_connected_use_item_adopts_actual_pumpkin_equippable_and_preserves_controls() {
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.build_registries();
            game.set_allow_commands(true);
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)),
            );
            game.block_ticks
                .add_empty_chunk(ChunkPos::new(0, 0))
                .unwrap();
            assert!(game.run_command("fill 0 199 0 15 199 15 stone"));
            assert!(game.world.loaded(8, 8));
            game.lan_host(
                crate::game::lan::LanSettings {
                    port: 0,
                    guest_commands: true,
                    force_game_mode: false,
                },
                NetConfig {
                    announce: false,
                    ..NetConfig::loopback(0, "pumpkin-use-test".into(), 2)
                },
                HubConfig {
                    default_game_mode: 0,
                    guest_commands: true,
                    force_game_mode: false,
                    hardcore: false,
                    view_distance: 2,
                    simulation_distance: 2,
                    dimension: "minecraft:overworld".into(),
                    allow_flight: false,
                    seed: 0,
                    max_players: 2,
                    integrated: false,
                    levels: Default::default(),
                    level_seeds: Default::default(),
                },
            )
            .unwrap();
            let (tx, _rx) = mpsc::channel();
            let mut guest = Guest::new(
                Joined {
                    conn: 2,
                    uuid: 207,
                    name: "PumpkinGuest".into(),
                    properties: Vec::new(),
                    info: ClientInformation::new(2),
                    brand: "test".into(),
                    sender: PlayerSender::new(tx, Arc::default()),
                    latency: Arc::new(AtomicI32::new(0)),
                    address: "127.0.0.1:25565".parse().unwrap(),
                    expected: false,
                },
                2,
                [8.5, 200.0, 8.5],
                2,
            );
            guest.dimension = "minecraft:overworld".into();
            guest.placed = true;
            guest.loaded = true;
            guest.selected = 0;
            guest.permission = 2;
            game.lan.as_mut().unwrap().hub.guests.push(guest);
            let clock = game.world.game_time;
            let eq = "minecraft:equippable={slot:\"head\"}";
            let marker = "minecraft:custom_data={h207:207}";
            let helmet = "minecraft:diamond_helmet[minecraft:custom_data={old:1}]";
            let mut failures = Vec::new();
            for (i, (label, mode, component, count, old, changed)) in [
            ("empty", "survival", Some(eq), 3, None, true),
            ("swap", "survival", Some(eq), 1, Some(helmet), true),
            ("creative", "creative", Some(eq), 3, None, true),
            ("prototype", "survival", None, 3, None, false),
            ("false", "survival", Some("minecraft:equippable={slot:\"head\",swappable:0b}"), 3, None, false),
            ("binding", "survival", Some(eq), 3, Some("minecraft:diamond_helmet[minecraft:enchantments={\"minecraft:binding_curse\":1},minecraft:custom_data={old:1}]"), false),
            ("same", "survival", Some(eq), 3, Some("minecraft:carved_pumpkin[minecraft:equippable={slot:\"head\"},minecraft:custom_data={h207:207}]"), false),
        ].into_iter().enumerate() {
            let held = format!("minecraft:carved_pumpkin[{}{}{}]", component.unwrap_or(""), if component.is_some() { "," } else { "" }, marker);
            let before = game.as_guest(2, |g| {
                assert_eq!(g.ui.registries.permission, 2);
                assert!(g.run_command("clear @s") || g.inventory.iter().all(Option::is_none));
                g.run_command("item replace entity @s armor.head with air");
                g.run_command(&format!("gamemode {mode}"));
                assert!(g.run_command(&format!("item replace entity @s weapon.mainhand with {held} {count}")));
                if let Some(old) = old { assert!(g.run_command(&format!("item replace entity @s armor.head with {old}"))); }
                assert_eq!(g.player.as_ref().unwrap().position, Vec3::new(8.5, 200.0, 8.5));
                let hand = g.held().unwrap().clone();
                assert_eq!(hand.item.as_ref(), "minecraft:carved_pumpkin");
                assert_eq!(hand.count, count);
                assert!(hand.data().unwrap().other("minecraft:custom_data").is_some());
                let actual = hand.data().unwrap().other("minecraft:equippable");
                assert_eq!(actual.is_some(), component.is_some());
                if let Some(actual) = actual {
                    assert_eq!(actual.get("slot"), Some(&sunrise_core::nbt::Tag::String("head".into())));
                    assert_eq!(actual.get("swappable").and_then(sunrise_client::item_data::component_bool), if label == "false" { Some(false) } else { None });
                }
                if label == "binding" { assert_eq!(g.armor[0].as_ref().unwrap().enchantment_level("minecraft:binding_curse"), 1); }
                if label == "same" { assert!(g.armor[0].as_ref().unwrap().same_item_same_components(&hand)); }
                (g.inventory.clone(), g.armor.clone(), hand)
            }).unwrap();
            let sequence = 207 + i as i32;
            game.lan_guest_packet(2, Serverbound::UseItem(UseItem { hand: 0, yaw: 0.0, pitch: 0.0, sequence }));
            assert!(game.lan.as_ref().unwrap().hub.guest(2).unwrap().ack_up_to.get() >= sequence);
            game.as_guest(2, |g| {
                let mut expected_inventory = before.0.clone();
                let mut expected_armor = before.1.clone();
                if changed {
                    let mut worn = before.2.clone();
                    worn.count = 1;
                    expected_armor[0] = Some(worn);
                    if label == "swap" { expected_inventory[0] = before.1[0].clone(); }
                    else if label == "empty" { expected_inventory[0].as_mut().unwrap().count = 2; }
                }
                if g.inventory != expected_inventory || g.armor != expected_armor {
                    failures.push((label, g.inventory.clone(), g.armor.clone()));
                }
                assert_eq!(g.world.game_time, clock);
            }).unwrap();
        }
            assert!(
                failures.is_empty(),
                "actual connected UseItem Equippable adoption failures: {failures:?}"
            );
        });
    }
}

#[cfg(test)]
pub(in crate::game) mod stand_lodestone_inventory_tick_tests {
    use sunrise_client::motion::PlayerMotion;
    use sunrise_core::{geometry::Vec3, position::BlockPos};

    #[test]
    #[ignore = "daily"]
    fn actual_stand_equipment_compasses_tick_after_loaded_lodestone_removal() {
        let mut resources = None;
        let mut game = crate::game::game_tests::tests::server_game(&mut resources);
        game.build_registries();
        game.set_allow_commands(true);
        game.level.health = 20.0;
        game.player = Some(PlayerMotion::new(game.motion, Vec3::new(14.5, 200.0, 14.5)).unwrap());
        game.block_ticks
            .add_empty_chunk(BlockPos::new(0, 0, 0).chunk())
            .unwrap();
        assert!(game.run_command("forceload add 0 0"));
        assert!(game.run_command("fill 0 199 0 15 199 15 stone"));
        assert!(game.run_command("setblock 12 200 12 lodestone"));
        assert!(game.world.loaded(12, 12));
        let mut ids = Vec::new();
        for (n, dim, tracked, target) in [
            (1, "minecraft:overworld", true, true),
            (2, "minecraft:overworld", false, true),
            (3, "minecraft:the_nether", true, true),
            (4, "minecraft:overworld", true, false),
        ] {
            let who = format!("0000a551-0000-0000-0000-{n:012x}");
            let x = f64::from(n * 2) + 0.5;
            assert!(game.run_command(&format!("summon armor_stand {x} 200.0 4.5 {{UUID:[I;42321,0,0,{n}],Health:20f,NoGravity:1b}}")));
            let id = game
                .armor_stands
                .iter()
                .find(|a| game.persist.uuid_of(a.id) == Some([42321, 0, 0, n]))
                .unwrap()
                .id;
            let tracker = format!(
                "{}tracked:{tracked}",
                if target {
                    format!("target:{{dimension:\"{dim}\",pos:[I;12,200,12]}},")
                } else {
                    String::new()
                }
            );
            for hand in ["mainhand", "offhand"] {
                assert!(game.run_command(&format!("item replace entity {who} weapon.{hand} with compass[minecraft:lodestone_tracker={{{tracker}}},minecraft:custom_data={{marker:{n}}}] 1")));
            }
            let a = game.armor_stands.iter().find(|a| a.id == id).unwrap();
            assert_eq!(a.position, Vec3::new(x, 200.0, 4.5));
            assert!(a.no_gravity);
            assert!(a.first_tick, "fresh actual stand has not ticked yet");
            assert_eq!(a.health, 20.0);
            for stack in a.equipment.iter().take(2).flatten() {
                assert_eq!(&*stack.item, "minecraft:compass");
                assert_eq!(stack.count, 1);
                assert_eq!(
                    stack
                        .data()
                        .unwrap()
                        .other("minecraft:lodestone_tracker")
                        .unwrap()
                        .get("target")
                        .is_some(),
                    target
                );
            }
            assert!(game.persist.raw.remove(&id).is_some());
            ids.push((id, n, target));
        }
        for hand in ["mainhand", "offhand"] {
            assert!(game.run_command(&format!("item replace entity @s weapon.{hand} with compass[minecraft:lodestone_tracker={{target:{{dimension:\"minecraft:overworld\",pos:[I;12,200,12]}},tracked:true}}] 1")));
        }
        assert!(game.run_command("setblock 12 200 12 air"));
        assert_eq!(
            &*game.res.facts().get(game.world.block(12, 200, 12)).name,
            "minecraft:air"
        );
        game.world.game_time += 1;
        game.tick_lodestone_compasses();
        for slot in [0, crate::game::shield::OFFHAND_SLOT] {
            assert!(
                game.stack_in(slot)
                    .unwrap()
                    .data()
                    .unwrap()
                    .other("minecraft:lodestone_tracker")
                    .unwrap()
                    .get("target")
                    .is_none(),
                "actual player tick positive control"
            );
        }
        assert!(
            game.entity_ticking_area()(2, 4),
            "actual entity-ticking chunk admission"
        );
        game.tick_armor_stands().unwrap();
        let mut mismatches = Vec::new();
        for (id, n, target) in ids {
            let a = game.armor_stands.iter().find(|a| a.id == id).unwrap();
            assert_eq!(a.position, Vec3::new(f64::from(n * 2) + 0.5, 200.0, 4.5));
            assert_eq!(a.health, 20.0);
            assert!(a.no_gravity);
            for (slot, stack) in a.equipment.iter().take(2).enumerate() {
                let stack = stack.as_ref().unwrap();
                assert_eq!(stack.count, 1);
                let data = stack.data().unwrap();
                assert_eq!(
                    data.other("minecraft:custom_data").unwrap().get("marker"),
                    Some(&sunrise_core::nbt::Tag::Int(n))
                );
                let present = data
                    .other("minecraft:lodestone_tracker")
                    .unwrap()
                    .get("target")
                    .is_some();
                if present != (target && n != 1) {
                    mismatches.push((n, slot, present));
                }
            }
            let saved = sunrise_client::entity_nbt::armor_stand_to_nbt(
                a,
                [42321, 0, 0, n],
                None,
                &Default::default(),
            );
            assert!(
                saved.get("equipment").is_some(),
                "actual no-raw model writer"
            );
            for slot in 0..2 {
                let key = sunrise_client::entities::ArmorStand::SLOTS[slot];
                let saved_target = saved
                    .get("equipment")
                    .unwrap()
                    .get(key)
                    .unwrap()
                    .get("components")
                    .unwrap()
                    .get("minecraft:lodestone_tracker")
                    .unwrap()
                    .get("target")
                    .is_some();
                let modeled_target = a.equipment[slot]
                    .as_ref()
                    .unwrap()
                    .data()
                    .unwrap()
                    .other("minecraft:lodestone_tracker")
                    .unwrap()
                    .get("target")
                    .is_some();
                assert_eq!(
                    saved_target, modeled_target,
                    "no-raw writer preserves actual tracker state"
                );
            }
        }
        assert!(
            mismatches.is_empty(),
            "actual stand equipment inventory tick mismatches: {mismatches:?}"
        );
    }
}

#[cfg(test)]
pub(in crate::game) mod postbaseline_lodestone_controls {
    //! Added after baseline: source-backed slots and preservation, not new runtime breadth.
    use super::Game;
    use std::sync::Arc;
    use sunrise_client::{chunk::Chunk, entities::ArmorStand, items::ItemStack};
    use sunrise_core::position::BlockPos;

    fn stack<'g>(game: &'g Game<'_>, id: u32, slot: usize) -> &'g ItemStack {
        game.armor_stands
            .iter()
            .find(|a| a.id == id)
            .unwrap()
            .equipment[slot]
            .as_ref()
            .unwrap()
    }
    fn has_target(stack: &ItemStack) -> bool {
        stack
            .data()
            .unwrap()
            .other("minecraft:lodestone_tracker")
            .unwrap()
            .get("target")
            .is_some()
    }
    fn item(id: &str, tracker: &str, marker: usize, count: usize) -> String {
        format!(
            "{{id:\"{id}\",count:{count},components:{{\"minecraft:lodestone_tracker\":{{{tracker}}},\"minecraft:custom_data\":{{marker:{marker}}}}}}}"
        )
    }

    #[test]
    #[ignore = "daily"]
    fn postbaseline_six_slots_noncompass_guard_and_dimension_preservation() {
        let mut resources = None;
        let mut game = crate::game::game_tests::tests::server_game(&mut resources);
        game.build_registries();
        game.set_allow_commands(true);
        game.block_ticks
            .add_empty_chunk(BlockPos::new(0, 0, 0).chunk())
            .unwrap();
        assert!(game.run_command("forceload add 0 0"));
        assert!(game.run_command("fill 0 199 0 15 199 15 stone"));
        assert!(game.run_command("setblock 12 200 12 lodestone"));
        let tracked = "target:{dimension:\"minecraft:overworld\",pos:[I;12,200,12]},tracked:true";
        let slots = ArmorStand::SLOTS
            .iter()
            .enumerate()
            .map(|(slot, key)| format!("{key}:{}", item("minecraft:compass", tracked, slot, 1)))
            .collect::<Vec<_>>()
            .join(",");
        assert!(game.run_command(&format!("summon armor_stand 4.5 200.0 4.5 {{UUID:[I;42321,0,0,100],Health:20f,NoGravity:1b,equipment:{{{slots}}}}}")));
        let mixed = format!(
            "mainhand:{},offhand:{},head:{},feet:{}",
            item("minecraft:stone", tracked, 66, 3),
            item(
                "minecraft:compass",
                "target:{dimension:\"minecraft:the_nether\",pos:[I;12,200,12]},tracked:true",
                77,
                1
            ),
            item(
                "minecraft:compass",
                "target:{dimension:\"minecraft:overworld\",pos:[I;12,200,12]},tracked:false",
                88,
                1
            ),
            item("minecraft:compass", "tracked:true", 99, 1)
        );
        assert!(game.run_command(&format!("summon armor_stand 8.5 200.0 8.5 {{UUID:[I;42321,0,0,101],Health:20f,NoGravity:1b,equipment:{{{mixed}}}}}")));
        game.world
            .chunks
            .insert((4, 4), Arc::new(Chunk::empty(4, 4)));
        game.block_ticks
            .add_empty_chunk(BlockPos::new(64, 0, 64).chunk())
            .unwrap();
        game.load_ticking_neighbourhood(0, 0);
        assert!(game.run_command("fill 64 199 64 79 199 79 stone"));
        assert!(game.run_command(&format!("summon armor_stand 65.5 200.0 65.5 {{UUID:[I;42321,0,0,102],Health:20f,NoGravity:1b,equipment:{{mainhand:{}}}}}",item("minecraft:compass",tracked,100,1))));
        let id = |n| {
            game.armor_stands
                .iter()
                .find(|a| game.persist.uuid_of(a.id) == Some([42321, 0, 0, n]))
                .unwrap()
                .id
        };
        let (all, mixed, inactive) = (id(100), id(101), id(102));
        // Direct class ticks do not refresh the headless fixture's tickets.
        // Use the existing ticket model: only chunk zero is forced to tick.
        game.tickets.0.update(Some(crate::ticket_levels::Sources {
            forced: vec![(0, 0)],
            ..Default::default()
        }));
        assert!(game.entity_ticking_area()(4, 4));
        assert!(!game.entity_ticking_area()(65, 65));
        assert!(game.world.loaded(65, 65));
        let before: Vec<_> = (0..6)
            .map(|slot| {
                game.armor_stands
                    .iter()
                    .find(|a| a.id == mixed)
                    .unwrap()
                    .equipment[slot]
                    .as_ref()
                    .map(sunrise_core::nbt::Tag::from)
            })
            .collect();
        assert!(has_target(stack(&game, mixed, 0)));
        assert!(has_target(stack(&game, mixed, 1)));
        assert!(has_target(stack(&game, mixed, 5)));
        assert!(!has_target(stack(&game, mixed, 2)));
        assert!(has_target(stack(&game, inactive, 0)));
        for slot in 0..6 {
            assert!(has_target(stack(&game, all, slot)));
        }
        let dimension = game.dim;
        let chunks: std::collections::HashSet<_> = game.world.chunks.keys().copied().collect();
        assert!(game.run_command("setblock 12 200 12 air"));
        game.world.game_time += 1;
        game.tick_armor_stands().unwrap();
        assert!(
            !game
                .armor_stands
                .iter()
                .find(|a| a.id == all)
                .unwrap()
                .first_tick,
            "active stand really ticked"
        );
        assert!(
            game.armor_stands
                .iter()
                .find(|a| a.id == inactive)
                .unwrap()
                .first_tick,
            "non-ticking guard unchanged"
        );
        for slot in 0..6 {
            let s = stack(&game, all, slot);
            assert!(!has_target(s), "source-backed equipment slot {slot}");
            assert_eq!(s.count, 1);
            assert_eq!(
                s.data()
                    .unwrap()
                    .other("minecraft:custom_data")
                    .unwrap()
                    .get("marker"),
                Some(&sunrise_core::nbt::Tag::Int(slot as i32))
            );
        }
        let after: Vec<_> = (0..6)
            .map(|slot| {
                game.armor_stands
                    .iter()
                    .find(|a| a.id == mixed)
                    .unwrap()
                    .equipment[slot]
                    .as_ref()
                    .map(sunrise_core::nbt::Tag::from)
            })
            .collect();
        assert_eq!(
            before, after,
            "actual non-compass/untracked/other-dimension/targetless stacks preserved"
        );
        assert!(has_target(stack(&game, inactive, 0)));
        assert_eq!(game.dim, dimension);
        assert_eq!(
            game.world
                .chunks
                .keys()
                .copied()
                .collect::<std::collections::HashSet<_>>(),
            chunks
        );
    }
}

#[cfg(test)]
pub(in crate::game) mod setup_anim_tests {
    #[allow(unused_imports)]
    use super::*;
    #[allow(unused_imports)]
    use std::sync::atomic::{AtomicUsize, Ordering};

    pub(in crate::game) static HEARD: AtomicUsize = AtomicUsize::new(0);
    pub(in crate::game) struct Listener;
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Listener {
        fn humanoid_setup_anim(
            &self,
            _: &dyn mod_api::ClientHost,
            event: &mut mod_api::HumanoidSetupAnimEvent<'_>,
        ) {
            use mod_api::HumanoidPart;
            assert_eq!(event.entity_id, 73);
            assert_eq!(event.entity_type, TYPE_ID);
            assert_eq!(event.partial_tick, 0.5);
            assert_eq!(
                event.parts.get(HumanoidPart::Head).unwrap()[1],
                [30.0_f32.to_radians(), 0.0, 0.0]
            );
            let right = event.parts.get(HumanoidPart::RightArm).unwrap()[1];
            assert!((right[0] + std::f32::consts::PI / 5.0).abs() < 1e-6);
            assert_eq!(
                event.parts.get(HumanoidPart::RightLeg).unwrap()[1],
                [-1.413_716_7, std::f32::consts::PI / 10.0, 0.078_539_82]
            );
            assert!(!event.humanoid.is_using_item);
            for part in [
                HumanoidPart::Head,
                HumanoidPart::Body,
                HumanoidPart::LeftArm,
                HumanoidPart::RightArm,
                HumanoidPart::LeftLeg,
                HumanoidPart::RightLeg,
            ] {
                assert!(event.parts.set_rotation(part, [1.0, 2.0, 3.0]));
                assert!(event.parts.set_position(part, [7.0, 8.0, 9.0]));
            }
            HEARD.fetch_add(1, Ordering::Relaxed);
        }
    }
}
