#[allow(unused_imports)]
use super::*;

impl Game<'_> {
    /// `AbstractContainerMenu.setItem`/`initializeContents`: the state id a
    /// click answers with is the open menu's, not another window's.
    pub(in crate::game) fn remote_menu_state(&mut self, container: i32, state: i32) {
        if let Some(r) = self.remote.as_mut() {
            let open = r.menu.as_ref().map_or(0, |m| m.0);
            if container == open {
                r.state_id = state;
            } else if container == 0 {
                r.inventory_state_id = state;
            }
        }
    }

    /// Change the active menu without losing window 0's cursor or state counter.
    pub(in crate::game) fn remote_set_menu(
        &mut self,
        menu: Option<(i32, String)>,
        title: Option<sunrise_client::text::Text>,
    ) {
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        match (r.menu.as_ref().is_some(), menu.is_some()) {
            (false, true) => {
                r.inventory_carried = self.carried.take();
                r.inventory_state_id = std::mem::take(&mut r.state_id);
                for (saved, active) in r.inventory_craft_grid.iter_mut().zip(&mut self.craft_grid) {
                    *saved = active.take();
                }
                r.inventory_craft_result = r.craft_result.take();
            }
            (true, false) => {
                self.carried = r.inventory_carried.take();
                r.state_id = std::mem::take(&mut r.inventory_state_id);
                self.craft_grid.fill(None);
                for (saved, active) in r.inventory_craft_grid.iter_mut().zip(&mut self.craft_grid) {
                    *active = saved.take();
                }
                r.craft_result = r.inventory_craft_result.take();
            }
            (true, true) => {
                self.carried = None;
                r.state_id = 0;
                self.craft_grid.fill(None);
                r.craft_result = None;
            }
            (false, false) => {}
        }
        r.menu.set(menu);
        r.set_menu_title(title);
    }
    /// The server's open menu: its window id and menu type (0, "inventory"
    /// when none is open).
    pub(in crate::game) fn remote_menu(&self) -> (i32, String) {
        match self.remote.as_ref().and_then(|r| r.menu.as_ref().cloned()) {
            Some((id, name)) => (id, name),
            None => (0, "inventory".to_owned()),
        }
    }
    /// `MultiPlayerGameMode.handleContainerInput` for each click of `ops` on
    /// the server's open menu (a quick craft drag is several): the verified
    /// menu engine predicts them on this client's copy, then each goes out as
    /// a `ContainerClick` with the slots it changed and the carried stack
    /// after it; the server corrects anything it sees differently. A menu the
    /// engine doesn't model sends the clicks unpredicted. False when the batch
    /// cannot be submitted.
    pub(in crate::game) fn remote_menu_ops(&mut self, ops: &[(Option<usize>, i32, Input)]) -> bool {
        // A displayed native screen whose server menu was already closed stays
        // visible until removed, but cannot submit clicks to window 0.
        if self.shown_native_screen().is_some() && self.active_native_screen().is_none() {
            return false;
        }
        // CrafterScreen.slotClicked: a slot toggle goes out before the click.
        if let [(Some(slot), button, input)] = ops {
            self.crafter_slot_clicked(*slot, *button, *input);
        }
        let (container, menu) = self.remote_menu();
        let mut clicks = Vec::with_capacity(ops.len());
        for &(slot, button, input) in ops {
            let slot = match slot {
                None => -999,
                Some(internal) => match Self::menu_slot_index(&menu, internal) {
                    Some(s) => s as i16,
                    None => return false,
                },
            };
            clicks.push((slot, button, input));
        }
        if mod_api::__private::has_part_hook(mod_api::HookId::ItemStackedOnOther)
            && clicks
                .iter()
                .any(|&(_, _, input)| matches!(input, Input::Pickup | Input::QuickCraft))
            && let Some(done) = self.remote_stacked_menu_ops(container, &clicks)
        {
            return done;
        }
        let mut prepared = None;
        let mut threw = false;
        if self
            .station
            .as_ref()
            .is_some_and(|s| s.kind == crate::game::stations::Kind::Anvil)
        {
            self.anvil_menu_session(|game, m, _| {
                let mut predicted = Vec::with_capacity(clicks.len());
                let mut dropped = false;
                for &(slot, button, input) in &clicks {
                    let before = menu_items(m);
                    let thrown = m.dropped.len();
                    if game
                        .run_client_anvil_click(m, i32::from(slot), button, input)
                        .is_err()
                    {
                        return Some(false);
                    }
                    dropped |= m.dropped.len() > thrown;
                    let changed = changed_slots(&before, m);
                    predicted.push((slot, button, input, changed, m.carried.clone()));
                }
                let packets = game.prepare_clicks(container, &predicted);
                let commit = packets.is_ok();
                prepared = Some(packets.map(|packets| (predicted, packets)));
                threw = commit && dropped;
                Some(commit)
            });
        } else {
            self.menu_session_with_game(|game, m, env, _| {
                let Some((predicted, dropped)) = predict_clicks(m, env, &clicks) else {
                    // The server throws on it too: nothing is predicted.
                    return Some(false);
                };
                let packets = game.prepare_clicks(container, &predicted);
                let commit = packets.is_ok();
                prepared = Some(packets.map(|packets| (predicted, packets)));
                threw = commit && dropped;
                Some(commit)
            });
        }
        match prepared {
            Some(Ok((predicted, packets))) => self.send_clicks(predicted, packets),
            other => {
                if let Some(Err(error)) = other {
                    log::warn!("Sending menu clicks without prediction: {error}");
                }
                self.send_unpredicted_clicks(container, clicks);
            }
        }
        // LivingEntity.drop(.., Prediction.PREDICTED): the client swings its own
        // main hand; the server shows the swing only to the other players.
        if threw {
            self.swing_hand(Hand::Main, SwingAnimation::DEFAULT);
        }
        true
    }

    /// Client-prediction path for `remote_menu_ops` when an `ItemPart` listens to
    /// `item_stacked_on_other`: splits `clicks` into gestures (each `QuickCraft` 0..=2 drag as one
    /// gesture, each other click as one gesture), runs `ItemPart` overrides on the client host
    /// (`HostSide::Client`) for each resolved pickup gesture inside one rollbackable outer
    /// transaction, hashes and sends the predicted `ContainerClick` packets, and keeps server
    /// reconciliation authoritative.
    ///
    /// Returns `None` when the open menu is not modeled by `sunrise_client::menu` (or an engine
    /// step throws), letting `remote_menu_ops` fall back to unpredicted clicks.
    pub(in crate::game) fn remote_stacked_menu_ops(
        &mut self,
        container: i32,
        clicks: &[(i16, i32, Input)],
    ) -> Option<bool> {
        use crate::game::stacked_on_other::{Pickup, local_pickup_id};

        let mut gestures: Vec<&[(i16, i32, Input)]> = Vec::new();
        let mut i = 0;
        while i < clicks.len() {
            if clicks[i].2 == Input::QuickCraft && (clicks[i].1 & 3) == 0 {
                let start = i;
                i += 1;
                while i < clicks.len() && clicks[i].2 == Input::QuickCraft {
                    let end = (clicks[i].1 & 3) == 2;
                    i += 1;
                    if end {
                        break;
                    }
                }
                gestures.push(&clicks[start..i]);
            } else {
                gestures.push(&clicks[i..i + 1]);
                i += 1;
            }
        }

        let saved_craft_grid = self.craft_grid.clone();
        let saved_craft_result = self.remote.as_ref().and_then(|r| r.craft_result.clone());
        let tracks_craft_result = matches!(
            self.remote_menu().1.as_str(),
            "inventory" | "minecraft:crafting"
        );
        let outcome = self.storage_scope(|game, _| {
            let mut all_predicted: Vec<Predicted> = Vec::with_capacity(clicks.len());
            let mut threw = false;

            for gesture in gestures {
                let mut found = None;
                let mut engine_threw = false;
                let modeled = game.menu_session(|menu, env, _| {
                    let (last, prefix) = gesture.split_last()?;
                    for &(slot, button, input) in prefix {
                        if menu.click(env, i32::from(slot), button, input).is_err() {
                            engine_threw = true;
                            return Some(false);
                        }
                    }
                    found = menu.stacked_click(env, i32::from(last.0), last.1, last.2);
                    Some(false)
                });
                if !modeled || engine_threw {
                    return (Err(None), false);
                }

                if let Some(frozen) =
                    found.filter(|f| crate::game::stacked_listens_for_side(false, f))
                {
                    let (carried, resolved_slot, primary) =
                        (frozen.carried.clone(), frozen.slot, frozen.primary);
                    let mut step_result: Option<(Vec<Predicted>, bool)> = None;
                    let pickup = game.stacked_pickup(
                        local_pickup_id(),
                        frozen.clone(),
                        move |game| (game.carried == carried).then(|| frozen.clone()),
                        |game, carried, clicked| {
                            let mut took = false;
                            let _ = game.menu_session(|menu, env, _| {
                                if let Some((_, prefix)) = gesture.split_last() {
                                    for &(slot, button, input) in prefix {
                                        if menu.click(env, i32::from(slot), button, input).is_err()
                                        {
                                            return Some(false);
                                        }
                                    }
                                }
                                took = menu.probe_carried_vanilla_override(
                                    env,
                                    resolved_slot,
                                    primary,
                                    carried,
                                    clicked,
                                );
                                Some(false)
                            });
                            took
                        },
                        |game, answer| {
                            let player = game.native_acting_id();
                            game.journal_player(player);
                            if let Some(key) = game.container {
                                game.journal_block_entity(key);
                            }
                            game.menu_session(|m, env, _| {
                                let Some(predicted) = predict_clicks_with(m, env, gesture, answer)
                                else {
                                    return Some(false);
                                };
                                step_result = Some(predicted);
                                Some(true)
                            });
                        },
                    );
                    if pickup == Pickup::Refused {
                        return (Err(Some(false)), false);
                    }
                    let Some((predicted, dropped)) = step_result else {
                        return (Err(None), false);
                    };
                    if tracks_craft_result && let Some(r) = game.remote.as_mut() {
                        for (.., changed, _) in &predicted {
                            if let Some((_, result)) = changed.iter().find(|(slot, _)| *slot == 0) {
                                r.craft_result = result.clone();
                            }
                        }
                    }
                    all_predicted.extend(predicted);
                    threw |= dropped;
                } else {
                    let player = game.native_acting_id();
                    game.journal_player(player);
                    if let Some(key) = game.container {
                        game.journal_block_entity(key);
                    }
                    let mut step_result: Option<(Vec<Predicted>, bool)> = None;
                    let modeled = game.menu_session(|m, env, _| {
                        let Some(predicted) = predict_clicks(m, env, gesture) else {
                            return Some(false);
                        };
                        step_result = Some(predicted);
                        Some(true)
                    });
                    let Some((predicted, dropped)) = step_result.filter(|_| modeled) else {
                        return (Err(None), false);
                    };
                    if tracks_craft_result && let Some(r) = game.remote.as_mut() {
                        for (.., changed, _) in &predicted {
                            if let Some((_, result)) = changed.iter().find(|(slot, _)| *slot == 0) {
                                r.craft_result = result.clone();
                            }
                        }
                    }
                    all_predicted.extend(predicted);
                    threw |= dropped;
                }
            }

            let packets = match game.prepare_clicks(container, &all_predicted) {
                Ok(packets) => packets,
                Err(error) => {
                    log::warn!("Refused menu click: {error}");
                    return (Err(Some(false)), false);
                }
            };
            (Ok((all_predicted, packets, threw)), true)
        });
        let (all_predicted, packets, threw) = match outcome {
            Ok(ready) => ready,
            Err(early) => {
                self.craft_grid = saved_craft_grid;
                if let Some(r) = self.remote.as_mut() {
                    r.craft_result = saved_craft_result;
                }
                return early;
            }
        };
        self.send_clicks(all_predicted, packets);
        if threw {
            self.swing_hand(Hand::Main, SwingAnimation::DEFAULT);
        }
        Some(true)
    }
    /// A QUICK_MOVE on the server's open menu with nothing predicted: the
    /// client's slots stay as they are and every slot it moved comes back from
    /// the server (`broadcastChanges`), the answer the next step waits for.
    /// A prediction the server agrees with gets no answer at all.
    pub(in crate::game) fn remote_quick_move_unpredicted(&mut self, internal: usize) -> bool {
        let (container, menu) = self.remote_menu();
        let Some(slot) = Self::menu_slot_index(&menu, internal) else {
            return false;
        };
        self.send_unpredicted_clicks(container, [(slot as i16, 0, Input::QuickMove)]);
        true
    }
    /// A Shift double click on a server (`AbstractContainerScreen.mouseReleased`):
    /// a predicted QUICK_MOVE for every slot the engine moves, each sent.
    pub(in crate::game) fn remote_quick_move_all(&mut self, slot: usize, moved: &ItemStack) {
        let (container, _) = self.remote_menu();
        let mut sent: Vec<Predicted> = Vec::new();
        let mut prepared = None;
        if self
            .station
            .as_ref()
            .is_some_and(|s| s.kind == crate::game::stations::Kind::Anvil)
        {
            self.anvil_menu_session(|game, m, to_menu| {
                let clicked = to_menu(slot)?;
                let mut before = menu_items(m);
                let mut modeled = true;
                if clicked < m.slot_count() && m.can_take_for_pick_all(clicked) {
                    for t in 0..m.slot_count() {
                        let eligible = {
                            let env = sunrise_client::menu::Env {
                                enchantments: game.item_enchantments(),
                                ..game.menu_env()
                            };
                            m.quick_move_all_accepts(&env, clicked, t, moved)
                        };
                        if eligible {
                            if game
                                .run_client_anvil_click(m, t as i32, 0, Input::QuickMove)
                                .is_err()
                            {
                                modeled = false;
                                break;
                            }
                            let changed = changed_slots(&before, m);
                            sent.push((t as i16, 0, Input::QuickMove, changed, m.carried.clone()));
                            before = menu_items(m);
                        }
                    }
                }
                let packets = game.prepare_clicks(container, &sent);
                let commit = modeled && packets.is_ok();
                prepared = Some(packets);
                Some(commit)
            });
        } else {
            self.menu_session_with_game(|game, m, env, to_menu| {
                let clicked = to_menu(slot)?;
                let mut before = menu_items(m);
                let mut each = |m: &sunrise_client::menu::Menu, t: usize| {
                    let changed = changed_slots(&before, m);
                    sent.push((t as i16, 0, Input::QuickMove, changed, m.carried.clone()));
                    before = menu_items(m);
                };
                let modeled = m
                    .quick_move_all_each(env, clicked, moved, &mut each)
                    .is_ok();
                let packets = game.prepare_clicks(container, &sent);
                let commit = modeled && packets.is_ok();
                prepared = Some(packets);
                Some(commit)
            });
        }
        match prepared {
            Some(Ok(packets)) => self.send_clicks(sent, packets),
            Some(Err(error)) => {
                log::warn!("Sending quick moves without prediction: {error}");
                self.send_unpredicted_clicks(
                    container,
                    sent.into_iter()
                        .map(|(slot, button, input, ..)| (slot, button, input)),
                );
            }
            None => (),
        }
    }
    /// Don't publish an unhashable local prediction. The server runs the click
    /// normally and broadcasts changed slots; an unhashable cursor claims no match.
    pub(in crate::game) fn send_unpredicted_clicks(
        &mut self,
        container: i32,
        clicks: impl IntoIterator<Item = (i16, i32, Input)>,
    ) {
        let state = self.remote.as_ref().map_or(0, |r| r.state_id);
        let carried = match self.hashed_stack(&self.carried) {
            Ok(carried) => carried,
            Err(error) => {
                log::warn!("Cannot hash the carried stack for a menu click: {error}");
                None
            }
        };
        for (slot, button, input) in clicks {
            self.send(S::ContainerClick(play::ContainerClick {
                container,
                state,
                slot,
                button: button as i8,
                input: input_ordinal(input),
                changed: Vec::new(),
                carried: carried.clone(),
            }));
        }
    }

    pub(in crate::game) fn prepare_clicks(
        &self,
        container: i32,
        clicks: &[Predicted],
    ) -> sunrise_protocol::Result<Vec<play::ContainerClick>> {
        let state = self.remote.as_ref().map_or(0, |r| r.state_id);
        clicks
            .iter()
            .map(|(slot, button, input, changed, carried)| {
                Ok(play::ContainerClick {
                    container,
                    state,
                    slot: *slot,
                    button: *button as i8,
                    input: input_ordinal(*input),
                    changed: changed
                        .iter()
                        .map(|(i, stack)| Ok((*i, self.hashed_stack(stack)?)))
                        .collect::<sunrise_protocol::Result<_>>()?,
                    carried: self.hashed_stack(carried)?,
                })
            })
            .collect()
    }

    pub(in crate::game) fn send_clicks(
        &mut self,
        clicks: Vec<Predicted>,
        packets: Vec<play::ContainerClick>,
    ) {
        // A result the engine claims to have changed is the one the server
        // agrees to (through `HashedStack.matches`), so it sends none: keep it. A result the
        // engine doesn't claim (a recipe it doesn't know) the server sends.
        if matches!(
            self.remote_menu().1.as_str(),
            "inventory" | "minecraft:crafting"
        ) && let Some(r) = self.remote.as_mut()
        {
            for (.., changed, _) in &clicks {
                if let Some((_, result)) = changed.iter().find(|(slot, _)| *slot == 0) {
                    r.craft_result = result.clone();
                }
            }
        }
        for packet in packets {
            self.send(S::ContainerClick(packet));
        }
    }
    /// The server's open menu is the inventory or a chest-like container
    /// (chests, barrels, ender chests: `generic_9xN`; shulker boxes).
    pub(in crate::game) fn remote_storage_open(&self) -> bool {
        match self.remote.as_ref().and_then(|r| r.menu.as_ref()) {
            None => true,
            Some((_, name)) => {
                name == "inventory"
                    || name.starts_with("minecraft:generic_9x")
                    || name == "minecraft:shulker_box"
            }
        }
    }
    /// A click outside a server's window (slot -999): predicted like
    /// `MultiPlayerGameMode.handleContainerInput` (the carried stack after
    /// `doClick`; the server drops the stack and swings the hand).
    pub(in crate::game) fn remote_throw(
        &mut self,
        right: bool,
        input: sunrise_client::menu::Input,
    ) {
        use sunrise_client::menu::Input;
        let container = self
            .remote
            .as_ref()
            .and_then(|r| r.menu.as_ref())
            .map_or(0, |m| m.0);
        let mut predicted = self.carried.clone();
        let threw = input == Input::Pickup && predicted.is_some();
        if input == Input::Pickup
            && let Some(c) = predicted.as_mut()
        {
            if right && c.count > 1 {
                c.count -= 1;
            } else {
                predicted = None;
            }
        }
        let carried = match self.hashed_stack(&predicted) {
            Ok(carried) => carried,
            Err(error) => {
                log::warn!("Sending drop click without prediction: {error}");
                self.send_unpredicted_clicks(container, [(-999, i32::from(right), input)]);
                return;
            }
        };
        if threw {
            self.carried = predicted;
            self.swing_arm();
        }
        let state = self.remote.as_ref().map_or(0, |r| r.state_id);
        self.send(S::ContainerClick(play::ContainerClick {
            container,
            state,
            slot: -999,
            button: i8::from(right),
            input: if input == Input::Throw { 4 } else { 0 },
            changed: Vec::new(),
            carried,
        }));
    }

    /// The creative screen calls `Player.drop` before `handleCreativeModeItemDrop`;
    /// its predicted swing is not conditional on whether a packet can go out.
    pub(in crate::game) fn creative_screen_drop(&mut self, thrown: ItemStack) {
        if !thrown.is_empty() {
            self.swing_hand(Hand::Main, SwingAnimation::DEFAULT);
        }
        self.remote_creative_throw(thrown);
    }

    pub(in crate::game) fn remote_close_container(&mut self) {
        let id = self
            .remote
            .as_ref()
            .and_then(|r| r.menu.as_ref().map(|m| m.0))
            .unwrap_or(0);
        self.send(S::ContainerClose(play::ContainerClose { container: id }));
        self.remote_close_local();
    }
    /// `LocalPlayer.closeContainer` as the creative inventory closes: its
    /// `ItemPickerMenu` is window 0, and a server hears the close as it does
    /// any container screen's (`AbstractContainerScreen.onClose`).
    pub(in crate::game) fn close_creative_menu(&self) {
        self.send(S::ContainerClose(play::ContainerClose { container: 0 }));
    }
    /// `MultiPlayerGameMode.handleInventoryButtonClick`: button `button` of
    /// the menu the server opened goes to it (`ContainerButtonClick`), whose
    /// menu decides what it does (`clickMenuButton`). No sound: that is the
    /// screen's. False while no such menu is open.
    pub(in crate::game) fn remote_button_click(&self, button: i32) -> bool {
        let Some(container) = self
            .remote
            .as_ref()
            .and_then(|r| r.menu.as_ref())
            .map(|(w, _)| *w)
        else {
            return false;
        };
        self.send(S::ContainerButtonClick(play::ContainerButtonClick {
            container,
            button,
        }));
        true
    }
    pub(in crate::game) fn remote_close_local(&mut self) {
        self.remote_set_menu(None, None);
        self.remove_remote_menu_screen();
    }

    /// A server's trading window (MerchantScreen over a ClientSideMerchant).
    pub(in crate::game) fn remote_open_merchant(
        &mut self,
        container: i32,
        title: String,
        text: sunrise_client::text::Text,
    ) {
        self.remote_set_menu(
            Some((container, "minecraft:merchant".to_owned())),
            Some(text.clone()),
        );
        let mut session = sunrise_client::merchant::Session::new(Vec::new(), true);
        session.client_side = true;
        self.open_remote_merchant(session, (!title.is_empty()).then_some(text));
    }
    /// ClientboundMerchantOffersPacket.
    pub(in crate::game) fn remote_merchant_offers(&mut self, p: play::MerchantOffers) {
        if self.open_menu_name(p.container) != "minecraft:merchant" {
            return;
        }
        let cost = |g: &Self, c: &play::NetCost| {
            g.to_stack(&Some(c.clone().into()))
                .map(|stack| sunrise_client::merchant::ItemCost { stack })
        };
        let offers: Vec<_> = p
            .offers
            .iter()
            .filter_map(|o| {
                let mut offer = sunrise_client::merchant::MerchantOffer {
                    base_cost_a: cost(self, &o.cost_a)?,
                    cost_b: match &o.cost_b {
                        Some(b) => Some(cost(self, b)?),
                        None => None,
                    },
                    result: self.to_stack(&Some(o.result.clone()))?,
                    uses: o.uses,
                    max_uses: o.max_uses,
                    reward_exp: true,
                    special_price_diff: o.special_price,
                    demand: o.demand,
                    price_multiplier: o.price_multiplier,
                    xp: o.xp,
                };
                // setToOutOfStock.
                if o.out_of_stock {
                    offer.uses = offer.max_uses;
                }
                Some(offer)
            })
            .collect();
        if let Some(m) = self.station.as_mut().and_then(|s| s.merchant.as_mut()) {
            m.session.offers = offers;
            m.session.level = p.level;
            m.session.xp = p.xp;
            m.session.villager = p.show_progress;
        }
    }
    /// The type of the menu a server opened (`MenuType`'s id).
    pub(in crate::game) fn remote_menu_type(&self) -> Option<&str> {
        Some(self.remote.as_ref()?.menu.as_ref()?.1.as_str())
    }
    /// The title the server gave the menu it opened.
    pub(in crate::game) fn remote_menu_title(&self) -> Option<sunrise_client::text::Text> {
        self.remote.as_ref()?.menu_title.clone()
    }
    /// A language change reaches the server-sent components too: their titles
    /// are resolved when the packet arrives, with the session's language.
    pub(in crate::game) fn refresh_remote_language(&mut self) {
        if let Some(lang) = sunrise_client::lang::current()
            && let Some(r) = self.remote.as_mut()
        {
            r.lang = Arc::new(lang.map().clone());
        }
    }
    /// A server's chest-like menu (`MenuType.GENERIC_9xN`): its rows and title.
    pub(in crate::game) fn remote_chest_menu(&self) -> Option<(usize, sunrise_client::text::Text)> {
        let r = self.remote.as_ref()?;
        let rows = r
            .menu
            .as_ref()?
            .1
            .strip_prefix("minecraft:generic_9x")?
            .parse()
            .ok()?;
        Some((
            rows,
            r.menu_title
                .clone()
                .unwrap_or_else(sunrise_client::text::Text::empty),
        ))
    }
}
