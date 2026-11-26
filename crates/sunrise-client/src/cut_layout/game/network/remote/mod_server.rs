

#[cfg(test)]
#[path = "interaction_event_tests_server.rs"]
pub(in crate::game) mod interaction_event_tests;

#[cfg(test)]
#[path = "passage_cross_tests_server.rs"]
pub(in crate::game) mod passage_cross_tests;

#[cfg(test)]
#[path = "riptide_tests_server.rs"]
pub(in crate::game) mod riptide_tests;

#[cfg(test)]
pub(in crate::game) mod tests {
    pub(in crate::game::server_side) use crate::game::remote::tests::connect_menu_test;
    use sunrise_items::items::ItemStack;

    pub(in crate::game) mod click_ending_tests {
        include!("click_ending_tests_server.rs");
    }

    pub(in crate::game) mod dig_abort_tests {
        include!("dig_abort_tests_server.rs");
    }
}

#[cfg(test)]
#[path = "movement_input_tests_server.rs"]
pub(in crate::game) mod movement_input_tests;

#[cfg(test)]
#[path = "placement_prediction_tests_server.rs"]
pub(in crate::game) mod placement_prediction_tests;
#[path = "recipe_properties_server.rs"]
pub(crate) mod recipe_properties;

#[path = "general_server.rs"]
pub(crate) mod general;
