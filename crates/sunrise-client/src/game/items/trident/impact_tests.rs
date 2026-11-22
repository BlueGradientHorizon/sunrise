use super::*;
use crate::game::{
    native_entity_refs::tests::run_fresh_cases,
    native_projectiles::impact_probe::{Action, with_probe},
};
use std::sync::atomic::Ordering;
use sunrise_client::mobs::Mob;

#[test]
#[ignore = "daily"]
fn a_discarded_trident_does_not_apply_its_captured_impact() {
    const CASE: &str = "SUNRISE_TRIDENT_DISCARD_CASE";
    if run_fresh_cases(
        "game::trident::impact_tests::a_discarded_trident_does_not_apply_its_captured_impact",
        CASE,
        &["block", "entity"],
    ) {
        return;
    }
    let pig = std::env::var(CASE).as_deref() == Ok("entity");
    // Probe returns Continue. A live hit would stick in the block or damage the pig.
    with_probe(Action::Discard, true, |game, observed| {
        if pig {
            let kind = game.res.mob_types().get("minecraft:pig").unwrap().clone();
            game.mobs
                .push(Mob::new(72, kind, Vec3::new(3.0, 64.0, 0.5), 0.0));
        } else {
            let stone = game
                .pack
                .assets
                .registry
                .default_state("minecraft:stone")
                .unwrap();
            game.world.set(2, 64, 0, stone).unwrap();
        }
        let water = game
            .pack
            .assets
            .registry
            .default_state("minecraft:water")
            .unwrap();
        game.world.set(1, 64, 0, water).unwrap();
        let mut t = ThrownTrident::remote(71, Vec3::new(0.5, 64.5, 0.5), 0.0, 0.0);
        t.velocity = Vec3::new(4.0, 0.0, 0.0);
        t.fire_ticks = 100;
        t.owner = 7;
        t.left_owner = true;
        t.loyalty = 1;
        t.pickup = Pickup::Allowed;
        game.tridents.push(t);
        let handle = game.entity(71).unwrap();
        game.native_joins_skip();
        game.audio.capture = Some(Vec::new());
        game.tick_tridents();
        assert!(game.entity_position(handle).is_none());
        assert_eq!(game.tridents[0].fire_ticks, 0);
        game.native_leaves_flush();
        game.tick_tridents();
        game.native_leaves_flush();
        assert!(game.tridents.is_empty());
        assert!(game.dropped.is_empty());
        assert_eq!(*observed.leaves.lock().unwrap(), [71]);
        assert_eq!(observed.lands.load(Ordering::Relaxed), 0);
        assert_eq!(observed.deaths.load(Ordering::Relaxed), 0);
        let hits = observed.hits.lock().unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].projectile, 71);
        assert_eq!(hits[0].kind, "minecraft:trident");
        assert_eq!(hits[0].entity, pig.then_some(72));
        assert_eq!(hits[0].projectile_pos, hits[0].at);
        if pig {
            assert_eq!(game.mobs[0].health, 10.0);
        } else {
            assert_eq!(hits[0].block, [2, 64, 0]);
        }
        let sounds = game.audio.capture.as_ref().unwrap();
        assert_eq!(
            sounds
                .iter()
                .filter(|sound| { sound.event == "minecraft:entity.generic.extinguish_fire" })
                .count(),
            1
        );
        assert!(sounds.iter().all(|sound| {
            !matches!(
                sound.event.as_str(),
                "minecraft:item.trident.hit"
                    | "minecraft:item.trident.hit_ground"
                    | "minecraft:item.trident.return"
            )
        }));
    });
}
