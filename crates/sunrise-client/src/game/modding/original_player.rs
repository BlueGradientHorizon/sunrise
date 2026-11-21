//! The old `ServerPlayer` retained across `restoreFrom`, independently of its replacement.
use super::Game;
use mod_api::{
    ActiveEffect, Attachments, InventoryView, LevelRef, OriginalPlayerAccess, PlayerExperience,
};
use sunrise_core::nbt::Compound;
use sunrise_items::equipment::Equipment;
use sunrise_items::items::ItemStack;

/// Per-player attachment state and active replacement player UUIDs during clone hooks.
#[derive(Default)]
pub(super) struct PlayerState {
    pub(super) attachments: sunrise_mod_runtime::attachments::ResidentAttachments,
    pub(super) cloning_players: Vec<u128>,
}

/// Pre-copy state of the original `ServerPlayer`, lent directly to `player_clone`.
pub(in crate::game) struct RetainedOriginalPlayer {
    uuid: u128,
    position: [f64; 3],
    dimension: String,
    level: Option<LevelRef>,
    health: f32,
    experience: PlayerExperience,
    effects: Vec<ActiveEffect>,
    inventory: Vec<Option<ItemStack>>,
    armor: [Option<ItemStack>; 4],
    offhand: Option<ItemStack>,
    equipment: Equipment,
    persistent_data: Option<Compound>,
    attachments: Attachments,
}

impl OriginalPlayerAccess for RetainedOriginalPlayer {
    fn uuid(&self) -> u128 {
        self.uuid
    }

    fn position(&self) -> [f64; 3] {
        self.position
    }

    fn dimension(&self) -> &str {
        &self.dimension
    }

    fn level(&self) -> Option<LevelRef> {
        self.level
    }

    fn health(&self) -> f32 {
        self.health
    }

    fn experience(&self) -> PlayerExperience {
        self.experience
    }

    fn effects(&self) -> &[ActiveEffect] {
        &self.effects
    }

    fn with_inventory(&self, read: &mut dyn for<'a> FnMut(InventoryView<'a>)) {
        super::native_player_inventory::lend_inventory_view(
            &self.inventory,
            &self.armor,
            &self.offhand,
            &self.equipment,
            read,
        );
    }

    fn persistent_data(&self) -> Option<&Compound> {
        self.persistent_data.as_ref()
    }

    fn attachments_mut(&mut self) -> &mut Attachments {
        &mut self.attachments
    }
}

impl Game<'_> {
    /// `ServerPlayer.restoreFrom`: retain the old player's state before replacing any data.
    pub(in crate::game) fn retain_original_player(
        &self,
        conn: u32,
        detached_attachments: Option<Attachments>,
    ) -> Option<RetainedOriginalPlayer> {
        if !mod_api::__private::listened(mod_api::HookId::PlayerClone) {
            return None;
        }
        let guest = self.lan.as_ref()?.hub.guest(conn)?;
        Some(RetainedOriginalPlayer {
            uuid: guest.uuid,
            position: guest.position,
            dimension: guest.dimension.clone(),
            level: self.level(&guest.dimension),
            health: guest.health,
            experience: PlayerExperience::from(mod_api::__private::built::PlayerExperience {
                level: guest.xp_level,
                progress: guest.xp_progress,
                total: guest.xp_total,
            }),
            effects: guest
                .effects
                .list
                .iter()
                .map(super::native_living_effects::active_effect)
                .collect(),
            inventory: guest.inventory.clone(),
            armor: guest.armor.clone(),
            offhand: guest.offhand.clone(),
            equipment: guest.equipment.clone(),
            persistent_data: super::native_player_persistent_data::persistent_data_compound(
                guest.saved.as_ref(),
            )
            .cloned(),
            attachments: detached_attachments.unwrap_or_else(|| (*guest.attachments).clone()),
        })
    }

    /// Dispatch `player_clone`, keeping `player` resolvable before placement and dropping
    /// `original` immediately when the hook ends or unwinds.
    pub(in crate::game) fn dispatch_player_clone_with_original(
        &mut self,
        mut original: RetainedOriginalPlayer,
        player: u128,
        name: &str,
        was_death: bool,
    ) {
        self.player_state.cloning_players.push(player);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.native_player_clone(
                player,
                name,
                was_death,
                mod_api::OriginalPlayer::new(&mut original),
            );
        }));
        self.player_state.cloning_players.pop();
        if let Err(panic) = result {
            std::panic::resume_unwind(panic);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Dim, native_guest_turn_tests::with_guests};
    use mod_api::{AttachmentType, Holder, Hooks, PlayerCloneEvent, ServerHost};
    use std::sync::{Arc, Mutex};
    use sunrise_core::nbt::{JavaString, Tag};

    const VALUE: AttachmentType<i32> = AttachmentType {
        namespace: "clone",
        name: "unsaved",
        default: || 0,
        codec: None,
        copy_on_death: false,
        sync: false,
    };
    const DEFAULTED: AttachmentType<i32> = AttachmentType {
        namespace: "clone",
        name: "defaulted",
        default: || 41,
        codec: None,
        copy_on_death: false,
        sync: false,
    };
    const RETAINED: AttachmentType<i32> = AttachmentType {
        namespace: "clone",
        name: "retained",
        default: || 0,
        codec: Some(mod_api::AttachmentCodec {
            encode: |value| Ok([("value".into(), Tag::Int(*value))].into()),
            decode: |value| match value.get(&JavaString::from("value")) {
                Some(Tag::Int(value)) => Ok(*value),
                _ => Err(mod_api::AttachmentError::Decode),
            },
        }),
        copy_on_death: true,
        sync: false,
    };
    const SAVED: AttachmentType<i32> = AttachmentType {
        name: "saved",
        copy_on_death: false,
        ..RETAINED
    };

    struct Listener {
        death: bool,
        keep: bool,
        heard: Arc<Mutex<usize>>,
    }

    #[mod_api::__subscriptions]
    impl Hooks for Listener {
        fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
            r.register_attachment_type("clone:unsaved", false).unwrap();
            r.register_attachment_type("clone:defaulted", false)
                .unwrap();
            r.register_attachment_type("clone:retained", false).unwrap();
            r.register_attachment_type("clone:saved", false).unwrap();
        }

        fn player_clone(&self, host: &mut dyn ServerHost, event: &mut PlayerCloneEvent<'_>) {
            assert_eq!(event.was_death, self.death);
            assert_eq!(event.original.uuid(), 12);
            assert_eq!(event.original.position(), [7.0, 64.0, -9.0]);
            let dimension = if self.death { Dim::Nether } else { Dim::End };
            assert_eq!(
                event.original.dimension().unwrap().to_string(),
                dimension.id()
            );
            assert_eq!(
                event.original.level().unwrap().dimension_id(),
                dimension.id()
            );
            assert_eq!(event.original.health(), if self.death { 0.0 } else { 13.0 });
            assert_eq!(event.original.experience().level, 7);
            let effects = event.original.effects();
            assert_eq!(effects.len(), 1);
            assert_eq!(effects[0].duration, 200);
            assert_eq!(effects[0].amplifier, 1);

            event.original.with_inventory(|inv| {
                if self.death && !self.keep {
                    assert!(inv.get(2).is_none());
                    assert!(inv.get(36).is_none());
                    assert!(inv.get(40).is_none());
                } else {
                    assert_eq!(inv.get(2).unwrap().count(), 4);
                    assert_eq!(inv.get(36).unwrap().count(), 1);
                    assert_eq!(inv.get(40).unwrap().count(), 2);
                }
                assert!(inv.get(99).is_none());
            });

            let outside = JavaString::from("outside");
            let persisted = JavaString::from("PlayerPersisted");
            let persisted_value = Tag::compound([("inside", Tag::Int(2))]);
            let original_data = event.original.persistent_data().unwrap();
            assert_eq!(original_data.get(&outside), Some(&Tag::Int(1)));
            assert_eq!(original_data.get(&persisted), Some(&persisted_value));
            let replacement = host.player(event.player).unwrap();
            let replacement_data = replacement.persistent_data(host).unwrap();
            assert_eq!(replacement_data.get(&outside), None);
            assert_eq!(replacement_data.get(&persisted), Some(&persisted_value));
            let replacement_effects = mod_api::LivingRef::new(replacement.entity())
                .effects(host)
                .unwrap();
            assert_eq!(replacement_effects.len(), usize::from(!self.death));

            const MISSING: AttachmentType<i32> = AttachmentType {
                name: "missing",
                ..VALUE
            };
            // Never declared, so refused by name (#1448), not read as its default.
            assert_eq!(
                event.original.attachment(&MISSING),
                Err(mod_api::AttachmentError::Undeclared {
                    package_id: "clone".into(),
                    attachment_id: "clone:missing".into(),
                })
            );

            // Declared-but-absent key inserts its default on original only, never replacement.
            assert_eq!(event.original.has_attachment(&DEFAULTED), Ok(false));
            assert_eq!(event.original.existing_attachment(&DEFAULTED), Ok(None));
            assert_eq!(
                event
                    .original
                    .with_decoded_attachment(&DEFAULTED, |value| value.copied()),
                Ok(None)
            );
            assert_eq!(event.original.attachment(&DEFAULTED), Ok(41));
            assert_eq!(event.original.has_attachment(&DEFAULTED), Ok(true));
            assert_eq!(
                event
                    .original
                    .with_decoded_attachment(&DEFAULTED, |value| value.copied()),
                Ok(Some(41))
            );
            assert_eq!(DEFAULTED.has(host, Holder::Player(event.player)), Ok(false));

            // Saved codec-backed entry decodes once on existing_attachment and caches it.
            assert_eq!(event.original.has_attachment(&SAVED), Ok(true));
            assert_eq!(
                event
                    .original
                    .with_decoded_attachment(&SAVED, |value| value.copied()),
                Ok(None)
            );
            assert_eq!(event.original.existing_attachment(&SAVED), Ok(Some(31)));
            assert_eq!(
                event
                    .original
                    .with_decoded_attachment(&SAVED, |value| value.copied()),
                Ok(Some(31))
            );
            assert_eq!(
                SAVED.get(host, Holder::Player(event.player)),
                Ok(if self.death { 0 } else { 31 })
            );

            assert_eq!(event.original.attachment(&VALUE), Ok(73));
            assert_eq!(event.original.attachment(&RETAINED), Ok(17));
            assert_eq!(
                VALUE.get(host, Holder::Player(event.player)),
                Ok(if self.death { 0 } else { 73 })
            );
            assert_eq!(RETAINED.get(host, Holder::Player(event.player)), Ok(17));

            // Writes and removals on original affect only original, leaving replacement untouched.
            assert_eq!(event.original.set_attachment(&VALUE, 100), Ok(()));
            assert_eq!(event.original.existing_attachment(&VALUE), Ok(Some(100)));
            assert_eq!(
                VALUE.get(host, Holder::Player(event.player)),
                Ok(if self.death { 0 } else { 73 })
            );
            assert_eq!(event.original.remove_attachment(&RETAINED), Ok(Some(17)));
            assert_eq!(event.original.has_attachment(&RETAINED), Ok(false));
            assert_eq!(RETAINED.get(host, Holder::Player(event.player)), Ok(17));

            // Mutating the replacement leaves original untouched.
            VALUE.set(host, Holder::Player(event.player), 99).unwrap();
            assert_eq!(event.original.existing_attachment(&VALUE), Ok(Some(100)));
            *self.heard.lock().unwrap() += 1;
        }
    }

    struct Observer {
        heard: Arc<Mutex<usize>>,
    }

    #[mod_api::__subscriptions]
    impl Hooks for Observer {
        fn player_clone(&self, host: &mut dyn ServerHost, event: &mut PlayerCloneEvent<'_>) {
            // Second listener observes the first listener's mutations on the retained original
            // without any of them leaking onto the replacement player.
            assert_eq!(
                event
                    .original
                    .with_decoded_attachment(&DEFAULTED, |value| value.copied()),
                Ok(Some(41))
            );
            assert_eq!(DEFAULTED.has(host, Holder::Player(event.player)), Ok(false));
            assert_eq!(event.original.existing_attachment(&VALUE), Ok(Some(100)));
            assert_eq!(VALUE.get(host, Holder::Player(event.player)), Ok(99));
            assert_eq!(event.original.has_attachment(&RETAINED), Ok(false));
            assert_eq!(RETAINED.get(host, Holder::Player(event.player)), Ok(17));
            assert_eq!(
                event
                    .original
                    .with_decoded_attachment(&SAVED, |value| value.copied()),
                Ok(Some(31))
            );
            *self.heard.lock().unwrap() += 1;
        }
    }

    fn clone_case(death: bool, keep: bool) {
        let heard = Arc::new(Mutex::new(0));
        let observer_heard = Arc::new(Mutex::new(0));
        let mods = vec![
            mod_api::ModEntry::new(
                "clone",
                Box::new(Listener {
                    death,
                    keep,
                    heard: heard.clone(),
                }),
            ),
            mod_api::ModEntry::new(
                "observer",
                Box::new(Observer {
                    heard: observer_heard.clone(),
                }),
            ),
        ]
        .into();
        mod_platform::testing::with_mods(mods, || {
            with_guests(|game| {
                game.options.dedicated = Some(Vec::new());
                let dimension = if death { Dim::Nether } else { Dim::End };
                game.enter_level(dimension).unwrap();
                game.ui.rules.set(
                    "keep_inventory",
                    sunrise_client::gamerules::Value::Bool(keep),
                );
                let host = game.lan.as_mut().unwrap();
                host.hub.guests.update(0, |guest| {
                    guest.dimension = dimension.id().to_owned();
                    guest.position = [7.0, 64.0, -9.0];
                    guest.dead = death;
                    guest.health = if death { 0.0 } else { 13.0 };
                    guest.xp_level = 7;
                    guest
                        .effects
                        .list
                        .push(sunrise_client::effects::EffectInstance::from_def(
                            sunrise_client_core::potions::effect_by_index(
                                mod_api::__private::effect_index(
                                    mod_platform::__private::part_tests::effect_key(
                                        "minecraft:speed",
                                    ),
                                )
                                .unwrap(),
                            )
                            .unwrap(),
                            200,
                            1,
                        ));
                    // Respawn starts after death drops; keep-inventory leaves these intact.
                    if !death || keep {
                        guest.inventory[2] = Some(ItemStack::new("minecraft:diamond", 4));
                        guest.armor[3] = Some(ItemStack::new("minecraft:diamond_boots", 1));
                        guest.offhand = Some(ItemStack::new("minecraft:apple", 2));
                    }
                    let mut neoforge_data = Compound::new();
                    neoforge_data.insert("outside".into(), Tag::Int(1));
                    neoforge_data.insert(
                        "PlayerPersisted".into(),
                        Tag::compound([("inside", Tag::Int(2))]),
                    );
                    guest.saved = Some(Tag::compound([(
                        "NeoForgeData",
                        Tag::Compound(neoforge_data),
                    )]));
                });
                let attachments = host.hub.guests.attachments_mut(12).unwrap();
                RETAINED.set(attachments, Holder::Level, 17).unwrap();
                SAVED.set(attachments, Holder::Level, 31).unwrap();
                // Keep codec-backed entries undecoded, beside an unserialized binding.
                let saved = Attachments::from_nbt(attachments.to_nbt());
                attachments.replace_contents(saved);
                VALUE.set(attachments, Holder::Level, 73).unwrap();
                if death {
                    game.guest_respawn(12);
                } else {
                    let play = host.play.entry(12).or_default();
                    play.won_game = true;
                    play.respawn_requested = true;
                    game.handle_server_portal(12);
                }
                assert_eq!(*heard.lock().unwrap(), 1);
                assert_eq!(*observer_heard.lock().unwrap(), 1);
                assert!(game.player_state.cloning_players.is_empty());
            });
        });
    }

    #[test]
    #[ignore = "daily"]
    fn death_clone_retains_original_and_expires_after_dispatch() {
        crate::game::native_entity_refs::tests::run_with_game_stack(|| {
            clone_case(true, false);
            clone_case(true, true);
        });
    }

    #[test]
    #[ignore = "daily"]
    fn end_return_clone_retains_end_state_and_expires_after_dispatch() {
        crate::game::native_entity_refs::tests::run_with_game_stack(|| {
            clone_case(false, true);
        });
    }

    #[test]
    #[ignore = "daily"]
    fn unsubscribed_clone_does_not_capture() {
        mod_platform::testing::with_mods(Vec::<mod_api::ModEntry>::new().into(), || {
            with_guests(|game| assert!(game.retain_original_player(12, None).is_none()));
        });
    }
}
