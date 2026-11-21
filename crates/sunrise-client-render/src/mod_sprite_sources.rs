//! The atlas sprite source types native mods register (NeoForge's `RegisterSpriteSourcesEvent`),
//! bound once at start so a resource load invokes each type's provider directly when listing an
//! atlas definition.
use crate::atlas_sources::{ModSources, is_builtin_type};
use mod_api::{
    __private::game::{CallbackOwner, SpriteSourceRegistration},
    Answered, Identifier, ResourceReader, SpriteSourceFn, SpriteSourceInput, SpriteSourceOutput,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, RwLock},
};
use sunrise_core::nbt::Tag;

struct BoundSpriteSource {
    owner: CallbackOwner,
    list: Arc<SpriteSourceFn>,
}

static SOURCES: RwLock<BTreeMap<String, Arc<BoundSpriteSource>>> = RwLock::new(BTreeMap::new());

#[cfg(test)]
static SOURCES_FIXTURE: std::sync::Mutex<Option<BTreeMap<String, Arc<BoundSpriteSource>>>> =
    std::sync::Mutex::new(None);

#[cfg(test)]
static FIXTURE_CHANGED: std::sync::Condvar = std::sync::Condvar::new();

fn bind_into(
    bound: &mut BTreeMap<String, Arc<BoundSpriteSource>>,
    sources: Vec<Answered<SpriteSourceRegistration>>,
) -> Vec<String> {
    let mut kept = Vec::new();
    let mut seen = BTreeSet::new();
    for Answered { answer, mod_id, .. } in sources {
        let id = answer.id;
        let Some(owner) = CallbackOwner::new(mod_id) else {
            log::warn!("Sprite source type {id} has no installed owner {mod_id}");
            continue;
        };
        let key = id.to_string();
        let entry = Arc::new(BoundSpriteSource {
            owner,
            list: answer.list,
        });
        if let Some(earlier) = bound.insert(key.clone(), entry) {
            log::warn!(
                "Sunrise: sprite source type {key} registered by {} is replaced by {mod_id}'s registration",
                earlier.owner.id()
            );
        } else if is_builtin_type(&key) {
            log::warn!(
                "Sunrise: sprite source type {key} registered by the game is replaced by {mod_id}'s registration"
            );
        }
        if seen.insert(key.clone()) {
            kept.push(key);
        }
    }
    kept
}

/// Binds the sprite source providers native mods registered once at start, before resources load.
/// Any valid `Identifier` (including `minecraft:` and empty-path ids) is accepted; a later
/// registration for the same id replaces the earlier one (last-writer-wins) with a warning naming
/// both mods, and overriding a built-in vanilla source type logs a warning naming the mod.
/// Returns the unique ids kept in this batch, in first-registration order.
pub fn register(sources: Vec<Answered<SpriteSourceRegistration>>) -> Vec<String> {
    let mut bound = SOURCES
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    bind_into(&mut bound, sources)
}

fn lookup(kind: &str) -> Option<Arc<BoundSpriteSource>> {
    #[cfg(test)]
    if let Some(fixture) = SOURCES_FIXTURE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
    {
        return fixture.get(kind).cloned();
    }
    SOURCES
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(kind)
        .cloned()
}

fn is_registered(kind: &str) -> bool {
    #[cfg(test)]
    if let Some(fixture) = SOURCES_FIXTURE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
    {
        return fixture.contains_key(kind);
    }
    SOURCES
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains_key(kind)
}

/// The native mods' source types, declared but never answered: for a load that draws nothing.
pub struct Declared;
impl ModSources for Declared {
    fn is_registered(&self, kind: &str) -> bool {
        is_registered(kind)
    }
    fn ask(
        &self,
        _: &Identifier,
        _: &str,
        _: &Tag,
        _: &Arc<dyn ResourceReader>,
    ) -> Vec<SpriteSourceOutput> {
        Vec::new()
    }
}

/// The native mods' bound sprite source providers, invoked on the resource loader as registrations.
pub struct Registered;
impl ModSources for Registered {
    fn is_registered(&self, kind: &str) -> bool {
        is_registered(kind)
    }
    fn ask(
        &self,
        atlas: &Identifier,
        kind: &str,
        config: &Tag,
        resources: &Arc<dyn ResourceReader>,
    ) -> Vec<SpriteSourceOutput> {
        let Some(entry) = lookup(kind) else {
            return Vec::new();
        };
        let _phase =
            mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::Registration);
        let input = SpriteSourceInput::new(atlas, config, resources.as_ref());
        match entry
            .owner
            .call("sprite source", "sprite_source", || (entry.list)(&input))
        {
            Ok(Ok(outputs)) => outputs,
            Ok(Err(reason)) => {
                log::warn!(
                    "atlas {atlas}: mod {} sprite source {kind} failed: {reason}",
                    entry.owner.id()
                );
                Vec::new()
            }
            Err(error) => {
                log::warn!("atlas {atlas}: sprite source {kind}: {error}");
                Vec::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atlas_sources::{
        Listing, SpriteOrigin, SpriteSource, decode, list, test_mods::Mods,
    };
    use mod_api::{
        __private::game::{RegistrationContextNew, take_callback_panic_notices},
        RegistrationContext,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct SourcesFixture;

    impl SourcesFixture {
        fn from_registrations(
            sources: Vec<Answered<SpriteSourceRegistration>>,
        ) -> (Self, Vec<String>) {
            let mut bound = BTreeMap::new();
            let kept = bind_into(&mut bound, sources);
            let mut fixture = SOURCES_FIXTURE
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            while fixture.is_some() {
                fixture = FIXTURE_CHANGED
                    .wait(fixture)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
            *fixture = Some(bound);
            (Self, kept)
        }
    }

    impl Drop for SourcesFixture {
        fn drop(&mut self) {
            let mut fixture = SOURCES_FIXTURE
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *fixture = None;
            FIXTURE_CHANGED.notify_all();
        }
    }

    fn id(text: &str) -> Identifier {
        Identifier::parse(text).unwrap()
    }
    fn textures() -> Vec<String> {
        [
            "srctest:item/apple",
            "srctest:item/gem",
            "srctest:item/stick",
        ]
        .map(String::from)
        .to_vec()
    }
    fn pixels(sprite: &str, width: u32, height: u32, rgba: Vec<u8>) -> SpriteSourceOutput {
        SpriteSourceOutput::Pixels {
            sprite: id(sprite),
            width,
            height,
            rgba,
        }
    }
    /// A definition of a directory source, the registered source, then a single file.
    fn definition() -> Vec<SpriteSource> {
        decode(
            r#"{"sources":[
                {"type":"directory","source":"item","prefix":"item/"},
                {"type":"srctest:own","shade":7},
                {"type":"single","resource":"srctest:item/stick","sprite":"srctest:late"}
            ]}"#,
            &Mods::new(&["srctest:own"], |_, _, _, _| Vec::new()),
        )
        .unwrap()
    }

    #[test]
    fn registration_accepts_minecraft_namespace_overrides_builtins_and_last_writer_wins() {
        struct Earlier;
        #[mod_api::__subscriptions]
        impl mod_api::Hooks for Earlier {
            fn register_sprite_sources(
                &self,
                _: &RegistrationContext,
                event: &mut mod_api::RegisterSpriteSourcesEvent<'_>,
            ) {
                event.register(id("srcreg:shared"), |_| {
                    Ok(vec![pixels("srcreg:from_earlier", 1, 1, vec![1; 4])])
                });
                event.register(id("minecraft:single"), |_| {
                    Ok(vec![pixels("srcreg:single_earlier", 1, 1, vec![2; 4])])
                });
            }
        }
        struct Later;
        #[mod_api::__subscriptions]
        impl mod_api::Hooks for Later {
            fn register_sprite_sources(
                &self,
                _: &RegistrationContext,
                event: &mut mod_api::RegisterSpriteSourcesEvent<'_>,
            ) {
                event.register(id("srcreg:shared"), |_| {
                    Ok(vec![pixels("srcreg:from_later", 1, 1, vec![3; 4])])
                });
                event.register(id("minecraft:single"), |_| {
                    Ok(vec![pixels("srcreg:single_override", 1, 1, vec![4; 4])])
                });
                event.register(id("minecraft:custom"), |_| {
                    Ok(vec![pixels("srcreg:mc_custom", 1, 1, vec![5; 4])])
                });
            }
        }

        let mods: mod_api::Mods = vec![
            mod_api::ModEntry::new("earlier_src", Box::new(Earlier)),
            mod_api::ModEntry::new("later_src", Box::new(Later)),
        ]
        .into();
        mod_platform::testing::with_mods(mods, || {
            let mut registrations = Vec::new();
            mod_api::dispatch_register_sprite_sources(
                &RegistrationContext::new(),
                &mut registrations,
            );
            let (_fixture, kept) = SourcesFixture::from_registrations(registrations);
            assert_eq!(
                kept,
                ["srcreg:shared", "minecraft:single", "minecraft:custom"]
            );
            assert!(
                Declared.is_registered("srcreg:shared")
                    && Declared.is_registered("minecraft:single")
                    && Declared.is_registered("minecraft:custom")
            );
            let source = crate::resource_source::ResourceSource::indexed(
                [
                    (
                        "assets/minecraft/atlases/blocks.json".to_owned(),
                        sunrise_assets::resource_source::Resource::bytes(
                            br#"{"sources":[
                                {"type":"srcreg:shared"},
                                {"type":"single","resource":"srctest:item/apple"},
                                {"type":"minecraft:custom"}
                            ]}"#
                            .to_vec(),
                        ),
                    ),
                    (
                        "assets/srctest/textures/item/apple.png".to_owned(),
                        sunrise_assets::resource_source::Resource::bytes(b"png".to_vec()),
                    ),
                ]
                .into(),
            );
            let listing = Listing::build(&source, &Registered);
            assert_eq!(
                listing
                    .sprites("minecraft:blocks")
                    .keys()
                    .collect::<Vec<_>>(),
                [
                    "srcreg:from_later",
                    "srcreg:mc_custom",
                    "srcreg:single_override"
                ],
                "later registration replaces earlier one and overrides built-in minecraft:single"
            );
        });
    }

    #[test]
    fn panic_disables_only_that_provider_with_notice_and_non_panic_err_does_not_latch() {
        let _notice_lock = crate::mod_model_loaders::PANIC_NOTICE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let panic_calls = Arc::new(AtomicUsize::new(0));
        let flaky_calls = Arc::new(AtomicUsize::new(0));
        let healthy_calls = Arc::new(AtomicUsize::new(0));

        struct Panicker(Arc<AtomicUsize>);
        #[mod_api::__subscriptions]
        impl mod_api::Hooks for Panicker {
            fn register_sprite_sources(
                &self,
                _: &RegistrationContext,
                event: &mut mod_api::RegisterSpriteSourcesEvent<'_>,
            ) {
                let calls = Arc::clone(&self.0);
                event.register(id("srctest:panics"), move |_| {
                    calls.fetch_add(1, Ordering::Relaxed);
                    panic!("sprite source boom");
                });
            }
        }
        struct Flaky(Arc<AtomicUsize>);
        #[mod_api::__subscriptions]
        impl mod_api::Hooks for Flaky {
            fn register_sprite_sources(
                &self,
                _: &RegistrationContext,
                event: &mut mod_api::RegisterSpriteSourcesEvent<'_>,
            ) {
                let calls = Arc::clone(&self.0);
                event.register(id("srctest:flaky"), move |_| {
                    if calls.fetch_add(1, Ordering::Relaxed) == 0 {
                        Err("missing palette".into())
                    } else {
                        Ok(vec![pixels("srctest:recovered", 1, 1, vec![7; 4])])
                    }
                });
            }
        }
        struct Healthy(Arc<AtomicUsize>);
        #[mod_api::__subscriptions]
        impl mod_api::Hooks for Healthy {
            fn register_sprite_sources(
                &self,
                _: &RegistrationContext,
                event: &mut mod_api::RegisterSpriteSourcesEvent<'_>,
            ) {
                let calls = Arc::clone(&self.0);
                event.register(id("srctest:healthy"), move |_| {
                    calls.fetch_add(1, Ordering::Relaxed);
                    Ok(vec![pixels("srctest:ok", 1, 1, vec![8; 4])])
                });
            }
        }

        let mods: mod_api::Mods = vec![
            mod_api::ModEntry::new("src_panicker", Box::new(Panicker(Arc::clone(&panic_calls)))),
            mod_api::ModEntry::new("src_flaky", Box::new(Flaky(Arc::clone(&flaky_calls)))),
            mod_api::ModEntry::new("src_healthy", Box::new(Healthy(Arc::clone(&healthy_calls)))),
        ]
        .into();
        mod_platform::testing::with_mods(mods, || {
            let mut registrations = Vec::new();
            mod_api::dispatch_register_sprite_sources(
                &RegistrationContext::new(),
                &mut registrations,
            );
            let (_fixture, kept) = SourcesFixture::from_registrations(registrations);
            assert_eq!(kept, ["srctest:panics", "srctest:flaky", "srctest:healthy"]);

            let atlas_source = |kind: &str| {
                crate::resource_source::ResourceSource::indexed(
                    [(
                        "assets/minecraft/atlases/blocks.json".to_owned(),
                        sunrise_assets::resource_source::Resource::bytes(
                            format!(r#"{{"sources":[{{"type":"{kind}"}}]}}"#).into_bytes(),
                        ),
                    )]
                    .into(),
                )
            };

            let _ = take_callback_panic_notices();
            let first_panic = Listing::build(&atlas_source("srctest:panics"), &Registered);
            assert!(first_panic.sprites("minecraft:blocks").is_empty());
            assert_eq!(panic_calls.load(Ordering::Relaxed), 1);
            let notices = take_callback_panic_notices();
            assert!(
                notices.iter().any(|notice| notice.mod_id == "src_panicker" && notice.kind == "sprite source"),
                "expected panic notice, got {notices:?}"
            );

            // Disabled after panic: a second load does not invoke the panicking callback again.
            let second_panic = Listing::build(&atlas_source("srctest:panics"), &Registered);
            assert!(second_panic.sprites("minecraft:blocks").is_empty());
            assert_eq!(panic_calls.load(Ordering::Relaxed), 1);

            // Non-panic Err does not latch or queue a panic notice.
            let first_flaky = Listing::build(&atlas_source("srctest:flaky"), &Registered);
            assert!(first_flaky.sprites("minecraft:blocks").is_empty());
            assert_eq!(flaky_calls.load(Ordering::Relaxed), 1);
            assert!(take_callback_panic_notices().is_empty());

            let second_flaky = Listing::build(&atlas_source("srctest:flaky"), &Registered);
            assert_eq!(flaky_calls.load(Ordering::Relaxed), 2);
            assert_eq!(
                second_flaky
                    .sprites("minecraft:blocks")
                    .keys()
                    .collect::<Vec<_>>(),
                ["srctest:recovered"]
            );

            // Another mod's provider is unaffected by the panic.
            let healthy = Listing::build(&atlas_source("srctest:healthy"), &Registered);
            assert_eq!(healthy_calls.load(Ordering::Relaxed), 1);
            assert_eq!(
                healthy
                    .sprites("minecraft:blocks")
                    .keys()
                    .collect::<Vec<_>>(),
                ["srctest:ok"]
            );
        });
    }

    #[test]
    fn an_unregistered_type_fails_its_definition_by_name() {
        let text = r#"{"sources":[{"type":"directory","source":"item","prefix":""},{"type":"srcfree:nobody"}]}"#;
        assert_eq!(
            decode(text, &Mods::none()).unwrap_err().to_string(),
            "sources[1]: unknown sprite source type srcfree:nobody"
        );
        let registered = Mods::new(&["srcfree:nobody", "srcfree:"], |_, _, _, _| Vec::new());
        assert!(matches!(
            decode(text, &registered).unwrap().as_slice(),
            [_, SpriteSource::Registered { kind, .. }] if kind == "srcfree:nobody"
        ));
        // Leading colon resolves into minecraft:, and valid empty-path ids are accepted.
        let leading_and_empty = r#"{"sources":[
            {"type":":single","resource":":item/stick"},
            {"type":"srcfree:"}
        ]}"#;
        assert!(matches!(
            decode(leading_and_empty, &registered).unwrap().as_slice(),
            [
                SpriteSource::Single { resource, .. },
                SpriteSource::Registered { kind, .. },
            ] if resource == "minecraft:item/stick" && kind == "srcfree:"
        ));
    }

    #[test]
    fn the_source_is_asked_with_its_entry_and_nothing_the_entries_before_it_listed() {
        let mut asked = Vec::new();
        list(&definition(), &textures(), &mut |kind, config| {
            assert!(
                matches!(config, Tag::Compound(_)),
                "an object converts to a compound"
            );
            asked.push((kind.to_owned(), config.get("shade").cloned()));
            Vec::new()
        });
        assert_eq!(
            asked,
            // A JSON number converts to the narrowest tag that holds it.
            [("srctest:own".to_owned(), Some(Tag::Byte(7)))]
        );
    }

    #[test]
    fn outputs_apply_in_order_and_a_later_sprite_replaces_an_earlier_one() {
        let listed = list(&definition(), &textures(), &mut |_, _| {
            vec![
                // Replaces the directory's apple, then is replaced by the next output.
                pixels("srctest:item/apple", 1, 1, vec![1, 1, 1, 255]),
                SpriteSourceOutput::Texture {
                    sprite: id("srctest:item/apple"),
                    texture: id("srctest:item/gem"),
                },
                pixels("srctest:new", 1, 2, vec![9; 8]),
                // The single source after the mod's replaces this one.
                pixels("srctest:late", 1, 1, vec![0; 4]),
            ]
        });
        assert_eq!(
            listed.sprites["srctest:item/apple"],
            SpriteOrigin::File("srctest:item/gem".into())
        );
        assert_eq!(
            listed.sprites["srctest:new"],
            SpriteOrigin::Pixels((1, 2, vec![9; 8]))
        );
        assert_eq!(
            listed.sprites["srctest:late"],
            SpriteOrigin::File("srctest:item/stick".into())
        );
        assert_eq!(
            listed.by_mods.iter().collect::<Vec<_>>(),
            ["srctest:item/apple", "srctest:new"],
            "the mod's, but not what a later built-in source took back"
        );
        let generated = crate::atlas_sources::generate(&listed.sprites["srctest:new"], &|_| None);
        assert_eq!(
            generated,
            Some((1, 2, vec![9; 8])),
            "a mod's pixels need no file"
        );
    }

    #[test]
    fn remove_drops_a_listed_sprite_and_an_unlisted_id_changes_nothing() {
        let listed = list(&definition(), &textures(), &mut |_, _| {
            vec![
                SpriteSourceOutput::Remove {
                    sprite: id("srctest:item/gem"),
                },
                SpriteSourceOutput::Remove {
                    sprite: id("srctest:never_listed"),
                },
            ]
        });
        let ids: Vec<&str> = listed.sprites.keys().map(String::as_str).collect();
        assert_eq!(
            ids,
            ["srctest:item/apple", "srctest:item/stick", "srctest:late"]
        );
    }

    #[test]
    fn what_cant_be_a_sprite_is_skipped_alone() {
        let side = 32768_u32;
        let listed = list(&definition(), &textures(), &mut |_, _| {
            vec![
                pixels("srctest:short", 2, 2, vec![0; 15]),
                pixels("srctest:long", 2, 2, vec![0; 17]),
                pixels("srctest:empty", 0, 4, Vec::new()),
                // The product overflows: refused without a panic.
                pixels("srctest:overflow", u32::MAX, u32::MAX, vec![0; 4]),
                pixels(
                    "srctest:too_wide",
                    side + 1,
                    1,
                    vec![0; (side as usize + 1) * 4],
                ),
                SpriteSourceOutput::Texture {
                    sprite: id("srctest:missing"),
                    texture: id("srctest:item/nothing"),
                },
                pixels("srctest:widest", side, 1, vec![0; side as usize * 4]),
                pixels("srctest:good", 2, 2, vec![0; 16]),
            ]
        });
        let ids: Vec<&str> = listed.sprites.keys().map(String::as_str).collect();
        assert_eq!(
            ids,
            [
                "srctest:good",
                "srctest:item/apple",
                "srctest:item/gem",
                "srctest:item/stick",
                "srctest:late",
                "srctest:widest"
            ]
        );
    }
}
