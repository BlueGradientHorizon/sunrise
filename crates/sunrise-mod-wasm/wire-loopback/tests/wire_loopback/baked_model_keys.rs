use super::*;
use mod_api::prelude::*;
use mod_api::{
    BakedModelKey, BakedModelView, BakedQuad, BlockState, BlockStateRecord, Direction, ModelError,
};

/// The game's published store: generation 5 holds slot 2, stone's one model.
struct Store {
    stone: BlockState,
    air: BlockState,
}
impl mod_api::Host for Store {}
mod_api::impl_host_areas!(Store);
impl mod_api::ClientHost for Store {}
mod_api::impl_client_host_areas!(Store, except(services));
impl ServicesClientHost for Store {
    fn render_assets(&self) -> &dyn mod_api::RenderAssets {
        self
    }
}
fn key(generation: u64, slot: u32) -> BakedModelKey {
    mod_api::__private::built::baked_model_key(generation, slot)
}
fn stone() -> BakedModelView {
    let mut quad = BakedQuad::new(
        [
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
        ],
        [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]],
        "minecraft:block/stone",
        Direction::Up,
    );
    quad.cull = Some(Direction::Up);
    quad.light_emission = 3;
    let mut view = mod_api::__private::built::baked_model_view();
    view.ambient_occlusion = false;
    view.particle = Some("minecraft:block/stone".into());
    view.quads = vec![quad];
    view
}
impl mod_api::RenderAssets for Store {
    fn block_state_models(&self, state: BlockState) -> Result<Vec<BakedModelKey>, ModelError> {
        if state == self.stone {
            Ok(vec![key(5, 2)])
        } else if state == self.air {
            Ok(Vec::new())
        } else {
            Err(ModelError::UnknownModel)
        }
    }
    fn baked_model(&self, model: BakedModelKey) -> Result<BakedModelView, ModelError> {
        if model == key(5, 2) {
            Ok(stone())
        } else if model == key(4, 2) {
            Err(ModelError::StaleModel)
        } else {
            Err(ModelError::UnknownModel)
        }
    }
}

#[test]
fn baked_model_keys_round_trip_as_generation_and_slot_and_refuse_generation_zero() {
    let mut out = Writer::new();
    out.put(&key(5, 2));
    let mut expected = 5u64.to_le_bytes().to_vec();
    expected.extend(2u32.to_le_bytes());
    assert_eq!(out.bytes(), expected);
    assert_eq!(
        Reader::decode::<BakedModelKey>(out.bytes()).unwrap(),
        key(5, 2)
    );
    let mut zero = Writer::new();
    zero.put(&0u64);
    zero.put(&2u32);
    assert!(Reader::decode::<BakedModelKey>(zero.bytes()).is_err());
}

#[test]
fn wire_loopback_a_guest_reads_a_states_keys_and_a_copy_of_its_model() {
    mod_api::__private::initialize().unwrap();
    let mods: mod_api::Mods = vec![].into();
    mod_api::testing::with_mods(mods.clone(), || {
        use mod_api::__private::{ModPhase, PhaseOwner};
        mod_api::__private::part_tests::install_parts_fixture(&mods).unwrap();
        let properties = std::collections::BTreeMap::new();
        mod_api::__private::resolve_block_keys(
            ["minecraft:stone", "minecraft:air", "minecraft:dirt"],
            |_| {
                sunrise_mod_runtime::block_definitions::BlockDefinition::new(&[&properties], 0)
                    .map_err(|error| format!("{error:?}"))
            },
        )
        .unwrap();
        let state = |name| {
            BlockState::from(BlockStateRecord::new(
                mod_api::__private::block_key(name).unwrap(),
                0,
            ))
        };
        let stone_state = state("minecraft:stone");
        let air = state("minecraft:air");
        // Dirt has an issued token for Wire, but the fixture store knows only stone and air.
        let unknown_block = state("minecraft:dirt");
        let unknown_ordinal = BlockStateRecord::new(stone_state.block(), u16::MAX).into();
        let mut store = Store {
            stone: stone_state,
            air,
        };
        let mut frame = boundary::Frame::default();
        frame.set_client_host_mut(&mut store);
        let mut cx = boundary::Cx::default();
        cx.frames.push(frame);
        let cx = std::cell::RefCell::new(cx);
        guest::set_transport(Some(Rc::new(move |id, bytes| {
            let mut out = Writer::new();
            // SAFETY: the store and its frame live throughout the synchronous transport.
            unsafe { boundary::serve(id, &mut *cx.borrow_mut(), bytes, &mut out) }.unwrap();
            out.into_bytes()
        })));
        for phase in [ModPhase::Setup, ModPhase::Reloading, ModPhase::World] {
            let _phase = PhaseOwner::scope(phase);
            let assets = guest::Guest.render_assets();
            let keys = assets.block_state_models(stone_state).unwrap();
            assert_eq!(keys, [key(5, 2)]);
            assert_eq!(assets.block_state_models(air), Ok(Vec::new()));
            for invalid in [unknown_block, unknown_ordinal] {
                assert_eq!(
                    assets.block_state_models(invalid),
                    Err(ModelError::UnknownModel)
                );
            }
            assert_eq!(assets.baked_model(keys[0]), Ok(stone()));
            assert_eq!(assets.baked_model(key(4, 2)), Err(ModelError::StaleModel));
            assert_eq!(assets.baked_model(key(5, 7)), Err(ModelError::UnknownModel));
        }
        guest::set_transport(None);
    });
}
