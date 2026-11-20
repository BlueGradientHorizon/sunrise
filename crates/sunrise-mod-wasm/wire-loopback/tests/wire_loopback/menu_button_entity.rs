//! Optional block-entity views and the carried stack cross menu-button dispatch together.
use super::*;
use mod_api::item::menu::MenuButtonContext;
use mod_api::{MenuButtonEvent, MenuPart};

struct World;
impl mod_api::Host for World {}
mod_api::impl_host_areas!(World);
impl mod_api::ServerHost for World {}
mod_api::impl_server_host_areas!(World);

struct Button {
    drop_entity: bool,
}
#[mod_api::sunrise_part]
impl MenuPart for Button {
    fn menu_button(&self, _: &mut dyn mod_api::ServerHost, event: &mut MenuButtonEvent<'_>) {
        if let Some(entity) = event.entity.as_mut() {
            entity.data.push(9);
        }
        if let Some(stack) = event.carried.as_mut() {
            stack.set_count(stack.count() + 1).unwrap();
        }
        if self.drop_entity {
            event.entity = None;
        }
    }
}

fn proxy(drop_entity: bool) -> WasmHandler {
    WasmHandler {
        index: guest::add_handler(vec![guest::view::<dyn MenuPart>(Arc::new(Button {
            drop_entity,
        }))]),
        module: WasmMod {
            group: 0,
            subscribed: &[],
            listeners: &[],
            corrupt: false,
        },
    }
}

fn event_context() -> MenuButtonContext {
    MenuButtonContext {
        player: 123,
        player_entity_id: 42,
        pos: Some([1, 2, 3]),
        id: 7,
    }
}

#[test]
fn menu_button_optional_entity_and_carried_stack_use_checked_wire_writeback() {
    install_counter();
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
    let item = REGISTERED_ITEM.get().unwrap();
    let context = event_context();
    let mut host = World;

    let mut entity = mod_api::ModBlockEntity::default();
    entity.data = vec![4];
    let mut carried = Some(mod_api::ItemStack::new(item, 2).unwrap());
    let mut event = MenuButtonEvent {
        menu: "test:menu",
        block: None,
        context: &context,
        entity: Some(mod_api::ModBlockEntityView::new(&mut entity)),
        carried: &mut carried,
    };
    proxy(false).menu_button(&mut host, &mut event);
    drop(event);
    assert_eq!(entity.data, [4, 9]);
    assert_eq!(carried.as_ref().unwrap().count(), 3);

    let mut carried = Some(mod_api::ItemStack::new(item, 5).unwrap());
    let mut event = MenuButtonEvent {
        menu: "test:menu",
        block: None,
        context: &context,
        entity: None,
        carried: &mut carried,
    };
    proxy(false).menu_button(&mut host, &mut event);
    drop(event);
    assert_eq!(carried.as_ref().unwrap().count(), 6);

    let mut entity = mod_api::ModBlockEntity::default();
    entity.data = vec![12];
    let before_revision = entity.revision();
    let mut carried = Some(mod_api::ItemStack::new(item, 8).unwrap());
    let mut event = MenuButtonEvent {
        menu: "test:menu",
        block: None,
        context: &context,
        entity: Some(mod_api::ModBlockEntityView::new(&mut entity)),
        carried: &mut carried,
    };
    proxy(true).menu_button(&mut host, &mut event);
    drop(event);
    assert_eq!(
        entity.data,
        [12],
        "a stale Some-to-None reply must not apply"
    );
    assert_eq!(entity.revision(), before_revision);
    assert_eq!(carried.as_ref().unwrap().count(), 8);
}
