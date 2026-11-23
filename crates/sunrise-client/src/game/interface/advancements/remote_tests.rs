use super::*;
use crate::game::{in_own_process, native_entity_refs::tests::with_client_game};
use sunrise_core::nbt::Tag;
use sunrise_protocol::{
    item::Stack,
    packets::play::{Advancement, AdvancementDisplay, UpdateAdvancements},
};

fn displayed(id: &str, parent: Option<&str>, x: f32, y: f32) -> Advancement {
    Advancement {
        id: id.into(),
        parent: parent.map(str::to_owned),
        display: Some(AdvancementDisplay {
            title: Tag::String(id.into()),
            description: Tag::String("Test advancement".into()),
            icon: Stack::new(1, 1),
            frame: 0,
            show_toast: true,
            hidden: false,
            background: parent.is_none().then(|| "test:background".into()),
        }),
        requirements: vec![vec!["c".into()]],
        telemetry: false,
        x,
        y,
    }
}

#[test]
#[ignore = "daily"]
fn remote_tree_rejects_unresolved_nodes_and_retains_removed_progress() {
    in_own_process(
        "game::advancements_screen::remote_tests::remote_tree_rejects_unresolved_nodes_and_retains_removed_progress",
        || {
            with_client_game(|game| {
                game.init_remote_for_replay().unwrap();
                let root = displayed("test:root", None, 0.0, 0.0);
                let child = displayed("test:child", Some(&root.id), 1.0, 1.5);
                let grand = displayed("test:grand", Some(&child.id), 2.0, 3.0);
                let sibling = displayed("test:sibling", Some(&root.id), 1.0, 2.5);
                let other = displayed("test:other", None, 0.0, 0.0);
                let orphan = displayed("test:orphan", Some("test:missing_parent"), 9.0, 9.0);
                let cyc_a = displayed("test:cyc_a", Some("test:cyc_b"), 4.0, 0.0);
                let cyc_b = displayed("test:cyc_b", Some(&cyc_a.id), 5.0, 0.0);
                game.remote_advancements(UpdateAdvancements {
                    reset: false,
                    added: vec![
                        grand.clone(),
                        child.clone(),
                        orphan.clone(),
                        cyc_a,
                        cyc_b,
                        root.clone(),
                        sibling.clone(),
                        other.clone(),
                    ],
                    removed: vec![],
                    progress: [&root, &child, &grand, &orphan]
                        .into_iter()
                        .map(|a| (a.id.clone(), vec![("c".into(), Some(0))]))
                        .collect(),
                    show: true,
                });
                assert_eq!(
                    game.ui.registries.advancements,
                    [&root, &sibling, &other, &child, &grand].map(|a| a.id.clone()),
                );
                let position = |game: &Game<'_>, a: &Advancement| {
                    let i = game.adv.tree.by_id[&a.id];
                    game.adv.positions.as_ref().unwrap()[i]
                };
                assert_eq!(position(game, &grand), (2.0, 3.0));
                assert_eq!(position(game, &sibling), (1.0, 2.5));
                assert_eq!(game.adv.progress.criteria.len(), 3);
                assert_eq!(game.toast_manager.queued.len(), 3);

                game.set_screen(Screen::Advancements);
                game.adv_ui.tab = Some(game.adv.tree.by_id[&other.id]);
                game.adv_ui.scroll = Some((12.0, 34.0));
                game.remote_advancements(UpdateAdvancements {
                    reset: false,
                    added: vec![],
                    removed: vec![root.id.clone()],
                    progress: vec![(grand.id.clone(), vec![("c".into(), None)])],
                    show: true,
                });
                assert_eq!(
                    game.ui.registries.advancements,
                    std::slice::from_ref(&other.id)
                );
                assert!(game.adv.progress.has(&root.id, "c"));
                assert!(game.adv.progress.has(&grand.id, "c"));
                assert_eq!(game.adv_view().tabs, [0]);
                assert_eq!(game.adv_ui.tab, Some(0));
                assert_eq!(game.adv_ui.scroll, Some((12.0, 34.0)));

                game.remote_advancements(UpdateAdvancements {
                    reset: false,
                    added: vec![child.clone(), root.clone()],
                    removed: vec![other.id],
                    progress: vec![],
                    show: true,
                });
                assert_eq!(game.adv_view().tabs, [0]);
                assert_eq!(game.adv_ui.tab, Some(0));
                assert_eq!(game.adv_ui.scroll, None);
                assert!(game.adv_done(game.adv.tree.by_id[&child.id]));
                assert_eq!(position(game, &child), (1.0, 1.5));
                assert_eq!(game.toast_manager.queued.len(), 3);

                game.remote_advancements(UpdateAdvancements {
                    reset: true,
                    added: vec![root.clone()],
                    removed: vec![],
                    progress: vec![],
                    show: true,
                });
                assert_eq!(game.ui.registries.advancements, [root.id]);
                assert!(game.adv.progress.criteria.is_empty());
            });
        },
    );
}
