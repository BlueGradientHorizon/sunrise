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
    /// The `AdvancementToast` or `RecipeToast` in its slot at `y`.
    pub(in crate::game) fn draw_advancement_toast(
        &self,
        batch: &mut HudBatch,
        t: &Toast,
        y: f32,
        multiplier: f64,
    ) {
        let x = batch.width - 160.0 * t.slide.portion;
        let fully_for = t.slide.fully_for;
        let (title, frame, icon) = (&t.title, t.frame, &t.icon);
        let empty = Text::literal("");
        let description = t.description.as_ref().unwrap_or(&empty);
        let recipe = t
            .is_recipe()
            .then(|| &t.recipes[t.displayed_recipe(fully_for, multiplier)]);
        if let Some((station, item)) = recipe {
            // RecipeToast: black description under a purple title, station scaled 0.6 over result.
            let atlas = &self.gui.atlas;
            batch.sprite(atlas, "toast/recipe", x, y);
            batch.text_styled(atlas, title, x + 30.0, y + 7.0, 0xFF50_0050, false);
            batch.text_styled(atlas, description, x + 30.0, y + 18.0, 0xFF00_0000, false);
            let start = batch.vertices.len();
            self.draw_item_model(batch, &ItemStack::new(station, 1), x + 3.0, y + 3.0);
            batch.scale_since(start, x, y, 0.6, 0.6);
            self.draw_item_model(batch, item, x + 8.0, y + 8.0);
            return;
        }
        let atlas = &self.gui.atlas;
        batch.sprite(atlas, "toast/advancement", x, y);
        let (_, color) = toast_label(frame);
        // AdvancementToast.extractRenderState: single line sits under label; longer wraps at 125.
        let lines = atlas.font.split(&description.resolve_legacy(), 125.0);
        let label = title;
        let rgb = color & 0x00FF_FFFF;
        if let [line] = lines.as_slice() {
            batch.text_styled(atlas, label, x + 30.0, y + 7.0, color, false);
            batch.text_styled(atlas, line, x + 30.0, y + 18.0, u32::MAX, false);
        } else if fully_for < 1500 {
            let fade = ((1500 - fully_for) as f32 / 300.0).clamp(0.0, 1.0);
            let alpha = (fade * 255.0).floor() as u32;
            if alpha != 0 {
                batch.text_styled(atlas, label, x + 30.0, y + 11.0, alpha << 24 | rgb, false);
            }
        } else {
            let fade = ((fully_for - 1500) as f32 / 300.0).clamp(0.0, 1.0);
            let alpha = (fade * 252.0).floor() as u32;
            if alpha != 0 {
                let mut line_y = y + (16 - lines.len() as i32 * 9 / 2) as f32;
                for line in &lines {
                    batch.text_styled(
                        atlas,
                        line,
                        x + 30.0,
                        line_y,
                        alpha << 24 | 0x00FF_FFFF,
                        false,
                    );
                    line_y += 9.0;
                }
            }
        }
        self.draw_item(batch, icon, x + 8.0, y + 8.0);
    }
    /// `ClientAdvancements.update`: update the tree before applying progress.
    pub(in crate::game) fn remote_advancements(
        &mut self,
        p: sunrise_protocol::packets::play::UpdateAdvancements,
    ) {
        if p.reset {
            self.adv.progress = Progress::default();
        }
        if self.adv.remote_tree.is_none() || p.reset || !p.removed.is_empty() || !p.added.is_empty()
        {
            self.update_remote_advancement_tree(p.reset, &p.removed, p.added);
        }
        let tree = self.adv.tree.clone();
        for (id, criteria) in p.progress {
            let Some(a) = tree.get(&id) else {
                log::warn!("Server sent progress for unknown advancement {id}");
                continue;
            };
            self.adv.progress.set_remote(
                a,
                criteria
                    .into_iter()
                    .filter_map(|(name, at)| at.map(|_| name)),
            );
            if !p.reset
                && p.show
                && self.adv.progress.done(a)
                && let Some(d) = a.display.as_ref().filter(|d| d.show_toast)
            {
                self.add_advancement_toast(Toast {
                    icon: d.icon.clone(),
                    frame: d.frame,
                    title: Text::translate(toast_label(d.frame).0, &[]),
                    description: Some(text_of(&d.title)),
                    slide: Slide::default(),
                    recipes: Vec::new(),
                    changed: false,
                    last_changed: 0,
                });
            }
        }
        self.remote_advancement_tabs_updated(p.reset);
    }
    /// `AdvancementTree.remove` and `AdvancementTree.addAll`: prune subtrees, then admit additions.
    pub(in crate::game) fn update_remote_advancement_tree(
        &mut self,
        reset: bool,
        removed: &[String],
        added: Vec<sunrise_protocol::packets::play::Advancement>,
    ) {
        let mut tree = Tree::default();
        let mut remote = RemoteTree::default();
        if !reset && let Some(old_remote) = self.adv.remote_tree.take() {
            let old = &self.adv.tree;
            remote.nodes = old_remote.nodes;
            let mut deleted = vec![false; old.list.len()];
            let mut stack = Vec::new();
            for id in removed {
                if let Some(&i) = old.by_id.get(id)
                    && !deleted[i]
                {
                    stack.push(old_remote.current[i]);
                    while let Some(node) = stack.pop() {
                        let node = &remote.nodes[node];
                        if let Some(&i) = old.by_id.get(&node.id) {
                            deleted[i] = true;
                        }
                        // Even if this ID is gone, this node can still point to live descendants.
                        stack.extend(node.children.iter().copied());
                    }
                } else {
                    log::warn!("Server removed unknown advancement {id}");
                }
            }
            for (i, a) in old.list.iter().enumerate() {
                if !deleted[i] {
                    tree.by_id.insert(a.id.clone(), tree.list.len());
                    tree.list.push(a.clone());
                    remote.current.push(old_remote.current[i]);
                    remote.positions.push(old_remote.positions[i]);
                }
            }
        } else {
            for id in removed {
                log::warn!("Server removed unknown advancement {id}");
            }
        }

        let locations: Vec<_> = added.iter().map(|a| (a.id.clone(), (a.x, a.y))).collect();
        let mut pending = added;
        let mut unresolved = Vec::new();
        while !pending.is_empty() {
            let before = pending.len();
            for a in pending.drain(..) {
                let parent = if let Some(id) = &a.parent {
                    let Some(&i) = tree.by_id.get(id) else {
                        unresolved.push(a);
                        continue;
                    };
                    Some(remote.current[i])
                } else {
                    None
                };
                let a = self.remote_advancement(a);
                let node = remote.nodes.len();
                if let Some(parent) = parent {
                    remote.nodes[parent].children.push(node);
                }
                remote.nodes.push(RemoteNode {
                    id: a.id.clone(),
                    parent,
                    children: Vec::new(),
                    displayed: a.display.is_some(),
                });
                let i = *tree.by_id.entry(a.id.clone()).or_insert(tree.list.len());
                if i == tree.list.len() {
                    tree.list.push(a);
                    remote.current.push(node);
                    remote.positions.push((0.0, 0.0));
                } else {
                    tree.list[i] = a;
                    remote.current[i] = node;
                }
            }
            if unresolved.len() == before {
                let ids: Vec<_> = unresolved.iter().map(|a| &a.id).collect();
                log::error!("Couldn't load advancements: {ids:?}");
                break;
            }
            std::mem::swap(&mut pending, &mut unresolved);
        }
        // ClientAdvancements.update applies all locations in packet order.
        for (id, position) in locations {
            if let Some(&i) = tree.by_id.get(&id) {
                remote.positions[i] = position;
            }
        }
        remote.prune_unreachable();
        self.ui.registries.advancements = tree.list.iter().map(|a| a.id.clone()).collect();
        self.adv.tree = Arc::new(tree);
        self.adv.remote_tree = Some(remote);
    }
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
