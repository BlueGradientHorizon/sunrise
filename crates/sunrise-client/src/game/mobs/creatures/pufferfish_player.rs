//! Pufferfish.playerTouch in the real player's tick, independent of fish AI.

/// `ClientboundGameEventPacket.PUFFER_FISH_STING`.
pub(in crate::game) const PUFFER_FISH_STING: u8 = 9;

#[cfg(test)]
#[path = "pufferfish_sting_tests.rs"]
pub(in crate::game) mod pufferfish_sting_tests;
