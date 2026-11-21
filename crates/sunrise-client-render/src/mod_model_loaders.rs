//! The model loaders native mods register (NeoForge's `ModelEvent.RegisterLoaders`), bound at
//! registration time so a resource load resolves each model's loader directly.
use mod_api::{
    __private::game::{CallbackOwner, ModelLoaderRegistration},
    Answered, ModelLoaderFn, ModelLoaderInput, ModelLoaderQuad,
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
};
use sunrise_assets::{
    model::Direction as FaceDirection,
    model_loaders::{LoaderGeometry, LoaderQuad},
};
use sunrise_core::geometry::Direction;

struct BoundModelLoader {
    owner: CallbackOwner,
    bake: Arc<ModelLoaderFn>,
}

static LOADERS: OnceLock<Arc<BTreeMap<String, BoundModelLoader>>> = OnceLock::new();

#[cfg(test)]
pub(crate) static PANIC_NOTICE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
static LOADERS_FIXTURE: std::sync::Mutex<Option<Arc<BTreeMap<String, BoundModelLoader>>>> =
    std::sync::Mutex::new(None);

#[cfg(test)]
static FIXTURE_CHANGED: std::sync::Condvar = std::sync::Condvar::new();

fn freeze(
    loaders: Vec<Answered<ModelLoaderRegistration>>,
) -> (BTreeMap<String, BoundModelLoader>, Vec<String>) {
    let mut bound = BTreeMap::new();
    let mut kept = Vec::new();
    for Answered { answer, mod_id, .. } in loaders {
        let key = answer.id.to_string();
        if matches!(key.as_str(), "neoforge:composite" | "neoforge:obj") {
            log::error!("Mod {mod_id}: model loader {key} collides with a built-in model loader");
            continue;
        }
        if bound.contains_key(&key) {
            log::error!(
                "Mod {mod_id}: model loader {key} is already registered; the first registration is kept"
            );
            continue;
        }
        let Some(owner) = CallbackOwner::new(mod_id) else {
            log::error!("Model loader {key} has no installed owner {mod_id}");
            continue;
        };
        bound.insert(
            key.clone(),
            BoundModelLoader {
                owner,
                bake: answer.bake,
            },
        );
        kept.push(key);
    }
    (bound, kept)
}

/// Binds the model loaders native mods registered once at start, before resources load. Any valid
/// `Identifier` (including `minecraft:` and empty-path ids) is accepted; built-in loader
/// collisions (`neoforge:composite`, `neoforge:obj`) and duplicates are logged as errors and left
/// out. Returns the ids kept, in registration order.
pub fn register(loaders: Vec<Answered<ModelLoaderRegistration>>) -> Vec<String> {
    let (bound, kept) = freeze(loaders);
    if LOADERS.set(Arc::new(bound)).is_err() {
        log::warn!("model loaders were registered already; these are left out");
        return Vec::new();
    }
    kept
}

fn active_loaders() -> Option<Arc<BTreeMap<String, BoundModelLoader>>> {
    #[cfg(test)]
    if let Some(fixture) = LOADERS_FIXTURE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
    {
        return Some(fixture);
    }
    LOADERS.get().cloned()
}

/// What the mods' bound loaders answer, for a model library (`ModelLibrary::with_loaders`). It runs
/// on the resource loader, as a registration.
pub struct ModLoaders;

impl LoaderGeometry for ModLoaders {
    fn quads(
        &self,
        loader: &str,
        model: &str,
        config: &Value,
        textures: &BTreeMap<String, String>,
    ) -> Option<Vec<LoaderQuad>> {
        let loaders = active_loaders()?;
        let entry = loaders.get(loader)?;
        let Ok(config) = sunrise_core::json_nbt::json_to_tag(config) else {
            log::warn!("model {model}: loader {loader}: its JSON doesn't convert to NBT; no quads");
            return Some(Vec::new());
        };
        let _phase =
            mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::Registration);
        let input = ModelLoaderInput {
            model,
            config: &config,
            textures,
        };
        let quads = match entry
            .owner
            .call("model loader", "bake_model_loader", || (entry.bake)(&input))
        {
            Ok(Ok(quads)) => checked(loader, model, quads),
            Ok(Err(reason)) => {
                log::warn!(
                    "model {model}: mod {} loader {loader} refused it: {reason}",
                    entry.owner.id()
                );
                Vec::new()
            }
            Err(error) => {
                log::warn!("model {model}: loader {loader}: {error}");
                Vec::new()
            }
        };
        Some(quads)
    }
}

/// The quads a mod answered, as the baker takes them: a quad with a position or UV that isn't
/// finite is skipped by itself.
fn checked(loader: &str, model: &str, quads: Vec<ModelLoaderQuad>) -> Vec<LoaderQuad> {
    quads
        .into_iter()
        .filter_map(|quad| {
            let finite = quad
                .positions
                .iter()
                .flatten()
                .chain(quad.uvs.iter().flatten())
                .all(|value| value.is_finite());
            if !finite {
                log::warn!("model {model}: loader {loader}: a quad that isn't finite; skipped");
                return None;
            }
            Some(LoaderQuad {
                positions: quad.positions,
                uvs: quad.uvs,
                texture: quad.texture.trim_start_matches('#').to_owned(),
                cull: quad.cull.map(face),
                tint_index: quad.tint_index,
                shade: quad.shade,
            })
        })
        .collect()
}

fn face(direction: Direction) -> FaceDirection {
    match direction {
        Direction::Down => FaceDirection::Down,
        Direction::Up => FaceDirection::Up,
        Direction::North => FaceDirection::North,
        Direction::South => FaceDirection::South,
        Direction::West => FaceDirection::West,
        Direction::East => FaceDirection::East,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{
        __private::game::{RegistrationContextNew, take_callback_panic_notices},
        Identifier, RegistrationContext,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    use sunrise_assets::{
        bake::{BlockModelTransform, bake_block_model},
        model::CuboidModel,
        resolve::ModelLibrary,
    };
    use sunrise_core::nbt::Tag;

    struct LoadersFixture;

    impl LoadersFixture {
        fn from_registrations(
            loaders: Vec<Answered<ModelLoaderRegistration>>,
        ) -> (Self, Vec<String>) {
            let (bound, kept) = freeze(loaders);
            let mut fixture = LOADERS_FIXTURE
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            while fixture.is_some() {
                fixture = FIXTURE_CHANGED
                    .wait(fixture)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
            *fixture = Some(Arc::new(bound));
            (Self, kept)
        }
    }

    impl Drop for LoadersFixture {
        fn drop(&mut self) {
            let mut fixture = LOADERS_FIXTURE
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *fixture = None;
            FIXTURE_CHANGED.notify_all();
        }
    }

    fn quad(texture: &str, x: f32) -> ModelLoaderQuad {
        ModelLoaderQuad {
            positions: [
                [x, 0.0, 0.5],
                [0.0, 0.0, 0.5],
                [0.0, 1.0, 0.5],
                [x, 1.0, 0.5],
            ],
            uvs: [[1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0]],
            texture: texture.into(),
            cull: Some(Direction::Up),
            tint_index: -1,
            shade: true,
        }
    }

    #[test]
    fn large_quad_counts_over_65536_are_accepted() {
        let many = vec![quad("side", 1.0); 70_000];
        assert_eq!(checked("l", "m", many).len(), 70_000);
    }

    #[test]
    fn a_quad_that_isnt_finite_is_skipped_alone() {
        let mut uv_nan = quad("side", 1.0);
        uv_nan.uvs[2][1] = f32::NAN;
        let kept = checked(
            "l",
            "m",
            vec![quad("side", f32::INFINITY), uv_nan, quad("#side", 0.5)],
        );
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].positions[0][0], 0.5);
        assert_eq!(kept[0].texture, "side", "a #reference names the same key");
        assert_eq!(kept[0].cull, Some(FaceDirection::Up));
    }

    /// A native mod's loader: a quad as wide as the model's `width`, in sixteenths.
    struct Panel;
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Panel {
        fn register_model_loaders(
            &self,
            _: &RegistrationContext,
            event: &mut mod_api::RegisterModelLoadersEvent<'_>,
        ) {
            event.register(Identifier::parse("rendtest:panel").unwrap(), |input| {
                let width = match input.config.get("width") {
                    Some(Tag::Byte(width)) => f32::from(*width) / 16.0,
                    _ => 1.0,
                };
                assert_eq!(
                    input.textures.get("side").map(String::as_str),
                    Some("minecraft:block/panel")
                );
                Ok(vec![quad("side", width)])
            });
        }
    }

    /// The registered loader's quads are baked with the model; a reload, a new library over the
    /// model as the packs now give it, asks again; with no bound loader the model has no quads.
    #[test]
    fn a_mods_loader_bakes_its_quads_and_answers_again_on_reload() {
        let library = |width: u8| {
            let json = format!(
                r#"{{"loader":"rendtest:panel","width":{width},"textures":{{"side":"block/panel"}}}}"#
            );
            ModelLibrary::new(
                [("t:panel".to_owned(), CuboidModel::from_json(&json).unwrap())].into(),
            )
            .unwrap()
            .with_loaders(Arc::new(ModLoaders))
        };
        let widths = |library: &ModelLibrary| {
            bake_block_model(
                &library.resolve("t:panel").unwrap(),
                BlockModelTransform::new([0; 3], false),
            )
            .unwrap()
            .iter()
            .map(|q| (q.positions[0][0], q.material.sprite.clone()))
            .collect::<Vec<_>>()
        };
        let mods: mod_api::Mods = vec![mod_api::ModEntry::new("panel", Box::new(Panel))].into();
        mod_platform::testing::with_mods(mods, || {
            let mut loaders = Vec::new();
            mod_api::dispatch_register_model_loaders(&RegistrationContext::new(), &mut loaders);
            let (_fixture, kept) = LoadersFixture::from_registrations(loaders);
            assert_eq!(kept, ["rendtest:panel"]);
            assert_eq!(
                widths(&library(8)),
                [(0.5, "minecraft:block/panel".to_owned())]
            );
            assert_eq!(
                widths(&library(4)),
                [(0.25, "minecraft:block/panel".to_owned())]
            );
        });
        mod_platform::testing::with_mods(Vec::new().into(), || {
            assert!(widths(&library(8)).is_empty());
        });
    }

    #[test]
    fn minecraft_namespace_is_accepted_and_duplicates_and_builtins_are_rejected() {
        struct First;
        #[mod_api::__subscriptions]
        impl mod_api::Hooks for First {
            fn register_model_loaders(
                &self,
                _: &RegistrationContext,
                event: &mut mod_api::RegisterModelLoadersEvent<'_>,
            ) {
                event.register(
                    Identifier::parse("minecraft:rendtest_first").unwrap(),
                    |_| Ok(vec![quad("side", 0.25)]),
                );
                event.register(Identifier::parse("rendtest:").unwrap(), |_| {
                    Ok(vec![quad("side", 0.5)])
                });
                event.register(Identifier::parse("neoforge:composite").unwrap(), |_| {
                    Ok(vec![quad("side", 1.0)])
                });
                event.register(Identifier::parse("neoforge:obj").unwrap(), |_| {
                    Ok(vec![quad("side", 1.0)])
                });
            }
        }
        struct Second;
        #[mod_api::__subscriptions]
        impl mod_api::Hooks for Second {
            fn register_model_loaders(
                &self,
                _: &RegistrationContext,
                event: &mut mod_api::RegisterModelLoadersEvent<'_>,
            ) {
                event.register(
                    Identifier::parse("minecraft:rendtest_first").unwrap(),
                    |_| Ok(vec![quad("side", 0.75)]),
                );
            }
        }
        let mods: mod_api::Mods = vec![
            mod_api::ModEntry::new("first_loader", Box::new(First)),
            mod_api::ModEntry::new("second_loader", Box::new(Second)),
        ]
        .into();
        mod_platform::testing::with_mods(mods, || {
            let mut loaders = Vec::new();
            mod_api::dispatch_register_model_loaders(&RegistrationContext::new(), &mut loaders);
            let (_fixture, kept) = LoadersFixture::from_registrations(loaders);
            assert_eq!(kept, ["minecraft:rendtest_first", "rendtest:"]);
            let textures =
                BTreeMap::from([("side".to_owned(), "minecraft:block/panel".to_owned())]);
            let baked = ModLoaders
                .quads(
                    "minecraft:rendtest_first",
                    "t:vanilla_ns",
                    &serde_json::json!({"loader": "minecraft:rendtest_first"}),
                    &textures,
                )
                .unwrap();
            assert_eq!(baked.len(), 1);
            assert_eq!(baked[0].positions[0][0], 0.25, "first registration wins");
            let empty_path = ModLoaders
                .quads(
                    "rendtest:",
                    "t:empty_path",
                    &serde_json::json!({"loader": "rendtest:"}),
                    &textures,
                )
                .unwrap();
            assert_eq!(empty_path.len(), 1);
            assert_eq!(empty_path[0].positions[0][0], 0.5);
            assert!(
                ModLoaders
                    .quads(
                        "rendtest:unregistered",
                        "t:unknown",
                        &serde_json::json!({}),
                        &textures,
                    )
                    .is_none()
            );
        });
    }

    #[test]
    fn panic_disables_loader_with_notice_and_non_panic_err_does_not_latch() {
        let _notice_lock = PANIC_NOTICE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let panic_calls = Arc::new(AtomicUsize::new(0));
        let flaky_calls = Arc::new(AtomicUsize::new(0));

        struct Panicker(Arc<AtomicUsize>);
        #[mod_api::__subscriptions]
        impl mod_api::Hooks for Panicker {
            fn register_model_loaders(
                &self,
                _: &RegistrationContext,
                event: &mut mod_api::RegisterModelLoadersEvent<'_>,
            ) {
                let calls = Arc::clone(&self.0);
                event.register(Identifier::parse("rendtest:panics").unwrap(), move |_| {
                    calls.fetch_add(1, Ordering::Relaxed);
                    panic!("boom");
                });
            }
        }

        struct Flaky(Arc<AtomicUsize>);
        #[mod_api::__subscriptions]
        impl mod_api::Hooks for Flaky {
            fn register_model_loaders(
                &self,
                _: &RegistrationContext,
                event: &mut mod_api::RegisterModelLoadersEvent<'_>,
            ) {
                let calls = Arc::clone(&self.0);
                event.register(Identifier::parse("rendtest:flaky").unwrap(), move |_| {
                    if calls.fetch_add(1, Ordering::Relaxed) == 0 {
                        Err("bad config".into())
                    } else {
                        Ok(vec![quad("side", 0.5)])
                    }
                });
            }
        }

        let mods: mod_api::Mods = vec![
            mod_api::ModEntry::new(
                "loader_panicker",
                Box::new(Panicker(Arc::clone(&panic_calls))),
            ),
            mod_api::ModEntry::new("loader_flaky", Box::new(Flaky(Arc::clone(&flaky_calls)))),
        ]
        .into();
        mod_platform::testing::with_mods(mods, || {
            let mut loaders = Vec::new();
            mod_api::dispatch_register_model_loaders(&RegistrationContext::new(), &mut loaders);
            let (_fixture, kept) = LoadersFixture::from_registrations(loaders);
            assert_eq!(kept, ["rendtest:panics", "rendtest:flaky"]);

            let textures =
                BTreeMap::from([("side".to_owned(), "minecraft:block/panel".to_owned())]);
            let cfg = serde_json::json!({});

            let _ = take_callback_panic_notices();
            assert_eq!(
                ModLoaders.quads("rendtest:panics", "t:p", &cfg, &textures),
                Some(Vec::new())
            );
            assert_eq!(panic_calls.load(Ordering::Relaxed), 1);
            let notices = take_callback_panic_notices();
            assert!(
                notices.contains(&("loader_panicker", "model loader")),
                "expected panic notice, got {notices:?}"
            );
            // Disabled after panic: second call does not invoke the callback again.
            assert_eq!(
                ModLoaders.quads("rendtest:panics", "t:p", &cfg, &textures),
                Some(Vec::new())
            );
            assert_eq!(panic_calls.load(Ordering::Relaxed), 1);

            // Non-panic Err does not latch or queue a panic notice.
            assert_eq!(
                ModLoaders.quads("rendtest:flaky", "t:f", &cfg, &textures),
                Some(Vec::new())
            );
            assert_eq!(flaky_calls.load(Ordering::Relaxed), 1);
            assert!(take_callback_panic_notices().is_empty());
            let recovered = ModLoaders
                .quads("rendtest:flaky", "t:f", &cfg, &textures)
                .unwrap();
            assert_eq!(flaky_calls.load(Ordering::Relaxed), 2);
            assert_eq!(recovered.len(), 1);
            assert_eq!(recovered[0].positions[0][0], 0.5);
        });
    }
}
