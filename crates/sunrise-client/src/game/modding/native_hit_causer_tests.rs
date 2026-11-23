//! A hit's listener reads its causing entity (`DamageSource.getEntity`) at the damage sites
//! beyond melee: a projectile's shooter, an explosion's igniter, the wearer of Thorns, a
//! pufferfish stinging a mob inside its own tick, and a parked guest whose blast it is.
use super::native_entity_refs::tests::{bind_local_player, run_fresh_cases, with_game};
use super::native_living_refs::tests::summon_mob;
use mod_api::{Flow, HitHost, Hooks, LivingIncomingDamageEvent};
use std::sync::{Arc, Mutex};
use sunrise_client::command::args::GameMode;
use sunrise_client::mobs::{Damage, DamageKind, Who};
use sunrise_client::motion::PlayerMotion;
use sunrise_core::geometry::Vec3;

/// One `living_incoming_damage` call: the hit entity, the causing id its source names, and
/// what `causing()` read of it (its id and health).
#[derive(Clone, Debug, PartialEq)]
struct Seen {
    entity: i32,
    source_causing: i32,
    causing: Option<(u32, Option<f32>)>,
}

struct Probe(Arc<Mutex<Vec<Seen>>>);

#[mod_api::__subscriptions]
impl Hooks for Probe {
    fn living_incoming_damage(
        &self,
        level: &mut dyn HitHost,
        event: &mut LivingIncomingDamageEvent<'_>,
    ) -> Flow {
        let causing = level.causing().map(|view| (view.id(), view.health()));
        self.0.lock().unwrap().push(Seen {
            entity: event.entity,
            source_causing: event.source.causing,
            causing,
        });
        Flow::Continue
    }
}

/// The `LivingRef` health of entity `id`, which the view must agree with.
fn health(game: &super::Game<'_>, id: u32) -> Option<f32> {
    game.living(game.entity(id)?)?.health(game)
}

/// The hits on `victim` the probe saw, each checked to have `causer` as its causing entity,
/// read as the full host reads it now.
fn assert_caused(seen: &Mutex<Vec<Seen>>, victim: u32, causer: u32, causer_health: Option<f32>) {
    let seen = std::mem::take(&mut *seen.lock().unwrap());
    let hits: Vec<_> = seen.iter().filter(|s| s.entity == victim as i32).collect();
    assert!(!hits.is_empty(), "no hit on {victim}: {seen:?}");
    for hit in hits {
        assert_eq!(hit.source_causing, causer as i32, "{hit:?}");
        assert_eq!(hit.causing, Some((causer, causer_health)), "{hit:?}");
    }
}

fn with_probe(test: impl FnOnce(&mut super::Game<'_>, &Mutex<Vec<Seen>>)) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
        "probe",
        Box::new(Probe(seen.clone())),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || {
        with_game(|game| {
            game.player =
                Some(PlayerMotion::new(game.motion, Vec3::new(40.0, 70.0, 40.0)).unwrap());
            bind_local_player(game, 7, 7);
            game.set_game_mode(GameMode::Survival);
            test(game, &seen);
        })
    });
}

/// A blast centred on `at`, whose causing entity is `owner` (a player when `player`).
fn blast(game: &mut super::Game<'_>, at: [f64; 3], owner: u32, player: bool, guest: Option<u32>) {
    game.combat.blast_owner = Some((owner, sunrise_client::text::Text::literal("owner")));
    game.combat.blast_owner_player = player;
    game.combat.blast_guest = guest;
    game.server_explode(at, 3.0, super::explosion::BlockInteraction::Keep, false)
        .unwrap();
    game.combat.blast_owner = None;
    game.combat.blast_owner_player = false;
    game.combat.blast_guest = None;
}

fn projectile(game: &mut super::Game<'_>, seen: &Mutex<Vec<Seen>>) {
    let shooter = summon_mob(game, "minecraft:skeleton", "{NoAI:1b}");
    let pig = summon_mob(game, "minecraft:pig", "{NoAI:1b}");
    let mut shot = sunrise_client::projectiles::Projectile::new(
        sunrise_client::projectiles::ProjectileKind::Arrow {
            base_damage: 2.0,
            critical: false,
            effect: None,
        },
        Vec3::new(0.5, 70.0, 0.5),
        Some(Who::Mob(shooter)),
    );
    shot.id = 912;
    let mut damage = Damage::of(DamageKind::Arrow);
    damage.attacker = Some(Who::Mob(shooter));
    damage.from = Some([0.5, 70.0, 0.5]);
    assert!(game.damage_projectile_with_weapon(&shot, pig, 2.0, damage, None));
    assert_caused(seen, pig, shooter, health(game, shooter));
}

fn explosion(game: &mut super::Game<'_>, seen: &Mutex<Vec<Seen>>) {
    let creeper = summon_mob(game, "minecraft:creeper", "{NoAI:1b}");
    let pig = summon_mob(game, "minecraft:pig", "{NoAI:1b}");
    // The creeper stands out of the blast; the pig in it.
    game.mobs
        .iter_mut()
        .find(|m| m.id == creeper)
        .unwrap()
        .position = Vec3::new(30.5, 70.0, 0.5);
    blast(game, [1.5, 70.0, 0.5], creeper, false, None);
    assert_caused(seen, pig, creeper, health(game, creeper));
}

fn thorns(game: &mut super::Game<'_>, seen: &Mutex<Vec<Seen>>) {
    let zombie = summon_mob(game, "minecraft:zombie", "{NoAI:1b}");
    let mut chest = sunrise_client::items::ItemStack::new("minecraft:diamond_chestplate", 1);
    chest.data_mut().enchantments = vec![("minecraft:thorns".into(), 255)];
    game.armor[1] = Some(chest);
    game.thorns(zombie);
    assert_caused(seen, zombie, 7, health(game, 7));
}

fn sting(game: &mut super::Game<'_>, seen: &Mutex<Vec<Seen>>) {
    // A server level whose chunks around the origin are loaded, so the mob tick runs there.
    game.options.dedicated = Some(Vec::new());
    for (x, z) in (-1..=1).flat_map(|x| (-1..=1).map(move |z| (x, z))) {
        let mut chunk = sunrise_client::chunk::Chunk::empty(x, z);
        chunk.lit = true;
        game.world.chunks.insert((x, z), Arc::new(chunk));
    }
    game.player.as_mut().unwrap().position = Vec3::new(3.5, 70.0, 3.5);
    let fish = summon_mob(game, "minecraft:pufferfish", "{NoAI:1b}");
    let cow = summon_mob(game, "minecraft:cow", "{NoAI:1b}");
    game.mobs
        .iter_mut()
        .find(|m| m.id == fish)
        .unwrap()
        .extra
        .size = 2;
    game.world.game_time += 1;
    game.tick_mobs().unwrap();
    // The fish's health as the sting read it: the tick goes on after the hit, but nothing
    // hurts the fish.
    assert_caused(seen, cow, fish, health(game, fish));
}

/// A guest's blast going off while the guest is parked: its record is the causer, as the
/// `LivingRef` reads answer for it then.
fn guest(seen: &Mutex<Vec<Seen>>) {
    super::native_guest_turn_tests::with_guests(|game| {
        let pig = summon_mob(game, "minecraft:pig", "{NoAI:1b}");
        assert!(game.lan_acting != Some(12));
        let expected = health(game, 12);
        assert!(
            expected.is_some(),
            "the parked guest's LivingRef reads answer"
        );
        blast(game, [1.5, 70.0, 0.5], 12, true, Some(12));
        assert_caused(seen, pig, 12, expected);
    });
}

#[test]
#[ignore = "daily"]
fn a_hit_listener_reads_the_causer_at_each_damage_site() {
    const CASE: &str = "SUNRISE_TEST_HIT_CAUSERS";
    if run_fresh_cases(
        "game::native_hit_causer_tests::a_hit_listener_reads_the_causer_at_each_damage_site",
        CASE,
        &["projectile", "explosion", "thorns", "sting", "guest"],
    ) {
        return;
    }
    let case = std::env::var(CASE).unwrap();
    if case == "guest" {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "probe",
            Box::new(Probe(seen.clone())),
        )]
        .into();
        return mod_platform::testing::with_mods(mods, || guest(&seen));
    }
    with_probe(|game, seen| match case.as_str() {
        "projectile" => projectile(game, seen),
        "explosion" => explosion(game, seen),
        "thorns" => thorns(game, seen),
        "sting" => sting(game, seen),
        other => panic!("unknown case {other}"),
    });
}
