#[allow(unused_imports)]
pub(in crate::game::client_side) use super::{
    Game,
    stations::{INPUT, Kind, RESULT, Station},
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::trading::GREY;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::trading::GameMaps;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::trading::MadeMaps;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::trading::MerchantState;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::trading::SHIFT;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::trading::drag_scroll;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::trading::inventory_slot;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::trading::merchant_display_name;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::trading::merchant_title;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::trading::on_scroller;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::trading::scroller_cursor;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::trading::scroller_rect;
#[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::trading::*;
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::mobs::{
    Mob, Species, render::VILLAGER_TYPES, villager,
};
#[allow(unused_imports)]
pub(in crate::game::client_side) use sunrise_client::{
    hud::HudBatch,
    items::ItemStack,
    merchant::{self, MerchantOffer, TradeContext, TradeFeedback},
    random_sequences::RandomSequences,
    text::Text,
    widgets::{Cursor, request_cursor},
};

impl Game<'_> {
    /// `MerchantScreen` rendering.
    pub(in crate::game::client_side) fn draw_merchant(&mut self, batch: &mut HudBatch) -> bool {
        let (ox, oy) = self.inventory_origin(batch);
        let local = (self.client.cursor.0 - ox, self.client.cursor.1 - oy);
        let Some(m) = self.station.as_ref().and_then(|s| s.merchant.as_ref()) else {
            return false;
        };
        let (progress, custom) = (m.progress(), m.name.clone());
        let (profession, level, xp, scroll, shop, future, focused, dragging) = (
            m.session
                .profession
                .trim_start_matches("minecraft:")
                .to_owned(),
            m.session.level,
            m.session.xp,
            m.scroll,
            m.session.selection_hint,
            m.session.future_xp,
            m.focused,
            m.dragging,
        );
        let offers = m.session.offers.clone();
        let costs: Vec<ItemStack> = offers.iter().map(|o| o.cost_a(self.items())).collect();
        self.render_transparent_background("merchant", batch);
        let atlas = &self.gui.atlas;
        if let Some(mut r) = atlas.sprites.get("gui/container/villager").copied() {
            let (du, dv) = ((r.u1 - r.u0) / r.w as f32, (r.v1 - r.v0) / r.h as f32);
            r.u1 = r.u0 + 276.0 * du;
            r.v1 = r.v0 + 166.0 * dv;
            batch.rect_uv(ox + SHIFT, oy, 276.0, 166.0, r, u32::MAX);
        }
        self.draw_screen_phase(batch, sunrise_client::screen_draw::RenderPhase::Background);
        let tr = |k: &str, f: &str| sunrise_client::lang::get_or(k, f);
        let name = merchant_display_name(progress, &profession, custom);
        // The screen's title is the merchant's name; its label adds the level.
        super::screen_info::name_container(|| name.plain());
        // `StringDecomposer.iterateFormatted`: `§` codes a name holds apply at draw time.
        let title = merchant_title(progress, level, name).resolve_legacy();
        let tw = atlas.font.width_text(&title);
        batch.text_styled(
            atlas,
            &title,
            ox + SHIFT + (49.0 + 138.0 - (tw / 2.0).floor()),
            oy + 6.0,
            GREY,
            false,
        );
        let trades = tr("merchant.trades", "Trades");
        let tw = HudBatch::text_width(atlas, &trades);
        batch.text(
            atlas,
            &trades,
            ox + SHIFT + 5.0 - (tw / 2.0).floor() + 48.0,
            oy + 6.0,
            GREY,
            false,
        );
        batch.text(
            atlas,
            &tr("container.inventory", "Inventory"),
            ox + SHIFT + 107.0,
            oy + 72.0,
            GREY,
            false,
        );
        if offers.get(shop).is_some_and(MerchantOffer::out_of_stock) {
            batch.sprite(
                atlas,
                "container/villager/out_of_stock",
                ox + SHIFT + 182.0,
                oy + 35.0,
            );
        }
        // Experience bar (levels 1-4).
        if progress && level < 5 {
            batch.sprite(
                atlas,
                "container/villager/experience_bar_background",
                ox + SHIFT + 136.0,
                oy + 16.0,
            );
            let min = merchant::min_xp(level);
            if xp >= min && merchant::can_level_up(level) {
                let mult = 102.0 / (merchant::max_xp(level) - min) as f32;
                let w = ((mult * (xp - min) as f32).floor()).min(102.0);
                batch.sprite_part(
                    atlas,
                    "container/villager/experience_bar_current",
                    ox + SHIFT + 136.0,
                    oy + 16.0,
                    w / 102.0,
                );
                if future > 0 {
                    let fw = (future as f32 * mult).floor().min(102.0 - w);
                    if let Some(mut r) = atlas
                        .sprites
                        .get("container/villager/experience_bar_result")
                        .copied()
                    {
                        let du = (r.u1 - r.u0) / r.w as f32;
                        r.u0 += w * du;
                        r.u1 = r.u0 + fw * du;
                        batch.rect_uv(ox + SHIFT + 136.0 + w, oy + 16.0, fw, 5.0, r, u32::MAX);
                    }
                }
            }
        }
        // Scroller (`extractScroller`): its thumb, or the disabled sprite.
        if let Some([x, y, w, h]) = scroller_rect(offers.len(), scroll) {
            batch.sprite(atlas, "container/villager/scroller", ox + x, oy + y);
            if let Some(cursor) = scroller_cursor(local, [x, y, w, h], dragging) {
                request_cursor(cursor);
            }
        } else {
            batch.sprite(
                atlas,
                "container/villager/scroller_disabled",
                ox + SHIFT + 94.0,
                oy + 18.0,
            );
        }
        // Offer buttons.
        let mut offer_tooltip: Option<ItemStack> = None;
        for (row, (index, offer)) in offers.iter().enumerate().skip(scroll).take(7).enumerate() {
            let (bx, by) = (ox + SHIFT + 5.0, oy + 18.0 + 20.0 * row as f32);
            let hover = local.0 >= 5.0 + SHIFT
                && local.0 < 93.0 + SHIFT
                && local.1 >= 18.0 + 20.0 * row as f32
                && local.1 < 38.0 + 20.0 * row as f32;
            let sprite = if hover || focused == Some(row) {
                "widget/button_highlighted"
            } else {
                "widget/button"
            };
            batch.sprite_scaled(&self.gui.atlas, sprite, bx, by, 88.0, 20.0, u32::MAX);
            let y = oy + 16.0 + 1.0 + 20.0 * row as f32 + 2.0;
            let base = offer.base_cost_a.stack.clone();
            let cost_a = costs[index].clone();
            let x = ox + SHIFT + 10.0;
            if base.count == cost_a.count {
                self.draw_item(batch, &cost_a, x, y);
            } else {
                // Discounted: the base price struck through, then the new price.
                self.draw_item(
                    batch,
                    &ItemStack {
                        count: 1,
                        ..cost_a.clone()
                    },
                    x,
                    y,
                );
                let atlas = &self.gui.atlas;
                let t = base.count.to_string();
                batch.text(
                    atlas,
                    &t,
                    x + 17.0 - HudBatch::text_width(atlas, &t),
                    y + 9.0,
                    u32::MAX,
                    true,
                );
                let t = cost_a.count.to_string();
                batch.text(
                    atlas,
                    &t,
                    x + 14.0 + 17.0 - HudBatch::text_width(atlas, &t),
                    y + 9.0,
                    u32::MAX,
                    true,
                );
                batch.sprite(
                    atlas,
                    "container/villager/discount_strikethrough",
                    x + 7.0,
                    y + 12.0,
                );
            }
            if let Some(b) = offer.cost_b() {
                self.draw_item(batch, &b, ox + SHIFT + 40.0, y);
            }
            let arrow = if offer.out_of_stock() {
                "trade_arrow_out_of_stock"
            } else {
                "trade_arrow"
            };
            batch.sprite(
                &self.gui.atlas,
                &format!("container/villager/{arrow}"),
                ox + SHIFT + 60.0,
                y + 3.0,
            );
            self.draw_item(batch, &offer.result, ox + SHIFT + 73.0, y);
            if hover {
                let rel = local.0 - (5.0 + SHIFT);
                offer_tooltip = if rel < 20.0 {
                    Some(cost_a)
                } else if rel > 30.0 && rel < 50.0 {
                    offer.cost_b()
                } else if rel > 65.0 {
                    Some(offer.result.clone())
                } else {
                    None
                };
            }
        }
        // Slots, hover highlight, carried stack, tooltip.
        let slots: Vec<usize> = (0..36).chain([INPUT, INPUT + 1, RESULT]).collect();
        let drag = self.drag_view();
        self.draw_hovered_slot_highlight(batch, false, ox, oy);
        for i in slots {
            let (sx, sy) = self.slot_position(i);
            let stack = self.slot_mut(i).clone();
            self.draw_slot_item(batch, drag.as_ref(), i, &stack, ox + sx, oy + sy);
        }
        self.draw_hovered_slot_highlight(batch, true, ox, oy);
        // An out-of-stock trade: "the villager needs to restock" over the
        // result (isHovering(186, 35, 22, 21), canRestock).
        let deprecated = progress
            && offers.get(shop).is_some_and(MerchantOffer::out_of_stock)
            && local.0 >= 186.0 + SHIFT - 1.0
            && local.0 < 186.0 + SHIFT + 22.0 + 1.0
            && local.1 >= 35.0 - 1.0
            && local.1 < 35.0 + 21.0 + 1.0;
        let slot_tooltip = if !deprecated && offer_tooltip.is_none() {
            self.slot_at(batch, self.client.cursor.0, self.client.cursor.1)
                .and_then(|slot| self.slot_mut(slot).clone())
        } else {
            None
        };
        let browser_tip = self.draw_mod_browser(batch);
        self.draw_screen_phase(batch, sunrise_client::screen_draw::RenderPhase::Foreground);
        if self.carried.is_some() {
            self.draw_carried(batch);
        }
        // MerchantScreen.extractContents offers the restock message and trade preview before
        // AbstractContainerScreen.extractTooltip. Only the actual slot checks the carried item.
        if deprecated {
            let text = sunrise_client::lang::get_or(
                "merchant.deprecated",
                "Villagers restock up to two times per day.",
            );
            let lines: Vec<(String, u32)> =
                text.split('\n').map(|l| (l.to_owned(), u32::MAX)).collect();
            self.draw_tooltip_lines(batch, &lines, self.client.cursor.0, self.client.cursor.1);
        } else if let Some(stack) =
            offer_tooltip.or_else(|| slot_tooltip.filter(|s| self.tooltip_with_carried(s)))
        {
            self.draw_stack_tooltip(batch, &stack, self.client.cursor.0, self.client.cursor.1);
        }
        if let Some(tooltip) = browser_tip {
            self.draw_mod_browser_tooltip(batch, tooltip);
        }
        true
    }
    /// Clicks on the offer list; true when one was hit. A press on the
    /// scroller holds it (`MerchantScreen.mouseClicked`).
    pub(in crate::game::client_side) fn merchant_click(&mut self, local: (f32, f32)) -> bool {
        let Some(m) = self.station.as_mut().and_then(|s| s.merchant.as_mut()) else {
            return false;
        };
        m.dragging = on_scroller(local, m.session.offers.len());
        if m.dragging {
            return true;
        }
        for i in 0..7 {
            let (x, y) = (5.0 + SHIFT, 18.0 + 20.0 * i as f32);
            if local.0 >= x
                && local.0 < x + 88.0
                && local.1 >= y
                && local.1 < y + 20.0
                && i + m.scroll < m.session.offers.len()
            {
                let index = i + m.scroll;
                if let Some(m) = self.station.as_mut().and_then(|s| s.merchant.as_mut()) {
                    m.focused = Some(i);
                }
                self.audio
                    .0
                    .play("minecraft:ui.button.click", None, 0.25, 1.0);
                self.try_move_items(index);
                return true;
            }
        }
        false
    }
    /// `MerchantScreen.mouseDragged`: the held scroller follows the cursor.
    pub(in crate::game::client_side) fn merchant_drag(&mut self) -> bool {
        let Some(size) = self
            .renderer
            .0
            .as_ref()
            .map(|r| [r.config.width, r.config.height])
        else {
            return false;
        };
        let (_, oy) = self.inventory_origin(&HudBatch::new(size, self.settings.gui_scale_inputs()));
        let y = self.client.cursor.1 - oy;
        if let Some(m) = self.station.as_mut().and_then(|s| s.merchant.as_mut())
            && m.dragging
        {
            m.scroll = drag_scroll(y, m.session.offers.len());
            return true;
        }
        false
    }
    /// `AbstractButton.keyPressed` on the focused offer button: Enter or
    /// Space presses it again, key repeats included, so holding Space
    /// refills the payment slots after every trade. Focus stays on the
    /// button through slot clicks (`ContainerEventHandler.mouseClicked`).
    pub(in crate::game::client_side) fn merchant_key(
        &mut self,
        key: winit::keyboard::KeyCode,
    ) -> bool {
        use winit::keyboard::KeyCode;
        if !matches!(key, KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space) {
            return false;
        }
        let Some(m) = self.station.as_ref().and_then(|s| s.merchant.as_ref()) else {
            return false;
        };
        // Buttons past the last offer are hidden, and a hidden button is inactive.
        let Some(index) = m
            .focused
            .map(|row| row + m.scroll)
            .filter(|&index| index < m.session.offers.len())
        else {
            return false;
        };
        self.audio
            .0
            .play("minecraft:ui.button.click", None, 0.25, 1.0);
        self.try_move_items(index);
        true
    }
    pub(in crate::game::client_side) fn open_merchant_state_client(
        &mut self,
        state: MerchantState,
    ) {
        assert!(
            !self.is_logical_server(),
            "open_merchant_state_client called on Server role"
        );
        let mut s = Station::new_kind(Kind::Merchant, [0, 0, 0]);
        s.merchant = Some(Box::new(state));
        self.station = Some(s);
        self.set_screen(super::Screen::Inventory);
        self.capture(false);
        self.set_mapping_held("key.use", false);
    }
    /// Opens a client-only merchant preview without a mob (`sunrise-ctl trade`).
    pub(in crate::game) fn open_preview_merchant(
        &mut self,
        profession: &str,
        variant: &str,
        level: i32,
    ) {
        assert!(
            !self.is_logical_server(),
            "open_preview_merchant called on Server role"
        );
        let profession = if profession.contains(':') {
            profession.to_owned()
        } else {
            format!("minecraft:{profession}")
        };
        let variant = format!("minecraft:{}", variant.trim_start_matches("minecraft:"));
        let level = level.clamp(1, 5);
        let origin = self
            .player
            .as_ref()
            .map_or([0.0; 3], |p| [p.position.x, p.position.y, p.position.z]);
        let mut offers = Vec::new();
        for l in 1..=level {
            let set = self.stations().trades.set_for(&profession, l);
            if let Some(sequence) = self.stations().trades.sequence(&set) {
                let mut random = merchant::sequence_random(self.level.seed, &sequence);
                let ctx = TradeContext {
                    items: self.items(),
                    enchantments: &self.stations().enchantments,
                    variant: &variant,
                    origin,
                    maps: None,
                };
                offers.extend(
                    self.stations()
                        .trades
                        .offers_from_set(&ctx, &set, &mut random),
                );
            }
        }
        let mut session = merchant::Session::new(offers, true);
        session.profession = profession;
        session.variant = variant;
        session.level = level;
        session.xp = merchant::min_xp(level);
        session.origin = origin;
        self.open_merchant_state_client(MerchantState {
            session: Box::new(session),
            scroll: 0,
            dragging: false,
            focused: None,
            mob: None,
            name: None,
        });
    }
    /// A server's trading window (no mob here: the server keeps it).
    pub(in crate::game) fn open_remote_merchant(
        &mut self,
        session: merchant::Session,
        name: Option<Text>,
    ) {
        self.open_merchant_state_client(MerchantState {
            session: Box::new(session),
            scroll: 0,
            dragging: false,
            focused: None,
            mob: None,
            name,
        });
    }
    /// `MerchantScreen.postButtonClick`: predict payment move locally and send SelectTrade.
    pub(in crate::game::client_side) fn try_move_items(&mut self, index: usize) {
        self.menu_session(|menu, env, _| {
            menu.merchant_select(env, index);
            Some(true)
        });
        if self.remote.is_some() {
            if let Some(m) = self.station.as_mut().and_then(|s| s.merchant.as_mut()) {
                m.session.selection_hint = index;
            }
            self.send(sunrise_protocol::packets::play::Serverbound::SelectTrade(
                sunrise_protocol::packets::play::SelectTrade { item: index as i32 },
            ));
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod select_on_a_server_tests {
    #[allow(unused_imports)]
pub(in crate::game::client_side) use crate::game::server_side::trading::select_on_a_server_tests::*;

    /// Picking a trade on a server moves payment locally, sends SelectTrade, and leaves effects to server.
    #[test]
    #[ignore = "daily"]
    fn picking_a_trade_on_a_server_moves_the_payment_here_too() {
        use crate::game::native_entity_refs::tests::with_client_game;
        use sunrise_client::items::ItemStack;
        use sunrise_client::merchant::{ItemCost, MerchantOffer};
        use sunrise_protocol::packets::play::Serverbound as S;
        with_client_game(|game| {
            let (mut peer, _writer) = crate::game::remote::tests::connect_menu_test(game);
            peer.set_read_timeout(Some(std::time::Duration::from_millis(50)))
                .unwrap();
            game.remote_open_merchant(1, String::new(), sunrise_client::text::Text::literal(""));
            let offer = MerchantOffer {
                base_cost_a: ItemCost {
                    stack: ItemStack::new("minecraft:emerald", 1),
                },
                cost_b: None,
                result: ItemStack::new("minecraft:bread", 6),
                uses: 0,
                max_uses: 12,
                reward_exp: true,
                special_price_diff: 0,
                demand: 0,
                price_multiplier: 0.05,
                xp: 1,
            };
            if let Some(m) = game.station.as_mut().and_then(|s| s.merchant.as_mut()) {
                m.session.offers = vec![offer];
            }
            game.inventory[0] = Some(ItemStack::new("minecraft:emerald", 5));
            while peer.recv().is_ok() {}
            game.try_move_items(0);
            let mut payment = None;
            game.menu_session(|menu, _, _| {
                payment = menu.slot(0).map(|s| (s.item.to_string(), s.count));
                Some(false)
            });
            assert_eq!(payment, Some(("minecraft:emerald".to_owned(), 5)));
            let mut picked = Vec::new();
            while let Ok((id, bytes)) = peer.recv() {
                if let Ok(S::SelectTrade(p)) = S::decode(id, &bytes) {
                    picked.push(p.item);
                }
            }
            assert_eq!(picked, [0]);
            // Shift-clicking result predicts the move while merchant effects stay on the server.
            let traded = game.stats.get("custom", "minecraft:traded_with_villager");
            game.remote_menu_ops(&[(
                Some(super::RESULT),
                0,
                sunrise_client::menu::Input::QuickMove,
            )]);
            let mut changed = Vec::new();
            while let Ok((id, bytes)) = peer.recv() {
                if let Ok(S::ContainerClick(p)) = S::decode(id, &bytes) {
                    changed.extend(
                        p.changed
                            .iter()
                            .map(|(slot, stack)| (*slot, stack.is_some())),
                    );
                }
            }
            changed.sort();
            assert_eq!(changed, [(0, false), (2, false), (38, true)]);
            assert!(game.orbs.is_empty());
            assert_eq!(
                game.stats.get("custom", "minecraft:traded_with_villager"),
                traded
            );
        });
    }
}
