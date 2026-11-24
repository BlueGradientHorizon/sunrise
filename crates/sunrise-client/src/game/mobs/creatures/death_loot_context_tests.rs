#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use std::sync::{Arc, Mutex};

pub(in crate::game) type Observation = (String, i32, i32, Option<u32>);
pub(in crate::game) struct Probe(pub(in crate::game) Arc<Mutex<Vec<Observation>>>);
// Veto the first death; the next victim must still reach the drop hook.
pub(in crate::game) struct CancelOnce(pub(in crate::game) std::sync::atomic::AtomicBool);
#[mod_api::__subscriptions]
impl mod_api::Hooks for CancelOnce {
    fn living_death(
        &self,
        _: &mut dyn mod_api::ServerHost,
        _: &mut mod_api::LivingDeathEvent<'_>,
    ) -> mod_api::Flow {
        if self.0.swap(false, std::sync::atomic::Ordering::Relaxed) {
            mod_api::Flow::Cancel
        } else {
            mod_api::Flow::Continue
        }
    }
}
#[mod_api::__subscriptions]
impl mod_api::Hooks for Probe {
    fn loot_drops(
        &self,
        host: &mut dyn mod_api::ServerHost,
        event: &mut mod_api::LootDropsEvent<'_>,
    ) {
        let source = event.context.damage_source.unwrap();
        assert!(event.context.tool.is_none(), "entity context has no TOOL");
        let hand = host
            .entity(source.causing as u32)
            .and_then(|entity| host.living(entity))
            .and_then(|living| {
                living.with_hand_item(host, mod_api::Hand::Main, |stack| {
                    (stack.damage(), stack.count())
                })
            });
        if source.causing == 12 {
            assert_eq!(
                hand.map(|(damage, _)| damage),
                Some(31),
                "later listeners read the earlier loot listener's hand edit"
            );
        }
        self.0.lock().unwrap().push((
            source.damage_type.into(),
            source.causing,
            source.direct,
            hand.map(|(_, count)| count),
        ));
    }
}

pub(in crate::game) struct ChangeKillerHand(pub(in crate::game) mod_api::ItemStack);
#[mod_api::__subscriptions]
impl mod_api::Hooks for ChangeKillerHand {
    fn living_death(
        &self,
        host: &mut dyn mod_api::ServerHost,
        _: &mut mod_api::LivingDeathEvent<'_>,
    ) -> mod_api::Flow {
        let entity = host.entity(12).unwrap();
        let living = host.living(entity).unwrap();
        assert!(
            living
                .set_hand_item(host, mod_api::Hand::Main, Some(self.0.clone()))
                .unwrap()
        );
        mod_api::Flow::Continue
    }
    fn loot_drops(
        &self,
        host: &mut dyn mod_api::ServerHost,
        event: &mut mod_api::LootDropsEvent<'_>,
    ) {
        assert!(event.context.tool.is_none());
        let entity = host
            .entity(event.context.damage_source.unwrap().causing as u32)
            .unwrap();
        let living = host.living(entity).unwrap();
        assert_eq!(
            living
                .with_hand_item(host, mod_api::Hand::Main, |stack| stack.project())
                .flatten(),
            Some(self.0.clone()),
            "loot sees the complete stack installed during living_death"
        );
        assert!(
            living
                .set_hand_item(
                    host,
                    mod_api::Hand::Main,
                    Some({
                        let mut later = self.0.clone();
                        later.set_damage(31);
                        later
                    })
                )
                .unwrap()
        );
    }
}

pub(in crate::game) struct Nested;
#[mod_api::__subscriptions]
impl mod_api::Hooks for Nested {
    fn loot_drops(
        &self,
        host: &mut dyn mod_api::ServerHost,
        event: &mut mod_api::LootDropsEvent<'_>,
    ) {
        if event.context.entity.map(|e| e.0) != Some(90) {
            return;
        }
        let source = event.context.damage_source;
        let tool = event.context.tool.cloned();
        assert!(host.commands().run_command("kill @e[type=minecraft:pig]"));
        assert_eq!(event.context.damage_source, source);
        assert_eq!(event.context.tool, tool.as_ref());
    }
}

pub(in crate::game) struct BlockTool(pub(in crate::game) mod_api::ItemStack);
#[mod_api::__subscriptions]
impl mod_api::Hooks for BlockTool {
    fn loot_drops(&self, _: &mut dyn mod_api::ServerHost, event: &mut mod_api::LootDropsEvent<'_>) {
        assert!(event.context.damage_source.is_none());
        assert!(event.context.block.is_some());
        assert_eq!(event.context.tool, Some(&self.0));
    }
}
#[test]
#[ignore = "daily"]
fn death_loot_change_preserves_block_tool() {
    crate::game::native_entity_refs::tests::with_game(|game| {
        game.build_registries();
        let mut tool = ItemStack::new("minecraft:diamond_pickaxe", 1);
        tool.damage = 17;
        tool.enchant("minecraft:fortune", 3);
        let expected = tool.try_into_api().unwrap();
        let stone = game
            .pack
            .assets
            .registry
            .default_state("minecraft:stone")
            .unwrap();
        let mods = vec![mod_api::ModEntry::new(
            "block_tool",
            Box::new(BlockTool(expected)),
        )]
        .into();
        mod_platform::testing::with_mods(mods, || {
            game.native_block_loot([0, 64, 0], stone, None, Some(&tool), None, &mut Vec::new());
        });
    });
}
