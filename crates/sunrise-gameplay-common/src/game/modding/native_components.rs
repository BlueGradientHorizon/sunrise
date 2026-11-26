//! Native item data component helpers.
#[allow(unused_imports)]
use crate::game::CommonGame as Game;
use crate::game::native_hooks::creator_mod_id::GameHooks as _;
#[allow(unused_imports)]
use sunrise_core::nbt::Tag;
#[allow(unused_imports)]
pub use sunrise_items::items::ItemStack;

/// Failure to access a native item component.
#[derive(Debug)]
pub(crate) enum NativeComponentError {
    UnknownType,
    InvalidValue(sunrise_protocol::Error),
}
impl std::fmt::Display for NativeComponentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownType => f.write_str("unregistered component type"),
            Self::InvalidValue(error) => write!(f, "invalid component value: {error}"),
        }
    }
}
impl std::error::Error for NativeComponentError {}

/// Use the attack calculations with a private RNG snapshot, never the world's live source.
pub fn stack_attack_bonus_preview(
    enchantments: &sunrise_items::enchant::Enchantments,
    stack: &ItemStack,
    ctx: &sunrise_items::enchant::EffectContext<'_>,
    random: &sunrise_core::random::LegacyRandom,
    base: f32,
) -> f32 {
    let mut random = random.clone();
    let enchanted = crate::game::combat_common::stack_enchanted_damage(
        enchantments,
        stack,
        ctx,
        &mut random,
        base,
    ) - base;
    let (fall, flying) = ctx.direct_motion.unwrap_or_default();
    let smash = if &*stack.item == "minecraft:mace"
        && crate::game::mace_common::can_smash(fall, flying)
    {
        crate::game::mace_common::stack_smash_damage(enchantments, stack, fall, ctx, &mut random)
    } else {
        0.0
    };
    enchanted + smash
}

impl crate::game::CommonGame<'_> {
    /// Item `item`'s default value of component `component`, in its prototype
    /// (`Item.components().get`): `minecraft:max_damage` on an iron sword is `250`, data
    /// packs' and native mods' changes included. Read from the item report's JSON, so a
    /// number is an `Int`, `Long` or `Double` and a boolean a `Byte`. None where the item
    /// hasn't the component, and for an unknown item.
    pub fn item_default_component(
        &self,
        item: mod_api::ItemKey,
        component: &str,
    ) -> Option<Tag> {
        let item = mod_api::__private::item_key_id(item)?;
        let prototype = self.pack.item_models.prototypes.get(item)?;
        Some(sunrise_game_data::scoreboard::json_to_tag(
            prototype.get(component)?,
        ))
    }
    /// `Item.getName(ItemStack)` of a mod item: what its `item_name` Part
    /// answers, or `None` for the default. One table lookup for an item no
    /// Part names.
    pub fn mod_item_name(
        &self,
        stack: &ItemStack,
    ) -> Option<sunrise_game_data::text::Text> {
        sunrise_sim::stations::item_name_part(&self.loop_context, stack)
    }
    /// Effective components, including defaults and removals, in the gameplay component view.
    pub(in crate::game) fn item_combat_component(
        &self,
        stack: &ItemStack,
        id: &str,
    ) -> Option<serde_json::Value> {
        self.pack
            .item_models
            .components_view(&sunrise_sim::item_models::stack_view(stack))
            .get(id)
            .cloned()
    }
    /// Materialize the effective enchantments for both attacks and detached previews.
    pub fn item_combat_stack(&self, stack: &ItemStack) -> ItemStack {
        let mut effective = stack.clone();
        let enchantments = self
            .item_combat_component(stack, "minecraft:enchantments")
            .unwrap_or_else(|| serde_json::json!({}));
        let tag = sunrise_game_data::scoreboard::json_to_tag(
            &serde_json::json!({"minecraft:enchantments": enchantments}),
        );
        let (_, extra) = sunrise_items::item_data::decode_components(Some(&tag));
        effective.data_mut().enchantments =
            extra.map_or_else(Vec::new, |data| data.enchantments.clone());
        effective
    }
}

pub trait NativeComponentsGameExt1<'__sg>: crate::game::SideGame<'__sg> {
    /// Metadata backing ItemResource::damageable through the gameplay property path.
     fn item_resource_damageable(&self, stack: &ItemStack) -> bool {
        sunrise_sim::stations::is_damageable(self.items(), stack)
    }
    /// Metadata backing ItemResource::foil through the gameplay property path.
     fn item_resource_foil(&self, stack: &ItemStack) -> bool {
        stack.has_foil(
            self.items()
                .enchantments
                .props(&stack.item)
                .and_then(|p| p.glint),
        )
    }
    /// Metadata backing ItemResource::repairable through the gameplay property path.
     fn item_resource_repairable(
        &self,
        stack: &ItemStack,
        material: mod_api::ItemKey,
    ) -> bool {
        let Some(id) = mod_api::__private::item_key_id(material) else {
            return false;
        };
        let Some(item) = self.items().get(id) else {
            return false;
        };
        let material = ItemStack {
            item: item.id.clone(),
            count: 1,
            damage: 0,
            extra: None,
        };
        sunrise_sim::stations::is_valid_repair_item(
            &self.items().enchantments,
            self.items(),
            stack,
            &material,
        )
    }
    /// Item `item`'s cooking-fuel burn time in ticks (`FuelValues.burnDuration` of its `cooking_fuel`
    /// component, the data pack's context int provider resolved), None where it is no fuel. API
    /// @API:item_components@.
     fn item_burn_time(&self, item: mod_api::ItemKey) -> Option<i32> {
        let item = mod_api::__private::item_key_id(item)?;
        let (time, _) = self.items().get(item)?.fuel.as_ref()?;
        Some(self.active_providers().int_value(time))
    }
    /// Item `item`'s chance to raise a composter's level (its `compostable` component's layers provider
    /// resolved), None where it is not compostable. API @API:item_components@.
     fn item_compost_chance(&self, item: mod_api::ItemKey) -> Option<f32> {
        let item = mod_api::__private::item_key_id(item)?;
        let layers = self.items().get(item)?.compostable.as_ref()?;
        Some(self.active_providers().layer_chance(layers))
    }
    /// The slot `stack` is equipped to (`Equippable.slot`) after its component patch, behind
    /// `ItemResource::equippable_slot`, through the opaque backend. API @API:item_components@.
     fn item_equippable_slot(&self, stack: &ItemStack) -> Option<String> {
        sunrise_sim::item_sync::equippable_slot(stack, self.items())
    }
    /// `InstrumentItem.getUseDuration`: effective named or inline instrument; no instrument means zero.
     fn native_item_instrument_duration(&self, stack: &ItemStack) -> i32 {
        if !stack.carries_component(self.items().get(&stack.item), "minecraft:instrument") {
            return 0;
        }
        if let Some(value) = stack
            .data()
            .and_then(|data| data.other("minecraft:instrument"))
        {
            return match value {
                Tag::String(id) => self
                    .items()
                    .instruments
                    .get(&id.to_string_lossy())
                    .map_or(0, |value| value.use_ticks()),
                _ => value
                    .get("use_duration")
                    .and_then(sunrise_items::properties::component_float)
                    .map_or(0, sunrise_items::items::instrument_use_ticks),
            };
        }
        let Some(value) = self
            .pack
            .item_models
            .prototypes
            .get(&*stack.item)
            .and_then(|item| item.get("minecraft:instrument"))
        else {
            return 0;
        };
        match value.as_str() {
            Some(id) => self
                .items()
                .instruments
                .get(id)
                .map_or(0, |value| value.use_ticks()),
            None => value
                .get("use_duration")
                .and_then(serde_json::Value::as_f64)
                .map_or(0, |seconds| {
                    sunrise_items::items::instrument_use_ticks(seconds as f32)
                }),
        }
    }
    /// `ItemStack.getUseDuration`, behind ItemResource's read; gameplay owns the Part call.
     fn item_resource_use_duration(&self, stack: &ItemStack) -> Option<i32> {
        self.items().get(&stack.item)?;
        Some(self.use_duration(stack))
    }
    /// `IItemExtension.getCreatorModId`, behind ItemResource's read: the item
    /// id's namespace, or what the item's `creator_mod_id` Part answers.
     fn item_resource_creator_mod_id(
        &self,
        stack: &ItemStack,
    ) -> Option<String> {
        self.items().get(&stack.item)?;
        let namespace = stack.item.split_once(':').map_or("minecraft", |(ns, _)| ns);
        let mut mod_id = Some(namespace.to_owned());
        if mod_api::__private::item_listens(mod_api::HookId::CreatorModId, &stack.item) {
            self.native_creator_mod_id(stack, &mut mod_id);
        }
        mod_id
    }
    /// `ItemStack.isBarVisible`, `getBarWidth`, `getBarColor`, before client-only hooks.
     fn item_resource_bar(&self, stack: &ItemStack) -> Option<mod_api::ItemBar> {
        use super::api_values::TryIntoApi;
        let snapshot = stack.try_into_api().ok()?;
        let resource = mod_api::ItemResource::of(&snapshot);
        let component = |id| {
            resource.value(
                <Self::State as crate::game::Side>::as_host(self),
                &mod_api::ComponentId::new(id).expect("built-in component"),
            )
        };
        if self.items().get(&stack.item)?.bundle {
            let contents = component("minecraft:bundle_contents")?;
            let patch = Tag::compound([("minecraft:bundle_contents", contents)]);
            let (damage, extra) = sunrise_items::item_data::decode_components(Some(&patch));
            let effective = ItemStack {
                item: stack.item.clone(),
                count: 1,
                damage,
                extra,
            };
            let (width, color) =
                sunrise_items::bundle::Contents::of(self.items(), &effective)?.bar()?;
            return Some(mod_api::ItemBar { width, color });
        }
        if !resource.damageable(<Self::State as crate::game::Side>::as_host(self)) {
            return None;
        }
        let max_damage = component("minecraft:max_damage")
            .as_ref()
            .and_then(sunrise_items::properties::component_int)?;
        let damage = component("minecraft:damage")
            .as_ref()
            .and_then(sunrise_items::properties::component_int)?;
        sunrise_items::properties::damage_bar(damage, max_damage)
            .try_into_api()
            .unwrap()
    }
     fn item_resource_attack_damage_bonus(
        &self,
        stack: &ItemStack,
        attacker: mod_api::LivingRef,
        target: mod_api::EntityRef,
        base_damage: f32,
    ) -> Option<f32> {
        let (_, _, fall, flying) = self.item_combat_actor(attacker)?;
        if self.entity_level(attacker.entity())?.dimension() != self.dim.key()
            || self.entity_level(target)?.dimension() != self.dim.key()
        {
            return None;
        }
        self.native_entity_read_target(target)?;
        let target = self.entity_kind(target)?.id(<Self::State as crate::game::Side>::as_host(self))?;
        let actor = self
            .entity_kind(attacker.entity())?
            .id(<Self::State as crate::game::Side>::as_host(self))?;
        let source = self.item_resource_damage_source(stack, attacker)?;
        let source = source.damage_type.to_string();
        let effective = self.item_combat_stack(stack);
        let ctx = sunrise_items::enchant::EffectContext {
            this: Some(target.as_str()),
            attacker: Some(actor.as_str()),
            direct_attacker: Some(actor.as_str()),
            is_direct: true,
            damage_type: Some(&source),
            items: Some(self.items()),
            direct_motion: Some((fall, flying)),
            ..Default::default()
        };
        Some(stack_attack_bonus_preview(
            self.item_enchantments(),
            &effective,
            &ctx,
            &self.world.random,
            base_damage,
        ))
    }
    /// Actor state shared by detached stack queries, without swapping equipment or players.
     fn item_combat_actor(
        &self,
        actor: mod_api::LivingRef,
    ) -> Option<(bool, bool, f64, bool)> {
        self.side_item_combat_actor(actor)
    }
     fn item_resource_entity_attack_range(
        &self,
        stack: &ItemStack,
        attacker: mod_api::LivingRef,
    ) -> Option<mod_api::ItemAttackRange> {
        self.items().get(&stack.item)?;
        let (player, creative, _, _) = self.item_combat_actor(attacker)?;
        let default =
            self.living_attribute_value(attacker, "minecraft:entity_interaction_range")?;
        Some(self.stack_attack_range(Some(stack), player, creative, default))
    }
     fn item_resource_damage_source(
        &self,
        stack: &ItemStack,
        attacker: mod_api::LivingRef,
    ) -> Option<mod_api::ItemDamageSource> {
        self.items().get(&stack.item)?;
        let (player, _, fall, flying) = self.item_combat_actor(attacker)?;
        let name = self.stack_melee_damage_type(Some(stack), (fall, flying), player);
        self.health_resources.damage.id(&name)?;
        Some(mod_api::ItemDamageSource {
            damage_type: mod_api::Identifier::parse(&name).ok()?,
            direct: Some(attacker.entity()),
            causing: Some(attacker.entity()),
            position: None,
        })
    }
     fn item_resource_can_be_hurt_by(
        &self,
        stack: &ItemStack,
        source: &mod_api::ItemDamageSource,
    ) -> Option<bool> {
        self.stack_can_be_hurt_by(stack, &source.damage_type.to_string())
    }
    /// `ItemStack.canBeHurtBy`, shared by armour wear and detached stack reads.
     fn stack_can_be_hurt_by(
        &self,
        stack: &ItemStack,
        name: &str,
    ) -> Option<bool> {
        self.items().get(&stack.item)?;
        stack_can_be_hurt_by(
            &self.pack.item_models,
            &self.health_resources.damage,
            stack,
            name,
        )
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> NativeComponentsGameExt1<'__sg> for G {}


// Native mods use these helpers.
#[allow(dead_code)]
pub trait NativeComponentsGameExt2<'__sg>: crate::game::SideGame<'__sg> {
    /// Read a native component, including the item's default when not patched.
     fn native_component<'a>(
        &'a self,
        stack: &'a ItemStack,
        id: &str,
    ) -> Result<Option<&'a Tag>, NativeComponentError>
    where
        '__sg: 'a,
    {
        if sunrise_protocol::item::custom::id(id).is_none() {
            return Err(NativeComponentError::UnknownType);
        }
        if let Some(data) = stack.data() {
            if data.other(&format!("!{id}")).is_some() {
                return Ok(None);
            }
            if let Some(value) = data.other(id) {
                return Ok(Some(value));
            }
        }
        Ok(self
            .items()
            .get(&stack.item)
            .and_then(|item| item.custom_components.get(id)))
    }

    /// Set a registered native component after checking the network NBT limits.
     fn native_set_component(
        &self,
        stack: &mut ItemStack,
        id: &str,
        value: Tag,
    ) -> Result<(), NativeComponentError> {
        if sunrise_protocol::item::custom::id(id).is_none() {
            return Err(NativeComponentError::UnknownType);
        }
        sunrise_core::nbt::validate_component_value(&value).map_err(|error| {
            NativeComponentError::InvalidValue(sunrise_protocol::Error::new(error))
        })?;
        let default = self
            .items()
            .get(&stack.item)
            .and_then(|item| item.custom_components.get(id));
        let value = (default != Some(&value)).then_some(value);
        let removed = format!("!{id}");
        let data = stack.data_mut();
        data.set_other(&removed, None);
        data.set_other(id, value);
        stack.tidy();
        Ok(())
    }

    /// Remove a native component, including its item default.
     fn native_remove_component(
        &self,
        stack: &mut ItemStack,
        id: &str,
    ) -> Result<(), NativeComponentError> {
        if sunrise_protocol::item::custom::id(id).is_none() {
            return Err(NativeComponentError::UnknownType);
        }
        let default = self
            .items()
            .get(&stack.item)
            .is_some_and(|item| item.custom_components.contains_key(id));
        let removed = format!("!{id}");
        let data = stack.data_mut();
        data.set_other(id, None);
        data.set_other(&removed, default.then(|| Tag::Compound(Default::default())));
        stack.tidy();
        Ok(())
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> NativeComponentsGameExt2<'__sg> for G {}


pub trait NativeComponentsGameExt3<'__sg>: crate::game::SideGame<'__sg> {
    /// `EntityType.is(TagKey)`: whether `entity_type` (`minecraft:player`) is in the entity
    /// type tag `tag` (`minecraft:cannot_be_pushed_onto_boats`).
     fn entity_type_in_tag(&self, entity_type: &str, tag: &str) -> bool {
        self.entity_tag_matches(tag, entity_type)
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> NativeComponentsGameExt3<'__sg> for G {}


/// Supply the gameplay component snapshot to the shared stack rule.
pub fn stack_can_be_hurt_by(
    models: &sunrise_assets::item_model::ItemModels,
    catalog: &sunrise_world::damage_source::Catalog,
    stack: &ItemStack,
    name: &str,
) -> Option<bool> {
    let default = models
        .prototypes
        .get(stack.item.as_ref())
        .and_then(|components| components.get("minecraft:damage_resistant"));
    sunrise_sim::item_damage::stack_can_be_hurt_by(default, catalog, stack, name)
}

pub const ITEM_ENVIRONMENT_DAMAGE: [&str; 4] = [
    "minecraft:on_fire",
    "minecraft:lava",
    "minecraft:in_fire",
    "minecraft:cactus",
];

pub fn item_damage_resolver<'a>(
    models: &'a sunrise_assets::item_model::ItemModels,
    catalog: &'a sunrise_world::damage_source::Catalog,
) -> impl Fn(&ItemStack, &str) -> bool + 'a {
    move |stack, damage| stack_can_be_hurt_by(models, catalog, stack, damage).unwrap_or(true)
}

use crate::game::side_ops::modapi_modding::ModapiModdingSideGameExt as _;

use crate::game::tables::GameItems as _;

use crate::game::commands_common::CommandsCommonGameExt2 as _;

use crate::game::held_items::HeldItemsGameExt1 as _;

use crate::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;
