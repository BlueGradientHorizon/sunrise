#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cauldron_interactions::Callback;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cauldron_interactions::Edit;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cauldron_interactions::Registrar;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cauldron_interactions::TABLE;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cauldron_interactions::Table;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cauldron_interactions::clear;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cauldron_interactions::install;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cauldron_interactions::snapshot;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::cauldron_interactions::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{
    __private::{
        RetainedCallback,
        game::{RegistrationContextNew, discard_registration},
    },
    HookId,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::{cell::RefCell, collections::BTreeSet, sync::Arc};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::cauldron::interactions::Dispatcher;

#[cfg(test)]
#[path = "cauldron_interactions/tests_client.rs"]
pub(in crate::game) mod tests;
