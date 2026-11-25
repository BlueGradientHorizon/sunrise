#[allow(unused_imports)]
use super::*;

#[cfg(test)]
#[path = "tests/pickup_post_tests.rs"]
pub(in crate::game) mod pickup_post_tests;

#[cfg(test)]
#[path = "tests/orb_merge_contract_tests.rs"]
pub(in crate::game) mod orb_merge_contract_tests;
