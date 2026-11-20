//! The general entity tick (#792 stage 4b) through the generated transport:
//! a guest part's `general_entity_tick` crosses with the row's kind and
//! entity, one call per row. The attachment store and the trap's rollback
//! are the game tests' to exercise — here the guest records each row it is
//! told into a thread-local, which is all the crossing itself carries.
use super::*;
use mod_api::{EntityPart, EntityRef, GeneralEntityTickEvent, ServerHost, WorldEpoch};
use std::cell::RefCell;

thread_local! {
    /// The rows the guest part was told, in order: their ids.
    static COUNTS: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
}

/// The host the part answers: nothing but its frame.
struct World;

impl mod_api::Host for World {}
mod_api::impl_host_areas!(World, except(config, entities));
impl mod_api::prelude::ConfigHost for World {
    fn dimension(&self) -> &'static str {
        "minecraft:overworld"
    }
}
impl mod_api::prelude::EntitiesHost for World {
    fn level(&self, _: &str) -> Option<mod_api::LevelRef> {
        None
    }
}
impl ServerHost for World {}
mod_api::impl_server_host_areas!(World);

/// The guest's part: the fixture's counter, recording each row it is told.
struct Counter;

#[mod_api::sunrise_part]
impl EntityPart for Counter {
    fn general_entity_tick(&self, _: &mut dyn ServerHost, event: &mut GeneralEntityTickEvent<'_>) {
        assert_eq!(event.kind, "wasm_fixture:robot", "the row's kind crosses");
        COUNTS.with_borrow_mut(|counts| counts.push(event.entity.id()));
    }
}

fn row(id: u32) -> EntityRef {
    EntityRef::new(WorldEpoch::new(1), id)
}

/// Two rows of one type through the crossing: the guest is told each row's
/// kind and id, in order — the part is shared, the row is its own.
#[test]
fn wire_loopback_general_entity_tick_tells_each_row_its_kind_and_id() {
    let proxy = WasmHandler {
        index: guest::add_handler(vec![guest::view::<dyn EntityPart>(Arc::new(Counter))]),
        module: WasmMod {
            group: 0,
            subscribed: &[],
            listeners: &[],
            corrupt: false,
        },
    };
    let mut world = World;
    for id in [7, 8] {
        proxy.general_entity_tick(
            &mut world,
            &mut GeneralEntityTickEvent {
                kind: "wasm_fixture:robot",
                entity: row(id),
            },
        );
    }
    assert_eq!(COUNTS.with_borrow(Clone::clone), [7, 8]);
}
