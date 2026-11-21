//! Call-scoped read access to the parameters of a loot evaluation.

use crate::{ItemKey, ItemStackView, Tag, Text, TextFormError};
use serde_json::Value;

// hidden: the engine lends its resident parameter rows; mods read LootParamsView.
#[doc(hidden)]
#[derive(Default)]
pub struct LootParamsParts<'a> {
    pub queried_table_id: Option<&'a str>,
    pub queried_stack: Option<QueriedStack<'a>>,
    pub entity: Option<&'a dyn LootEntityAccess>,
    pub last_damage_player: Option<&'a dyn LootEntityAccess>,
    pub damage: Option<&'a dyn LootDamageAccess>,
    pub tool: Option<&'a dyn LootToolAccess>,
    pub block: Option<&'a str>,
    pub biome: Option<&'a str>,
    pub origin: Option<[f64; 3]>,
    pub block_entity: Option<&'a Tag>,
    pub level: Option<&'a dyn LootLevelAccess>,
    pub luck: f32,
}

type NameProjection = fn(&Value) -> Result<Text, TextFormError>;
type QueriedStack<'a> = (
    &'a dyn LootStackAccess,
    fn(&dyn LootStackAccess) -> Option<ItemStackView<'_>>,
);

/// The parameters supplied to a single loot evaluation. Missing parameters are
/// absent rather than synthesized. Reading facts does not advance the roll's random source.
// no wire: lends the evaluator's call-scoped rows.
// exhaustive: constructed only at the engine integration boundary.
pub struct LootParamsView<'a>(LootParamsParts<'a>, NameProjection);

impl<'a> LootParamsView<'a> {
    // hidden: the evaluator constructs this loan from its existing parameter rows.
    #[doc(hidden)]
    pub fn new(parts: LootParamsParts<'a>, project_name: NameProjection) -> Self {
        Self(parts, project_name)
    }

    /// The original queried table id. The first id wins, nested rolls never set it,
    /// and an unset id reads as `UNKNOWN_LOOT_TABLE` until the root completes.
    pub fn queried_table_id(&self) -> &'a str {
        self.0
            .queried_table_id
            .unwrap_or(sunrise_core::UNKNOWN_LOOT_TABLE)
    }

    /// The nonempty inventory stack being queried, if supplied.
    pub fn queried_stack(&self) -> Option<ItemStackView<'a>> {
        let (stack, view) = self.0.queried_stack?;
        view(stack)
    }

    /// The entity named by `THIS_ENTITY`.
    pub fn entity(&self) -> Option<LootEntityView<'a>> {
        Some(LootEntityView(self.0.entity?, self.1))
    }

    /// The player named by `LAST_DAMAGE_PLAYER`.
    pub fn last_damage_player(&self) -> Option<LootEntityView<'a>> {
        Some(LootEntityView(self.0.last_damage_player?, self.1))
    }

    /// The damage source supplied to this roll.
    pub fn damage(&self) -> Option<LootDamageView<'a>> {
        Some(LootDamageView(self.0.damage?, self.1))
    }

    /// The tool supplied to this roll.
    pub fn tool(&self) -> Option<LootToolView<'a>> {
        self.0.tool.map(LootToolView)
    }

    /// Block identifier in a block or block-interaction roll.
    pub fn block(&self) -> Option<&'a str> {
        self.0.block
    }

    /// Biome identifier, when supplied.
    pub fn biome(&self) -> Option<&'a str> {
        self.0.biome
    }

    /// The world-space origin supplied to the roll.
    pub fn origin(&self) -> Option<[f64; 3]> {
        self.0.origin
    }

    /// The block entity's saved tag, with metadata, when supplied.
    pub fn block_entity(&self) -> Option<&'a Tag> {
        self.0.block_entity
    }

    /// The level in which the roll takes place, when supplied.
    pub fn level(&self) -> Option<LootLevelView<'a>> {
        self.0.level.map(LootLevelView)
    }

    /// Luck applied to this roll.
    pub fn luck(&self) -> f32 {
        self.0.luck
    }
}

// hidden: an evaluator reads its existing TOOL parameter through this adapter.
#[doc(hidden)]
pub trait LootToolAccess {
    fn item_id(&self) -> &str;
    fn enchantment_at(&self, index: usize) -> Option<(&str, i32)>;
}

/// A borrowed tool parameter. Unlike an output candidate, this view exposes only
/// what both tool representations supply: item identity and enchantments.
// no wire: lends the active roll's tool.
// exhaustive: constructed only by LootParamsView.
pub struct LootToolView<'a>(&'a dyn LootToolAccess);

impl<'a> LootToolView<'a> {
    /// The tool's item identifier.
    pub fn item_id(&self) -> &'a str {
        self.0.item_id()
    }

    /// The tool's item key, if its identifier is registered.
    pub fn item(&self) -> Option<ItemKey> {
        crate::__private::item_key(self.item_id())
    }

    /// Tool enchantment identifiers and levels in stored order.
    pub fn enchantments(&self) -> impl Iterator<Item = (&'a str, i32)> + use<'a> {
        let tool = self.0;
        (0..).map_while(move |index| tool.enchantment_at(index))
    }
}

// hidden: lends entity rows; no entity handle or engine type crosses the mod boundary.
#[doc(hidden)]
pub trait LootEntityAccess {
    fn kind(&self) -> &str;
    fn living(&self) -> bool;
    fn on_fire(&self) -> bool;
    fn baby(&self) -> bool;
    fn vehicle(&self) -> Option<&dyn LootEntityAccess>;
    fn cube_size(&self) -> Option<i32>;
    fn sheep(&self) -> Option<(&str, bool)>;
    fn captain(&self) -> Option<bool>;
    fn frog_variant(&self) -> Option<&str>;
    fn chicken_variant(&self) -> Option<&str>;
    fn powered(&self) -> bool;
    fn equipment(&self, slot: &str) -> Option<&dyn LootStackAccess>;
    fn equipment_at(&self, index: usize) -> Option<(&str, &dyn LootStackAccess)>;
}

/// Entity predicate facts supplied to a loot roll, not a live entity handle.
// no wire: lends the evaluator's entity row and its relationships.
// exhaustive: constructed only from active loot parameters.
pub struct LootEntityView<'a>(&'a dyn LootEntityAccess, NameProjection);

impl<'a> LootEntityView<'a> {
    /// Entity type identifier.
    pub fn kind(&self) -> &'a str {
        self.0.kind()
    }

    /// Whether this entity is living.
    pub fn living(&self) -> bool {
        self.0.living()
    }

    /// Whether this entity is on fire.
    pub fn on_fire(&self) -> bool {
        self.0.on_fire()
    }

    /// Whether this entity is a baby.
    pub fn baby(&self) -> bool {
        self.0.baby()
    }

    /// The entity that this entity is riding, if any.
    pub fn vehicle(&self) -> Option<Self> {
        Some(Self(self.0.vehicle()?, self.1))
    }

    /// A cube mob's size, when applicable.
    pub fn cube_size(&self) -> Option<i32> {
        self.0.cube_size()
    }

    /// A sheep's colour and whether it is sheared, when applicable.
    pub fn sheep(&self) -> Option<(&'a str, bool)> {
        self.0.sheep()
    }

    /// Whether this raider is a captain, or `None` if not a raider.
    pub fn captain(&self) -> Option<bool> {
        self.0.captain()
    }

    /// Frog variant identifier, when applicable.
    pub fn frog_variant(&self) -> Option<&'a str> {
        self.0.frog_variant()
    }

    /// Chicken variant identifier, when applicable.
    pub fn chicken_variant(&self) -> Option<&'a str> {
        self.0.chicken_variant()
    }

    /// Whether this entity is a charged creeper.
    pub fn powered(&self) -> bool {
        self.0.powered()
    }

    /// The raw candidate in the named slot (for example, `mainhand`), if supplied.
    pub fn equipment(&self, slot: &str) -> Option<LootStackView<'a>> {
        Some(LootStackView(self.0.equipment(slot)?, self.1))
    }

    /// Equipment slot names and raw candidates in the source row's order.
    pub fn equipment_iter(&self) -> impl Iterator<Item = (&'a str, LootStackView<'a>)> + use<'a> {
        let Self(entity, project_name) = *self;
        (0..).map_while(move |index| {
            let (slot, stack) = entity.equipment_at(index)?;
            Some((slot, LootStackView(stack, project_name)))
        })
    }
}

// hidden: lends damage source and direct entity without projecting either row.
#[doc(hidden)]
pub trait LootDamageAccess {
    fn kind(&self) -> &str;
    fn source(&self) -> Option<&dyn LootEntityAccess>;
    fn direct(&self) -> Option<&dyn LootEntityAccess>;
}

/// Borrowed damage-source facts.
// no wire: lends the evaluator's damage row and its entity parameters.
// exhaustive: constructed only from active loot parameters.
pub struct LootDamageView<'a>(&'a dyn LootDamageAccess, NameProjection);

impl<'a> LootDamageView<'a> {
    /// Damage type identifier.
    pub fn kind(&self) -> &'a str {
        self.0.kind()
    }

    /// The source of the damage, such as the shooter of a projectile.
    pub fn source(&self) -> Option<LootEntityView<'a>> {
        Some(LootEntityView(self.0.source()?, self.1))
    }

    /// The entity that inflicted the damage directly, such as the projectile.
    pub fn direct(&self) -> Option<LootEntityView<'a>> {
        Some(LootEntityView(self.0.direct()?, self.1))
    }
}

// hidden: lends the evaluator's level row.
#[doc(hidden)]
pub trait LootLevelAccess {
    fn dimension(&self) -> &str;
    fn game_time(&self) -> i64;
    fn day_time(&self) -> i64;
    fn raining(&self) -> bool;
    fn thundering(&self) -> bool;
}

/// Borrowed dimension, time and weather facts for a loot roll.
// no wire: lends the evaluator's level row.
// exhaustive: constructed only from active loot parameters.
pub struct LootLevelView<'a>(&'a dyn LootLevelAccess);

impl<'a> LootLevelView<'a> {
    /// The dimension identifier.
    pub fn dimension(&self) -> &'a str {
        self.0.dimension()
    }

    /// Elapsed game ticks.
    pub fn game_time(&self) -> i64 {
        self.0.game_time()
    }

    /// The level's day time, not reduced to the current day.
    pub fn day_time(&self) -> i64 {
        self.0.day_time()
    }

    /// Whether the level is raining.
    pub fn raining(&self) -> bool {
        self.0.raining()
    }

    /// Whether it is thundering.
    pub fn thundering(&self) -> bool {
        self.0.thundering()
    }
}

// hidden: lends an existing signed loot candidate, not a normalized inventory stack.
#[doc(hidden)]
pub trait LootStackAccess: LootToolAccess {
    fn count(&self) -> i32;
    fn damage(&self) -> i32;
    fn map(&self) -> bool;
    fn components(&self) -> &[(String, Option<Tag>)];
    fn enchantments(&self) -> &[(String, i32)];
    fn potion(&self) -> Option<&str>;
    fn stew_effects(&self) -> &[(String, i32)];
    fn ominous_amplifier(&self) -> Option<i32>;
    fn exploration(&self) -> Option<LootExplorationView<'_>>;
    fn names(&self) -> &[(String, Value)];
}

/// A borrowed, signed loot candidate, not an occupied inventory slot.
// no wire: lends the existing candidate and its component patches.
// exhaustive: constructed only from active loot storage.
pub struct LootStackView<'a>(&'a dyn LootStackAccess, NameProjection);

impl<'a> LootStackView<'a> {
    /// Candidate item identifier.
    pub fn item_id(&self) -> &'a str {
        self.0.item_id()
    }

    /// Candidate item key, if registered.
    pub fn item(&self) -> Option<ItemKey> {
        crate::__private::item_key(self.item_id())
    }

    /// Signed intermediate count. Non-positive counts need not become inventory stacks.
    pub fn count(&self) -> i32 {
        self.0.count()
    }

    /// Signed intermediate damage value.
    pub fn damage(&self) -> i32 {
        self.0.damage()
    }

    /// Whether to create a map id after the roll.
    pub fn map(&self) -> bool {
        self.0.map()
    }

    /// Component edits in stored order, including repeated keys. `None` removes the component.
    pub fn components(
        &self,
    ) -> impl ExactSizeIterator<Item = (&'a str, Option<&'a Tag>)> + use<'a> {
        self.0
            .components()
            .iter()
            .map(|(id, tag)| (id.as_str(), tag.as_ref()))
    }

    /// Enchantment identifiers and levels in stored order.
    pub fn enchantments(&self) -> impl ExactSizeIterator<Item = (&'a str, i32)> + use<'a> {
        self.0
            .enchantments()
            .iter()
            .map(|(id, level)| (id.as_str(), *level))
    }

    /// Potion identifier, if supplied.
    pub fn potion(&self) -> Option<&'a str> {
        self.0.potion()
    }

    /// Suspicious stew effect identifiers and durations in ticks, in stored order.
    pub fn stew_effects(&self) -> impl ExactSizeIterator<Item = (&'a str, i32)> + use<'a> {
        self.0
            .stew_effects()
            .iter()
            .map(|(id, ticks)| (id.as_str(), *ticks))
    }

    /// Ominous bottle amplifier, if supplied.
    pub fn ominous_amplifier(&self) -> Option<i32> {
        self.0.ominous_amplifier()
    }

    /// Deferred exploration-map request, if supplied.
    pub fn exploration(&self) -> Option<LootExplorationView<'a>> {
        self.0.exploration()
    }

    /// Number of custom or item name entries stored by loot functions.
    pub fn name_count(&self) -> usize {
        self.0.names().len()
    }

    /// Read a name at its exact index. Projects only that name's text tree;
    /// a projection error is distinct from an absent entry.
    pub fn name(&self, index: usize) -> Option<Result<(&'a str, Text), TextFormError>> {
        let (id, raw) = self.0.names().get(index)?;
        Some((self.1)(raw).map(|text| (id.as_str(), text)))
    }
}

/// Borrowed settings for a deferred exploration-map search.
// no wire: lends the search's identifiers from a loot candidate.
// exhaustive: constructed only from active loot storage.
pub struct LootExplorationView<'a> {
    destination: &'a str,
    decoration: &'a str,
    zoom: u8,
    search_radius: i32,
    skip_existing_chunks: bool,
}

impl<'a> LootExplorationView<'a> {
    // hidden: borrow the evaluator's already decoded map settings.
    #[doc(hidden)]
    pub fn new(
        destination: &'a str,
        decoration: &'a str,
        zoom: u8,
        search_radius: i32,
        skip_existing_chunks: bool,
    ) -> Self {
        Self {
            destination,
            decoration,
            zoom,
            search_radius,
            skip_existing_chunks,
        }
    }

    /// Destination structure tag identifier, without `#`.
    pub fn destination(&self) -> &'a str {
        self.destination
    }

    /// Map decoration identifier.
    pub fn decoration(&self) -> &'a str {
        self.decoration
    }

    /// Map zoom.
    pub fn zoom(&self) -> u8 {
        self.zoom
    }

    /// Search radius in chunks.
    pub fn search_radius(&self) -> i32 {
        self.search_radius
    }

    /// Whether to skip previously discovered structures.
    pub fn skip_existing_chunks(&self) -> bool {
        self.skip_existing_chunks
    }
}
