#[cfg(test)]
#[path = "shared/item_fall_on_tests.rs"]
pub(in crate::game) mod item_fall_on_tests;
#[path = "shared/level_helpers.rs"]
pub(in crate::game) mod level_helpers;
#[cfg(test)]
#[path = "shared/particle_count_tests.rs"]
pub(in crate::game) mod particle_count_tests;
#[cfg(test)]
use level_helpers::block_entity_for_block;
#[cfg(test)]
#[path = "shared/audio_math_tests.rs"]
pub(in crate::game) mod audio_math_tests;
#[cfg(test)]
#[path = "shared/block_property_tests.rs"]
pub(in crate::game) mod block_property_tests;
#[cfg(test)]
#[path = "shared/gameplay_hook_tests.rs"]
pub(in crate::game) mod gameplay_hook_tests;
#[path = "shared/item_fallthrough.rs"]
pub(in crate::game) mod item_fallthrough;
#[cfg(test)]
#[path = "shared/label_tests.rs"]
pub(in crate::game) mod label_tests;
#[cfg(test)]
#[path = "shared/live_damage_tests.rs"]
pub(in crate::game) mod live_damage_tests;
#[path = "shared/loot.rs"]
pub(in crate::game) mod loot;
#[cfg(test)]
#[path = "shared/packet_tests.rs"]
pub(in crate::game) mod packet_tests;
#[cfg(test)]
#[path = "shared/runtime_environment_tests.rs"]
pub(in crate::game) mod runtime_environment_tests;
#[cfg(test)]
#[path = "shared/screen_position_tests.rs"]
pub(in crate::game) mod screen_position_tests;
#[cfg(test)]
#[path = "shared/slot_icon_tests.rs"]
pub(in crate::game) mod slot_icon_tests;
#[cfg(test)]
#[path = "shared/slot_tooltip_tests.rs"]
pub(in crate::game) mod slot_tooltip_tests;
#[path = "shared/system_helpers.rs"]
pub(in crate::game) mod system_helpers;
#[path = "shared/world_helpers.rs"]
pub(in crate::game) mod world_helpers;

use crate::game::api_values::{TryIntoApi, TryIntoEngine};
use crate::game::item_particle_send::supported;
#[cfg(test)]
use screen_chars::chars_to_type;

/// Write back a `menu_button` cursor edit without revalidating an untouched stack.
pub(in crate::game) fn apply_menu_button_cursor(
    items: &sunrise_items::items::Items,
    original: &Option<mod_api::ItemStack>,
    cursor: &Option<mod_api::ItemStack>,
    carried: &mut Option<ItemStack>,
) {
    if cursor == original {
        return;
    }
    match cursor.try_into_engine(items) {
        Ok(updated) => *carried = updated,
        Err(error) => {
            log::warn!("mod menu_button write refused: {error:?}");
        }
    }
}

#[cfg(feature = "client")]
#[cfg(test)]
use crate::game::client_side::native_hooks_client::posed_camera;

/// `Level.setBlock`'s flags, for the `native_set_block_property` helpers:
/// `flags::UPDATE_CLIENTS` as a vanilla crop grows, `flags::UPDATE_ALL` as a
/// furnace sets its `lit`.
#[allow(unused_imports)]
pub(in crate::game) use sunrise_world::level::flags;

/// The largest turn a shake gives, in degrees, on each axis.
pub(in crate::game) const SHAKE_MAX_DEGREES: f32 = 10.0;
/// The largest offset a shake gives, in blocks, along each axis.
pub(in crate::game) const SHAKE_MAX_OFFSET: f32 = 0.5;

#[cfg(test)]
use camera_math::bounded_shake;
#[allow(unused_imports)]
pub(in crate::game) use camera_math::hand_pose_matrix;

/// The pose a bounded shake multiplies into the projection, after the view
/// bobbing: the offset, then roll, pitch and yaw.
pub(in crate::game) fn shake_pose(shake: Shake) -> glam::Mat4 {
    let [x, y, z] = shake.offset;
    glam::Mat4::from_translation(glam::Vec3::new(-x, -y, -z))
        * glam::Mat4::from_rotation_z(shake.roll.to_radians())
        * glam::Mat4::from_rotation_x(shake.pitch.to_radians())
        * glam::Mat4::from_rotation_y(shake.yaw.to_radians())
}

/// The player as a native mod reads it.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub struct NativePlayer {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// Degrees; 0 faces south, 90 west (`Direction.fromYRot`).
    pub yaw: f32,
    pub pitch: f32,
    /// `minecraft:overworld`, `minecraft:the_nether`, `minecraft:the_end`.
    pub dimension: &'static str,
}

/// How the panel lays out a recipe of `native_panel_recipes`.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeRecipeLayout {
    /// A crafting grid of `width` x `height` slots, an arrow and the result.
    Grid,
    /// One input over a flame, an arrow and the result (furnaces, smokers,
    /// campfires).
    Cooking,
    /// One input, an arrow and the result (the stonecutter).
    Single,
    /// Template, base and addition, an arrow and the result.
    Smithing,
    /// A potion and an ingredient, an arrow and the result.
    Brewing,
}

/// One result of a recipe the panel shows (`native_panel_recipes`): a slot
/// cycling through `stacks`, one a second, and the chance in percent the recipe
/// gives it (`Some(25.0)` draws "25%"); `None` gives it every time.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeRecipeResult {
    pub(crate) stacks: Vec<mod_api::Stack>,
    pub(crate) chance: Option<f32>,
}

/// A recipe the panel shows (`native_panel_recipes`). Each slot, and each
/// result, cycles through its stacks, one a second; an empty slot shows
/// nothing. A crafting recipe has one result and no chance.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeRecipeView {
    pub(crate) layout: NativeRecipeLayout,
    /// The grid's size (`Grid` only), at most 3 x 3; slots past it continue
    /// in rows under it, 27 in all.
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) slots: Vec<Vec<mod_api::Stack>>,
    /// Up to four, left to right after the arrow.
    pub(crate) results: Vec<NativeRecipeResult>,
    /// Fluids the recipe takes and gives, `amount` in millibuckets, each in
    /// a well with its amount under it; at most eight each. A well shows the
    /// fluid's client look (`data/<mod>/sunrise/fluid/`: sprite and tint), a
    /// flat square of vanilla's colour for water and lava and else grey when
    /// the fluid has none.
    pub(crate) fluid_inputs: Vec<FluidStack>,
    pub(crate) fluid_outputs: Vec<FluidStack>,
    /// A short line under the recipe (a cooking time), or "".
    pub(crate) note: String,
    /// The action button beside it: none, enabled, or shown greyed.
    pub(crate) action: Option<bool>,
}

/// An item of [`NativeRecipes::items`], by its index there, with a count
/// and the components it has beyond the item's own, in vanilla's item
/// syntax (`[minecraft:potion_contents={potion:"minecraft:swiftness"}]`),
/// or "".
#[allow(dead_code)]
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NativeRecipeStack {
    pub(crate) item: u32,
    pub(crate) count: u32,
    pub(crate) components: String,
}

/// The station a recipe is made at.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeRecipeKind {
    CraftingShaped,
    CraftingShapeless,
    Smelting,
    Blasting,
    Smoking,
    Campfire,
    Stonecutting,
    Smithing,
    Brewing,
}

/// A recipe of [`NativeRecipes`].
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeRecipe {
    /// `minecraft:oak_planks`.
    pub(crate) id: String,
    pub(crate) kind: NativeRecipeKind,
    /// The recipe book's tab (`crafting_building_blocks`, `furnace_food`),
    /// or "".
    pub(crate) category: String,
    /// Recipes the book shows on one button share a group, or "".
    pub(crate) group: String,
    /// A shaped recipe's grid: `slots` holds its cells row by row. 0 for
    /// the other kinds.
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// Each slot's stacks, any one of which fits; an empty list is an empty
    /// cell. Cooking and stonecutting: the input. Smithing: the template,
    /// the base, the addition. Brewing: the bottle, the reagent.
    pub(crate) slots: Vec<Vec<NativeRecipeStack>>,
    /// What it makes; several when that varies (a trim on any armour).
    pub(crate) results: Vec<NativeRecipeStack>,
    /// Cooking: game ticks and experience; 0 for the other kinds.
    pub(crate) cooking_time: u32,
    pub(crate) experience: f32,
    /// The recipe book's id for it (one per way the book shows it), as
    /// `known_recipes` lists them; -1 for the kinds the book doesn't
    /// show (campfire, stonecutting, smithing, brewing).
    pub(crate) book_id: i32,
}

/// An item of [`NativeRecipes::items`].
#[allow(dead_code)]
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NativeItem {
    /// `minecraft:diamond`, `<mod>:<path>`.
    pub(crate) id: String,
    /// In the player's language.
    pub(crate) name: String,
    /// Its item tags, without the `#`.
    pub(crate) tags: Vec<String>,
}

/// Every item and recipe the game knows (`native_recipes`): `items` in the
/// registry's order, which each [`NativeRecipeStack::item`] indexes; the
/// recipes are the book's in its order, then campfire, stonecutting,
/// smithing and brewing.
#[allow(dead_code)]
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NativeRecipes {
    pub(crate) items: Vec<NativeItem>,
    pub(crate) recipes: Vec<NativeRecipe>,
}

/// `native_living_use_totem` for the damage code: whether it
/// canceled the totem. The entity and the source are named as the damage hook names them.
pub(in crate::game) fn use_totem(
    level: DamageLevel<'_>,
    hurting: Hurting<'_>,
    off_hand: bool,
) -> bool {
    if !mod_api::__private::listened(mod_api::HookId::LivingUseTotem) {
        return false;
    }
    let (entity, kind, source) = hurt_server_args(hurting, level.attack());
    let hand = if off_hand { Hand::Off } else { Hand::Main };
    let mut cancel = false;
    HitLevel(level).native_living_use_totem(
        entity,
        kind,
        &source,
        hand,
        "minecraft:totem_of_undying",
        &mut cancel,
    );
    cancel
}

/// The level a hit or a landing happens in, as its hooks' level host: its blocks, its random and its
/// own attachments. Not `GameLevel`, which a hit can't build: a mob's hit lands in the mob
/// tick, where the world is read by every mob and its random is lent out, and building one
/// per hit would allocate its entity boxes.
pub(in crate::game) struct HitLevel<'a>(pub(in crate::game) DamageLevel<'a>);

/// Let listeners override the vanilla invulnerability decision for this hit.
pub(in crate::game) fn invulnerability_check(
    level: DamageLevel<'_>,
    hurting: Hurting<'_>,
    vanilla: bool,
) -> bool {
    if !mod_api::__private::listened(mod_api::HookId::EntityInvulnerabilityCheck) {
        return vanilla;
    }
    let (entity, kind, source) = hurt_server_args(hurting, level.attack());
    let mut invulnerable = vanilla;
    HitLevel(level).native_entity_invulnerability_check(
        entity,
        kind,
        &source,
        vanilla,
        &mut invulnerable,
    );
    invulnerable
}

pub(in crate::game) fn engine_reduction(
    reduction: mod_api::Reduction,
) -> sunrise_world::living_damage::Reduction {
    match reduction {
        mod_api::Reduction::Invulnerability => {
            sunrise_world::living_damage::Reduction::Invulnerability
        }
        mod_api::Reduction::Armor => sunrise_world::living_damage::Reduction::Armor,
        mod_api::Reduction::Enchantments => sunrise_world::living_damage::Reduction::Enchantments,
        mod_api::Reduction::MobEffects => sunrise_world::living_damage::Reduction::MobEffects,
        mod_api::Reduction::Absorption => sunrise_world::living_damage::Reduction::Absorption,
        mod_api::Reduction::InnateResistance => {
            sunrise_world::living_damage::Reduction::InnateResistance
        }
        _ => unreachable!("unknown API reduction"),
    }
}

pub(in crate::game) struct LiveDamageRead<'a>(
    pub(in crate::game) &'a sunrise_world::living_damage::DamageState,
);

impl mod_api::__private::DamageContainerRead for LiveDamageRead<'_> {
    fn original(&self) -> f32 {
        self.0.original()
    }
    fn new_damage(&self) -> f32 {
        self.0.new_damage()
    }
    fn blocked(&self) -> f32 {
        self.0.blocked()
    }
    fn shield_damage(&self) -> f32 {
        self.0.shield_damage()
    }
    fn post_attack_invulnerability_ticks(&self) -> i32 {
        self.0.post_attack_invulnerability_ticks()
    }
    fn should_cause_side_effects(&self) -> bool {
        self.0.should_cause_side_effects()
    }
    fn inflicted(&self) -> f32 {
        self.0.inflicted()
    }
    fn health_damage(&self) -> f32 {
        self.0.health_damage()
    }
    fn reduction(&self, reduction: mod_api::Reduction) -> f32 {
        self.0.reduction(engine_reduction(reduction))
    }
}

pub(in crate::game) struct LiveDamageContainer<'a>(pub(in crate::game) &'a mut DamageContainer);

impl mod_api::__private::DamageContainerRead for LiveDamageContainer<'_> {
    fn original(&self) -> f32 {
        self.0.original()
    }
    fn new_damage(&self) -> f32 {
        self.0.new_damage()
    }
    fn blocked(&self) -> f32 {
        self.0.blocked()
    }
    fn shield_damage(&self) -> f32 {
        self.0.shield_damage()
    }
    fn post_attack_invulnerability_ticks(&self) -> i32 {
        self.0.post_attack_invulnerability_ticks()
    }
    fn should_cause_side_effects(&self) -> bool {
        self.0.should_cause_side_effects()
    }
    fn inflicted(&self) -> f32 {
        self.0.inflicted()
    }
    fn health_damage(&self) -> f32 {
        self.0.health_damage()
    }
    fn reduction(&self, reduction: mod_api::Reduction) -> f32 {
        self.0.reduction(engine_reduction(reduction))
    }
}

impl mod_api::__private::DamageContainerBackend for LiveDamageContainer<'_> {
    fn set_new_damage(&mut self, value: f32) {
        self.0.record_new_damage(value);
    }
    fn set_post_attack_invulnerability_ticks(&mut self, value: i32) {
        self.0.record_post_attack_invulnerability_ticks(value);
    }
    fn add_reduction_modifier(
        &mut self,
        reduction: mod_api::Reduction,
        owner: Option<&'static str>,
        modifier: Box<dyn FnMut(mod_api::DamageContainerView<'_>, f32) -> f32>,
    ) {
        let mut wrapped = mod_api::__private::wrap_reduction_modifier(owner, modifier);
        self.0.add_reduction_modifier(
            engine_reduction(reduction),
            Box::new(move |engine_container: &DamageContainer, amount: f32| {
                wrapped(
                    mod_api::DamageContainerView::new(&LiveDamageRead(engine_container)),
                    amount,
                )
            }),
        );
    }
}

/// `native_living_incoming_damage` for the damage code: whether it
/// canceled the hit.
pub(in crate::game) fn incoming_damage(
    level: DamageLevel<'_>,
    hurting: Hurting<'_>,
    victim: Victim,
    container: &mut DamageContainer,
) -> bool {
    let listens_incoming = mod_api::__private::listened(mod_api::HookId::LivingIncomingDamage);
    if listens_incoming
        || mod_api::__private::listened(mod_api::HookId::LivingDamagePre)
        || mod_api::__private::listened(mod_api::HookId::LivingDamagePost)
    {
        container.enable_live_reductions();
    }
    if !listens_incoming {
        return false;
    }
    let (entity, kind, source) = hurt_server_args(hurting, level.attack());
    let mut canceled = false;
    let attack = level.attack();
    let Ok(weapon) = attack.weapon_of(source.direct).try_into_api() else {
        return false;
    };
    let causing_weapon = (source.causing != 0 && source.causing == attack.attacker)
        .then_some(attack.held[0].as_ref())
        .flatten();
    let Ok(causing_weapon) = causing_weapon.try_into_api() else {
        return false;
    };
    let mut live = LiveDamageContainer(container);
    HitLevel(level.caused_by(source.causing)).native_living_incoming_damage(
        mod_api::__private::LivingIncomingDamageCall {
            entity,
            kind,
            source: &source,
            victim: &victim,
            weapon: weapon.as_ref(),
            causing_weapon: causing_weapon.as_ref(),
            container: mod_api::DamageContainer::new(&mut live),
            canceled: &mut canceled,
        },
    );
    canceled
}

/// `native_living_damage_pre` for the damage code.
pub(in crate::game) fn damage_pre(
    level: DamageLevel<'_>,
    hurting: Hurting<'_>,
    container: &mut DamageContainer,
) {
    if !mod_api::__private::listened(mod_api::HookId::LivingDamagePre) {
        return;
    }
    container.enable_live_reductions();
    let (entity, kind, source) = hurt_server_args(hurting, level.attack());
    let mut live = LiveDamageContainer(container);
    HitLevel(level.caused_by(source.causing)).native_living_damage_pre(
        mod_api::__private::LivingDamagePreCall {
            entity,
            kind,
            source: &source,
            container: mod_api::DamageContainer::new(&mut live),
        },
    );
}

/// `native_living_damage_post` for the damage code.
pub(in crate::game) fn damage_post(
    level: DamageLevel<'_>,
    hurting: Hurting<'_>,
    container: &DamageContainer,
) {
    if !mod_api::__private::listened(mod_api::HookId::LivingDamagePost) {
        return;
    }
    let (entity, kind, source) = hurt_server_args(hurting, level.attack());
    let live = LiveDamageRead(container);
    HitLevel(level.caused_by(source.causing)).native_living_damage_post(
        entity,
        kind,
        &source,
        mod_api::DamageContainerView::new(&live),
    );
}

/// `native_living_shield_block` for the damage code. Its one
/// caller. The hurt entity and the source are named as the damage hook names them.
pub(in crate::game) fn shield_block(
    level: DamageLevel<'_>,
    hurting: Hurting<'_>,
    damage: f32,
    original_blocked: bool,
    block: &mut ShieldBlock,
) {
    let (entity, kind, source) = hurt_server_args(hurting, level.attack());
    HitLevel(level).native_living_shield_block(
        entity,
        kind,
        &source,
        damage,
        original_blocked,
        block,
    );
}

/// `native_living_knockback` for the knockback code: whether it
/// canceled the push. Its one caller. The pushed entity and the source are named as the
/// damage hook names them.
pub(in crate::game) fn knockback(
    level: DamageLevel<'_>,
    pushed: Hurting<'_>,
    knock: &mut Knock,
) -> bool {
    if !mod_api::__private::listened(mod_api::HookId::LivingKnockback) {
        return false;
    }
    let attack = level.attack();
    let attacker = match &pushed {
        Hurting::Mob(mob, damage) => match damage.attacker.or(mob.ai.last_hurt_by) {
            Some(Who::Mob(id)) => id as i32,
            _ => attack.attacker,
        },
        Hurting::Player(_, source)
        | Hurting::Dragon(_, source)
        | Hurting::ArmorStand(_, source)
        | Hurting::Entity(_, _, source) => match source.causing {
            0 => attack.attacker,
            causing => causing,
        },
    };
    let (kill_credit, credit_main) = level.knockback_credit(&pushed);
    let Ok(credit_main) = credit_main.map(TryIntoApi::try_into_api).transpose() else {
        return false;
    };
    let (entity, kind, source) = hurt_server_args(pushed, level.attack());
    let mut canceled = false;
    let [main, off] = &attack.held;
    let (Ok(main), Ok(off)) = (main.try_into_api(), off.try_into_api()) else {
        return false;
    };
    let entry = level.knockback_entry();
    HitLevel(level).native_living_knockback(mod_api::__private::LivingKnockbackCall {
        entity,
        kind,
        source: &source,
        attacker,
        attacker_main: main.as_ref(),
        attacker_off: off.as_ref(),
        kill_credit,
        kill_credit_main: credit_main.as_ref(),
        entry,
        strength: &mut knock.strength,
        x: &mut knock.x,
        z: &mut knock.z,
        canceled: &mut canceled,
    });
    canceled
}

/// The damage hooks every level of the game lends to its hits (`Game.damage_hooks`).
pub(in crate::game) const DAMAGE_HOOKS: DamageHooks = DamageHooks {
    invulnerable: Some(invulnerability_check),
    incoming: Some(incoming_damage),
    pre: Some(damage_pre),
    post: Some(damage_post),
    owner_post_listened: Some(|| mod_api::__private::listened(mod_api::HookId::LivingDamagePost)),
    knockback: Some(knockback),
    totem: Some(use_totem),
    shield: Some(shield_block),
    accepted: Some(crate::game::native_hooks::living_death::capture_hit),
};
/// The contacts of a simulating entity with a block, as the simulation's own code (client-lib)
/// reports them.
pub(in crate::game) const BLOCK_CONTACT_HOOKS: sunrise_sim::block_contact::Hooks =
    sunrise_sim::block_contact::Hooks {
        step_listens: |state| {
            crate::game::api_values::block_state::block_listens(mod_api::HookId::StepOn, state)
        },
        step_on: |level, step, movement| {
            use self::bounce_restitution::StepOnView;
            use sunrise_sim::block_contact::StepLevel;
            match level {
                StepLevel::Server(level) => StepOnView(mod_api::LevelRandomSide::Server(
                    &mut HitLevel(level.reborrow()),
                ))
                .native_step_on(step, movement),
                StepLevel::Client(level) => StepOnView(mod_api::LevelRandomSide::Client(
                    &crate::game::native_surface::FrictionView(level),
                ))
                .native_step_on(step, movement),
            }
        },
        any_inside: || mod_api::__private::has_scoped_hook(mod_api::HookId::EntityInside),
        inside_listens: |state| {
            crate::game::api_values::block_state::block_listens(
                mod_api::HookId::EntityInside,
                state,
            )
        },
        pass_listens: |state| {
            crate::game::api_values::block_state::block_listens(
                mod_api::HookId::MinecartPass,
                state,
            )
        },
        fall_listens: |state| {
            crate::game::api_values::block_state::block_listens(mod_api::HookId::FallOn, state)
        },
        bounce_listens: |state| {
            crate::game::api_values::block_state::block_listens(mod_api::HookId::BounceOn, state)
        },
        inside_fire: |level, dimension, entity, kind, state, pos, precise, staged, writes| {
            // The whole callback is staged: a panic is contained and the walk
            // keeps the caller's collector and write list as they were (#2247's
            // rule: a mod's panic never breaks the model).
            let fired = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                HitLevel(level.reborrow()).native_entity_inside(
                    crate::game::native_hooks::fire::EntityInsideCall {
                        dimension,
                        entity,
                        kind,
                        state,
                        pos,
                        precise,
                        effects: staged,
                        writes,
                    },
                );
            }));
            if let Err(payload) = fired {
                if let Some(text) = payload.downcast_ref::<&str>() {
                    eprintln!("an entity_inside part panicked at {text:?}");
                } else if let Some(text) = payload.downcast_ref::<String>() {
                    eprintln!("an entity_inside part panicked at {text:?}");
                }
            }
        },
        fall_on: |dimension, level, c, distance, damage_multiplier, writes| {
            HitLevel(level.reborrow()).native_fall_on(
                crate::game::native_hooks::fire::FallOnCall {
                    dimension,
                    entity: c.entity,
                    kind: c.kind,
                    state: c.state,
                    pos: c.position,
                    distance,
                    damage_multiplier,
                    writes,
                },
            );
        },
        bounce_on: |dimension, level, c, fall_distance, movement| {
            let mut answer = [movement.x, movement.y, movement.z];
            HitLevel(level.reborrow()).native_bounce_on(
                crate::game::native_hooks::fire::BounceOnCall {
                    dimension,
                    entity: c.entity,
                    kind: c.kind,
                    state: c.state,
                    pos: c.position,
                    fall_distance,
                    movement: &mut answer,
                },
            );
            *movement = sunrise_core::geometry::Vec3::new(answer[0], answer[1], answer[2]);
        },
    };
#[allow(unused_imports)]
pub(in crate::game) use living_death::hurt_server_args;
#[allow(unused_imports)]
pub(in crate::game) use sunrise_sim::health::mob_source;

/// Whether a native mod's particles can be sent. Block particle types need
/// a known state (`state_ok`), item particles a validated stack; trail and vibration
/// aren't supported by the network dispatch. A mod's own type must be namespaced.
pub(in crate::game) fn mod_particles_ok(
    options: &ParticleOptions,
    state: Option<BlockStateId>,
    state_ok: impl Fn(BlockStateId) -> bool,
    positions: &[[f64; 3]],
    spread: [f64; 3],
    speed: f64,
) -> bool {
    use ParticleType as T;
    // A native mod's own type: the name has to be namespaced, and a client
    // draws it only when its pack has the sprite set.
    if let Some(name) = &options.mod_particle {
        let namespaced = name
            .split_once(':')
            .is_some_and(|(ns, path)| !ns.is_empty() && !path.is_empty());
        if !namespaced {
            return false;
        }
    }
    let supported = match options.ty {
        T::Block | T::BlockMarker | T::FallingDust | T::DustPillar | T::BlockCrumble => {
            state.is_some_and(state_ok)
        }
        T::Item | T::Trail | T::Vibration => supported(options),
        T::Geyser | T::GeyserPlume | T::GeyserBase | T::GeyserPoof => options
            .geyser
            .is_some_and(|(_, impulse)| impulse.is_finite()),
        _ => true,
    };
    supported
        && positions.iter().flatten().all(|v| v.is_finite())
        && spread
            .into_iter()
            .all(|v| v.is_finite() && v.abs() <= f32::MAX as f64)
        && speed.is_finite()
        && speed.abs() <= f32::MAX as f64
        && options.color.into_iter().all(f32::is_finite)
        && options.to_color.into_iter().all(f32::is_finite)
        && options.scale.is_finite()
}

pub(in crate::game) fn dimension_id(dimension: LevelDimension) -> &'static str {
    match dimension {
        LevelDimension::Overworld => "minecraft:overworld",
        LevelDimension::Nether => "minecraft:the_nether",
        LevelDimension::End => "minecraft:the_end",
        LevelDimension::Custom(key) => key.id(),
    }
}

/// A mob's kind as the hook points name it (`minecraft:zombie`).
pub(in crate::game) fn mob_kind(mob: &sunrise_sim::mobs::Mob) -> String {
    crate::game::native_namespaced_id::namespaced(&mob.kind.id)
}

/// The level behaviour's answer, or `None` to try `useWithoutItem`.
/// `ServerPlayerGameMode.useItemOn` lets both FAIL and PASS go to the item.
/// Both success swing sources swing on this server path.
pub(in crate::game) fn block_use_result(
    answer: mod_api::InteractionResult,
) -> Option<sunrise_world::level::InteractionResult> {
    use sunrise_world::level::InteractionResult;
    match answer {
        mod_api::InteractionResult::Success | mod_api::InteractionResult::SuccessServer => {
            Some(InteractionResult::Success)
        }
        mod_api::InteractionResult::Consume => Some(InteractionResult::Consume),
        mod_api::InteractionResult::TryWithEmptyHand => None,
        _ => Some(InteractionResult::Pass),
    }
}

/// What the mods answered a right click on a block (`right_click_block`).
pub(in crate::game) enum BlockClickAnswer {
    /// Cancelled: this answers the click instead of the block and the item.
    Canceled(mod_api::InteractionResult),
    /// The block's use and the item's `useOn` run as these say.
    Go {
        use_block: TriState,
        use_item: TriState,
    },
}

/// What `BonemealEvent` settled for one use of bone meal.
pub(in crate::game) struct BonemealAsk {
    /// `Some(successful)` when a mod canceled the event: `growCrop`'s answer.
    pub crop: Option<bool>,
    /// `isValidBonemealTarget` as the event asked it, when a mod listens:
    /// `growCrop` takes it instead of asking again.
    pub check: Option<BonemealCheck>,
}

/// Where the game calls the hook points.
impl<__Side: crate::game::Side> Game<'_, __Side> {
    /// The chosen phase's broadcast event, before its scoped behaviour.
    pub(in crate::game) fn use_item_on_block(
        &mut self,
        hit: &super::Hit,
        hand: Hand,
        stack: Option<&ItemStack>,
        phase: UsePhase,
    ) -> Option<mod_api::InteractionResult> {
        if !mod_api::__private::listened(mod_api::HookId::UseItemOnBlock) {
            return None;
        }
        let snapshot = stack.try_into_api().ok()?;
        let click = self.block_click(hit, hand, snapshot.as_ref());
        let (mut result, mut canceled) = (mod_api::InteractionResult::TryWithEmptyHand, false);
        self.native_use_item_on_block(&click, phase, &mut result, &mut canceled);
        if !canceled {
            return None;
        }
        if phase == UsePhase::ItemAfterBlock
            && result.consumes_action()
            && let Some(stack) = stack
        {
            self.with_hand(hand, |game| {
                game.side_award_stat("used", &stack.item, 1);
                game.side_apply_after_use_components(stack);
            });
        }
        Some(result)
    }

    /// Tells the mods of the chunks that came into this level.
    pub(in crate::game) fn native_chunks_loaded(&mut self) {
        self.side_chunks_loaded();
    }
    /// Tells the mods of the chunks leaving this level, after their last save.
    pub(in crate::game) fn native_chunks_unloaded(
        &mut self,
        unloaded: &[std::sync::Arc<sunrise_level::chunk::Chunk>],
    ) {
        self.side_chunks_unloaded(unloaded);
    }
    /// `EntityBlock.newBlockEntity` of block `name`: Sunrise's own, else a
    /// native mod's, made as a write of the level creates it
    /// (`GameLevel::new_block_entity`). Only a declared mod type builds the
    /// level.
    #[cfg(test)]
    pub(in crate::game) fn new_block_entity(&mut self, name: &str) -> Option<BlockEntity> {
        if sunrise_game_data::block_entity_types::type_for(name).is_none() {
            return block_entity_for_block(
                self.pack.assets.registry,
                self.res.facts(),
                name,
                Compound::default(),
            );
        }
        self.with_level(|_, level, _| level.new_block_entity(name))
    }
    /// Compile static state-factory ownership once per level, including on clients.
    pub(in crate::game) fn ensure_stateful_entity_table(&mut self) {
        if self.world.block_entities.has_stateful_entity_table() {
            return;
        }
        let registry = self.pack.assets.registry;
        let mut blocks = FxHashMap::default();
        let states = (0..registry.len() as u32)
            .map(|id| {
                let Some(state) = registry.state(BlockStateId(id)) else {
                    return false;
                };
                if state.name.starts_with("minecraft:") {
                    return false;
                }
                *blocks.entry(state.name.as_str()).or_insert_with(|| {
                    let mut stateful = false;
                    self.native_stateful_block_entity(&state.name, &mut stateful);
                    stateful
                })
            })
            .collect();
        self.world.block_entities.set_stateful_entities(states);
    }
    /// Create a block entity from an actual state using the level's creation hook.
    pub(in crate::game) fn new_block_entity_for_state(
        &mut self,
        state: BlockStateId,
    ) -> Option<BlockEntity> {
        self.with_level(|_, level, _| level.new_block_entity_for_state(state))
    }
    /// A mod block's container at `key` opens its menu: its slots grown to
    /// the menu's. False for any other block.
    pub(in crate::game) fn open_mod_menu(&mut self, key: Pos) -> bool {
        let state = self.world.block(key.0, key.1, key.2).0;
        let Some(c) = self.world.block_entities.mod_container(state) else {
            return false;
        };
        let slots = c.slots;
        if c.menu.is_none() {
            return false;
        }
        let Some(BlockEntity::Mod(m)) = self.world.block_entities.get_mut(&key) else {
            return false;
        };
        if m.items.len() < slots {
            m.items.resize(slots, None);
        }
        true
    }
    /// The menu type of the mod container at `key`, if it has one.
    pub(in crate::game) fn mod_menu_at(&self, key: Pos) -> Option<&'static str> {
        let state = self.world.block(key.0, key.1, key.2).0;
        let c = self.world.block_entities.mod_container(state)?;
        mod_menu(c.menu.as_deref()?).map(|(menu, _)| menu)
    }
}

/// A supported type for a block's container menu. A transient container is never block-backed.
pub(in crate::game) fn mod_menu(name: &str) -> Option<(&'static str, usize)> {
    MOD_MENUS
        .into_iter()
        .find(|(menu, _)| *menu == name)
        .or_else(|| {
            sunrise_level::containers::mod_menu_type(name)
                .filter(|menu| menu.owned_slots.is_none())
                .map(|menu| (menu.id, menu.slots))
        })
}

/// `state` with its property `key` set to `value`; none when the block has
/// no such property or value.
pub(in crate::game) fn with_property(
    registry: &BlockRegistry,
    state: BlockStateId,
    key: &str,
    value: &str,
) -> Option<BlockStateId> {
    let mut s = registry.state(state)?.clone();
    match s.properties.get(key) {
        Some(v) if v == value => return Some(state),
        Some(_) => {}
        None => return None,
    }
    s.properties.insert(key.into(), value.into());
    registry.resolve(&s).ok()
}

/// The state a mod's property write changes `state` to; none when the block
/// has no such property or value, or already has it: `LevelChunk.setBlockState`
/// returns null for the same state, so `Level.setBlock` returns false.
pub(in crate::game) fn property_change(
    registry: &BlockRegistry,
    state: BlockStateId,
    key: &str,
    value: &str,
) -> Option<BlockStateId> {
    with_property(registry, state, key, value).filter(|&id| id != state)
}

/// `config/` in the game folder, beside `options.txt`, made when missing.
pub(in crate::game) fn config_dir(game_dir: &Path) -> std::io::Result<PathBuf> {
    let dir = game_dir.join("config");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub(in crate::game) fn client_packet_view(
    packet: &sunrise_protocol::packets::play::Clientbound,
) -> ClientPacket {
    use mod_api::Identifier;
    use std::sync::LazyLock;
    use sunrise_protocol::packets::play::Clientbound;

    static TYPES: LazyLock<Vec<Identifier>> =
        LazyLock::new(|| packet_types(sunrise_protocol::ids::play::clientbound::NAMES));
    let payload = match packet {
        Clientbound::CustomPayload(payload) => Some(payload),
        _ => None,
    };
    packet_view(&TYPES, packet.id(), payload)
}

/// `client_packet_send`'s view of a packet the game sends: as `client_packet_view`.
pub(in crate::game) fn serverbound_packet_view(
    packet: &sunrise_protocol::packets::play::Serverbound,
) -> ClientPacket {
    use mod_api::Identifier;
    use std::sync::LazyLock;
    use sunrise_protocol::packets::play::Serverbound;

    static TYPES: LazyLock<Vec<Identifier>> =
        LazyLock::new(|| packet_types(sunrise_protocol::ids::play::serverbound::NAMES));
    let payload = match packet {
        Serverbound::CustomPayload(payload) => Some(payload),
        _ => None,
    };
    packet_view(&TYPES, packet.id(), payload)
}

/// One direction's packet type identifiers, by protocol id.
pub(in crate::game) fn packet_types(names: &[&str]) -> Vec<mod_api::Identifier> {
    names
        .iter()
        .map(|name| mod_api::Identifier::parse(name).expect("protocol packet type identifier"))
        .collect()
}

/// A packet's type, `sunrise:unknown` outside the protocol's table, and its custom payload.
pub(in crate::game) fn packet_view(
    types: &[mod_api::Identifier],
    id: i32,
    payload: Option<&sunrise_protocol::packets::configuration::CustomPayload>,
) -> ClientPacket {
    use mod_api::Identifier;
    use std::sync::LazyLock;

    static UNKNOWN: LazyLock<Identifier> =
        LazyLock::new(|| Identifier::parse("sunrise:unknown").unwrap());
    let id = usize::try_from(id)
        .ok()
        .and_then(|id| types.get(id))
        .unwrap_or(&UNKNOWN)
        .clone();
    let payload = payload.map(|p| ClientPayload::new(p.channel.clone(), p.data.clone()));
    ClientPacket::new(id, payload)
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[test]
    #[ignore = "daily"]
    fn declared_types_create_tick_and_reload_through_game() {
        use super::super::native_entity_refs::tests::{record_key, run_fresh_cases, with_game_at};
        use mod_api::{self, *};
        use std::sync::Arc;
        use sunrise_level::{chunk::Chunk, containers::BlockEntity};
        use sunrise_sim::save;

        const CHILD: &str = "SUNRISE_DECLARED_BE_GAME_CHILD";
        if run_fresh_cases(
            "game::native_hooks::tests::declared_types_create_tick_and_reload_through_game",
            CHILD,
            &["valid", "unknown_block", "unknown_type", "block_not_type"],
        ) {
            return;
        }

        const FIRST: &str = "be_test:press";
        const SECOND: &str = "be_test:ornate_banner";
        const EMPTY: &str = "be_test:blank";
        const PLAIN: &str = "be_test:plain_banner";
        #[derive(Default)]
        struct Keys {
            own: std::sync::OnceLock<BlockEntityTypeKey>,
            shared: std::sync::OnceLock<BlockEntityTypeKey>,
            vanilla: std::sync::OnceLock<BlockEntityTypeKey>,
        }
        trait Probe {
            fn value(&self) -> u32;
        }
        struct Marker(u32);
        impl Probe for Marker {
            fn value(&self) -> u32 {
                self.0
            }
        }
        const PROBE: CapabilityKey<dyn Probe> = CapabilityKey::new("be_test:probe");
        struct Press;
        #[mod_api::sunrise_part]
        impl BlockPart for Press {}
        #[mod_api::sunrise_part]
        impl BlockEntityPart for Press {
            fn block_entity(
                &self,
                _: LevelSide<'_>,
                _: &mut BlockEntityEvent<'_>,
            ) -> Option<Compound> {
                Some(Compound::from([("energy".into(), Tag::Long(6300))]))
            }
            fn block_entity_tick(&self, event: &mut BlockEntityTickEvent<'_>) {
                event.subject.with_view(|mut entity| {
                    let key = "energy".into();
                    let Some(Tag::Long(energy)) = entity.nbt().get(&key) else {
                        panic!("factory payload missing");
                    };
                    assert!(entity.set_nbt(key, Some(Tag::Long(energy + 1))));
                });
            }
        }
        struct User {
            keys: Arc<Keys>,
            unknown: Option<&'static str>,
        }
        #[mod_api::__subscriptions]
        impl Hooks for User {
            fn register(&self, registrar: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
                let kind = registrar.block_entity_type_key("be_test:shared");
                registrar
                    .block_entity_part(&kind, Press)
                    .expect("native type");
                registrar.block_entity_capability(&PROBE, &kind, |_, _, _, _, offer| {
                    offer.offer(|| Box::new(Marker(6300)))
                });
                record_key(&self.keys.shared, kind);
                let vanilla = registrar.block_entity_type_key("minecraft:chest");
                registrar.block_entity_capability(&PROBE, &vanilla, |_, _, _, _, offer| {
                    offer.offer(|| Box::new(Marker(26)))
                });
                record_key(&self.keys.vanilla, vanilla);
                if let Some(id) = self.unknown {
                    registrar.block_entity_type_key(id);
                }
            }
        }
        struct TestMod {
            invalid: bool,
            keys: Arc<Keys>,
        }
        #[mod_api::__subscriptions]
        impl Hooks for TestMod {
            fn register(&self, registrar: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
                let kind = registrar
                    .block_entity_type("shared")
                    .blocks(if self.invalid {
                        &["be_test:missing"]
                    } else {
                        &[FIRST, SECOND]
                    })
                    .build();
                record_key(&self.keys.own, kind);
                registrar
                    .block_entity_type("empty")
                    .blocks(&[EMPTY])
                    .build();
            }
            fn blocks(&self, _context: &mod_api::RegistrationContext, event: &mut BlocksEvent<'_>) {
                for block in [FIRST, SECOND, EMPTY, PLAIN]
                    .map(|id| mod_api::Block::new(id, "minecraft:white_banner"))
                {
                    event.register(block).unwrap();
                }
            }
        }
        let case = std::env::var(CHILD).unwrap();
        let invalid = case == "unknown_block";
        let unknown = match case.as_str() {
            "unknown_type" => Some("other:missing"),
            "block_not_type" => Some("minecraft:red_shulker_box"),
            _ => None,
        };
        let keys = Arc::new(Keys::default());
        let mods: Mods = Arc::from([
            ModEntry::new(
                "be_user",
                Box::new(User {
                    keys: keys.clone(),
                    unknown,
                }),
            ),
            ModEntry::new(
                "be_test",
                Box::new(TestMod {
                    invalid,
                    keys: keys.clone(),
                }),
            ),
        ]);
        mod_platform::__private::install(mods.clone());
        // `be_user` attaches a Part to the type `be_test` declares, which names `be_test`
        // among its dependencies.
        let package = |id: &str, depends_on: &[&str]| {
            PackageInfo::new(
                id.into(),
                "1".into(),
                PackageKind::Code,
                Side::Both,
                vec![id.into()],
                depends_on
                    .iter()
                    .map(|owner| {
                        PackageDependency::new(
                            (*owner).into(),
                            "*".into(),
                            DependencyKind::Required,
                            DependencyOrder::After,
                            Side::Both,
                            None,
                            None,
                        )
                    })
                    .collect(),
            )
        };
        let base = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        mod_platform::__private::with_test_package_inventory(
            vec![package("be_test", &[]), package("be_user", &["be_test"])],
            || {
                crate::game::api_values::install_parts(&mods).unwrap();
                let root = super::super::mods::content_root(&base, &Default::default());
                if invalid {
                    let error = root.unwrap_err().to_string();
                    assert!(error.contains("be_test:shared"), "{error}");
                    assert!(error.contains("be_test:missing"), "{error}");
                    // A refused declaration builds no content root. The roots folder's own `.lock`
                    // is made by every start and never deleted, so it is not a root.
                    let built: Vec<_> = std::fs::read_dir(".sunrise-cache/mod-content")
                        .into_iter()
                        .flatten()
                        .map(|entry| entry.unwrap().file_name())
                        .filter(|name| name != ".lock")
                        .collect();
                    assert!(built.is_empty(), "{built:?}");
                    return;
                }
                if let Some(id) = unknown {
                    let error = root.unwrap_err().to_string();
                    assert!(error.contains("be_user"), "{error}");
                    assert!(error.contains("unknown block entity type"), "{error}");
                    assert!(error.contains(id), "{error}");
                    assert!(sunrise_game_data::block_entity_types::current().is_none());
                    return;
                }
                assert_eq!(keys.shared.get(), keys.own.get());
                with_game_at(root.unwrap().prepared().to_path_buf(), |game| {
                    game.options.dedicated = Some(Vec::new());
                    game.world
                        .chunks
                        .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
                    let level = game.level(game.dim.id()).unwrap();
                    let stone = game
                        .pack
                        .assets
                        .registry
                        .default_state("minecraft:stone")
                        .unwrap();
                    for (x, block) in [
                        (1, FIRST),
                        (2, SECOND),
                        (3, EMPTY),
                        (4, PLAIN),
                        (5, "minecraft:chest"),
                    ] {
                        let state = game.pack.assets.registry.default_state(block).unwrap();
                        assert_eq!(
                            game.level_set_block(level, [x, 63, 0], stone, BlockUpdateFlags::NONE),
                            Ok(true)
                        );
                        assert_eq!(
                            game.level_set_block(level, [x, 64, 0], state, BlockUpdateFlags::NONE),
                            Ok(true)
                        );
                        let pos = (x, 64, 0);
                        if block == PLAIN {
                            // No declared type claims it, and no mod adds it to `minecraft:banner`'s
                            // valid blocks, so its template creates no entity (`validateBlockState`).
                            assert!(game.world.block_entities.get(&pos).is_none());
                            assert!(game.block_entity(level, [x, 64, 0]).is_none());
                            continue;
                        }
                        let handle = game.block_entity(level, [x, 64, 0]).unwrap();
                        if block == "minecraft:chest" {
                            assert_eq!(handle.kind(game).as_ref(), keys.vanilla.get());
                            let probe = level.capability(game, &PROBE, [x, 64, 0], None).unwrap();
                            assert_eq!((probe.value(), probe.owner()), (26, "be_user".into()));
                            continue;
                        }
                        let Some(BlockEntity::Mod(entity)) = game.world.block_entities.get(&pos)
                        else {
                            panic!("declared type did not override its banner template");
                        };
                        if block == EMPTY {
                            assert_eq!(entity.id(), "be_test:empty");
                            assert!(entity.nbt.is_empty());
                            continue;
                        }
                        assert_eq!(entity.id(), "be_test:shared");
                        assert_eq!(entity.nbt.get(&"energy".into()), Some(&Tag::Long(6300)));
                        assert_eq!(handle.kind(game).as_ref(), keys.shared.get());
                        let probe = level.capability(game, &PROBE, [x, 64, 0], None).unwrap();
                        assert_eq!((probe.value(), probe.owner()), (6300, "be_user".into()));
                        game.tick_mod_block_entity(pos);
                        let saved = save::block_entity_encode::encode(
                            pos,
                            game.world.block_entities.get(&pos).unwrap(),
                            block,
                            &Default::default(),
                        );
                        assert_eq!(saved.get("energy"), Some(&Tag::Long(6301)));
                        let (saved_pos, restored) =
                            save::decode_block_entity_for_block(&saved, block).unwrap();
                        assert_eq!(saved_pos, pos);
                        assert_eq!(restored.id(), "be_test:shared");
                        assert!(save::decode_block_entity_for_block(&saved, EMPTY).is_none());
                        game.world.block_entities.insert(pos, restored);
                        game.tick_mod_block_entity(pos);
                        let saved = save::block_entity_encode::encode(
                            pos,
                            game.world.block_entities.get(&pos).unwrap(),
                            block,
                            &Default::default(),
                        );
                        assert_eq!(saved.get("energy"), Some(&Tag::Long(6302)));
                    }

                    // A level hook's `LevelWriteHost::set_block` fires the same hook in its write.
                    let first = game.pack.assets.registry.default_state(FIRST).unwrap();
                    use crate::game::api_values::TryIntoApi;
                    let written = game.with_level(|_, level, ticks| {
                        LevelWriteHost::set_block(
                            &mut super::super::mod_api_host::LevelTicks::new(level, ticks),
                            [5, 64, 0],
                            first.try_into_api().unwrap(),
                            BlockUpdateFlags::NONE,
                        )
                    });
                    assert!(written);
                    let Some(BlockEntity::Mod(entity)) = game.world.block_entities.get(&(5, 64, 0))
                    else {
                        panic!("a level write did not create the declared block entity");
                    };
                    assert_eq!(entity.id(), "be_test:shared");
                    assert_eq!(entity.nbt.get(&"energy".into()), Some(&Tag::Long(6300)));
                });
            },
        );
    }

    #[test]
    fn observed_overlay_prefers_first_open_screen() {
        assert_eq!(
            crate::game::client_side::native_hooks_client::observed_overlay_id(&[
                (false, "sign"),
                (false, "book")
            ]),
            None
        );
        assert_eq!(
            crate::game::client_side::native_hooks_client::observed_overlay_id(&[
                (false, "sign"),
                (true, "book")
            ]),
            Some("book")
        );
        assert_eq!(
            crate::game::client_side::native_hooks_client::observed_overlay_id(&[
                (true, "sign"),
                (true, "book")
            ]),
            Some("sign")
        );
    }

    /// Exercise the gameplay constructor, where name-based entities compete
    /// with a mod block's sign template.
    #[test]
    #[ignore = "daily"]
    fn mod_sign_constructor_prioritizes_templates_over_suffixes() {
        // This fixture adds blocks that are absent from the shared vanilla
        // game's frozen process-wide block definitions.
        if super::super::native_entity_refs::tests::run_fresh_cases(
            "game::native_hooks::tests::mod_sign_constructor_prioritizes_templates_over_suffixes",
            "SUNRISE_SIGN_CONSTRUCTOR_CHILD",
            &["signs"],
        ) {
            return;
        }
        use super::super::{Options, Progress, Settings, load_resources};
        use sunrise_mod_package::content::{Block, Content};
        use sunrise_sim::mod_content::{ContentMod, build};

        let base = std::path::PathBuf::from(
            std::env::var_os("SUNRISE_TEST_RESOURCES")
                .expect("set SUNRISE_TEST_RESOURCES to prepared resources"),
        );
        let root = std::env::temp_dir().join(format!(
            "sunrise-sign-constructor-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cases = [
            ("ruby:notice_banner", "minecraft:oak_sign", false),
            ("ruby:board_shelf", "minecraft:oak_wall_sign", false),
            ("ruby:lantern_head", "minecraft:oak_hanging_sign", true),
            (
                "ruby:bracket_skull",
                "minecraft:oak_wall_hanging_sign",
                true,
            ),
        ];
        let mut content = Content::declared("ruby");
        let blocks: Vec<_> = cases
            .iter()
            .map(|&(id, template, _)| (id, template))
            .chain([("ruby:road_sign", "minecraft:stone")])
            .map(|(id, template)| Block {
                id: id.into(),
                template: template.into(),
                item: false,
                sign_text_color: (id == "ruby:notice_banner").then(|| "light_blue".into()),
                sign_text_glow: (id == "ruby:notice_banner").then_some(true),
                ..Default::default()
            })
            .collect();
        content.declare(&blocks, &[]).unwrap();
        build(
            &base,
            std::path::Path::new(""),
            &[ContentMod {
                id: "ruby".into(),
                content,
                jars: Vec::new(),
            }],
            &root,
            &sunrise_mod_package::enabled::EnabledMods::new([("ruby".into(), mod_api::Side::Both)]),
        )
        .unwrap();
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            super::ExecutionRole::IntegratedServer,
            &Progress::new(),
            &mut |_, _| unreachable!("no menu handoff"),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.execution_role = super::ExecutionRole::IntegratedServer;
        let mut game = crate::game::ServerGame::new(
            &loaded.resources,
            loaded.pack,
            options,
            settings.sim_settings(),
            None,
            crate::game::profile_resolver,
        )
        .unwrap();

        for (name, hanging) in cases
            .map(|(name, _, hanging)| (name, hanging))
            .into_iter()
            .chain([
                ("minecraft:oak_sign", false),
                ("minecraft:oak_hanging_sign", true),
            ])
        {
            match game.new_block_entity(name) {
                Some(super::BlockEntity::Sign(sign)) => {
                    assert_eq!(sign.hanging, hanging, "{name}")
                }
                entity => panic!("{name}: expected a sign, got {entity:?}"),
            }
        }
        // `createDefaultSignText`: a declared colour and glow on both sides of a new sign, black
        // and unlit on one that declares none.
        let mut text = |name| match game.new_block_entity(name) {
            Some(super::BlockEntity::Sign(sign)) => {
                assert_eq!(sign.front.color, sign.back.color);
                assert_eq!(sign.front.glowing, sign.back.glowing);
                (sign.front.color, sign.front.glowing)
            }
            entity => panic!("{name}: expected a sign, got {entity:?}"),
        };
        assert_eq!(text("ruby:notice_banner"), (3, true));
        assert_eq!(text("ruby:board_shelf"), (15, false));
        assert_eq!(text("minecraft:oak_sign"), (15, false));
        assert!(game.new_block_entity("ruby:road_sign").is_none());
        assert!(matches!(
            game.new_block_entity("minecraft:white_banner"),
            Some(super::BlockEntity::Banner(_))
        ));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(feature = "client")]
    #[test]
    #[ignore = "daily"]
    fn generated_fire_matches_dispatch() {
        use super::super::{Audio, Options, Progress, Settings, load_resources};
        use mod_api::{ClientHost, Flow, Hooks, ModEntry};
        use std::sync::{Arc, Mutex};

        #[derive(Default)]
        struct Listener(usize, Arc<Mutex<Vec<usize>>>);
        #[mod_api::sunrise_mod]
        impl Hooks for Listener {
            fn block_highlight(
                &self,
                _: &dyn ClientHost,
                _: &mut mod_api::BlockHighlightEvent,
            ) -> Flow {
                self.1.lock().unwrap().push(self.0);
                if self.0 == 1 {
                    Flow::Cancel
                } else {
                    Flow::Continue
                }
            }
            fn screen_exclusion_areas(
                &self,
                _: &dyn ClientHost,
                event: &mut mod_api::ScreenExclusionAreasEvent<'_>,
            ) {
                self.1.lock().unwrap().push(self.0);
                event.areas.push([self.0 as f32, 0.0, 0.0, 0.0]);
            }
            fn movement_input_update(
                &self,
                _: &dyn ClientHost,
                event: &mut mod_api::MovementInputUpdateEvent<'_>,
            ) {
                self.1.lock().unwrap().push(self.0);
                event.input.move_vector[1] += self.0 as f32;
            }
        }
        let root = std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
        let settings = Settings::default();
        let loaded = load_resources(
            sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            std::time::Instant::now(),
            false,
            crate::game::execution_role::ExecutionRole::Client,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.execution_role = crate::game::execution_role::ExecutionRole::Client;
        let game = crate::game::ClientGame::new(
            &loaded.resources,
            loaded.client_res.unwrap(),
            loaded.pack,
            loaded.client_pack.unwrap(),
            loaded.gui.unwrap(),
            options,
            settings,
            Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
                root.to_path_buf(),
            )),
            None,
            crate::game::server_side::integrated::serve,
            crate::game::server_side::integrated::run_component_harness_on_server,
            crate::game::session_verifier(),
            crate::game::profile_resolver,
        )
        .unwrap();
        let trace = Arc::new(Mutex::new(Vec::new()));
        let mods = (0..3)
            .map(|i| ModEntry::new("listener", Box::new(Listener(i, trace.clone()))))
            .collect::<Vec<_>>();
        mod_platform::testing::with_mods(mods.into(), || {
            let run = |fire: bool| {
                trace.lock().unwrap().clear();
                let mut canceled = false;
                if fire {
                    game.fire_block_highlight(
                        [1, 2, 3],
                        super::BlockStateId(7),
                        0.5,
                        &mut canceled,
                    );
                } else {
                    mod_api::dispatch_block_highlight(
                        &game,
                        [1, 2, 3],
                        crate::game::api_values::TryIntoApi::try_into_api(&super::BlockStateId(7))
                            .unwrap(),
                        0.5,
                        &mut canceled,
                    );
                }
                let cancel_trace = trace.lock().unwrap().clone();
                trace.lock().unwrap().clear();
                let view = crate::game::client_side::screen_build::ObservedScreen {
                    id: "inventory",
                    overlay: false,
                    mod_screen: None,
                    menu: None,
                    menu_generation: None,
                    dialog_key: None,
                    title: None,
                    rect: [0.0; 4],
                    screen_size: [0.0; 2],
                    slots: 0,
                    mouse: [0.0; 2],
                    partial_tick: 0.0,
                    regions: Vec::new(),
                };
                let mut areas = Vec::new();
                if fire {
                    game.fire_screen_exclusion_areas(&view.view(), &mut areas);
                } else {
                    mod_api::dispatch_screen_exclusion_areas(&game, &view.view(), &mut areas);
                }
                let collect_trace = trace.lock().unwrap().clone();
                trace.lock().unwrap().clear();
                let forward = game.movement_dispatch_fixture(fire);
                (
                    canceled,
                    cancel_trace,
                    areas,
                    collect_trace,
                    forward,
                    trace.lock().unwrap().clone(),
                )
            };
            let old = run(false);
            assert!(old.0);
            assert_eq!(old.1, [0, 1]);
            assert_eq!(old.3, [0, 1, 2]);
            assert_eq!(old.4, 4.0);
            assert_eq!(old, run(true));
        });
    }

    /// With no window to lay a screen out in there is no slot under the pointer.
    #[cfg(feature = "client")]
    #[test]
    #[ignore = "daily"]
    fn no_stack_is_hovered_without_a_window() {
        crate::game::in_own_process(
            "game::native_hooks::tests::no_stack_is_hovered_without_a_window",
            || {
                use super::super::{Audio, Options, Progress, Settings, load_resources};

                let root =
                    std::path::PathBuf::from(std::env::var_os("SUNRISE_TEST_RESOURCES").unwrap());
                let settings = Settings::default();
                let loaded = load_resources(
                    sunrise_assets::resource_source::ResourceRoot::directory(root.clone()),
                    Vec::new(),
                    None,
                    &root,
                    &settings,
                    std::time::Instant::now(),
                    false,
                    crate::game::execution_role::ExecutionRole::Client,
                    &Progress::new(),
                    &mut |_, _| unreachable!("no menu handoff"),
                )
                .unwrap();
                let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
                options.resources = root.clone();
                let game = crate::game::ClientGame::new(
                    &loaded.resources,
                    loaded.client_res.unwrap(),
                    loaded.pack,
                    loaded.client_pack.unwrap(),
                    loaded.gui.unwrap(),
                    options,
                    settings,
                    Audio::silent(&sunrise_assets::resource_source::ResourceSource::directory(
                        root.to_path_buf(),
                    )),
                    None,
                    crate::game::server_side::integrated::serve,
                    crate::game::server_side::integrated::run_component_harness_on_server,
                    crate::game::session_verifier(),
                    crate::game::profile_resolver,
                )
                .unwrap();
                assert_eq!(game.screen_hovered_stack(), None);
            },
        );
    }
}
