#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{Arg, Context, Game, R, Target, err};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::extra::CommandExtras;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::extra::ProfileFetch;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::extra::civil;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::extra::encode_profile;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::extra::filename_date_time;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::extra::num;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::extra::tr_error;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::extra::two_places;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::extra::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::commands::target_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::{collections::HashMap, time::Instant};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::actions::Action;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    command::CommandError,
    lan::waypoints::Icon,
    skins::{NameOrId, Resolve, ResolvedProfile},
    snbt,
    text::{self, ClickEvent, InlineObject, Style, Text},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::nbt::{JavaString, Tag};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::version::{
    BUILD_TIME, DATA_VERSION, STABLE, VANILLA_VERSION, VERSION_ID,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::packets::{
    Empty,
    play::{self, Clientbound as C, DialogHolder},
};

impl Game<'_> {
    #[inline(always)]
    pub(in crate::game) fn profile_fetches_pending_client(&self, fetches: &[ProfileFetch]) -> bool {
        let mut resolver = self.skin_profiles.0.borrow_mut();
        fetches
            .iter()
            .any(|f| matches!(resolver.state(&f.key), Resolve::Pending))
    }
    #[inline(always)]
    pub(in crate::game) fn tick_profile_fetches_client(&self, key: &NameOrId) -> Resolve {
        self.skin_profiles.0.borrow_mut().state(key)
    }
}
