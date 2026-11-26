

#[cfg(test)]
pub(crate) mod tests {

use super::*;

#[derive(Debug, PartialEq, Eq)]
    enum CheckpointEvent {
        LevelSaved,
        WorldSaved,
        LevelUnloaded,
        Closed(mod_api::WorldPhase, String),
    }
struct CheckpointMod(Arc<Mutex<Vec<CheckpointEvent>>>);
#[mod_api::__subscriptions]
    impl mod_api::Hooks for CheckpointMod {
        fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
            let kind = r
                .block_entity_type("counter")
                .blocks(&["crash_checkpoint:counter"])
                .build();
            r.block_entity_part(&kind, CheckpointCounter).unwrap();
        }

        fn blocks(&self, _: &mod_api::RegistrationContext, event: &mut mod_api::BlocksEvent<'_>) {
            event
                .register(
                    mod_api::Block::new("crash_checkpoint:counter", "minecraft:barrel")
                        .with_item(false),
                )
                .unwrap();
        }

        fn level_save(
            &self,
            _: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::LevelSaveEvent<'_>,
        ) {
            if event.dimension == Dim::Overworld.id() {
                self.0.lock().unwrap().push(CheckpointEvent::LevelSaved);
            }
        }

        fn world_saved(&self, _: &mut dyn mod_api::ServerHost, _: &mut mod_api::WorldSavedEvent) {
            self.0.lock().unwrap().push(CheckpointEvent::WorldSaved);
        }

        fn level_unload(
            &self,
            _: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::LevelUnloadEvent<'_>,
        ) {
            if event.dimension == Dim::Overworld.id() {
                self.0.lock().unwrap().push(CheckpointEvent::LevelUnloaded);
            }
        }

        fn world_lifecycle(
            &self,
            _: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::WorldLifecycleEvent<'_>,
        ) {
            use mod_api::WorldPhase::{WorldCloseFailed, WorldCloseFinal};
            if matches!(event.phase, WorldCloseFailed | WorldCloseFinal) {
                self.0.lock().unwrap().push(CheckpointEvent::Closed(
                    event.phase,
                    event.error_text.to_owned(),
                ));
            }
        }
    }
struct CheckpointCounter;
#[mod_api::sunrise_part]
    impl mod_api::BlockPart for CheckpointCounter {}
#[mod_api::sunrise_part]
    impl mod_api::BlockEntityPart for CheckpointCounter {
        fn block_entity_save_additional(
            &self,
            host: &dyn mod_api::LevelHost,
            event: &mut mod_api::BlockEntitySaveAdditionalEvent<'_>,
        ) {
            let entity = host
                .block_entity(event.pos)
                .expect("counter is still resident");
            let data = host
                .block_entity_data(entity)
                .expect("counter handle is live");
            event.tag.insert(
                "saved_progress".into(),
                data.get(&"progress".into()).unwrap().clone(),
            );
        }
    }
}
