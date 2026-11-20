use crate::game::{Game, native_entity_refs::tests::with_game};
use sunrise_client::{
    health::{damage_type, source_damage_type},
    mobs::{Damage, DamageKind, NamedDamage as Tags, Who, gear},
};
use sunrise_world::damage_source::Catalog;

#[test]
fn formerly_aliased_types_keep_their_id_and_registry_tags() {
    let built_in = [
        ("minecraft:indirect_magic", DamageKind::IndirectMagic),
        ("minecraft:fly_into_wall", DamageKind::FlyIntoWall),
    ];
    let named = [
        (
            "minecraft:dragon_breath",
            Tags::BYPASSES_ARMOR | Tags::NO_KNOCKBACK,
        ),
        (
            "minecraft:mob_projectile",
            Tags::IS_PROJECTILE | Tags::SULFUR_CUBE_WITH_BLOCK_IMMUNE_TO,
        ),
        (
            "minecraft:unattributed_fireball",
            Tags::IS_FIRE | Tags::IS_PROJECTILE,
        ),
        (
            "minecraft:explosion",
            Tags::IS_EXPLOSION | Tags::NO_KNOCKBACK,
        ),
        (
            "minecraft:player_explosion",
            Tags::IS_EXPLOSION | Tags::NO_KNOCKBACK,
        ),
        (
            "minecraft:bad_respawn_point",
            Tags::IS_EXPLOSION | Tags::NO_KNOCKBACK,
        ),
        (
            "minecraft:ender_pearl",
            Tags::IS_FALL | Tags::BYPASSES_ARMOR | Tags::NO_KNOCKBACK,
        ),
    ];
    // Model only the tags relevant to these cases. The raw tag fixture and
    // expected flags are separate so an omitted or misread tag fails the test.
    let types: Vec<_> = named
        .iter()
        .map(|(id, _)| *id)
        .chain(built_in.iter().map(|(id, _)| *id))
        .map(|id| {
            serde_json::json!({
                "id": id, "message_id": "test", "exhaustion": "00000000",
                "scaling": "never", "effects": "hurt", "death_message_type": "default"
            })
        })
        .collect();
    let tags = serde_json::json!({
        "minecraft:bypasses_armor": r#"{"values":["dragon_breath","ender_pearl"]}"#,
        "minecraft:no_knockback": r#"{"values":["dragon_breath","explosion","player_explosion","bad_respawn_point","ender_pearl"]}"#,
        "minecraft:is_projectile": r#"{"values":["mob_projectile","unattributed_fireball"]}"#,
        "minecraft:is_fire": r#"{"values":["unattributed_fireball"]}"#,
        "minecraft:is_explosion": r#"{"values":["explosion","player_explosion","bad_respawn_point"]}"#,
        "minecraft:is_fall": r#"{"values":["ender_pearl"]}"#,
        "minecraft:sulfur_cube_with_block_immune_to": r#"{"values":["mob_projectile"]}"#,
    });
    let catalog = Catalog::from_json(
        &serde_json::json!({"data_version": sunrise_world::DATA_VERSION, "types": types}),
        &tags,
    )
    .unwrap();

    for (id, expected_kind) in built_in.into_iter().chain(
        named
            .into_iter()
            .map(|(id, tags)| (id, DamageKind::Named(Tags::new(id, tags)))),
    ) {
        let kind = Game::damage_kind(&catalog, id);
        assert_eq!(kind, expected_kind);
        assert_eq!(damage_type(kind, false), id);
        assert_eq!(damage_type(kind, true), id);
    }
    let collision = Damage::of(Game::damage_kind(&catalog, "minecraft:fly_into_wall"));
    assert!(collision.bypasses_armor() && collision.no_knockback());
    assert!(!collision.causes_panic());
    let magic = Damage::of(Game::damage_kind(&catalog, "minecraft:indirect_magic"));
    assert!(magic.bypasses_armor() && magic.kind.witch_resistant_to());
    assert!(!magic.no_knockback());
}

#[test]
#[ignore = "daily"]
fn every_world_damage_type_round_trips_with_or_without_an_attacker() {
    with_game(|game| {
        let catalog = &game.health_resources.damage;
        for (_, definition) in catalog.entries() {
            let kind = Game::damage_kind(catalog, &definition.name);
            assert_eq!(damage_type(kind, false), definition.name);
            assert_eq!(damage_type(kind, true), definition.name);
        }
    });
}

#[test]
#[ignore = "daily"]
fn damage_command_uses_the_worlds_fall_tags() {
    with_game(|game| {
        game.build_registries();
        game.as_console(|game| {
            for (id, ty, health) in [
                (1, "minecraft:ender_pearl", 7.4),
                (2, "minecraft:fly_into_wall", 5.0),
            ] {
                assert!(game.run_command(&format!(
                    "summon pig 0.5 64 0.5 {{UUID:[I;83,0,0,{id}],NoAI:1b}}"
                )));
                gear::upgrade(
                    game.mobs.last_mut().unwrap(),
                    gear::FEET,
                    "minecraft:feather_falling",
                    4,
                );
                assert!(
                    game.run_command(&format!("damage 00000053-0000-0000-0000-{id:012x} 5 {ty}"))
                );
                let pig = game.mobs.last().unwrap();
                assert_eq!(pig.health, health, "{ty}");
                assert_eq!(source_damage_type(pig.last_damage.unwrap().0), ty);
            }
        });
    });
}

#[test]
#[ignore = "daily"]
fn damage_command_keeps_explicit_explosion_with_an_attacker() {
    with_game(|game| {
        game.build_registries();
        game.as_console(|game| {
            assert!(game.run_command("summon pig 0.5 64 0.5 {UUID:[I;83,0,0,1],NoAI:1b}"));
            assert!(game.run_command("summon pig 2.5 64 0.5 {UUID:[I;83,0,0,2],NoAI:1b}"));
            assert!(game.run_command(
                "damage 00000053-0000-0000-0000-000000000001 1 minecraft:explosion by 00000053-0000-0000-0000-000000000002"
            ));
            let hit = game.mobs[0].last_damage.unwrap().0;
            assert_eq!(hit.attacker, Some(Who::Mob(game.mobs[1].id)));
            assert_eq!(source_damage_type(hit), "minecraft:explosion");
        });
    });
}
