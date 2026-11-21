//! Hub clicks stage their slot edits, so rule queries can read the live Game and its guests
//! before the click writes its answer back.
use super::*;
use std::{cell::RefCell, rc::Rc};
use sunrise_client::{
    lan::menu::ContainerItems,
    menu::{PlanningRules, RuleAnswers, StackedOutcome, StoredRules},
};

impl Game<'_> {
    /// `ServerGamePacketListenerImpl.handleContainerClick` for the guest `conn`. The hub reads
    /// what the click may do and the container it clicks; a pickup the mods listen to is then
    /// decided with `stacked_pickup`. Slot rules read the same live game as a menu view. Returns
    /// the stacks the click threw out.
    pub(super) fn lan_hub_click(&mut self, conn: u32, p: &play::ContainerClick) -> Vec<ItemStack> {
        let Some(host) = self.lan.as_mut() else {
            return Vec::new();
        };
        let Some(mut prepared) = host.hub.prepare_click(conn, p, &self.world) else {
            return Vec::new();
        };
        let frozen = if Self::stacked_listened() {
            host.hub.stacked_click(&mut prepared, p)
        } else {
            None
        };
        let route = host.menus.entry(conn).or_default().hub.id();
        // The block entities behind the container the click writes into.
        let blocks: Vec<_> = host
            .hub
            .guest(conn)
            .and_then(|g| g.menu.as_ref())
            .map(|m| match m.slots {
                Slots::Block(key) => vec![key],
                Slots::Double(first, second) => vec![first, second],
                _ => Vec::new(),
            })
            .unwrap_or_default();
        let answers = RuleAnswers::default();
        let prepared = Rc::new(RefCell::new(Some(prepared)));
        let drops = Rc::new(RefCell::new(Vec::new()));
        if let Some(frozen) = frozen {
            let (shared, thrown, packet) = (prepared.clone(), drops.clone(), p.clone());
            let stored = &answers;
            let (stacks, slot) = (frozen.clone(), frozen.slot);
            self.stacked_pickup(
                route,
                frozen,
                move |game| {
                    let (carried, clicked) =
                        game.lan
                            .as_ref()?
                            .hub
                            .click_stacks(conn, slot, &game.world)?;
                    (carried == stacks.carried && clicked == stacks.clicked).then(|| stacks.clone())
                },
                |game, answer| {
                    let mut prepared = prepared.borrow_mut();
                    let (Some(prepared), Some(host)) = (prepared.as_mut(), game.lan.as_ref())
                    else {
                        answer.refused.set(true);
                        return;
                    };
                    let planning = PlanningRules {
                        rules: Some(&*game),
                        answers: &answers,
                    };
                    let insert_rules = prepared.container_insert_rules().to_vec();
                    let rules = ContainerItems {
                        items: world_items!(game),
                        enchantments: &world_stations!(game).enchantments,
                        slot_rules: Some(&planning),
                        stacked: Some(answer),
                        insert_rules: &insert_rules,
                    };
                    host.hub.plan_click(prepared, p, &rules);
                },
                move |game, answer| {
                    // What the click writes belongs to the action: the guest's inventory and
                    // cursor, and the container's block entities.
                    game.journal_guest_click(conn);
                    for key in blocks {
                        game.journal_block_entity(key);
                    }
                    game.hub_click_run(&packet, &shared, &thrown, answer, Some(stored))
                },
            );
        } else {
            self.hub_click_run(p, &prepared, &drops, None, None);
        }
        // A click the decision refused never ran: the container, the drag and the client are put
        // back as they were.
        if let Some(prepared) = prepared.take()
            && let Some(host) = self.lan.as_mut()
        {
            host.hub
                .finish_click(prepared, p, &mut self.world, (Vec::new(), Vec::new()));
        }
        drops.take()
    }

    /// Run the prepared click with the live game lent to the read-only slot rules, then apply.
    fn hub_click_run(
        &mut self,
        p: &play::ContainerClick,
        prepared: &RefCell<Option<sunrise_client::lan::containers::PreparedClick>>,
        drops: &RefCell<Vec<ItemStack>>,
        answer: Option<&StackedOutcome>,
        stored: Option<&RuleAnswers>,
    ) {
        let Some(mut click) = prepared.take() else {
            return;
        };
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        // Decided clicks use only the answers their plan kept; plain ones ask the game.
        let stored = stored.map(StoredRules);
        let insert_rules = click.container_insert_rules().to_vec();
        let rules = ContainerItems {
            items: world_items!(self),
            enchantments: &world_stations!(self).enchantments,
            slot_rules: match &stored {
                Some(stored) => Some(stored),
                None => Some(&*self),
            },
            stacked: answer,
            insert_rules: &insert_rules,
        };
        let outcome = host.hub.run_click(&mut click, p, &rules);
        if let Some(host) = self.lan.as_mut() {
            drops
                .borrow_mut()
                .extend(host.hub.finish_click(click, p, &mut self.world, outcome));
        }
    }
}
