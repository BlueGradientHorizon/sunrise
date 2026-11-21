use super::{damage_post, damage_pre, incoming_damage};
use crate::game::server_packs::drain_mod_fault_toasts;
use mod_api::{
    DamageContainerView, Flow, HitHost, Hooks, LivingDamagePostEvent, LivingDamagePreEvent,
    LivingIncomingDamageEvent, ModEntry, Mods, Reduction,
};
use std::cell::RefCell;
use std::sync::{
    Arc, Mutex, Once,
    atomic::{AtomicUsize, Ordering},
};
use sunrise_client::health::{HurtSource, Hurting, TestLevel, Victim};
use sunrise_client::toasts::SystemToast;
use sunrise_world::living_damage::{
    DamageContainer, DamageStat, DamageWorld, MagicAbsorb, Reduction as EngineReduction,
    actually_hurt,
};

static FAULT_LOCK: Mutex<()> = Mutex::new(());

thread_local! {
    static CAPTURED_LOGS: RefCell<Vec<(log::Level, String)>> = const { RefCell::new(Vec::new()) };
}

struct ThreadLogCapture;

impl log::Log for ThreadLogCapture {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }
    fn log(&self, record: &log::Record<'_>) {
        CAPTURED_LOGS.with_borrow_mut(|out| out.push((record.level(), record.args().to_string())));
    }
    fn flush(&self) {}
}

fn install_log_capture() -> bool {
    static LOGGER: ThreadLogCapture = ThreadLogCapture;
    static INIT: Once = Once::new();
    static INSTALLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    INIT.call_once(|| {
        if log::set_logger(&LOGGER).is_ok() {
            log::set_max_level(log::LevelFilter::Warn);
            INSTALLED.store(true, Ordering::Relaxed);
        }
    });
    INSTALLED.load(Ordering::Relaxed)
}

struct Target<'a> {
    level: &'a mut TestLevel,
    entity: i32,
    health: f32,
    absorption: f32,
}

impl Target<'_> {
    fn hurting(entity: i32) -> Hurting<'static> {
        Hurting::Player(
            entity,
            HurtSource {
                damage_type: "minecraft:mob_attack",
                causing: 7,
                direct: 7,
                position: Some([1.0, 2.0, 3.0]),
            },
        )
    }
}

impl DamageWorld for Target<'_> {
    type Error = ();
    type Attacker = ();
    fn invulnerable(&mut self) -> Result<bool, ()> {
        Ok(false)
    }
    fn after_armor(&mut self, damage: f32) -> Result<f32, ()> {
        Ok(damage * 0.5)
    }
    fn bypasses_armor(&mut self) -> Result<bool, ()> {
        Ok(false)
    }
    fn after_magic(&mut self, damage: f32) -> Result<MagicAbsorb, ()> {
        let after_effects = damage * 0.75;
        Ok(MagicAbsorb {
            damage: after_effects * 0.5,
            mob_effects: damage - after_effects,
            resisted: true,
            after_effects,
            protection: Some(12.5),
        })
    }
    fn absorption(&mut self) -> Result<f32, ()> {
        Ok(self.absorption)
    }
    fn set_absorption(&mut self, value: f32) -> Result<(), ()> {
        self.absorption = value;
        Ok(())
    }
    fn server_player_attacker(&mut self) -> Result<Option<()>, ()> {
        Ok(None)
    }
    fn award_attacker(&mut self, _: (), _: DamageStat, _: i32) -> Result<(), ()> {
        Ok(())
    }
    fn award_self(&mut self, _: DamageStat, _: i32) -> Result<(), ()> {
        Ok(())
    }
    fn source_food_exhaustion(&mut self) -> Result<f32, ()> {
        Ok(0.0)
    }
    fn cause_food_exhaustion(&mut self, _: f32) -> Result<(), ()> {
        Ok(())
    }
    fn record_damage(&mut self, _: f32) -> Result<(), ()> {
        Ok(())
    }
    fn health(&mut self) -> Result<f32, ()> {
        Ok(self.health)
    }
    fn set_health(&mut self, value: f32) -> Result<(), ()> {
        self.health = value;
        Ok(())
    }
    fn damage_event(&mut self) -> Result<(), ()> {
        Ok(())
    }
    fn damage_pre(&mut self, container: &mut DamageContainer) -> Result<(), ()> {
        damage_pre(self.level.level(), Self::hurting(self.entity), container);
        Ok(())
    }
}

fn victim() -> Victim {
    Victim {
        position: [0.0, 64.0, 0.0],
        view: [0.0, 0.0, 1.0],
    }
}

fn run_simple_hit(original: f32, absorption: f32) -> DamageContainer {
    let mut level = TestLevel::new(0);
    let mut container = DamageContainer::new(original);
    let canceled = incoming_damage(
        level.level(),
        Target::hurting(4201),
        victim(),
        &mut container,
    );
    assert!(!canceled);
    let damage = container.new_damage().max(0.0);
    container.record_new_damage(damage);
    container.record_should_cause_side_effects(true);
    let mut target = Target {
        level: &mut level,
        entity: 4201,
        health: 100.0,
        absorption,
    };
    actually_hurt(&mut target, &mut container, damage, true).unwrap();
    damage_post(level.level(), Target::hurting(4201), &container);
    container
}

#[derive(Debug, PartialEq)]
struct ObservedStage {
    who: &'static str,
    original: f32,
    new_damage: f32,
    invuln: f32,
    armor: f32,
    mob_effects: f32,
    enchantments: f32,
    blocked: f32,
    shield_damage: f32,
    post_ticks: i32,
    side_effects: bool,
    preceding: f32,
}

#[derive(Debug, PartialEq)]
struct ObservedPost {
    inflicted: f32,
    health_damage: f32,
    absorption: f32,
    side_effects: bool,
    blocked: f32,
    shield_damage: f32,
    post_ticks: i32,
}

#[test]
fn two_mods_reduction_order_live_views_pre_absorption_and_post() {
    let stages = Arc::new(Mutex::new(Vec::<ObservedStage>::new()));
    let post = Arc::new(Mutex::new(Vec::<ObservedPost>::new()));

    struct ModA(Arc<Mutex<Vec<ObservedStage>>>);
    #[mod_api::__subscriptions]
    impl Hooks for ModA {
        fn living_incoming_damage(
            &self,
            _: &mut dyn HitHost,
            event: &mut LivingIncomingDamageEvent<'_>,
        ) -> Flow {
            let log = Arc::clone(&self.0);
            event
                .container
                .add_reduction_modifier(Reduction::Armor, move |view, amount| {
                    log.lock().unwrap().push(ObservedStage {
                        who: "mod_a:armor",
                        original: view.original(),
                        new_damage: view.new_damage(),
                        invuln: view.reduction(Reduction::Invulnerability),
                        armor: view.reduction(Reduction::Armor),
                        mob_effects: view.reduction(Reduction::MobEffects),
                        enchantments: view.reduction(Reduction::Enchantments),
                        blocked: view.blocked(),
                        shield_damage: view.shield_damage(),
                        post_ticks: view.post_attack_invulnerability_ticks(),
                        side_effects: view.should_cause_side_effects(),
                        preceding: amount,
                    });
                    amount * 0.5
                });
            Flow::Continue
        }

        fn living_damage_pre(&self, _: &mut dyn HitHost, event: &mut LivingDamagePreEvent<'_>) {
            let log = Arc::clone(&self.0);
            event
                .container
                .add_reduction_modifier(Reduction::Absorption, move |view, amount| {
                    log.lock().unwrap().push(ObservedStage {
                        who: "mod_a:absorption",
                        original: view.original(),
                        new_damage: view.new_damage(),
                        invuln: view.reduction(Reduction::Invulnerability),
                        armor: view.reduction(Reduction::Armor),
                        mob_effects: view.reduction(Reduction::MobEffects),
                        enchantments: view.reduction(Reduction::Enchantments),
                        blocked: view.blocked(),
                        shield_damage: view.shield_damage(),
                        post_ticks: view.post_attack_invulnerability_ticks(),
                        side_effects: view.should_cause_side_effects(),
                        preceding: amount,
                    });
                    amount * 0.5
                });
        }
    }

    struct ModB {
        stages: Arc<Mutex<Vec<ObservedStage>>>,
        post: Arc<Mutex<Vec<ObservedPost>>>,
    }
    #[mod_api::__subscriptions]
    impl Hooks for ModB {
        fn living_incoming_damage(
            &self,
            _: &mut dyn HitHost,
            event: &mut LivingIncomingDamageEvent<'_>,
        ) -> Flow {
            let log = Arc::clone(&self.stages);
            event
                .container
                .add_reduction_modifier(Reduction::Armor, move |view, amount| {
                    log.lock().unwrap().push(ObservedStage {
                        who: "mod_b:armor",
                        original: view.original(),
                        new_damage: view.new_damage(),
                        invuln: view.reduction(Reduction::Invulnerability),
                        armor: view.reduction(Reduction::Armor),
                        mob_effects: view.reduction(Reduction::MobEffects),
                        enchantments: view.reduction(Reduction::Enchantments),
                        blocked: view.blocked(),
                        shield_damage: view.shield_damage(),
                        post_ticks: view.post_attack_invulnerability_ticks(),
                        side_effects: view.should_cause_side_effects(),
                        preceding: amount,
                    });
                    amount + 2.0
                });
            Flow::Continue
        }

        fn living_damage_post(&self, _: &mut dyn HitHost, event: &mut LivingDamagePostEvent<'_>) {
            let view: DamageContainerView<'_> = event.container;
            self.post.lock().unwrap().push(ObservedPost {
                inflicted: view.inflicted(),
                health_damage: view.health_damage(),
                absorption: view.reduction(Reduction::Absorption),
                side_effects: view.should_cause_side_effects(),
                blocked: view.blocked(),
                shield_damage: view.shield_damage(),
                post_ticks: view.post_attack_invulnerability_ticks(),
            });
        }
    }

    let mods: Mods = vec![
        ModEntry::new("mod_a", Box::new(ModA(Arc::clone(&stages)))),
        ModEntry::new(
            "mod_b",
            Box::new(ModB {
                stages: Arc::clone(&stages),
                post: Arc::clone(&post),
            }),
        ),
    ]
    .into();

    mod_platform::testing::with_mods(mods, || {
        let mut level = TestLevel::new(0);
        let mut container = DamageContainer::new(24.0);
        assert!(!incoming_damage(
            level.level(),
            Target::hurting(4200),
            victim(),
            &mut container,
        ));
        // Simulate shield block and cooldown stage updates before `actually_hurt`.
        container.record_block(2.0);
        container.record_shield_damage(3.0);
        let mut damage = container.new_damage() - 2.0;
        container.record_new_damage(damage);
        container.record_should_cause_side_effects(false);
        container.set_post_attack_invulnerability_ticks(15);
        damage = container.reduce(
            EngineReduction::Invulnerability,
            damage,
            2.0,
            damage - 2.0,
            true,
        );
        assert_eq!(damage, 20.0);

        let mut target = Target {
            level: &mut level,
            entity: 4200,
            health: 100.0,
            absorption: 4.0,
        };
        actually_hurt(&mut target, &mut container, damage, true).unwrap();
        let (health, absorption) = (target.health, target.absorption);
        damage_post(level.level(), Target::hurting(4200), &container);

        assert_eq!(container.reduction(EngineReduction::Armor), 7.0);
        assert_eq!(container.reduction(EngineReduction::MobEffects), 3.25);
        assert_eq!(container.reduction(EngineReduction::Enchantments), 4.875);
        assert_eq!(container.reduction(EngineReduction::Absorption), 2.0);
        assert_eq!(health, 97.125);
        assert_eq!(absorption, 2.0);
    });

    assert_eq!(
        *stages.lock().unwrap(),
        vec![
            ObservedStage {
                who: "mod_a:armor",
                original: 24.0,
                new_damage: 20.0,
                invuln: 2.0,
                armor: 0.0,
                mob_effects: 0.0,
                enchantments: 0.0,
                blocked: 2.0,
                shield_damage: 3.0,
                post_ticks: 15,
                side_effects: false,
                preceding: 10.0,
            },
            ObservedStage {
                who: "mod_b:armor",
                original: 24.0,
                new_damage: 20.0,
                invuln: 2.0,
                armor: 0.0,
                mob_effects: 0.0,
                enchantments: 0.0,
                blocked: 2.0,
                shield_damage: 3.0,
                post_ticks: 15,
                side_effects: false,
                preceding: 5.0,
            },
            ObservedStage {
                who: "mod_a:absorption",
                original: 24.0,
                new_damage: 4.875,
                invuln: 2.0,
                armor: 7.0,
                mob_effects: 3.25,
                enchantments: 4.875,
                blocked: 2.0,
                shield_damage: 3.0,
                post_ticks: 15,
                side_effects: false,
                preceding: 4.0,
            },
        ]
    );
    assert_eq!(
        *post.lock().unwrap(),
        vec![ObservedPost {
            inflicted: 4.875,
            health_damage: 2.875,
            absorption: 2.0,
            side_effects: false,
            blocked: 2.0,
            shield_damage: 3.0,
            post_ticks: 15,
        }]
    );
}

#[test]
fn panicking_reduction_callback_is_isolated_disabled_and_reported() {
    let _lock = FAULT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _ = mod_api::__private::take_callback_faults();

    let bad_incoming_runs = Arc::new(AtomicUsize::new(0));
    let bad_reduction_runs = Arc::new(AtomicUsize::new(0));
    let good_reduction_runs = Arc::new(AtomicUsize::new(0));

    struct BadMod {
        incoming: Arc<AtomicUsize>,
        reduction: Arc<AtomicUsize>,
    }
    #[mod_api::__subscriptions]
    impl Hooks for BadMod {
        fn living_incoming_damage(
            &self,
            _: &mut dyn HitHost,
            event: &mut LivingIncomingDamageEvent<'_>,
        ) -> Flow {
            self.incoming.fetch_add(1, Ordering::Relaxed);
            let runs = Arc::clone(&self.reduction);
            event
                .container
                .add_reduction_modifier(Reduction::Armor, move |_, _| {
                    runs.fetch_add(1, Ordering::Relaxed);
                    panic!("boom in armor reduction");
                });
            Flow::Continue
        }
    }

    struct GoodMod(Arc<AtomicUsize>);
    #[mod_api::__subscriptions]
    impl Hooks for GoodMod {
        fn living_incoming_damage(
            &self,
            _: &mut dyn HitHost,
            event: &mut LivingIncomingDamageEvent<'_>,
        ) -> Flow {
            let runs = Arc::clone(&self.0);
            event
                .container
                .add_reduction_modifier(Reduction::Armor, move |_, amount| {
                    runs.fetch_add(1, Ordering::Relaxed);
                    amount + 2.0
                });
            Flow::Continue
        }
    }

    let mods: Mods = vec![
        ModEntry::new(
            "bad_mod",
            Box::new(BadMod {
                incoming: Arc::clone(&bad_incoming_runs),
                reduction: Arc::clone(&bad_reduction_runs),
            }),
        ),
        ModEntry::new(
            "good_mod",
            Box::new(GoodMod(Arc::clone(&good_reduction_runs))),
        ),
    ]
    .into();

    mod_platform::testing::with_mods(mods, || {
        // Hit 1: bad_mod panics, leaves 10.0 unchanged, good_mod adds 2.0 -> 12.0.
        let first = run_simple_hit(20.0, 0.0);
        assert_eq!(first.reduction(EngineReduction::Armor), 12.0);
        assert_eq!(bad_incoming_runs.load(Ordering::Relaxed), 1);
        assert_eq!(bad_reduction_runs.load(Ordering::Relaxed), 1);
        assert_eq!(good_reduction_runs.load(Ordering::Relaxed), 1);
        assert_eq!(
            mod_api::__private::take_callback_faults(),
            vec![mod_api::__private::CallbackFault {
                mod_id: "bad_mod",
                callback: "living_damage_reduction",
                message: "boom in armor reduction".to_owned(),
            }]
        );

        // Hit 2: bad_mod's living_incoming_damage still fires, its reduction callback is disabled,
        // and good_mod continues running cleanly.
        let second = run_simple_hit(20.0, 0.0);
        assert_eq!(second.reduction(EngineReduction::Armor), 12.0);
        assert_eq!(bad_incoming_runs.load(Ordering::Relaxed), 2);
        assert_eq!(bad_reduction_runs.load(Ordering::Relaxed), 1);
        assert_eq!(good_reduction_runs.load(Ordering::Relaxed), 2);
        assert!(mod_api::__private::take_callback_faults().is_empty());
    });
}

#[test]
fn callback_fault_drains_into_red_error_toast_naming_the_mod() {
    let _lock = FAULT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _ = mod_api::__private::take_callback_faults();

    struct PanickingMod;
    #[mod_api::__subscriptions]
    impl Hooks for PanickingMod {
        fn living_incoming_damage(
            &self,
            _: &mut dyn HitHost,
            event: &mut LivingIncomingDamageEvent<'_>,
        ) -> Flow {
            event
                .container
                .add_reduction_modifier(Reduction::Armor, |_, _| panic!("toast fault"));
            Flow::Continue
        }
    }

    let first_mods: Mods = vec![ModEntry::new("faulty_mod", Box::new(PanickingMod))].into();
    mod_platform::testing::with_mods(first_mods, || {
        let _ = run_simple_hit(20.0, 0.0);
    });

    let mut slot: Option<SystemToast> = None;
    drain_mod_fault_toasts(&mut slot, Some);
    let toast = slot.as_ref().expect("fault must create a SystemToast");
    assert_eq!(
        toast.title_color(),
        sunrise_client::hud::rgba(255, 85, 85, 255)
    );
    assert_eq!(
        toast.title.component(),
        serde_json::json!({
            "translate": "sunrise.mods.callback_panicked.title",
            "with": [{"text": "faulty_mod"}],
        })
    );
    assert_eq!(
        toast.message.as_ref().unwrap().component(),
        serde_json::json!({
            "translate": "sunrise.mods.callback_panicked.description",
            "with": [{"text": "living_damage_reduction"}],
        })
    );
    assert!(mod_api::__private::take_callback_faults().is_empty());

    // A later fault resets the shown toast in place while preserving the red title color.
    let second_mods: Mods = vec![ModEntry::new("second_faulty_mod", Box::new(PanickingMod))].into();
    mod_platform::testing::with_mods(second_mods, || {
        let _ = run_simple_hit(20.0, 0.0);
    });
    drain_mod_fault_toasts(&mut slot, |_| {
        panic!("existing fault toast must be reset, not re-admitted")
    });
    let updated = slot.as_ref().unwrap();
    assert_eq!(
        updated.title_color(),
        sunrise_client::hud::rgba(255, 85, 85, 255)
    );
    assert_eq!(
        updated.title.component(),
        serde_json::json!({
            "translate": "sunrise.mods.callback_panicked.title",
            "with": [{"text": "second_faulty_mod"}],
        })
    );
}

#[test]
fn phase_refusal_in_reduction_callback_is_not_disabled_and_reports_each_time() {
    let _lock = FAULT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _ = mod_api::__private::take_callback_faults();
    let capture_active = install_log_capture();
    CAPTURED_LOGS.with_borrow_mut(Vec::clear);

    let attempts = Arc::new(AtomicUsize::new(0));

    struct RefusingMod(Arc<AtomicUsize>);
    #[mod_api::__subscriptions]
    impl Hooks for RefusingMod {
        fn living_incoming_damage(
            &self,
            _: &mut dyn HitHost,
            event: &mut LivingIncomingDamageEvent<'_>,
        ) -> Flow {
            let attempts = Arc::clone(&self.0);
            event
                .container
                .add_reduction_modifier(Reduction::Armor, move |_, amount| {
                    if attempts.fetch_add(1, Ordering::Relaxed) < 2 {
                        mod_api::__private::refuse(
                            "block_state",
                            mod_api::__private::ModPhase::Setup,
                        );
                    }
                    amount + 3.0
                });
            Flow::Continue
        }
    }

    struct FollowingMod;
    #[mod_api::__subscriptions]
    impl Hooks for FollowingMod {
        fn living_incoming_damage(
            &self,
            _: &mut dyn HitHost,
            event: &mut LivingIncomingDamageEvent<'_>,
        ) -> Flow {
            event
                .container
                .add_reduction_modifier(Reduction::Armor, |_, amount| amount + 1.0);
            Flow::Continue
        }
    }

    let mods: Mods = vec![
        ModEntry::new("refusing_mod", Box::new(RefusingMod(Arc::clone(&attempts)))),
        ModEntry::new("following_mod", Box::new(FollowingMod)),
    ]
    .into();

    mod_platform::testing::with_mods(mods, || {
        // Hit 1: refusal leaves 10.0 unchanged; following_mod still runs -> 11.0.
        let first = run_simple_hit(20.0, 0.0);
        assert_eq!(first.reduction(EngineReduction::Armor), 11.0);
        assert_eq!(attempts.load(Ordering::Relaxed), 1);
        assert!(mod_api::__private::take_callback_faults().is_empty());

        // Hit 2: callback is NOT disabled; refuses a second time and leaves 10.0 unchanged.
        let second = run_simple_hit(20.0, 0.0);
        assert_eq!(second.reduction(EngineReduction::Armor), 11.0);
        assert_eq!(attempts.load(Ordering::Relaxed), 2);
        assert!(mod_api::__private::take_callback_faults().is_empty());

        // Hit 3: once it stops refusing, it applies normally (10.0 + 3.0 + 1.0 = 14.0).
        let third = run_simple_hit(20.0, 0.0);
        assert_eq!(third.reduction(EngineReduction::Armor), 14.0);
        assert_eq!(attempts.load(Ordering::Relaxed), 3);
        assert!(mod_api::__private::take_callback_faults().is_empty());
    });

    if capture_active {
        let refusal_warnings = CAPTURED_LOGS.with_borrow(|logs| {
            logs.iter()
                .filter(|(level, msg)| {
                    *level == log::Level::Warn
                        && msg
                            .contains("native refusal: refusing_mod hook living_damage_reduction:")
                })
                .count()
        });
        assert_eq!(
            refusal_warnings, 2,
            "each phase refusal occurrence must be logged without latching"
        );
    }
}
