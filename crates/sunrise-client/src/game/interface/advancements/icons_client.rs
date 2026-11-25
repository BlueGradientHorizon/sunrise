use super::{Game, ItemStack};
use sunrise_protocol::item::Stack;

impl Game<'_, crate::game::client_side::ClientState> {
    /// `DisplayInfo.fromNetwork`: convert the template once, not while drawing it.
    pub(in crate::game) fn remote_advancement_icon(&self, mut icon: Stack) -> ItemStack {
        let remote = self.remote.as_ref().expect("remote advancement");
        if !sunrise_protocol::item::custom::permitted_with_names(
            &icon,
            remote.registries.custom_components,
            &*remote.registries,
        )
        .unwrap_or(false)
        {
            // Strip the full patch on refused components so no unnegotiated component reaches rendering.
            log::warn!("Drawing advancement icon without its refused component patch");
            icon.added.clear();
            icon.removed.clear();
            icon.wire = Default::default();
        }
        sunrise_client::item_sync::from_network(&icon, &remote.builtin, &remote.registries)
    }
}

#[cfg(test)]
#[path = "icons/tests_client.rs"]
pub(in crate::game) mod tests;
