//! The profiles service: account profiles through the server's `ProfileResolver.Cached`, the
//! resolver `/fetchprofile` and dynamic player heads use.
use super::Game;
use mod_api::{AccountProfile, AccountProfileProperty, ProfileResolution};
use sunrise_client::skins::{NameOrId, Resolve};

impl Game<'_> {
    /// Where the lookup of `key` stands, starting it the first time.
    pub(in crate::game) fn native_resolve_profile(&self, key: NameOrId) -> ProfileResolution {
        match self.skin_profiles.borrow_mut().state(&key) {
            Resolve::Pending => ProfileResolution::Pending,
            Resolve::Done(profile) => ProfileResolution::Done(profile.map(|profile| {
                AccountProfile {
                    uuid: profile.id,
                    name: profile.name,
                    properties: profile
                        .properties
                        .into_iter()
                        .map(|(name, value, signature)| AccountProfileProperty {
                            name,
                            value,
                            signature,
                        })
                        .collect(),
                }
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::game::{
        execution_role::ExecutionRole,
        native_entity_refs::tests::{run_fresh_cases, with_game},
    };
    use mod_api::{ProfileResolution, ServerHost};

    /// A name `StringUtil.isValidPlayerName` refuses answers at once with no profile, as
    /// `fetchByName` does without a request.
    #[test]
    #[ignore = "daily"]
    fn invalid_names_resolve_no_profile() {
        if run_fresh_cases(
            "game::server_side::native_profiles::tests::invalid_names_resolve_no_profile",
            "SUNRISE_PROFILES_CASE",
            &["server"],
        ) {
            return;
        }
        with_game(|game| {
            game.options.execution_role = ExecutionRole::IntegratedServer;
            for name in ["two words", "seventeen_letters", "ünicode"] {
                let host: &mut dyn ServerHost = game;
                assert_eq!(
                    host.profiles().resolve_name(name),
                    ProfileResolution::Done(None),
                    "{name}"
                );
            }
        });
    }
}
