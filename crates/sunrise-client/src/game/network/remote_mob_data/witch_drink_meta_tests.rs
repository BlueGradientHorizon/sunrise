//! A drinking witch is not using an item: `LivingEntity.setLivingEntityFlag(1, …)` has only
//! `startUsingItem` and `stopUsingItem` as callers, and `Witch.aiStep` counts her drink in its
//! own `usingTime` and `DATA_USING_ITEM`. So a host sends her living flags as 0 while she
//! drinks (a vanilla client would otherwise start a use of her potion), and her drink on her
//! own flag.
use super::super::lan;
use super::pet_pose_meta_tests::{meta_hub, root};
use sunrise_client::mobs::{Mob, MobTypes};
use sunrise_core::geometry::Vec3;
use sunrise_protocol::entity_data::Value;

/// A witch as the host tracks her (None without the prepared resources).
fn witch() -> Option<Mob> {
    let types = MobTypes::load(&root()).ok()?;
    let kind = std::sync::Arc::new(types.types.get("minecraft:witch")?.clone());
    Some(Mob::new(1, kind, Vec3::ZERO, 0.0))
}

/// What the host sends for `m` at `index`, if anything.
fn sent_at(m: &Mob, hub: &sunrise_client::lan::hub::Hub, index: u8) -> Option<Value> {
    let (mut meta, mut defaults) = (Vec::new(), Vec::new());
    lan::mob_meta(m, hub, None, 0, &mut meta, &mut defaults);
    meta.into_iter()
        .find(|(at, _, _)| *at == index)
        .map(|(_, _, value)| value)
}

#[test]
fn a_drinking_witch_sends_her_own_flag_and_no_item_use() {
    let Some(mut witch) = witch() else {
        return;
    };
    let hub = meta_hub();
    // `Witch.aiStep` as it starts a drink: 32 ticks, her flag set, the potion in hand.
    witch.extra.using_item = 32;
    witch.extra.flag = true;
    assert_eq!(sent_at(&witch, &hub, 8), None, "living flags stay 0");
    assert_eq!(sent_at(&witch, &hub, 17), Some(Value::Bool(true)));

    // The drink is done: neither is sent past its default.
    witch.extra.using_item = 0;
    witch.extra.flag = false;
    assert_eq!(sent_at(&witch, &hub, 8), None);
    assert_eq!(sent_at(&witch, &hub, 17), None);
}
