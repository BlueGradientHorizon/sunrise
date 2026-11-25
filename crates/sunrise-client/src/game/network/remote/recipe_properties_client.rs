#[allow(unused_imports)]
pub(in crate::game::client_side) use super::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::remote::recipe_properties::*;

impl Game<'_> {
    /// Minecraft.disconnect's `ClientPlayerNetworkEvent.LoggingOut`: on
    /// leaving a world, and on quitting the game from one. Once per
    /// connection, whichever way out runs first.
    pub(in crate::game) fn client_logging_out(&mut self) {
        let Some(remote) = self.remote.as_mut().filter(|r| !r.logged_out) else {
            return;
        };
        remote.logged_out = true;
        remote.camera = None;
        remote.detached_camera = None;
        let player = self
            .player
            .as_ref()
            .map(|_| (remote.entity_id, remote.profile));
        self.clear_spectated_entity_post_effect();
        self.reset_client_item_use();
        // Camera.reset clears initialization while keeping the last aligned position.
        self.client.view.initialized = false;
        self.native_client_player_logging_out(player.map(|p| p.0), player.map(|p| p.1));
        if self.mod_screen.0.is_some() {
            self.remote_close_local();
        }
        self.native_lifecycle.begin_close();
        self.native_lifecycle_world_closed();
        self.recipe_tables.server_properties = Default::default();
        self.recipe_tables.server_stations = None;
        self.refresh_recipe_property_context();
    }
    pub(in crate::game::client_side) fn refresh_remote_stonecutter(&mut self) {
        self.recipe_tables.server_stations = self.remote.as_ref().map(|remote| {
            let item = |id: i32| {
                remote
                    .builtin
                    .get("minecraft:item")?
                    .name(id)
                    .map(str::to_owned)
            };
            let category = |_: i32| None;
            let tag = |name: &str| {
                let name = sunrise_client::crafting::qualified(name);
                remote
                    .registries
                    .tag("minecraft:item", &name)
                    .into_iter()
                    .flatten()
                    .filter_map(|&id| item(id))
                    .collect()
            };
            let names = sunrise_client::recipe_book::PacketNames {
                item: &item,
                category: &category,
                tag: &tag,
            };
            self.stations().recipes.with_stonecutting(
                names.stonecutting(&self.recipe_tables.server_properties.stonecutter, |stack| {
                    self.decode_remote_stack(stack)
                }),
            )
        });
        if let Some(station) = self
            .station
            .as_mut()
            .filter(|station| station.kind == crate::game::stations::Kind::Stonecutter)
        {
            // The table and the menu's cached indices are one generation. Rebuild even
            // when the input stack has not changed since the previous packet.
            station.reset_stonecutter_generation();
            self.remote_station_slot_changed(crate::game::stations::INPUT);
        }
    }
    pub(in crate::game::client_side) fn remote_recipe_properties(
        &mut self,
        packet: sunrise_protocol::packets::recipes::UpdateRecipes,
    ) {
        self.recipe_tables.server_properties.replace(packet);
        self.refresh_remote_stonecutter();
        self.refresh_recipe_property_context();
    }
}
