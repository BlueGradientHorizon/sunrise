#[cfg(feature = "client")]
use crate::game::client_side::social::Social;
use std::collections::{HashMap, HashSet};
use sunrise_client::{social_screen::Page, ui::EditBox};

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
