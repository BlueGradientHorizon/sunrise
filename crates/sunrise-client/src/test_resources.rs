#[allow(
    unused_imports,
    reason = "standalone tests and probes that include this file may only use prepared_root"
)]
pub use sunrise_sim::test_resources::*;

#[path = "../../sunrise-core/src/prepared_test_root.rs"]
mod prepared_test_root;

#[allow(
    unused_imports,
    reason = "standalone probes and tests use subsets of the prepared-root boundary"
)]
pub(crate) use prepared_test_root::{
    SERVER_INPUTS, STREAMING_INPUTS, WORLDGEN_INPUTS, prepared_root, require_current_stamp,
    require_inputs, stamp_problems, unavailable_inputs,
};
