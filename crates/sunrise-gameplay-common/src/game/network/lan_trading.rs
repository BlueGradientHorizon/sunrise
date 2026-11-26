

#[cfg(test)]
pub(crate) mod tests {

use super::*;

use std::sync::{
        Arc,
        atomic::{AtomicI32, AtomicUsize},
    };

/// What the `trade_select` observer saw: the villager and offer index it
    /// was told, and what the merchant handle answered inside the hook.
    type Seen = Arc<std::sync::Mutex<Vec<(Option<u32>, usize, bool, bool)>>>;
}
