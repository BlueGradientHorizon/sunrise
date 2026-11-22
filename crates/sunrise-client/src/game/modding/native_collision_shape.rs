//! The `collision_shape` Part method (vanilla's `getCollisionShape` with its `CollisionContext`).
//! The client library asks through `native_client_hooks` from inside a move; only a state whose
//! block has a subscribing part is converted, and a part that faults loses only its answer.
use super::{
    Game,
    api_values::{TryIntoApi, block_state},
};
use std::{cell::RefCell, rc::Rc};
use sunrise_core::{geometry::VoxelShape, position::BlockPos};
use sunrise_world::{native_load_context::LoopContext, registry::BlockStateId};

thread_local! {
    /// The parts of the level being ticked, kept between ticks so their answers' storage and the
    /// names they share are not built again each tick.
    static PARTS: RefCell<Option<Rc<sunrise_client::collision_parts::ModParts>>> =
        const { RefCell::new(None) };
}

/// Dispatches through the shared Part dispatcher, which restores only the failed
/// invocation's output on unwind. Native callers receive the already-built shape.
pub(crate) fn collision_shape(
    context: &LoopContext,
    pos: BlockPos,
    state: BlockStateId,
    query: &mod_api::CollisionQuery,
    shape: &mut Option<VoxelShape>,
) {
    if !block_state::block_listens(mod_api::HookId::CollisionShape, state) {
        return;
    }
    let Ok(api_state) = state.try_into_api() else {
        return;
    };
    let mut answer = None;
    mod_api::dispatch_collision_shape(
        &sunrise_client_render::api_context::LoopReader(context).context(),
        pos,
        api_state,
        query,
        &mut answer,
    );
    *shape = answer.map(mod_api::__private::voxel_shape_into_core);
}

impl Game<'_> {
    /// Marks the states whose parts answer `collision_shape` in the collision tables, so scans
    /// keep them in the cells around their own. Between ticks, after the parts are resolved.
    pub(in crate::game) fn mark_collision_shape_states(&self) {
        let registry = self.collision.registry();
        self.collision.set_part_states(
            (0..registry.len())
                .map(|id| BlockStateId(id as u32))
                .filter(|state| {
                    block_state::block_listens(mod_api::HookId::CollisionShape, *state)
                }),
        );
    }

    /// The parts that answer this tick's collision queries, until the guard drops: none when no
    /// mod has a part that answers them, so the tick pays nothing.
    pub(in crate::game) fn collision_parts(
        &self,
    ) -> Option<sunrise_world::part_collision::InParts> {
        mod_api::__private::listened(mod_api::HookId::CollisionShape).then(|| {
            let parts = PARTS.with_borrow_mut(|slot| {
                let parts = slot.get_or_insert_with(|| {
                    Rc::new(sunrise_client::collision_parts::ModParts::new(
                        self.loop_context.clone(),
                    ))
                });
                parts.begin_tick(&self.loop_context);
                Rc::clone(parts)
            });
            sunrise_world::part_collision::InParts::enter(parts)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use sunrise_world::native_load_context::{DataLoadOrigin, LoadContext, LoadInputs, LoopSide};

    #[derive(Clone, Copy)]
    enum Mode {
        Answer,
        Panic,
        Refuse,
        Observe,
    }
    struct Part(Mode, Arc<AtomicUsize>);
    #[mod_api::sunrise_part]
    impl mod_api::BlockPart for Part {
        fn collision_shape(
            &self,
            _: &dyn mod_api::LoopContext,
            event: &mut mod_api::CollisionShapeEvent<'_>,
        ) {
            self.1.fetch_add(1, Ordering::SeqCst);
            let half = [[0.0, 0.0, 0.0, 1.0, 0.5, 1.0]];
            match self.0 {
                Mode::Answer => {
                    *event.shape = Some(mod_api::VoxelShape::try_from_boxes(&half).unwrap())
                }
                Mode::Panic => {
                    *event.shape = Some(mod_api::VoxelShape::empty());
                    panic!("a part that faults");
                }
                Mode::Refuse => {
                    if let Ok(shape) = mod_api::VoxelShape::try_from_boxes(&[[f64::NAN; 6]]) {
                        *event.shape = Some(shape);
                    }
                }
                Mode::Observe => {
                    assert!(event.shape.as_ref().unwrap().boxes().eq(half));
                    *event.shape = Some(mod_api::VoxelShape::empty());
                }
            }
        }
    }

    #[test]
    fn a_failed_shape_answer_does_not_erase_other_parts_on_the_same_block() {
        let calls: Vec<_> = (0..4).map(|_| Arc::new(AtomicUsize::new(0))).collect();
        struct Listener(Mode, Arc<AtomicUsize>);
        #[mod_api::__subscriptions]
        impl mod_api::Hooks for Listener {
            fn register(
                &self,
                registrar: &mut dyn mod_api::Registrar,
                _: &mod_api::RegistrationContext,
            ) {
                let key = registrar.block_key("minecraft:stone");
                registrar.block_part(key, Part(self.0, self.1.clone()));
            }
        }
        let mods: mod_api::Mods = [
            ("shape_a", Mode::Answer),
            ("shape_b", Mode::Panic),
            ("shape_c", Mode::Refuse),
            ("shape_d", Mode::Observe),
        ]
        .into_iter()
        .zip(&calls)
        .map(|((id, mode), calls)| {
            mod_api::ModEntry::new(id, Box::new(Listener(mode, calls.clone())))
        })
        .collect::<Vec<_>>()
        .into();
        mod_platform::testing::with_mods(mods.clone(), || {
            mod_platform::__private::part_tests::install_parts_fixture(&mods).unwrap();
            let facts = r#"{"defaults":{"minecraft:air":3,"minecraft:stone":0,"minecraft:dirt":1,"minecraft:gravel":2},"states":[["s",false,false,"","minecraft:stone"],["d",false,false,"","minecraft:dirt"],["g",false,false,"","minecraft:gravel"],["a",true,false,"","minecraft:air"]]}"#;
            let registry =
                sunrise_world::registry::BlockRegistry::from_facts_json(facts.as_bytes()).unwrap();
            crate::game::api_values::resolve_block_definitions(&registry).unwrap();
            mod_api::__private::resolve_parts(
                &Default::default(),
                &Default::default(),
                mod_api::__private::KnownIds {
                    block: &|_| true,
                    item: &|_| false,
                },
            )
            .unwrap();
            let inputs = Arc::new(LoadInputs::new(Default::default(), std::env::temp_dir()));
            let context = LoadContext::new(DataLoadOrigin::ServerDataLoad, inputs, [])
                .loop_context(LoopSide::Server, [] as [&str; 0]);
            let ask = |state: u32| {
                let mut shape = None;
                collision_shape(
                    &context,
                    BlockPos::new(1, 64, 1),
                    BlockStateId(state),
                    &mod_api::CollisionQuery::default(),
                    &mut shape,
                );
                shape
            };
            for _ in 0..2 {
                assert!(ask(0).unwrap().is_empty());
            }
            for count in calls {
                assert_eq!(count.load(Ordering::SeqCst), 2);
            }
        });
    }
}
