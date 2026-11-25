#[allow(unused_imports)]
pub(in crate::game::client_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::client_side) use super::api_values::{
    ItemBoundary, TryIntoApi, TryIntoEnginePreservingCount,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::host::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::__private::game::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::*;

#[path = "draw_client.rs"]
pub(in crate::game) mod draw;

#[path = "services_client.rs"]
pub(in crate::game) mod services;
