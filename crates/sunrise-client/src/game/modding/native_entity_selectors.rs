//! Mods' selector tokens (`@allies`, NeoForge's `EntitySelectorManager`): the frozen base
//! selections the command registries parse them with.
use mod_api::SelectorSort;
use sunrise_client_core::registries::SelectorToken;

/// The frozen base selections validated when the installed mods registered their tokens.
pub(in crate::game) fn selector_tokens() -> Vec<SelectorToken> {
    mod_api::__private::entity_selector_types()
        .map(|(name, base)| SelectorToken {
            name: name.to_owned(),
            tooltip: base.tooltip.clone(),
            players_only: base.players_only,
            limit: base.limit.map_or(usize::MAX, |limit| limit as usize),
            sort: match base.sort {
                SelectorSort::Nearest => "nearest",
                SelectorSort::Furthest => "furthest",
                SelectorSort::Random => "random",
                _ => "arbitrary",
            },
        })
        .collect()
}
