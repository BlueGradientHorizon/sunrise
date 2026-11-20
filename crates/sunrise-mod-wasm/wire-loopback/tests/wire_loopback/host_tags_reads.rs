//! A hook's `Host` answers the merged tags on wasm through the same imports a level view or a loot
//! context uses, with what a native listener gets from `host.tags()`.
use super::*;
use mod_api::prelude::TagsHost;

struct TagTable;
impl mod_api::Tags for TagTable {
    fn key(&self, id: &mod_api::Identifier) -> Option<mod_api::TagKey> {
        mod_api::__private::tag_key(id)
    }
    fn is_in_tag(&self, item: mod_api::ItemKey, tag: mod_api::TagKey) -> Option<bool> {
        assert_eq!(item, REGISTERED_ITEM.get().unwrap());
        mod_api::__private::with_tag_name(tag, |name| Some(name? == "test:ingredients"))
    }
}

struct World;
impl mod_api::Host for World {}
mod_api::impl_host_areas!(World, except(tags));
impl TagsHost for World {
    fn tags(&self) -> &dyn mod_api::Tags {
        &TagTable
    }
}

#[test]
fn a_host_answers_the_merged_tags_on_wasm_as_natively() {
    install_counter();
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
    let item = REGISTERED_ITEM.get().unwrap();
    let ingredients = mod_api::Identifier::parse("test:ingredients").unwrap();
    let other = mod_api::Identifier::parse("test:other").unwrap();
    let world = World;
    let mut frame = boundary::Frame::default();
    frame.set_host(&world);
    let mut cx = boundary::Cx::default();
    cx.frames.push(frame);
    let cx = std::cell::RefCell::new(cx);
    guest::set_transport(Some(Rc::new(move |id, bytes| {
        let mut out = Writer::new();
        unsafe { boundary::serve(id, &mut *cx.borrow_mut(), bytes, &mut out) }.unwrap();
        out.into_bytes()
    })));
    let host: &dyn mod_api::Host = &guest::Guest;
    let tags = host.tags();
    let key = tags.key(&ingredients).unwrap();
    let native = world.tags();
    assert_eq!(
        tags.is_in_tag(item, key),
        native.is_in_tag(item, native.key(&ingredients).unwrap())
    );
    assert_eq!(tags.is_in_tag(item, key), Some(true));
    assert_ne!(tags.key(&other).unwrap(), key);
    guest::set_transport(None);
}
