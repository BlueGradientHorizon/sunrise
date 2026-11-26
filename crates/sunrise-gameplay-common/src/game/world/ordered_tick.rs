

#[cfg(test)]
pub(crate) mod tests {

use sunrise_sim::join_seq::{JoinList, JoinSeq, Joined as _};

/// A listener's calls, in order: the phase and the entities told.
    type Heard = std::sync::Arc<std::sync::Mutex<Vec<(&'static str, Vec<u32>)>>>;
#[mod_api::__subscriptions]
    impl mod_api::Hooks for Displays {
        fn entity_tick_listened(
            &self,
            _: mod_api::HostSide<'_>,
            event: &mut mod_api::EntityTickListenedEvent,
        ) {
            event.listened = true;
        }
        fn entity_tick_pre(
            &self,
            _: mod_api::HostSide<'_>,
            event: &mut mod_api::EntityTickPreEvent<'_>,
        ) {
            let entities = event.entities.to_vec();
            self.heard.lock().unwrap().push(("pre", entities));
        }
        fn entity_tick_post(
            &self,
            _: mod_api::HostSide<'_>,
            event: &mut mod_api::EntityTickPostEvent<'_>,
        ) {
            let entities = event.entities.to_vec();
            self.heard.lock().unwrap().push(("post", entities));
        }
        fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
            for kind in self.named {
                r.entity_tick_each(kind);
            }
        }
    }
}
