//! A level callback's view answers the merged tags and the block-state facts through the same
//! imports a host frame uses, with the answers a native listener gets from the same view.
use super::*;
use mod_api::prelude::BlockStatesBackend;
use mod_api::{
    BlockMetadata, BlockState, BlockStateError, BlockStateRecord, BlockStates, LevelHost,
    LevelRandomHost, WeatheringAge,
};

struct Tables;
impl BlockStatesBackend for Tables {
    /// State 1 of the registered block is weathered; state 0 has no age.
    fn block_weathering_age(&self, state: BlockStateRecord) -> Option<WeatheringAge> {
        let weathered: BlockState =
            BlockStateRecord::new(REGISTERED_BLOCK.get().unwrap(), 1).into();
        (BlockState::from(state) == weathered).then_some(WeatheringAge::Weathered)
    }
}

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

/// A view that lends the game's tables.
struct WithTables;
impl LevelHost for WithTables {
    fn block_states(&self) -> BlockStates<'_> {
        BlockStates::of(&Tables)
    }
    fn tags(&self) -> &dyn mod_api::Tags {
        &TagTable
    }
}
impl LevelRandomHost for WithTables {}

/// A view with no tables, as the damage-hit view is.
struct NoTables;
impl LevelHost for NoTables {
    fn tags(&self) -> &dyn mod_api::Tags {
        &TagTable
    }
}
impl LevelRandomHost for NoTables {}

/// Serves the guest's calls from `frame`; the caller clears the transport.
fn serve(frame: boundary::Frame) {
    let mut cx = boundary::Cx::default();
    cx.frames.push(frame);
    let cx = std::cell::RefCell::new(cx);
    guest::set_transport(Some(Rc::new(move |id, bytes| {
        let mut out = Writer::new();
        unsafe { boundary::serve(id, &mut *cx.borrow_mut(), bytes, &mut out) }.unwrap();
        out.into_bytes()
    })));
}

#[test]
fn a_level_view_answers_tags_and_block_state_facts_on_wasm_as_natively() {
    install_counter();
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
    let block = REGISTERED_BLOCK.get().unwrap();
    let states: [BlockState; 2] = [0, 1].map(|index| BlockStateRecord::new(block, index).into());
    let ingredients = mod_api::Identifier::parse("test:ingredients").unwrap();
    let other = mod_api::Identifier::parse("test:other").unwrap();
    let with_tables = WithTables;
    let no_tables = NoTables;
    // Each view is lent as a `LevelHost` and as the `LevelRandomHost` that extends it.
    for (name, tables, native, lent_random) in [
        (
            "tables as LevelHost",
            true,
            &with_tables as &dyn LevelHost,
            false,
        ),
        ("tables as LevelRandomHost", true, &with_tables, true),
        ("no tables as LevelHost", false, &no_tables, false),
        ("no tables as LevelRandomHost", false, &no_tables, true),
    ] {
        let mut frame = boundary::Frame::default();
        if lent_random {
            match tables {
                true => frame.set_level_random_host(&with_tables),
                false => frame.set_level_random_host(&no_tables),
            }
        } else {
            frame.set_level_host(native);
        }
        serve(frame);
        let guest_level: &dyn LevelHost = &guest::Guest;
        let (guest_facts, native_facts) = (guest_level.block_states(), native.block_states());
        for state in states {
            assert_eq!(
                guest_facts.weathering_age(state),
                native_facts.weathering_age(state),
                "{name}"
            );
        }
        assert_eq!(
            guest_facts.weathering_age(states[1]),
            tables.then_some(WeatheringAge::Weathered),
            "{name}"
        );
        // A view with no tables refuses exactly as `BlockStates` does natively.
        if !tables {
            assert_eq!(guest_facts.name(block), native_facts.name(block), "{name}");
            assert_eq!(guest_facts.name(block), Err(BlockStateError::Unavailable));
        }
        let tags = guest_level.tags();
        let key = tags.key(&ingredients).unwrap();
        assert_eq!(
            tags.is_in_tag(REGISTERED_ITEM.get().unwrap(), key),
            native.tags().is_in_tag(
                REGISTERED_ITEM.get().unwrap(),
                native.tags().key(&ingredients).unwrap()
            ),
            "{name}"
        );
        assert_eq!(
            tags.is_in_tag(REGISTERED_ITEM.get().unwrap(), key),
            Some(true),
            "{name}"
        );
        assert_ne!(tags.key(&other).unwrap(), key, "{name}");
        guest::set_transport(None);
    }
}
