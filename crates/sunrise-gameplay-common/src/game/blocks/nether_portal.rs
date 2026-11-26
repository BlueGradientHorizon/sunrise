

#[cfg(test)]
pub(crate) mod tests {

use super::*;

use std::sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    };

#[mod_api::__subscriptions]
    impl mod_api::Hooks for Gate {
        fn entity_mount(
            &self,
            host: &mut dyn mod_api::ServerHost,
            e: &mut mod_api::EntityMountEvent<'_>,
        ) -> mod_api::Flow {
            if self.spare.load(Ordering::Relaxed) == 1004
                && !e.mounting
                && e.entity == 102
                && let Some(sibling) = host.entity(103)
            {
                assert!(sibling.discard(host));
            }
            mod_api::Flow::Continue
        }

        fn entity_travel_to_dimension(
            &self,
            host: &mut dyn mod_api::ServerHost,
            e: &mut mod_api::EntityTravelToDimensionEvent,
        ) -> mod_api::Flow {
            self.heard.lock().unwrap().push((
                e.entity.id(),
                e.dimension.to_string(),
                e.cause.to_string(),
            ));
            let attack = self.spare.load(Ordering::Relaxed);
            if attack == 1001
                && e.entity.id() == 102
                && self
                    .heard
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|e| e.0 == 102)
                    .count()
                    == 1
            {
                let root = mod_api::EntityRef::new(e.entity.epoch(), 101);
                assert!(root.start_riding(host, e.entity, true).is_ok());
            }
            if attack == 1002 && e.entity.id() == 102 {
                assert!(mod_api::EntityRef::new(e.entity.epoch(), 103).discard(host));
            }
            if self.discard {
                assert!(e.entity.discard(host));
                return mod_api::Flow::Continue;
            }
            if e.entity.id() == self.spare.load(Ordering::Relaxed) {
                mod_api::Flow::Cancel
            } else {
                mod_api::Flow::Continue
            }
        }
    }
}
