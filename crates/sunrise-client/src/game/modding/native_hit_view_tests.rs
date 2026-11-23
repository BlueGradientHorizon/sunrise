//! A hit listener reads its victim and causing entity through the hit's host
//! (`LivingIncomingDamageEvent.getEntity`, `DamageSource.getEntity`), at the three damage moments,
//! from their live bodies, and reads their attachments without writing them.
use super::native_entity_refs::tests::{bind_local_player, run_fresh_cases, with_game};
use super::native_living_refs::tests::summon_mob;
use mod_api::{
    ActiveEffect, AttachmentError, AttachmentType, EffectKey, Flow, Hand, HitHost, Holder, Hooks,
    LivingDamagePostEvent, LivingDamagePreEvent, LivingIncomingDamageEvent, LivingView,
};
use std::sync::{Arc, Mutex};
use sunrise_client::command::args::GameMode;
use sunrise_client::mobs::{Damage, DamageKind, Who};
use sunrise_client::motion::PlayerMotion;
use sunrise_core::geometry::Vec3;

/// The probe's attachment, set on the player and on some mobs before the hits.
static MARK: AttachmentType<i32> = AttachmentType {
    namespace: "probe",
    name: "mark",
    default: || -1,
    codec: None,
    copy_on_death: false,
    sync: false,
};

/// What a listener read of one living entity of a hit.
#[derive(Clone, Debug, PartialEq)]
struct Read {
    id: u32,
    kind: Option<mod_api::EntityTypeKey>,
    health: Option<f32>,
    absorption: Option<f32>,
    max_health: Option<f32>,
    max_health_attribute: Option<f64>,
    speed: Option<i32>,
    effects: Option<usize>,
    speed_affected: bool,
    main_hand: Option<(u32, bool)>,
    no_ai: Option<bool>,
    holder: Option<Holder>,
    has: Option<Result<bool, AttachmentError>>,
    mark: Option<Result<Option<i32>, AttachmentError>>,
    get: Option<Result<i32, AttachmentError>>,
    set: Option<Result<(), AttachmentError>>,
}

/// One hook call: which moment, the event's entity, and the two views.
#[derive(Clone, Debug)]
struct Seen {
    hook: &'static str,
    entity: i32,
    victim: Option<Read>,
    causing: Option<Read>,
}

fn speed() -> EffectKey {
    mod_platform::__private::part_tests::effect_key("minecraft:speed")
}

fn read(level: &mut dyn HitHost, view: impl FnOnce(&dyn HitHost) -> Option<Read>) -> Option<Read> {
    let mut read = view(&*level)?;
    // The attachments go through the host the view borrows; its reads come first.
    if let Some(holder) = read.holder {
        read.has = Some(MARK.has(level.attachments(), holder));
        read.mark = Some(MARK.get_existing(level.attachments(), holder));
        read.get = Some(MARK.get(level.attachments(), holder));
        read.set = Some(MARK.set(level.attachments(), holder, 99));
    }
    Some(read)
}

fn view_read(view: LivingView<'_>) -> Read {
    let stick = mod_api::__private::item_key("minecraft:stick");
    Read {
        id: view.id(),
        kind: view.kind(),
        health: view.health(),
        absorption: view.absorption(),
        max_health: view.max_health(),
        max_health_attribute: view.attribute_value("minecraft:max_health"),
        speed: view.effect(speed()).map(|e| e.amplifier),
        effects: view.effects().map(|list| list.len()),
        speed_affected: view.potion_affected(speed()),
        main_hand: view.with_hand_item(Hand::Main, |stack| {
            (stack.count(), Some(stack.item()) == stick)
        }),
        no_ai: view.no_ai(),
        holder: view.holder(),
        has: None,
        mark: None,
        get: None,
        set: None,
    }
}

struct Probe(Arc<Mutex<Vec<Seen>>>);

impl Probe {
    fn seen(&self, hook: &'static str, entity: i32, level: &mut dyn HitHost) {
        let victim = read(level, |host| host.victim().map(view_read));
        let causing = read(level, |host| host.causing().map(view_read));
        self.0.lock().unwrap().push(Seen {
            hook,
            entity,
            victim,
            causing,
        });
    }
}

#[mod_api::__subscriptions]
impl Hooks for Probe {
    fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
        r.register_attachment_type("probe:mark", false).unwrap();
    }
    fn living_incoming_damage(
        &self,
        level: &mut dyn HitHost,
        event: &mut LivingIncomingDamageEvent<'_>,
    ) -> Flow {
        self.seen("incoming", event.entity, level);
        Flow::Continue
    }
    fn living_damage_pre(&self, level: &mut dyn HitHost, event: &mut LivingDamagePreEvent<'_>) {
        self.seen("pre", event.entity, level);
        // No hurt cooldown keeps the next hit from the hooks.
        event
            .container
            .set_post_attack_invulnerability_ticks(0)
            .unwrap();
    }
    fn living_damage_post(&self, level: &mut dyn HitHost, event: &mut LivingDamagePostEvent<'_>) {
        self.seen("post", event.entity, level);
    }
}

/// What a `LivingRef` read answers for `id` now, in the probe's [`Read`] form, with the
/// attachment value the full host answers.
fn handle_read(game: &mut super::Game<'_>, id: u32) -> Read {
    let living = game.living(game.entity(id).unwrap()).unwrap();
    let stick = mod_api::__private::item_key("minecraft:stick");
    let mob = game.mob(living.entity());
    let holder = if Some(id) == game.lan_acting_entity {
        game.lan_acting_uuid.map(Holder::Player)
    } else {
        Some(Holder::Entity(id))
    };
    Read {
        id,
        kind: living.entity().kind(game),
        health: living.health(game),
        absorption: living.absorption(game),
        max_health: living.max_health(game),
        max_health_attribute: living.attribute_value(game, "minecraft:max_health"),
        speed: living.effect(game, speed()).map(|e| e.amplifier),
        effects: living.effects(game).map(|list| list.len()),
        speed_affected: living.potion_affected(game, speed()),
        main_hand: living.with_hand_item(game, Hand::Main, |stack| {
            (stack.count(), Some(stack.item()) == stick)
        }),
        no_ai: mob.and_then(|mob| mob.no_ai(game)),
        holder,
        has: None,
        mark: None,
        get: None,
        set: None,
    }
}

/// The attachment answers a listener sees on a read-only holder whose mark is `mark`.
fn read_only(mut read: Read, mark: Option<i32>) -> Read {
    read.has = Some(Ok(mark.is_some()));
    read.mark = Some(Ok(mark));
    read.get = Some(mark.ok_or(AttachmentError::ReadOnlyHolder));
    read.set = Some(Err(AttachmentError::ReadOnlyHolder));
    read
}

fn with_health(mut read: Read, health: f32) -> Read {
    read.health = Some(health);
    read
}

#[test]
#[ignore = "daily"]
fn a_hit_listener_reads_its_victim_and_causer_live_and_their_attachments_read_only() {
    const CASE: &str = "SUNRISE_TEST_HIT_VIEWS";
    if run_fresh_cases(
        "game::native_hit_view_tests::a_hit_listener_reads_its_victim_and_causer_live_and_their_attachments_read_only",
        CASE,
        &["hits"],
    ) {
        return;
    }
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
        "probe",
        Box::new(Probe(seen.clone())),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || {
        with_game(|game| {
            game.player = Some(PlayerMotion::new(game.motion, Vec3::ZERO).unwrap());
            bind_local_player(game, 7, 7);
            game.set_game_mode(GameMode::Survival);
            let pig = summon_mob(game, "minecraft:pig", "{NoAI:1b}");
            let cow = summon_mob(game, "minecraft:cow", "{}");
            let zombie = summon_mob(game, "minecraft:zombie", "{}");
            game.inventory[game.selected] =
                Some(sunrise_client::items::ItemStack::new("minecraft:stick", 5));
            for (id, amplifier) in [(7, 1), (zombie, 2)] {
                let living = game.living(game.entity(id).unwrap()).unwrap();
                let effect = ActiveEffect::new(speed(), 400, amplifier, false, true, true);
                assert!(game.living_add_effect(living, &effect));
            }
            MARK.set(game, Holder::Player(7), 3).unwrap();
            MARK.set(game, Holder::Entity(zombie), 5).unwrap();
            MARK.set(game, Holder::Entity(pig), 8).unwrap();

            // A mob's melee hit on the player: the player's body is lent by its combat, the
            // zombie by the hit's site.
            let player_before = handle_read(game, 7);
            let zombie_before = handle_read(game, zombie);
            assert_eq!(player_before.health, Some(20.0));
            assert_eq!(player_before.speed, Some(1));
            assert_eq!(player_before.main_hand, Some((5, true)));
            game.mob_hits_player_caused(
                4.0,
                [0.5, 70.0, 0.5],
                "minecraft:mob_attack",
                Some(zombie),
            );
            let player_after = handle_read(game, 7);
            assert!(player_after.health.unwrap() < 20.0);
            let calls = std::mem::take(&mut *seen.lock().unwrap());
            let hooks: Vec<_> = calls.iter().map(|c| (c.hook, c.entity)).collect();
            assert_eq!(hooks, [("incoming", 7), ("pre", 7), ("post", 7)]);
            let victim = |health| {
                Some(read_only(
                    with_health(player_before.clone(), health),
                    Some(3),
                ))
            };
            let causing = Some(read_only(zombie_before.clone(), Some(5)));
            // The health is taken between Pre and Post, and each moment reads it then.
            assert_eq!(calls[0].victim, victim(20.0));
            assert_eq!(calls[1].victim, victim(20.0));
            assert_eq!(calls[2].victim, victim(player_after.health.unwrap()));
            for call in &calls {
                assert_eq!(call.causing, causing, "{}", call.hook);
            }

            // The player's melee hit on a mob: the pig is the victim, the player the causer.
            let player_before = handle_read(game, 7);
            let pig_before = handle_read(game, pig);
            assert_eq!(pig_before.health, Some(10.0));
            assert_eq!(pig_before.no_ai, Some(true));
            assert!(game.mob_hits_mob(pig, 4.0, Damage::player([0.5, 70.0, 0.5])));
            let pig_after = handle_read(game, pig);
            assert!(pig_after.health.unwrap() < 10.0);
            let calls = std::mem::take(&mut *seen.lock().unwrap());
            let hooks: Vec<_> = calls.iter().map(|c| (c.hook, c.entity)).collect();
            assert_eq!(
                hooks,
                [
                    ("incoming", pig as i32),
                    ("pre", pig as i32),
                    ("post", pig as i32)
                ]
            );
            let victim = |health| Some(read_only(with_health(pig_before.clone(), health), Some(8)));
            assert_eq!(calls[0].victim, victim(10.0));
            assert_eq!(calls[1].victim, victim(10.0));
            assert_eq!(calls[2].victim, victim(pig_after.health.unwrap()));
            for call in &calls {
                assert_eq!(
                    call.causing,
                    Some(read_only(player_before.clone(), Some(3)))
                );
            }

            // A mob's melee hit on a mob, the attacker split from the victim in the same list:
            // a holder with no mark refuses to store its default.
            let cow_before = handle_read(game, cow);
            let zombie_before = handle_read(game, zombie);
            let source = Damage {
                kind: DamageKind::MobAttack,
                attacker: Some(Who::Mob(zombie)),
                from: Some([0.5, 70.0, 0.5]),
                attacker_species: None,
            };
            assert!(game.mob_hits_mob(cow, 2.0, source));
            let calls = std::mem::take(&mut *seen.lock().unwrap());
            assert_eq!(calls.len(), 3);
            assert_eq!(calls[0].victim, Some(read_only(cow_before, None)));
            for call in &calls {
                assert_eq!(
                    call.causing,
                    Some(read_only(zombie_before.clone(), Some(5)))
                );
            }
            assert_eq!(MARK.has(game, Holder::Entity(cow)), Ok(false));

            // A lethal hit's Post waits for the death to settle, and reads the bodies then.
            assert!(game.mob_hits_mob(cow, 100.0, Damage::player([0.5, 70.0, 0.5])));
            let calls = std::mem::take(&mut *seen.lock().unwrap());
            let hooks: Vec<_> = calls.iter().map(|c| c.hook).collect();
            assert_eq!(hooks, ["incoming", "pre", "post"]);
            let post = &calls[2];
            assert!(
                post.victim.as_ref().unwrap().health.unwrap() <= 0.0,
                "{post:?}"
            );
            assert_eq!(post.causing.as_ref().map(|read| read.id), Some(7));

            // A hit with no causing entity lends none.
            game.hurt_player_from(1.0, None, "minecraft:generic", Some(None), false);
            let calls = std::mem::take(&mut *seen.lock().unwrap());
            assert_eq!(calls.len(), 3);
            for call in &calls {
                assert!(call.victim.is_some(), "{}", call.hook);
                assert_eq!(call.causing, None, "{}", call.hook);
            }

            // Nothing a listener did wrote an attachment.
            assert_eq!(MARK.get_existing(game, Holder::Player(7)), Ok(Some(3)));
            assert_eq!(MARK.get_existing(game, Holder::Entity(zombie)), Ok(Some(5)));
            assert_eq!(MARK.get_existing(game, Holder::Entity(pig)), Ok(Some(8)));
        });
    });
}
