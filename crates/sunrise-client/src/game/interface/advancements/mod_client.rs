#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Dim, Game, chat::PLAYER_NAME, lan_actions::DisplacedPlayer, stats::Stats,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::advancements::AdvState;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::advancements::PlayerEntityState;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::advancements::RemoteNode;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::advancements::RemoteTree;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::advancements::TagSources;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::advancements::Tags;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::advancements::Toast;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::advancements::TravelMarks;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::advancements::View;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::advancements::adv_name;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::advancements::item_view;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::advancements::text_of;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::advancements::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use mod_api::{GameMode, ProgressType};
#[allow(unused_imports)]
pub(in crate::game::client_side) use serde_json::Value;
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::client_side) use std::time::Instant;
#[allow(unused_imports)]
pub(in crate::game::client_side) use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::actions::Action;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    advancements::{self as adv, EntityView, Frame, ItemView, Progress, Tree},
    effects::PlayerEffects,
    hud::{HudBatch, rgba},
    items::ItemStack,
    motion::PlayerMotion,
    text::{self, Style, Text},
    toasts::Slide,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::geometry::Vec3;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_core::random::RandomSource;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_protocol::packets::play::Clientbound as C;

#[path = "icons_client.rs"]
pub(in crate::game) mod icons;

impl Game<'_> {
    pub(in crate::game) fn add_advancement_toast(&mut self, mut toast: Toast) {
        let kind = if toast.is_recipe() {
            mod_api::ToastKind::Recipe
        } else {
            mod_api::ToastKind::Advancement
        };
        if self.admit_toast(kind, &mut toast.title, &mut toast.description) {
            self.client
                .toast_manager
                .queued
                .push_back(super::toast_manager::Toast::Advancement(toast));
        }
    }
    /// `AdvancementRewards.grant`'s pickup: volume 0.2 at the player's feet,
    /// pitch `(nextFloat - nextFloat) * 0.7 + 1`, times 2. `playSound(null, …)`
    /// so the player hears it. The draws are the player's own random
    /// (`player.getRandom()`), which no level draw shares.
    pub(in crate::game) fn advancement_pickup_sound(&mut self) {
        let Some(p) = self.player.as_ref() else {
            return;
        };
        let at = [p.position.x, p.position.y, p.position.z];
        let mut random = sunrise_client::random_source::entity();
        let pitch = ((random.next_float() - random.next_float()) * 0.7 + 1.0) * 2.0;
        self.audio
            .0
            .play("minecraft:entity.item.pickup", Some(at), 0.2, pitch);
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use super::*;
    #[allow(unused_imports)]
    pub(in crate::game::client_side) use crate::game::server_side::advancements::tests::*;

    #[test]
    #[ignore = "daily"]
    fn toast_admission_cancels_edits_and_preserves_queue_behavior() {
        crate::game::in_own_process(
            "game::client_side::advancements_client::tests::toast_admission_cancels_edits_and_preserves_queue_behavior",
            || {
                use crate::game::toast_manager::{SystemToastId, Toast as QueuedToast};
                use mod_api::{ClientHost, Flow, Hooks, ModEntry, ToastAddEvent, ToastKind};
                use std::time::Duration;
                use sunrise_client::toasts::SystemToast;
                struct Listener;
                #[mod_api::__subscriptions]
                impl Hooks for Listener {
                    fn toast_add(&self, _: &dyn ClientHost, event: &mut ToastAddEvent<'_>) -> Flow {
                        match event.description.as_ref().map(|text| &text.content) {
                            Some(mod_api::text::TextContent::Literal(value))
                                if value == "cancel" =>
                            {
                                Flow::Cancel
                            }
                            Some(mod_api::text::TextContent::Literal(value)) if value == "edit" => {
                                *event.title = mod_api::Text::literal("replacement");
                                event.title.style.color = Some("#123456".into());
                                *event.description = Some(mod_api::Text::literal("changed"));
                                Flow::Continue
                            }
                            _ => Flow::Continue,
                        }
                    }
                }
                let mods: mod_api::Mods = vec![ModEntry::new("toasts", Box::new(Listener))].into();
                mod_platform::testing::with_mods(mods, || {
                    super::super::native_entity_refs::tests::with_game(|game| {
                        game.options.execution_role =
                            super::super::execution_role::ExecutionRole::Client;
                        let advancement = |name: &str| Toast {
                            icon: ItemStack::new("minecraft:stone", 1),
                            frame: Frame::Challenge,
                            title: Text::translate("advancements.toast.challenge", &[]),
                            description: Some(Text::literal(name)),
                            slide: Slide::default(),
                            recipes: vec![],
                            changed: false,
                            last_changed: 0,
                        };
                        game.add_advancement_toast(advancement("cancel"));
                        assert!(game.client.toast_manager.queued.is_empty());
                        game.add_advancement_toast(advancement("untouched"));
                        let QueuedToast::Advancement(original) =
                            &game.client.toast_manager.queued[0]
                        else {
                            panic!("expected advancement toast")
                        };
                        assert_eq!(
                            original.title.component(),
                            serde_json::json!({"translate": "advancements.toast.challenge"})
                        );
                        assert_eq!(original.description.as_ref().unwrap().plain(), "untouched");
                        assert_eq!(original.icon, ItemStack::new("minecraft:stone", 1));
                        assert!(
                            original.slide.origin.is_none(),
                            "not animated before admission"
                        );
                        game.add_advancement_toast(advancement("edit"));
                        let QueuedToast::Advancement(edited) = &game.client.toast_manager.queued[1]
                        else {
                            panic!("expected advancement toast")
                        };
                        assert_eq!(edited.title.plain(), "replacement");
                        assert_eq!(edited.title.component()["color"], "#123456");
                        assert_eq!(edited.description.as_ref().unwrap().plain(), "changed");
                        assert_eq!(edited.frame, Frame::Challenge);
                        for kind in [
                            ToastKind::Recipe,
                            ToastKind::Tutorial,
                            ToastKind::System,
                            ToastKind::Other,
                        ] {
                            let mut title = Text::literal("title");
                            let mut description = Some(Text::literal("cancel"));
                            assert!(!game.admit_toast(kind, &mut title, &mut description));
                        }
                        let mut system =
                            sunrise_client::toasts::SystemToast::new("system".into(), None);
                        system.message = Some(Text::literal("edit"));
                        assert!(game.add_system_toast(None, system));
                        let Some(QueuedToast::System(_, system)) =
                            game.client.toast_manager.queued.back()
                        else {
                            panic!("expected system toast")
                        };
                        assert_eq!(system.title.plain(), "replacement");
                        assert_eq!(system.message.as_ref().unwrap().plain(), "changed");
                        for id in [SystemToastId::PackLoadFailure, SystemToastId::Narrator] {
                            assert!(game.add_system_toast(
                                Some(id),
                                SystemToast::new("Other notice".into(), None)
                            ));
                        }
                        assert_eq!(
                            game.client.toast_manager.update(1.0),
                            (None, true),
                            "both challenges share a promotion sound"
                        );
                        assert_eq!(game.client.toast_manager.visible.len(), 5);
                        assert_eq!(game.client.toast_manager.update(1.0), (Some(true), false));

                        game.add_toast(mod_api::ModToast::new(mod_api::Text::literal(
                            "Older mod toast",
                        )))
                        .unwrap();
                        game.admit_mod_toasts();
                        game.add_advancement_toast(advancement("newer advancement"));
                        let fading = game
                            .client
                            .toast_manager
                            .system_mut(SystemToastId::PackLoadFailure)
                            .unwrap();
                        fading.slide.origin = Some(Instant::now() - Duration::from_secs(1));
                        fading.force_hide();
                        assert_eq!(game.client.toast_manager.update(1.0), (Some(false), false));
                        assert_eq!(
                            game.client.toast_manager.queued.len(),
                            2,
                            "a sliding-out toast still owns its slot"
                        );
                        game.client
                            .toast_manager
                            .system_mut(SystemToastId::PackLoadFailure)
                            .unwrap()
                            .slide
                            .origin = Some(Instant::now() - Duration::from_secs(2));
                        assert_eq!(game.client.toast_manager.update(1.0), (None, false));
                        assert!(matches!(
                            game.client
                                .toast_manager
                                .visible
                                .last()
                                .map(|entry| &entry.toast),
                            Some(QueuedToast::Mod(_))
                        ));
                        assert!(matches!(
                            game.client.toast_manager.queued.front(),
                            Some(QueuedToast::Advancement(_))
                        ));

                        let narrator = game
                            .client
                            .toast_manager
                            .system_mut(SystemToastId::Narrator)
                            .unwrap();
                        narrator.slide.origin = Some(Instant::now() - Duration::from_secs(1));
                        narrator.force_hide();
                        assert_eq!(
                            game.client.toast_manager.update(1.0),
                            (Some(false), false),
                            "the first visibility change wins, even while another family slides in"
                        );

                        for role in [
                            super::super::execution_role::ExecutionRole::IntegratedServer,
                            super::super::execution_role::ExecutionRole::DedicatedServer,
                        ] {
                            game.options.execution_role = role;
                            let mut title = Text::literal("server");
                            let mut description = Some(Text::literal("edit"));
                            assert!(!game.admit_toast(
                                ToastKind::Advancement,
                                &mut title,
                                &mut description
                            ));
                            assert_eq!(title.plain(), "server", "server must not invoke listener");
                            assert_eq!(description.unwrap().plain(), "edit");
                        }
                    });
                });
            },
        );
    }
}
