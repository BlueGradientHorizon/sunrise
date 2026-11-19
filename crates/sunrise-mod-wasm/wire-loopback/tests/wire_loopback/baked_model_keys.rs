use super::*;
use mod_api::prelude::*;
use mod_api::{BakedModelKey, BakedModelView, BakedQuad, BlockStateId, Direction, ModelError};

/// The game's published store: generation 5 holds slot 2, stone's one model.
struct Store;
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
    fn block_state_models(&self, state: BlockStateId) -> Result<Vec<BakedModelKey>, ModelError> {
        match state.0 {
            1 => Ok(vec![key(5, 2)]),
            0 => Ok(Vec::new()),
            _ => Err(ModelError::UnknownModel),
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
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
    let mut store = Store;
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
    let host = &mut guest::Guest;
    let assets = host.render_assets();
    let keys = assets.block_state_models(BlockStateId(1)).unwrap();
    assert_eq!(keys, [key(5, 2)]);
    assert_eq!(assets.block_state_models(BlockStateId(0)), Ok(Vec::new()));
    assert_eq!(
        assets.block_state_models(BlockStateId(9)),
        Err(ModelError::UnknownModel)
    );
    assert_eq!(assets.baked_model(keys[0]), Ok(stone()));
    assert_eq!(assets.baked_model(key(4, 2)), Err(ModelError::StaleModel));
    assert_eq!(assets.baked_model(key(5, 7)), Err(ModelError::UnknownModel));
    guest::set_transport(None);
}
