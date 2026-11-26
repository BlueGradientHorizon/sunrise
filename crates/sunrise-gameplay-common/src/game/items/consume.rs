

#[cfg(test)]
pub(crate) mod teleport_hook_tests {

use super::*;

use mod_api::{Flow, Hooks, ItemConsumptionTeleportEvent, ModEntry, ServerHost};

use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };

struct ConsumptionProbe(Arc<Mutex<Vec<mod_api::ItemStack>>>, Arc<AtomicBool>);
#[mod_api::__subscriptions]
    impl Hooks for ConsumptionProbe {
        fn item_consumption_teleport(
            &self,
            host: &mut dyn ServerHost,
            event: &mut ItemConsumptionTeleportEvent<'_>,
        ) -> Flow {
            assert_eq!(
                event.consumer.entity(),
                host.players().acting_player().unwrap().entity()
            );
            self.0.lock().unwrap().push(event.consumed.clone());
            assert_eq!(event.consumed.count(), 7);
            assert_eq!(event.consumed.damage(), 3);
            assert!(event.consumed.components().is_some());
            if self.1.load(Ordering::SeqCst) {
                assert!(
                    event
                        .consumer
                        .entity()
                        .teleport_to(host, [3.0, 70.0, 3.0], None)
                );
                return Flow::Continue;
            }
            Flow::Cancel
        }
    }
}
