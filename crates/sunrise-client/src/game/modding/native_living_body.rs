//! A living entity's reads, one function per field over its borrowed body
//! ([`LivingBody`]): the `LivingRef` reads (docs/modding/src/how-to/native-mods.md, "Entity
//! and level handles") find the body their handle names and call these, and a hit's
//! `LivingView` calls the same ones on the body the hit lends, so both answer alike.
use super::native_living_effects::{active_effect, mob_effect};
use mod_api::{ActiveEffect, EquipmentSlot};
use std::sync::Arc;
use sunrise_sim::health::{FixedBody, LivingBody, PlayerBody, PlayerHealthRead};
use sunrise_sim::mobs::finalize::{self, EquipmentRead};
use sunrise_world::attribute_map::ValueKind;
use sunrise_world::attributes::entity::Catalog;

/// `LivingEntity.getHealth`.
pub(in crate::game) fn body_health(body: LivingBody<'_>) -> Option<f32> {
    match body {
        LivingBody::Mob(m) => Some(m.health),
        LivingBody::Player(p) => match p.health {
            PlayerHealthRead::Synced(data, ids) => data.get(ids.health).ok(),
            PlayerHealthRead::Record { health, .. } => Some(*health),
        },
        // An armor stand carries the fixed 20 the game gives it: lava
        // does not hurt it and splash water reads the very field.
        LivingBody::Stand(s) | LivingBody::Dragon(s) => Some(*s.health),
    }
}

/// `LivingEntity.getAbsorptionAmount`: none for an armor stand or the dragon, which carry none.
pub(in crate::game) fn body_absorption(body: LivingBody<'_>) -> Option<f32> {
    match body {
        LivingBody::Mob(m) => Some(m.absorption),
        LivingBody::Player(p) => match p.health {
            PlayerHealthRead::Synced(data, ids) => data.get(ids.absorption).ok(),
            PlayerHealthRead::Record {
                body,
                saved_absorption,
                ..
            } => Some(body.map_or(saved_absorption.unwrap_or(0.0), |h| h.absorption())),
        },
        LivingBody::Stand(_) | LivingBody::Dragon(_) => None,
    }
}

/// `LivingEntity.getMaxHealth`. Players read their attribute map; a mob, an armor stand and
/// the dragon their own fields (the armor stand the fixed 20 the game seeds it with, the
/// dragon its 200) until live attribute maps cover them.
pub(in crate::game) fn body_max_health(body: LivingBody<'_>) -> Option<f32> {
    match body {
        LivingBody::Mob(m) => Some(m.attrs.max_health),
        LivingBody::Player(p) => {
            player_attribute(p, "minecraft:max_health", ValueKind::Computed).map(|max| max as f32)
        }
        LivingBody::Stand(_) => Some(20.0),
        LivingBody::Dragon(_) => Some(super::dragon::MAX_HEALTH),
    }
}

/// `AttributeMap.getValue` or `getBaseValue` of `name`: None for an attribute the entity
/// type does not have. A mob, an armor stand and the dragon answer from their live attribute
/// map, built from `catalog` on the first read.
pub(in crate::game) fn body_attribute(
    body: LivingBody<'_>,
    catalog: &Arc<Catalog>,
    name: &str,
    kind: ValueKind,
) -> Option<f64> {
    match body {
        LivingBody::Mob(m) => m.extra.live.read(catalog, &m.kind.id, name, kind, || {
            sunrise_sim::entity_nbt::attribute_entries(m)
                .into_iter()
                .map(|(_, entry)| entry)
                .collect()
        }),
        LivingBody::Player(p) => player_attribute(p, name, kind),
        LivingBody::Stand(s) => s
            .live
            .read(catalog, "minecraft:armor_stand", name, kind, Vec::new),
        LivingBody::Dragon(d) => {
            d.live
                .read(catalog, "minecraft:ender_dragon", name, kind, Vec::new)
        }
    }
}

pub(in crate::game) fn player_attribute(
    player: PlayerBody<'_>,
    name: &str,
    kind: ValueKind,
) -> Option<f64> {
    let attributes = player.attributes?;
    let key = attributes.key(name)?;
    attributes.value(Some(key), kind).ok()
}

/// `LivingEntity.getEffect` of the effect at registry index `index`: none for an armor stand
/// or the dragon, which keep no effects.
pub(in crate::game) fn body_effect(body: LivingBody<'_>, index: usize) -> Option<ActiveEffect> {
    match body {
        LivingBody::Mob(m) => {
            let e = m
                .extra
                .effects
                .iter()
                .find(|e| e.registry_index == Some(index))?;
            Some(mob_effect(m, e))
        }
        LivingBody::Player(p) => p.effects?.get_registered(index).map(active_effect),
        LivingBody::Stand(_) | LivingBody::Dragon(_) => None,
    }
}

/// `LivingEntity.getActiveEffects`, in the order their store keeps them.
pub(in crate::game) fn body_effects(body: LivingBody<'_>) -> Option<Vec<ActiveEffect>> {
    match body {
        LivingBody::Mob(m) => Some(m.extra.effects.iter().map(|e| mob_effect(m, e)).collect()),
        LivingBody::Player(p) => Some(p.effects?.list.iter().map(active_effect).collect()),
        LivingBody::Stand(_) | LivingBody::Dragon(_) => None,
    }
}

/// `LivingEntity.hasEffect` of the effect at registry index `index`.
pub(in crate::game) fn body_potion_affected(body: LivingBody<'_>, index: usize) -> bool {
    match body {
        LivingBody::Mob(m) => m
            .extra
            .effects
            .iter()
            .any(|e| e.registry_index == Some(index)),
        LivingBody::Player(p) => p
            .effects
            .is_some_and(|effects| effects.get_registered(index).is_some()),
        LivingBody::Stand(_) | LivingBody::Dragon(_) => false,
    }
}

/// `LivingEntity.getItemBySlot`: the live stack, never a copy.
pub(in crate::game) fn body_equipment(
    body: LivingBody<'_>,
    slot: EquipmentSlot,
) -> Option<EquipmentRead<'_>> {
    let stack = match body {
        LivingBody::Mob(m) => return finalize::equipment_slot(m, slot),
        LivingBody::Player(p) => *p.equipment.get(slot as usize)?,
        LivingBody::Stand(s) | LivingBody::Dragon(s) => s.equipment.get(slot as usize),
    }?;
    Some(EquipmentRead::resident(stack))
}

/// The type a living body is of (`Entity.getType`), by name: the one mapping the full host's
/// `entity_kind` and a hit's view both resolve.
pub(in crate::game) fn body_kind_name<'a>(body: LivingBody<'a>) -> &'a str {
    match body {
        LivingBody::Mob(m) => &m.kind.id,
        LivingBody::Player(_) => PLAYER,
        LivingBody::Stand(_) => ARMOR_STAND,
        LivingBody::Dragon(_) => ENDER_DRAGON,
    }
}

/// A player's, an armor stand's and the dragon's type names.
pub(in crate::game) const PLAYER: &str = "minecraft:player";
pub(in crate::game) const ARMOR_STAND: &str = "minecraft:armor_stand";
pub(in crate::game) const ENDER_DRAGON: &str = "minecraft:ender_dragon";

/// The registry key of the type a living body is of (`Entity.getType`), as the accepted
/// resources resolved it: a mob's type keeps its own, the other three are in `keys`. The one read
/// both the full host's `entity_kind` and a hit's view make.
pub(in crate::game) fn body_kind(
    body: LivingBody<'_>,
    keys: sunrise_sim::health::LivingTypeKeys,
) -> Option<mod_api::EntityTypeKey> {
    match body {
        LivingBody::Mob(m) => m.kind.type_key,
        LivingBody::Player(_) => keys.player,
        LivingBody::Stand(_) => keys.armor_stand,
        LivingBody::Dragon(_) => keys.ender_dragon,
    }
}

/// The holder of a living body's attachments: a player's by UUID where its borrower knows it,
/// any other entity's by id.
pub(in crate::game) fn body_holder(body: LivingBody<'_>) -> Option<mod_api::Holder> {
    Some(match body {
        LivingBody::Mob(m) => mod_api::Holder::Entity(m.id),
        LivingBody::Player(p) => mod_api::Holder::Player(p.uuid?),
        LivingBody::Stand(s) | LivingBody::Dragon(s) => mod_api::Holder::Entity(s.id),
    })
}

/// `Mob.isNoAi`: None for anything but a mob.
pub(in crate::game) fn body_no_ai(body: LivingBody<'_>) -> Option<bool> {
    match body {
        LivingBody::Mob(m) => Some(m.no_ai),
        _ => None,
    }
}

/// An armor stand's body.
pub(in crate::game) fn stand_body(a: &sunrise_sim::entities::ArmorStand) -> LivingBody<'_> {
    LivingBody::Stand(FixedBody {
        id: a.id,
        health: &a.health,
        live: &a.live,
        equipment: &a.equipment,
    })
}

/// The dragon's body.
pub(in crate::game) fn dragon_body(d: &super::dragon::Dragon) -> LivingBody<'_> {
    LivingBody::Dragon(FixedBody {
        id: d.id,
        health: &d.health,
        live: &d.live,
        equipment: &d.equipment,
    })
}
