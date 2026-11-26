

#[cfg(test)]
pub(in crate::game) mod tests {
    use super::*;
    use sunrise_gameplay::game::mods::tests::Fixture;
    use sunrise_gameplay::game::mods::tests::smod;

    pub(in crate::game) mod inventory {
        include!("mod_inventory_tests_server.rs");
    }
}