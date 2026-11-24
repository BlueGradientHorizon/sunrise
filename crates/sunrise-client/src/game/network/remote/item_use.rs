//! Predicting item-on-block interactions on the client level, out of remote.rs,
//! which may not grow.
#[allow(unused_imports)]
use super::super::Game;
#[allow(unused_imports)]
use sunrise_client::first_person::Hand;
#[allow(unused_imports)]
use sunrise_client::items::ItemStack;

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    use crate::game::{
        Audio, Game, Hit, Options, Progress, Settings, execution_role::ExecutionRole,
        load_resources,
    };
    #[allow(unused_imports)]
    use sunrise_client::first_person::Hand;
    #[allow(unused_imports)]
    use sunrise_client::items::ItemStack;
}
