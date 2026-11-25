#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::native_components::ITEM_ENVIRONMENT_DAMAGE;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::native_components::NativeComponentError;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::native_components::item_damage_resolver;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::native_components::stack_attack_bonus_preview;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::native_components::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::items::ItemStack;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::nbt::Tag;

impl Game<'_> {
    pub(in crate::game) fn item_resource_attack_damage_bonus(
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
        let target = self.entity_kind(target)?.id(self)?;
        let actor = self.entity_kind(attacker.entity())?.id(self)?;
        let source = self.item_resource_damage_source(stack, attacker)?;
        let source = source.damage_type.to_string();
        let effective = self.item_combat_stack(stack);
        let ctx = sunrise_items::enchant::EffectContext {
            this: Some(target.as_str()),
            attacker: Some(actor.as_str()),
            direct_attacker: Some(actor.as_str()),
            is_direct: true,
            damage_type: Some(&source),
            items: Some(world_items!(self)),
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
    /// Roll through the game's enchantment registry and the active level's random source.
    pub(in crate::game) fn item_resource_enchant_randomly(
        &mut self,
        stack: &ItemStack,
        level: mod_api::LevelRef,
        power: i32,
        pool: &mod_api::Identifier,
    ) -> Result<Option<ItemStack>, mod_api::StackWriteError> {
        if level.dimension() != self.dim.key() || self.native_level_world(level).is_none() {
            return Ok(None);
        }
        let items = world_items!(self);
        let enchantments = std::sync::Arc::clone(self.item_enchantments());
        let pool = enchantments.tag(&pool.to_string());
        if pool.is_empty() {
            return Ok(None);
        }
        let value = enchantments.enchantable_of(stack);
        let picked = enchantments.select_with_value(
            items,
            &mut self.world.random,
            &stack.item,
            power,
            pool,
            value,
        );
        Ok(sunrise_client::stations::apply_enchantment_list(
            stack, picked,
        ))
    }
}

#[cfg(test)]
pub(in crate::game) mod item_resource_property_tests {
    #[allow(unused_imports)]
pub(in crate::game::server_side) use serde_json::Value;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::native_components::item_resource_property_tests::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::native_components::item_resource_property_tests::engine;

    pub(in crate::game) fn run_effective_resource_properties_match_vanilla() {
        let inputs = std::env::var("SUNRISE_ITEM_PROPERTIES_CASES")
            .map(|path| std::fs::read_to_string(path).unwrap())
            .unwrap_or_else(|_| {
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../tests/fixtures/item_resource_properties.jsonl"
                ))
                .into()
            });
        let expected = std::env::var("SUNRISE_ITEM_PROPERTIES_EXPECTED")
            .map(|path| std::fs::read_to_string(path).unwrap())
            .unwrap_or_else(|_| {
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../tests/fixtures/item_resource_properties_expected.jsonl"
                ))
                .into()
            });
        let rows = |text: &str| {
            text.lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| serde_json::from_str::<Value>(line).unwrap())
                .collect::<Vec<_>>()
        };
        let (inputs, expected) = (rows(&inputs), rows(&expected));
        assert!(inputs.len() >= 41);
        assert_eq!(inputs.len(), expected.len());
        crate::game::native_entity_refs::tests::with_game(|game| {
            // Authoritative tag reads are available only while a world is owned.
            game.session_lock.0 =
                Some(sunrise_client::save::SessionLock::acquire(&game.save.0.root).unwrap());
            for (input, expected) in inputs.iter().zip(&expected) {
                assert_eq!(input["name"], expected["name"]);
                let support::Outcome {
                    item,
                    material,
                    values,
                } = support::evaluate(game, input);
                assert_eq!(&values, expected, "{}", input["name"]);
                let stack = engine(&item, 1);
                let definitions = &game.items().enchantments;
                let props = definitions.props(&stack.item);
                assert_eq!(
                    game.use_duration(&stack),
                    expected["use_duration"].as_i64().unwrap() as i32
                );
                assert_eq!(
                    sunrise_client::stations::is_damageable(game.items(), &stack),
                    item.damageable(game)
                );
                assert_eq!(
                    stack.has_foil(props.and_then(|value| value.glint)),
                    item.foil(game)
                );
                assert_eq!(
                    sunrise_client::item_tooltip::rarity(
                        &stack,
                        props.map_or("common", |value| value.rarity.as_str())
                    ),
                    item.rarity(game).as_str()
                );
                assert_eq!(
                    sunrise_client::stations::is_valid_repair_item(
                        definitions,
                        game.items(),
                        &stack,
                        &engine(&material, 1)
                    ),
                    item.repairable(game, &material)
                );
            }
            game.session_lock.0 = None;
        });
    }
}
