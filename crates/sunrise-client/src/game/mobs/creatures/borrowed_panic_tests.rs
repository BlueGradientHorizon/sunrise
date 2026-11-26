use super::*;
use crate::game::native_entity_refs::tests::{run_fresh_cases, with_game};
use mod_api::__private::{HookPanicReport, take_hook_panics};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
use sunrise_mod_runtime::attachments::{AttachmentType, Holder};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum Family {
    Damage,
    Shield,
    Totem,
    Movement,
    Goal,
    MountedGoal,
    Knockback,
    Landing,
}
impl Family {
    pub(in crate::game) fn is_goal(self) -> bool {
        matches!(self, Self::Goal | Self::MountedGoal)
    }

    pub(in crate::game) fn hook(self) -> &'static str {
        match self {
            Self::Damage => "living_incoming_damage",
            Self::Shield => "living_shield_block",
            Self::Totem => "living_use_totem",
            Self::Movement => "living_speed",
            Self::Knockback => "living_knockback",
            Self::Landing => "fall_on",
            Self::Goal | Self::MountedGoal => unreachable!("goals use the unwind fixture"),
        }
    }
}

#[derive(Debug)]
pub(in crate::game) struct PanicToken(pub(in crate::game) u64);
pub(in crate::game) const LEVEL_SEED: i64 = 0x5eed;
pub(in crate::game) const FAILING_MOD: &str = "borrowed-panic";

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::game) enum Proposal {
    Cancelled(bool),
    Blocked(bool),
    Speed(f32),
    FallMultiplier(f32),
}

#[derive(Debug, PartialEq, Eq)]
pub(in crate::game) enum OutputNotice {
    Guest(i32),
    Egg,
}

pub(in crate::game) struct Probe {
    pub(in crate::game) family: Family,
    pub(in crate::game) calls: AtomicUsize,
    pub(in crate::game) posts: Mutex<Vec<u32>>,
    pub(in crate::game) first_proposal: Mutex<Option<Proposal>>,
    pub(in crate::game) healthy_proposals: Mutex<Vec<Proposal>>,
    pub(in crate::game) draw: Mutex<Option<i64>>,
    pub(in crate::game) outputs: Mutex<Vec<OutputNotice>>,
    pub(in crate::game) goal_payload: Mutex<Option<Box<PanicToken>>>,
}
impl Probe {
    pub(in crate::game) fn goal_callback(&self, family: Family, target: bool) {
        if self.family == family && target {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let payload = self.goal_payload.lock().unwrap().take();
            if let Some(payload) = payload {
                eprintln!("BORROWED_NATIVE_REACHED:{family:?}");
                std::panic::resume_unwind(payload);
            }
        }
    }

    pub(in crate::game) fn enter_failing(&self, input: Proposal) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        *self.first_proposal.lock().unwrap() = Some(input);
    }

    pub(in crate::game) fn progress(&self, host: &mut dyn mod_api::LevelRandomHost) {
        *self.draw.lock().unwrap() = host.with_random(|random| random.next_long());
        let _ = VALUE.set(host.attachments(), Holder::Level, 314);
    }
}

pub(in crate::game) fn assert_panic_report(mod_id: &'static str, hook: &'static str) {
    assert_eq!(take_hook_panics(), [HookPanicReport { mod_id, hook }]);
}
pub(in crate::game) struct GoalSpec {
    pub(in crate::game) probe: Arc<Probe>,
    pub(in crate::game) target: bool,
}
impl mod_api::Goal for GoalSpec {
    fn name(&self) -> &'static str {
        if self.target {
            "BorrowedTarget"
        } else {
            "BorrowedProbe"
        }
    }
    fn flags(&self) -> mod_api::GoalFlags {
        if self.target {
            mod_api::GoalFlags::TARGET
        } else {
            mod_api::GoalFlags::MOVE
        }
    }
    fn can_use(&mut self, _: &mut mod_api::GoalContext<'_>) -> bool {
        true
    }
    fn tick(&mut self, cx: &mut mod_api::GoalContext<'_>) {
        if self.probe.family == Family::MountedGoal {
            if !self.target {
                assert!(cx.move_to([3.0, 80.0, 0.5], 1.25));
                eprintln!("BORROWED_MOUNTED_MOVE_SUCCEEDED");
                self.probe
                    .goal_callback(Family::MountedGoal, cx.mob().entity().id() == 72);
            }
            return;
        }
        assert_eq!(
            cx.mob().entity().position(cx.host()),
            Some([2.0, 80.0, 0.5])
        );
        if !self.target {
            self.probe
                .goal_callback(Family::Goal, cx.mob().entity().id() == 72);
        }
    }
}
pub(in crate::game) struct GoalPart(pub(in crate::game) Arc<Probe>);
#[mod_api::sunrise_part]
impl mod_api::MobEffectPart for GoalPart {
    fn should_apply_effect_tick_this_tick(
        &self,
        _: &dyn mod_api::Host,
        event: &mut mod_api::ShouldApplyEffectTickThisTickEvent,
    ) {
        event.apply = self.0.family == Family::Knockback;
    }
    fn apply_effect_tick(
        &self,
        _: &dyn mod_api::Host,
        event: &mut mod_api::ApplyEffectTickEvent<'_>,
    ) {
        if self.0.family == Family::Knockback && event.entity == Some(72) {
            eprintln!("BORROWED_REAL_EFFECT_HURT");
            event.hurt("minecraft:mob_attack", 1.0);
        }
    }
}
pub(in crate::game) struct FailingLanding(pub(in crate::game) Arc<Probe>);
#[mod_api::sunrise_part]
impl mod_api::BlockPart for FailingLanding {
    fn fall_on(
        &self,
        host: &mut dyn mod_api::LevelRandomHost,
        event: &mut mod_api::FallOnEvent<'_>,
    ) {
        if event.entity == 72 {
            self.0
                .enter_failing(Proposal::FallMultiplier(event.damage_multiplier));
            self.0.progress(host);
            event.damage_multiplier = 0.0;
            panic!("borrowed landing callback");
        }
    }
}

pub(in crate::game) struct HealthyLanding(pub(in crate::game) Arc<Probe>);
#[mod_api::sunrise_part]
impl mod_api::BlockPart for HealthyLanding {
    fn fall_on(&self, _: &mut dyn mod_api::LevelRandomHost, event: &mut mod_api::FallOnEvent<'_>) {
        if event.entity == 72 {
            self.0
                .healthy_proposals
                .lock()
                .unwrap()
                .push(Proposal::FallMultiplier(event.damage_multiplier));
        }
    }
}
#[mod_api::sunrise_part]
impl mod_api::EntityPart for GoalPart {
    fn register_goals(&self, goals: &mut mod_api::GoalRegistrar<'_>) {
        goals.add_target_goal(
            0,
            GoalSpec {
                probe: self.0.clone(),
                target: true,
            },
        );
        goals.add_goal(
            0,
            GoalSpec {
                probe: self.0.clone(),
                target: false,
            },
        );
    }
}
pub(in crate::game) struct Hooks {
    pub(in crate::game) probe: Arc<Probe>,
    pub(in crate::game) panics: bool,
}
impl Hooks {
    pub(in crate::game) fn hear(&self, family: Family, target: bool, input: Proposal) -> bool {
        if self.probe.family != family || !target {
            return false;
        }
        if self.panics {
            self.probe.enter_failing(input);
        } else {
            self.probe.healthy_proposals.lock().unwrap().push(input);
        }
        self.panics
    }
}
#[mod_api::__subscriptions]
impl mod_api::Hooks for Hooks {
    fn item_spawn(&self, _: &mut dyn mod_api::ServerHost, event: &mut mod_api::ItemSpawnEvent<'_>) {
        if !self.panics && event.item == "minecraft:egg" {
            self.probe.outputs.lock().unwrap().push(OutputNotice::Egg);
        }
    }
    fn entity_tick_listened(
        &self,
        _: mod_api::HostSide<'_>,
        event: &mut mod_api::EntityTickListenedEvent,
    ) {
        if self.panics {
            event.listened = true;
        }
    }
    fn entity_tick_post(
        &self,
        _: mod_api::HostSide<'_>,
        event: &mut mod_api::EntityTickPostEvent<'_>,
    ) {
        if self.panics {
            self.probe
                .posts
                .lock()
                .unwrap()
                .extend_from_slice(event.entities);
        }
    }
    fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
        if self.probe.family == Family::Landing {
            let stone = r.block_key("minecraft:stone");
            if self.panics {
                r.block_part(stone, FailingLanding(self.probe.clone()));
            } else {
                r.block_part(stone, HealthyLanding(self.probe.clone()));
            }
        }
        if !self.panics {
            return;
        }
        r.register_attachment_type("borrowed-panic:retained", false)
            .unwrap();
        if self.probe.family.is_goal() {
            r.entity_part("minecraft:zombie", GoalPart(self.probe.clone()));
        }
        if self.probe.family == Family::Knockback {
            let effect = r.effect_key("borrowed-panic:hit");
            r.mob_effect_part(&effect, GoalPart(self.probe.clone()))
                .unwrap();
        }
    }
    fn living_knockback(
        &self,
        host: &mut dyn mod_api::LevelRandomHost,
        event: &mut mod_api::LivingKnockbackEvent<'_>,
    ) -> mod_api::Flow {
        if self.hear(
            Family::Knockback,
            event.entity == 72,
            Proposal::Cancelled(event.cancelled),
        ) {
            self.probe.progress(host);
            event.cancelled = true;
            panic!("borrowed knockback callback");
        }
        mod_api::Flow::Continue
    }
    fn living_incoming_damage(
        &self,
        host: &mut dyn mod_api::HitHost,
        event: &mut mod_api::LivingIncomingDamageEvent<'_>,
    ) -> mod_api::Flow {
        if !self.panics && event.kind == "minecraft:player" {
            self.probe
                .outputs
                .lock()
                .unwrap()
                .push(OutputNotice::Guest(event.entity));
        }
        if self.hear(
            Family::Damage,
            event.entity == 72,
            Proposal::Cancelled(event.cancelled),
        ) {
            self.probe.progress(host);
            event.cancelled = true;
            panic!("borrowed incoming damage callback");
        }
        mod_api::Flow::Continue
    }
    fn living_shield_block(
        &self,
        host: &mut dyn mod_api::HitHost,
        event: &mut mod_api::LivingShieldBlockEvent<'_>,
    ) {
        if self.hear(
            Family::Shield,
            event.entity == 72,
            Proposal::Blocked(event.block.blocked),
        ) {
            self.probe.progress(host);
            event.block.blocked = true;
            event.block.blocked_damage = event.damage;
            panic!("borrowed shield callback");
        }
    }
    fn living_speed(
        &self,
        _: &dyn mod_api::LoopContext,
        event: &mut mod_api::LivingSpeedEvent<'_>,
    ) {
        if self.hear(
            Family::Movement,
            event.entity == 72,
            Proposal::Speed(event.speed),
        ) {
            event.speed += 1.0;
            panic!("borrowed movement callback");
        }
    }
    fn living_use_totem(
        &self,
        host: &mut dyn mod_api::HitHost,
        event: &mut mod_api::LivingUseTotemEvent<'_>,
    ) -> mod_api::Flow {
        if self.hear(
            Family::Totem,
            event.entity == 72,
            Proposal::Cancelled(event.cancelled),
        ) {
            self.probe.progress(host);
            event.cancelled = true;
            panic!("borrowed totem callback");
        }
        mod_api::Flow::Continue
    }
}
pub(in crate::game) const VALUE: AttachmentType<i32> = AttachmentType {
    namespace: "borrowed-panic",
    name: "retained",
    default: || 0,
    codec: None,
    copy_on_death: false,
    sync: false,
};
pub(in crate::game) struct Scratch(pub(in crate::game) std::path::PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::game) enum Output {
    Quiet,
    Egg,
    Guest,
    Push,
    Mixed,
    Reset,
}
pub(in crate::game) const FRESH_CASE: &str = "SUNRISE_BORROWED_PANIC_CASE";
pub(in crate::game) const BOTH_LEVELS: &[&str] = &["overworld", "nether"];

pub(in crate::game) fn fresh_dimension(test: &str, cases: &[&str]) -> Option<Dim> {
    let name = format!("game::creatures::borrowed_panic_tests::{test}");
    if run_fresh_cases(&name, FRESH_CASE, cases) {
        return None;
    }
    Some(match std::env::var(FRESH_CASE).unwrap().as_str() {
        "overworld" => Dim::Overworld,
        "nether" => Dim::Nether,
        other => panic!("unexpected borrowed callback case {other}"),
    })
}

pub(in crate::game) fn fixture_game(
    output: Output,
    test: impl FnOnce(&mut crate::game::ServerGame<'_>),
) {
    if matches!(output, Output::Guest | Output::Mixed) {
        crate::game::native_guest_turn_tests::with_guests(test);
    } else {
        with_game(test);
    }
}

pub(in crate::game) fn eggs(game: &crate::game::ServerGame<'_>) -> u32 {
    game.dropped
        .iter()
        .filter(|d| d.stack.item.as_ref() == "minecraft:egg")
        .map(|d| d.stack.count)
        .sum()
}

pub(in crate::game) fn tick_in_level(game: &mut crate::game::ServerGame<'_>) -> Result<()> {
    sunrise_sim::block_contact::in_level(
        sunrise_sim::parallel::TickThread::mint(),
        Some(game.dim.id()),
        || game.tick_creature_entities(),
    )
}

pub(in crate::game) fn prime_hook(mob: &mut Mob, family: Family) {
    mob.invulnerable_time = 0;
    if matches!(family, Family::Knockback | Family::Totem) {
        // These hooks need the hit to pass LivingEntity.hurtServer's damageCooldownTime check.
        mob.invulnerable = 0;
    }
    match family {
        Family::Damage | Family::Shield | Family::Totem => {
            // Entity.baseTick deals an on-fire hit on the 20-tick boundary.
            mob.fire_ticks = 20;
            if family == Family::Totem {
                // A previous totem leaves fire resistance and absorption. Start another lethal hit.
                mob.retain_effects(|_| false);
                mob.absorption = 0.0;
                mob.health = 0.5;
                mob.extra
                    .set_main_hand(Some("minecraft:totem_of_undying".into()));
            }
        }
        Family::Knockback => {
            mob.add_effect(
                sunrise_level::brewing::effect("borrowed-panic:hit")
                    .unwrap()
                    .id,
                10,
                0,
            );
        }
        Family::Landing => {
            mob.no_ai = false;
            mob.position = Vec3::new(2.0, 80.1, 0.5);
            mob.velocity = Vec3::new(0.0, -1.0, 0.0);
            mob.fall_distance = 6.0;
            mob.ground.on_ground = false;
        }
        Family::Movement => {
            // NoAI skips aiStep's travel; let a moving, grounded mob read its speed.
            mob.no_ai = false;
            mob.position = Vec3::new(2.0, 80.0, 0.5);
            mob.velocity = Vec3::ZERO;
            mob.ground.on_ground = true;
            mob.ai.speed = 0.2;
        }
        Family::Goal | Family::MountedGoal => {}
    }
}

pub(in crate::game) fn fixture_mob(
    game: &crate::game::ServerGame<'_>,
    id: u32,
    family: Family,
    output: Output,
) -> Mob {
    let kind = game
        .res
        .mob_types()
        .get(match id {
            72 => "minecraft:zombie",
            73 => "minecraft:chicken",
            _ => match output {
                Output::Egg | Output::Reset => "minecraft:chicken",
                Output::Guest | Output::Mixed => "minecraft:slime",
                Output::Quiet | Output::Push => "minecraft:pig",
            },
        })
        .unwrap()
        .clone();
    let mut mob = Mob::new(id, kind, Vec3::new(f64::from(id - 70), 80.0, 0.5), 0.0);
    mob.no_ai = true;
    if output == Output::Push && id == 70 {
        mob.position.x = 1.2;
    }
    if family == Family::MountedGoal {
        mob.no_ai = false;
        mob.ground.on_ground = true;
        // Collision on the vehicle's first tick must keep it grounded.
        mob.velocity.y = -0.08;
        mob.ai.nav.speed_modifier = if id == 72 { 3.75 } else { 9.25 };
        if id == 72 {
            mob.extra.mount = Some(71);
        } else {
            mob.extra.rider = Some(72);
        }
    }
    match id {
        71 => {
            mob.ai.last_hurt_by_player = 5;
            if matches!(output, Output::Egg | Output::Guest | Output::Push) {
                mob.no_ai = false;
                mob.ground.on_ground = true;
            }
            match output {
                Output::Egg => mob.extra.egg_time = 1,
                // NoAI chickens still lay; keep the replacement's timer above zero on both ticks.
                Output::Reset => mob.extra.egg_time = 3,
                Output::Guest | Output::Mixed => sunrise_sim::mobs::cube::set_size(&mut mob, 2),
                Output::Quiet | Output::Push => {}
            }
        }
        72 => {
            if family.is_goal() {
                mob.no_ai = false;
                assert!(mob.ai.goals.has("BorrowedProbe"));
            }
            prime_hook(&mut mob, family);
        }
        73 => mob.extra.egg_time = 1,
        _ => {}
    }
    mob
}

pub(in crate::game) fn guest_health(game: &crate::game::ServerGame<'_>) -> [f32; 2] {
    let guests = &game.side.lan.as_ref().unwrap().hub.guests;
    [guests[0].health, guests[1].health]
}

pub(in crate::game) fn run_output(test: &str, family: Family, output: Output) {
    if let Some(dim) = fresh_dimension(test, BOTH_LEVELS) {
        run_fixture(family, output, dim);
    }
}

#[allow(unused_imports)]
pub(in crate::game) use crate::game::server_side::creatures_server::borrowed_panic_tests::run_fixture;

#[test]
#[ignore = "daily"]
fn damage() {
    run_output("damage", Family::Damage, Output::Quiet);
}
#[test]
#[ignore = "daily"]
fn shield() {
    run_output("shield", Family::Shield, Output::Quiet);
}
#[test]
#[ignore = "daily"]
fn totem() {
    run_output("totem", Family::Totem, Output::Quiet);
}
#[test]
#[ignore = "daily"]
fn movement() {
    run_output("movement", Family::Movement, Output::Quiet);
}
#[test]
#[ignore = "daily"]
fn knockback() {
    run_output("knockback", Family::Knockback, Output::Quiet);
}
#[test]
#[ignore = "daily"]
fn landing() {
    run_output("landing", Family::Landing, Output::Quiet);
}

#[test]
#[ignore = "daily"]
fn goal() {
    run_output("goal", Family::Goal, Output::Quiet);
}
#[test]
#[ignore = "daily"]
fn mounted_goal() {
    run_output("mounted_goal", Family::MountedGoal, Output::Quiet);
}

#[test]
#[ignore = "daily"]
fn retained_egg() {
    run_output("retained_egg", Family::Damage, Output::Egg);
}
#[test]
#[ignore = "daily"]
fn retained_guest() {
    run_output("retained_guest", Family::Damage, Output::Guest);
}
#[test]
#[ignore = "daily"]
fn retained_mixed_order() {
    run_output("retained_mixed_order", Family::Damage, Output::Mixed);
}
#[test]
#[ignore = "daily"]
fn retained_push() {
    run_output("retained_push", Family::Damage, Output::Push);
}
#[test]
#[ignore = "daily"]
fn retained_reset() {
    run_output("retained_reset", Family::Damage, Output::Reset);
}

#[derive(Default)]
pub(in crate::game) struct ConsumerProbe {
    pub(in crate::game) calls: AtomicUsize,
    pub(in crate::game) eggs: Mutex<Vec<u32>>,
    pub(in crate::game) posts: Mutex<Vec<u32>>,
}

impl ConsumerProbe {
    pub(in crate::game) fn assert_tick(&self, game: &crate::game::ServerGame<'_>, egg_count: u32) {
        assert_eq!(self.calls.load(Ordering::SeqCst), 1);
        assert_eq!(std::mem::take(&mut *self.posts.lock().unwrap()), [71, 72]);
        assert_eq!(eggs(game), egg_count);
        let joined: Vec<_> = game
            .dropped
            .iter()
            .filter(|d| d.stack.item.as_ref() == "minecraft:egg")
            .map(|d| d.id)
            .collect();
        assert_eq!(joined.len(), egg_count as usize);
        assert!(
            joined
                .iter()
                .enumerate()
                .all(|(i, id)| !joined[..i].contains(id))
        );
        assert_eq!(
            *self.eggs.lock().unwrap(),
            joined,
            "healthy observer missed a joined egg"
        );
    }
}

pub(in crate::game) struct Consumer {
    pub(in crate::game) probe: Arc<ConsumerProbe>,
    pub(in crate::game) panics: bool,
}
#[mod_api::__subscriptions]
impl mod_api::Hooks for Consumer {
    fn item_spawn(&self, _: &mut dyn mod_api::ServerHost, event: &mut mod_api::ItemSpawnEvent<'_>) {
        if event.item != "minecraft:egg" {
            return;
        }
        if self.panics {
            self.probe.calls.fetch_add(1, Ordering::SeqCst);
            panic!("real item consumer");
        }
        self.probe.eggs.lock().unwrap().push(event.entity);
    }

    fn entity_tick_listened(
        &self,
        _: mod_api::HostSide<'_>,
        event: &mut mod_api::EntityTickListenedEvent,
    ) {
        if self.panics {
            event.listened = true;
        }
    }

    fn entity_tick_post(
        &self,
        _: mod_api::HostSide<'_>,
        event: &mut mod_api::EntityTickPostEvent<'_>,
    ) {
        if self.panics {
            self.probe
                .posts
                .lock()
                .unwrap()
                .extend_from_slice(event.entities);
        }
    }
}

#[allow(unused_imports)]
pub(in crate::game) use crate::game::server_side::creatures_server::borrowed_panic_tests::with_consumer_game;

pub(in crate::game) fn queue_egg(game: &mut crate::game::ServerGame<'_>) {
    game.creatures
        .pending
        .entry(game.dim.id())
        .or_default()
        .batches
        .push_back(MobOutputBatch {
            normal: NormalMobActions::new(vec![MobEvent::LayEgg {
                mob: 71,
                at: [1.0, 80.0, 0.5],
                pitch: 1.0,
            }]),
            guests: Vec::new(),
        });
}

#[test]
#[ignore = "daily"]
fn retained_consumer_tail() {
    let Some(dim) = fresh_dimension("retained_consumer_tail", BOTH_LEVELS) else {
        return;
    };
    with_consumer_game(dim, |game, probe| {
        let kind = game
            .res
            .mob_types()
            .get("minecraft:chicken")
            .unwrap()
            .clone();
        for id in [71, 72] {
            let mut mob = Mob::new(
                id,
                kind.clone(),
                Vec3::new(f64::from(id - 70), 80.0, 0.5),
                0.0,
            );
            mob.extra.egg_time = 1;
            game.mobs.push(mob);
        }
        game.tick_creature_entities().unwrap();
        assert_panic_report("consumer", "item_spawn");
        probe.assert_tick(game, 2);
        assert!(game.mobs.iter().all(|m| m.extra.egg_time > 1));

        // An ordinary tick neither loses nor replays either accepted egg.
        game.tick_creature_entities().unwrap();
        probe.assert_tick(game, 2);
        assert!(take_hook_panics().is_empty());

        game.mobs[0].extra.egg_time = 1;
        game.tick_creature_entities().unwrap();
        probe.assert_tick(game, 3);
        assert!(take_hook_panics().is_empty());
    });
}

#[test]
#[ignore = "daily"]
fn retained_conditional_context() {
    run_conditional("retained_conditional_context", false);
}

#[test]
#[ignore = "daily"]
fn retained_nested_tail() {
    run_conditional("retained_nested_tail", true);
}

pub(in crate::game) fn run_conditional(test: &str, nested: bool) {
    let Some(dim) = fresh_dimension(test, &["overworld"]) else {
        return;
    };
    with_consumer_game(dim, |game, probe| {
        let kind = game
            .res
            .mob_types()
            .get("minecraft:chicken")
            .unwrap()
            .clone();
        for id in [71, 72] {
            let mut mob = Mob::new(
                id,
                kind.clone(),
                Vec3::new(f64::from(id - 70), 80.0, 0.5),
                0.0,
            );
            mob.no_ai = true;
            game.mobs.push(mob);
        }
        let stone = game
            .pack
            .assets
            .registry
            .default_state("minecraft:stone")
            .unwrap();
        game.world.set(4, 80, 4, stone).unwrap();
        let conditional = || MobEvent::IfBroke {
            mob: 71,
            effects: vec![MobEvent::Jump { mob: 71 }],
            otherwise: vec![MobEvent::Jump { mob: 72 }],
        };
        let egg = MobEvent::LayEgg {
            mob: 71,
            at: [1.0, 80.0, 0.5],
            pitch: 1.0,
        };
        let mut events = vec![MobEvent::BreakBlock {
            pos: [4, 80, 4],
            drop: false,
            by: Some(71),
            breaker: None,
        }];
        if nested {
            game.world.set(4, 80, 3, stone).unwrap();
            events.push(MobEvent::IfBroke {
                mob: 71,
                effects: vec![
                    MobEvent::BreakBlock {
                        pos: [4, 80, 3],
                        drop: false,
                        by: Some(72),
                        breaker: None,
                    },
                    egg,
                    MobEvent::IfBroke {
                        mob: 72,
                        effects: vec![MobEvent::Jump { mob: 71 }],
                        otherwise: Vec::new(),
                    },
                ],
                otherwise: Vec::new(),
            });
        } else {
            events.extend([egg, conditional()]);
        }
        // The same frame keeps its completed breaks; a new batch does not inherit them.
        let batch = game.creatures.pending.entry(game.dim.id()).or_default();
        batch.batches.push_back(MobOutputBatch {
            normal: NormalMobActions::new(events),
            guests: Vec::new(),
        });
        batch.batches.push_back(MobOutputBatch {
            normal: NormalMobActions::new(vec![conditional()]),
            guests: Vec::new(),
        });

        game.tick_creature_entities().unwrap();
        assert_panic_report("consumer", "item_spawn");
        probe.assert_tick(game, 1);
        assert_eq!(game.world.block(4, 80, 4), game.world.air);
        if nested {
            assert_eq!(game.world.block(4, 80, 3), game.world.air);
        }
        assert!(
            game.mobs[0].ai.jumping,
            "same frame lost its completed break"
        );
        assert!(
            game.mobs[1].ai.jumping,
            "fresh batch inherited an earlier break"
        );

        for mob in &mut game.mobs {
            mob.ai.jumping = false;
        }
        queue_egg(game);
        game.tick_creature_entities().unwrap();
        probe.assert_tick(game, 2);
        assert!(
            game.mobs.iter().all(|m| !m.ai.jumping),
            "completed conditional replayed"
        );
        assert!(take_hook_panics().is_empty());
    });
}

/// `living_breathe` with the owning host: a listener reads the mob's air and edits what it holds
/// through a transaction, and the game applies its answer to the air it had decided on, or
/// refuses when the listener changed that air and rolls the transactional edit back.
pub(in crate::game) struct Breather {
    pub(in crate::game) meddle: bool,
    pub(in crate::game) seen_air: Arc<std::sync::Mutex<Vec<i32>>>,
}
#[mod_api::__subscriptions]
impl mod_api::Hooks for Breather {
    fn living_breathe(
        &self,
        host: &mut dyn mod_api::ServerHost,
        event: &mut mod_api::LivingBreatheEvent,
    ) {
        use mod_api::HostTransaction;
        if event.entity.id() != 72 {
            return;
        }
        let entity = event.entity.entity();
        self.seen_air
            .lock()
            .unwrap()
            .push(entity.air_supply(host).unwrap());
        let place = mod_api::ItemPlace::Equipment {
            entity: 72,
            slot: mod_api::EquipmentSlot::MainHand,
        };
        host.transaction(|host, tx| place.edit(host, tx, |stack| stack.set_damage(5)))
            .unwrap();
        if self.meddle {
            assert!(entity.set_air_supply(host, 50));
        }
        event.can_breathe = false;
        event.consume = 7;
    }
}

#[allow(unused_imports)]
pub(in crate::game) use crate::game::server_side::creatures_server::borrowed_panic_tests::breathing_tick;

#[test]
#[ignore = "daily"]
fn a_mob_breathes_with_what_its_listener_answers_and_wrote() {
    // Vanilla would raise the 100 air by 4; the listener took 7 and damaged its sword.
    assert_eq!(breathing_tick(false), (vec![100], 93, 5));
}

#[test]
#[ignore = "daily"]
fn a_mob_whose_air_the_listener_changed_keeps_it_and_loses_the_transactional_writes() {
    // The decision read 100; the listener left 50, so the answer is refused rather than
    // recomputed (not 43, not 54), and the sword's edit is undone.
    assert_eq!(breathing_tick(true), (vec![100], 50, 0));
}

/// A listener that edits the mob's sword in a transaction and then panics in `living_breathe`
/// without touching `air`: under the unified failure rule, the panic is contained, the enclosing
/// transaction aborts (rolling back the sword edit and leaving air unchanged), and the listener is
/// disabled for `living_breathe` for the rest of the session so subsequent ticks run vanilla.
pub(in crate::game) struct BreathPanic(pub(in crate::game) Arc<AtomicUsize>);
#[mod_api::__subscriptions]
impl mod_api::Hooks for BreathPanic {
    fn living_breathe(
        &self,
        host: &mut dyn mod_api::ServerHost,
        event: &mut mod_api::LivingBreatheEvent,
    ) {
        use mod_api::HostTransaction;
        if event.entity.id() != 72 {
            return;
        }
        self.0.fetch_add(1, Ordering::SeqCst);
        let place = mod_api::ItemPlace::Equipment {
            entity: 72,
            slot: mod_api::EquipmentSlot::MainHand,
        };
        host.transaction(|host, tx| place.edit(host, tx, |stack| stack.set_damage(5)))
            .unwrap();
        panic!("real breathing listener");
    }
}

#[test]
#[ignore = "daily"]
fn player_living_breathe_panic_rolls_back_transactional_writes() {
    struct PlayerBreathPanic {
        player: u128,
        calls: Arc<AtomicUsize>,
    }
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for PlayerBreathPanic {
        fn living_breathe(
            &self,
            host: &mut dyn mod_api::ServerHost,
            _: &mut mod_api::LivingBreatheEvent,
        ) {
            use mod_api::HostTransaction;
            let place = mod_api::ItemPlace::PlayerHand {
                player: self.player,
                hand: mod_api::client::first_person::Hand::Main,
            };
            host.transaction(|host, tx| place.edit(host, tx, |stack| stack.set_damage(5)))
                .unwrap();
            assert_eq!(place.with_stack(host, |stack| stack.damage()), Some(5));
            self.calls.fetch_add(1, Ordering::SeqCst);
            panic!("player breathing listener");
        }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    crate::game::native_guest_turn_tests::with_guests(|game| {
        game.as_guest(12, |game| {
            game.player.as_mut().unwrap().air = 100;
            game.selected = 0;
            game.inventory[0] = Some(sunrise_items::items::ItemStack::new(
                "minecraft:iron_sword",
                1,
            ));
            let step = sunrise_sim::breathing::AirStep {
                air: 100,
                max: 300,
                vanilla: sunrise_sim::breathing::Breath {
                    can_breathe: true,
                    consume: 0,
                    refill: 4,
                },
                own: false,
            };
            mod_platform::testing::with_mods(
                vec![mod_api::ModEntry::new(
                    "player-breath-panic",
                    Box::new(PlayerBreathPanic {
                        player: game.native_acting_player().unwrap().uuid(),
                        calls: calls.clone(),
                    }),
                )]
                .into(),
                || {
                    assert!(game.ask_player_breath(step).is_none());
                    assert_eq!(calls.load(Ordering::SeqCst), 1);
                    assert_eq!(game.player.as_ref().unwrap().air, 100);
                    assert_eq!(game.inventory[0].as_ref().unwrap().damage, 0);
                    assert_eq!(
                        mod_api::__private::take_hook_panics(),
                        [mod_api::__private::HookPanicReport {
                            mod_id: "player-breath-panic",
                            hook: "living_breathe",
                        }]
                    );
                    // Second call skips the poisoned listener and accepts the vanilla breath.
                    assert_eq!(game.ask_player_breath(step), Some(step.vanilla));
                    assert_eq!(calls.load(Ordering::SeqCst), 1);
                    assert_eq!(game.inventory[0].as_ref().unwrap().damage, 0);
                },
            );
        })
        .unwrap();
    });
}
