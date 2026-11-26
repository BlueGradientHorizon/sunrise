

#[cfg(test)]
#[cfg(feature = "client")]
pub(crate) mod tests {

use super::*;

use mod_api::{self, ServerHost};

use std::sync::Arc;

/// Spares the entity whose id it holds, and logs each (entity, bolt) asked.
    pub(in crate::game) struct Spare(
        pub(in crate::game) u32,
        pub(in crate::game) Arc<std::sync::Mutex<Vec<(u32, u32)>>>,
    );
#[mod_api::__subscriptions]
    impl mod_api::Hooks for Spare {
        fn entity_struck_by_lightning(
            &self,
            _: &mut dyn ServerHost,
            e: &mut mod_api::EntityStruckByLightningEvent,
        ) -> mod_api::Flow {
            self.1.lock().unwrap().push((e.entity.id(), e.bolt.id()));
            if e.entity.id() == self.0 {
                mod_api::Flow::Cancel
            } else {
                mod_api::Flow::Continue
            }
        }
    }
}
