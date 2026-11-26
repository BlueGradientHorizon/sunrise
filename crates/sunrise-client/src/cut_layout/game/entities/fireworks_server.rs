

#[cfg(test)]
#[path = "fireworks/hit_attempt_tests_server.rs"]
pub(in crate::game) mod hit_attempt_tests;

#[cfg(test)]
#[path = "tests/fireworks_events_tests_client.rs"]
pub(in crate::game) mod events_tests;
#[path = "fireworks/impact_server.rs"]
pub(crate) mod impact;
