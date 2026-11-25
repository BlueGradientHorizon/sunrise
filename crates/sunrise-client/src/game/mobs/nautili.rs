//! Nautili on the game side (`AbstractNautilus.mobInteract`): taming with
//! pufferfish, feeding, the saddle and armour, riding, and the zombie
//! nautilus a drowned with a trident sometimes rides.
#[allow(unused_imports)]
use super::Game;
#[allow(unused_imports)]
use sunrise_client::{items::ItemStack, mobs::nautilus};
#[allow(unused_imports)]
use sunrise_core::random::RandomSource;

#[cfg(test)]
#[path = "tests/nautili_tests.rs"]
pub(in crate::game) mod nautili_tests;
