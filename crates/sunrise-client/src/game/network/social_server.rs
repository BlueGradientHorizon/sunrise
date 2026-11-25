#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Game, Screen};
#[cfg(feature = "client")]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::social::Social;
#[cfg(feature = "client")]
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::social::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::collections::{HashMap, HashSet};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    social_screen::{self, Action, Entry, Page, SocialView},
    tab_list::TabPlayer,
    text::Text,
    ui::EditBox,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use winit::keyboard::KeyCode;

#[cfg(feature = "client")]
impl Default for Social {
    fn default() -> Self {
        let mut search = EditBox::new(0.0, 0.0, 200.0, 15.0);
        search.max_length = 16;
        Self {
            hidden: HashSet::new(),
            names: HashMap::new(),
            page: Page::All,
            search,
            tooltip: Default::default(),
            scroll: 0.0,
            from_pause: false,
        }
    }
}
