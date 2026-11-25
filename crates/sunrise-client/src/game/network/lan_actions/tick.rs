#[allow(unused_imports)]
use super::*;

impl<__Side: crate::game::Side> Game<'_, __Side> {
    
}

#[cfg(test)]
impl crate::game::ServerGame<'_> {
    #[cfg(test)]
    pub(in crate::game) fn tick_guests(&mut self) {
        building_cache(self.lan.as_ref());
        self.tick_guests_inner();
        building_cache(None);
    }
}

#[cfg(test)]
#[path = "tests/building_cache.rs"]
pub(in crate::game) mod building_cache_tests;

#[cfg(test)]
#[path = "tests/dig_tick_order_tests.rs"]
pub(in crate::game) mod dig_tick_order_tests;

#[cfg(test)]
#[path = "tests/fall_sound_tests.rs"]
pub(in crate::game) mod fall_sound_tests;

#[cfg(test)]
#[path = "tests/spectator_body_tick_tests.rs"]
pub(in crate::game) mod spectator_body_tick_tests;

#[cfg(test)]
#[path = "tests/equipment_tick_tests.rs"]
pub(in crate::game) mod equipment_tick_tests;
