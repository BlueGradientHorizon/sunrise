//! Server-side mod entity-selector filtering (`@allies`, NeoForge's `EntitySelectorManager`).
use super::Game;
use super::commands::Target;
use sunrise_command::args::Selector;

impl Game<'_, crate::game::server_side::ServerState> {
    /// `select_in_level`, then, for a mod's token, only the candidates its type keeps.
    pub(in crate::game) fn native_selector_kept(
        &mut self,
        s: &Selector,
        candidates: Vec<Target>,
        origin: [f64; 3],
    ) -> Vec<(Target, f64)> {
        let selected = self.select_in_level(candidates, s, origin);
        let Some(token) = &s.token else {
            return selected;
        };
        let Some(selector) = mod_api::__private::entity_selector_type(token) else {
            return Vec::new();
        };
        let Ok(source) = self.native_command_origin(self.ui.registries.permission) else {
            return Vec::new();
        };
        selected
            .into_iter()
            .filter(|(target, _)| {
                let Some(entity) = self.native_command_entity_target(*target) else {
                    return false;
                };
                let host: &mut dyn mod_api::ServerHost = self;
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    selector.matches(host, &source, entity)
                }))
                .unwrap_or(false)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::SelectorSort;
    use std::sync::{Arc, Mutex};
    use sunrise_core::geometry::Vec3;

    struct Teammates(Arc<Mutex<Vec<(mod_api::CommandSource, u32)>>>);
    impl mod_api::EntitySelectorType for Teammates {
        fn base(&self) -> mod_api::SelectorBase {
            mod_api::SelectorBase::new(false, Some(1), SelectorSort::Nearest, "test.allies")
        }
        fn matches(
            &self,
            _: &mut dyn mod_api::ServerHost,
            source: &mod_api::CommandSource,
            entity: mod_api::EntityRef,
        ) -> bool {
            self.0.lock().unwrap().push((source.clone(), entity.id()));
            match entity.id() {
                112 | 115 => true,
                113 => panic!("fixture predicate failure"),
                _ => false,
            }
        }
    }
    struct TeamMod(Arc<Mutex<Vec<(mod_api::CommandSource, u32)>>>);
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for TeamMod {
        fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
            r.register_entity_selector("allies", Teammates(self.0.clone()));
        }
    }

    #[test]
    #[ignore = "daily"]
    fn nearest_teammate_filters_before_limit_and_drops_false_and_panicking_candidates() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new(
            "selector_fixture",
            Box::new(TeamMod(calls.clone())),
        )]
        .into();
        crate::game::api_values::install_parts(&mods).unwrap();
        mod_platform::testing::with_mods(mods, || {
            crate::game::native_guest_turn_tests::with_guests(|game| {
                game.build_registries();
                game.as_guest(12, |game| {
                    let kind = game.res.mob_types().get("minecraft:pig").unwrap().clone();
                    for (id, x) in [(111, 1.0), (112, 2.0), (113, 1.5), (114, 0.5), (115, 3.0)] {
                        game.mobs.push(sunrise_sim::mobs::Mob::new(
                            id,
                            kind.clone(),
                            Vec3::new(x, 0.0, 0.0),
                            0.0,
                        ));
                    }
                    let selector = Selector {
                        kind: 'e',
                        include_entities: true,
                        token: Some("allies".into()),
                        limit: 1,
                        sort: sunrise_command::args::Sort::Nearest,
                        pos: [Some(0.0); 3],
                        ..Default::default()
                    };
                    assert_eq!(game.select(&selector), [Target::Mob(112)]);
                    let calls = calls.lock().unwrap();
                    for id in [111, 112, 113, 114, 115] {
                        let (source, _) = calls
                            .iter()
                            .find(|(_, candidate)| *candidate == id)
                            .expect("every mob must be filtered before limit");
                        let mod_api::CommandSource::Player { player, .. } = source else {
                            panic!("expected the acting player source");
                        };
                        assert_eq!(player.uuid(), 12);
                        assert_eq!(player.entity().id(), 12);
                    }
                });
            });
        });
    }
}
