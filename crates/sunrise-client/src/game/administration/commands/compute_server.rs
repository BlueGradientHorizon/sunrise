#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{Arg, Context, Game, R, Target, err};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::compute::Arith;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::compute::ComputeCondition;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::compute::ComputeContext;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::compute::context_entities;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::compute::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::commands::target_entity_mut;
#[allow(unused_imports)]
pub(in crate::game::server_side) use serde_json::Value;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::borrow::Cow;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::providers::{ConditionContext, Evaluation};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    advancements::{self as adv, EntityView},
    command::CommandError,
    text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::java_number::float_to_string;

impl Game<'_, crate::game::server_side::ServerState> {
    /// Leaf predicates that read the command's world or entity context.
    pub(in crate::game::server_side) fn compute_leaf(
        &mut self,
        cond: &Value,
        c: &ComputeContext,
    ) -> bool {
        let Some(o) = cond.as_object() else {
            return false;
        };
        let kind = o
            .get("type")
            .or_else(|| o.get("condition"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim_start_matches("minecraft:");
        match kind {
            "match_block" | "block_state_property" => {
                let Some(p) = c.block else {
                    return false;
                };
                let state = self.world.block(p[0], p[1], p[2]);
                let Some(s) = self.pack.assets.registry.state(state) else {
                    return false;
                };
                let name = s.name.to_string();
                let blocks_ok = match o.get("blocks").or_else(|| o.get("block")) {
                    Some(Value::String(b)) => match b.strip_prefix('#') {
                        Some(tag) => self
                            .res
                            .items()
                            .block_tags
                            .get(tag.trim_start_matches("minecraft:"))
                            .is_some_and(|set| {
                                set.contains(&*name)
                                    || set.contains(name.trim_start_matches("minecraft:"))
                            }),
                        None => *b == name,
                    },
                    Some(Value::Array(list)) => list.iter().any(|b| b.as_str() == Some(&name)),
                    _ => true,
                };
                let props_ok = o
                    .get("state")
                    .or_else(|| o.get("properties"))
                    .and_then(Value::as_object)
                    .into_iter()
                    .flatten()
                    .all(|(k, v)| s.properties.get(k).map(String::as_str) == v.as_str());
                blocks_ok && props_ok
            }
            _ => {
                let source = self
                    .side
                    .command_source
                    .unwrap_or_else(|| self.player_source());
                // Scores are read from the holder's board only where
                // an `entity_scores` leaf may ask.
                let scored = adv::asks_scores(cond);
                let view = |game: &mut Self, t| {
                    if scored {
                        game.entity_view_scored(t, cond)
                    } else {
                        game.entity_view_asked(t, cond)
                    }
                };
                let this = source
                    .entity
                    .filter(|_| c.without_this.is_none())
                    .map(|t| view(self, t));
                let target = c.target.map(|t| view(self, t));
                let interacting = c.interacting.map(|t| view(self, t));
                let entities = context_entities(
                    c,
                    this.as_ref(),
                    source.pos,
                    target.as_ref(),
                    interacting.as_ref(),
                );
                self.conditions_hold_in(cond, &entities)
            }
        }
    }
    /// A registered provider's definition.
    pub(in crate::game::server_side) fn provider_file(
        &self,
        registry: &str,
        id: &str,
    ) -> Option<Value> {
        let id = sunrise_client::command::args::normalized(id);
        match registry {
            "minecraft:predicate" => self.side.fns.0.store.predicates.get(id.as_ref()),
            _ => self.context_registry(registry)?.get(id.as_ref()),
        }
        .cloned()
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::commands::compute::tests::Void;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::commands::compute::tests::holds;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::commands::compute::tests::of_type;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::commands::compute::tests::view;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::commands::compute::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::commands::target_entity_mut;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use serde_json::json;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use std::collections::BTreeMap;

    #[test]
    #[ignore = "daily"]
    fn published_predicates_and_nested_provider_shorthand_evaluate() {
        use crate::game::*;
        use serde_json::json;
        use std::{path::PathBuf, sync::Arc, time::Instant};
        use sunrise_assets::resource_source::{ResourceRoot, ResourceSource};

        let root = PathBuf::from(
            std::env::var_os("SUNRISE_TEST_RESOURCES")
                .expect("set SUNRISE_TEST_RESOURCES to prepared resources"),
        );
        let settings = Settings::default();
        let mut loaded = load_resources_uncached(
            ResourceRoot::directory(root.clone()),
            Vec::new(),
            None,
            &root,
            &settings,
            Instant::now(),
            false,
            execution_role::ExecutionRole::IntegratedServer,
            &Progress::new(),
            &mut |_, _| unreachable!(),
        )
        .unwrap();
        let fixture = std::env::current_dir()
            .unwrap()
            .join("compute-owner-providers");
        for (registry, value) in [
            ("context_int_provider", json!(17)),
            ("context_float_provider", json!(2.5)),
        ] {
            let dir = fixture.join("data/minecraft").join(registry).join("nested");
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("value.json"), value.to_string()).unwrap();
        }
        Arc::get_mut(&mut loaded.resources)
            .unwrap()
            .rest
            .get_mut()
            .unwrap()
            .providers = sunrise_client::providers::Providers::load(&fixture);
        std::fs::remove_dir_all(&fixture).unwrap();
        let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
        options.resources = root.clone();
        options.execution_role = execution_role::ExecutionRole::IntegratedServer;
        let mut game = crate::game::ServerGame::new(
            &loaded.resources,
            loaded.pack,
            loaded.gui.unwrap(),
            options,
            settings,
            None,
        )
        .unwrap();
        game.side.fns.0.store.predicates.insert(
            "minecraft:nested/true".into(),
            json!({"condition": "minecraft:all_of", "terms": []}),
        );
        let context = ComputeContext::default();
        for id in ["nested/value", "minecraft:nested/value"] {
            assert_eq!(
                game.provider_int(
                    &json!({"type": "minecraft:add", "inputs": [id, 3]}),
                    &context
                ),
                Ok(20),
            );
            assert_eq!(
                game.provider_float(
                    &json!({"type": "minecraft:add", "inputs": [id, 0.5]}),
                    &context
                ),
                Ok(3.0),
            );
        }
        for predicate in ["nested/true", "minecraft:nested/true"] {
            assert_eq!(
                game.provider_int(
                    &json!({
                        "type": "minecraft:conditional", "condition": predicate,
                        "on_true": "nested/value", "on_false": -1
                    }),
                    &context
                ),
                Ok(17),
            );
            assert_eq!(
                game.provider_float(
                    &json!({
                        "type": "minecraft:number_dispatcher",
                        "cases": [{"condition": predicate, "value": "nested/value"}],
                        "default": -1
                    }),
                    &context
                ),
                Ok(2.5),
            );
        }
        // `/compute entity`: `target_entity` is the selected entity, and the
        // source (here with no entity) is only `this`.
        game.set_allow_commands(true);
        assert!(game.run_command("scoreboard objectives add points dummy"));
        game.player = Some(
            sunrise_client::motion::PlayerMotion::new(
                game.motion,
                sunrise_core::geometry::Vec3::new(0.5, 64.0, 0.5),
            )
            .unwrap(),
        );
        assert!(game.run_command("scoreboard players set @s points 5"));
        let mut source = game.player_source();
        source.entity = None;
        game.side.command_source = Some(source);
        let on_target = |kind: &str| {
            json!({
                "type": "minecraft:conditional",
                "condition": {"condition": "minecraft:entity_properties", "entity": "target_entity",
                    "predicate": {"minecraft:entity_type": kind}},
                "on_true": 1, "on_false": 0
            })
        };
        let entity = ComputeContext {
            target: Some(Target::Player),
            ..ComputeContext::default()
        };
        assert_eq!(
            game.provider_int(&on_target("minecraft:player"), &entity),
            Ok(1)
        );
        assert_eq!(
            game.provider_int(&on_target("minecraft:cow"), &entity),
            Ok(0)
        );
        assert_eq!(
            game.provider_int(&on_target("minecraft:player"), &ComputeContext::default()),
            Ok(0)
        );
        // `entity_scores` reads the selected holder's live scoreboard value.
        let scored = |n: i32| {
            json!({
                "type": "minecraft:conditional",
                "condition": {"condition": "minecraft:entity_scores", "entity": "target_entity",
                    "scores": {"points": n}},
                "on_true": 1, "on_false": 0
            })
        };
        assert_eq!(game.provider_int(&scored(5), &entity), Ok(1));
        assert_eq!(game.provider_int(&scored(4), &entity), Ok(0));
    }
}
