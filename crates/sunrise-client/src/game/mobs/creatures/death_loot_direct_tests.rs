//! `DamageSource.isDirect` in a death's loot: a blow the attacker dealt
//! through another entity (a thrown trident, a splash potion, primed TNT) is
//! not direct.
use sunrise_client::mobs::{Damage, DamageKind, Who};
use sunrise_world::loot::{EntityLootParams, LootData, LootEntity, PackOverlay};

/// What a table that drops a stick only for indirect damage
/// (`damage_source_properties` with `is_direct: false`) drops for this death.
fn indirect_only_drops(damage: Damage, attacker: Option<LootEntity>) -> Vec<String> {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("items.json"), r#"{"minecraft:stick":{}}"#).unwrap();
    let defaults = sunrise_world::item_defaults::read(root.path()).unwrap();
    let table = serde_json::json!({"pools": [{
        "rolls": 1,
        "condition": {
            "type": "minecraft:damage_source_properties",
            "predicate": {"is_direct": false}
        },
        "entries": [{"type": "minecraft:item", "name": "minecraft:stick"}]
    }]});
    let files = vec![(
        "loot_table/test:indirect".to_owned(),
        vec![serde_json::to_vec(&table).unwrap()],
    )];
    let loot = LootData::load(root.path(), &PackOverlay::new(&files), &defaults).unwrap();
    let params = EntityLootParams {
        damage: Some(super::loot_damage(damage, attacker)),
        ..EntityLootParams::default()
    };
    let mut random = sunrise_core::random::LegacyRandom::new(1);
    loot.roll_entity("test:indirect", &mut random, &params)
        .iter()
        .map(|stack| stack.item.to_string())
        .collect()
}

#[test]
fn a_kill_through_a_thrown_trident_potion_or_tnt_is_not_direct() {
    let player = || Some(LootEntity::new("minecraft:player"));
    let mut trident = Damage::of(DamageKind::Thrown).with_direct("minecraft:trident");
    trident.attacker = Some(Who::Player);
    let potion = Damage::indirect_magic("minecraft:splash_potion", Some(Who::Player), [0.0; 3]);
    let mut tnt = Damage::of(DamageKind::Explosion).with_direct("minecraft:tnt");
    tnt.attacker = Some(Who::Player);
    // Primed TNT nobody lit: a direct entity and no attacker.
    let unlit = Damage::of(DamageKind::Explosion).with_direct("minecraft:tnt");
    for (damage, attacker) in [
        (trident, player()),
        (potion, player()),
        (tnt, player()),
        (unlit, None),
    ] {
        assert_eq!(
            indirect_only_drops(damage, attacker),
            ["minecraft:stick"],
            "{damage:?}"
        );
    }
    // The player's own blow, and a fall with no entity at all, are direct.
    assert!(indirect_only_drops(Damage::player([0.0; 3]), player()).is_empty());
    assert!(indirect_only_drops(Damage::fall(), None).is_empty());
}
