//! The mouse-key portion of Minecraft.handleKeybinds. Events only count clicks;
//! the client tick consumes them before player movement and CLIENT_TICK_END.

#[derive(Default)]
pub struct InteractionInput {
    clicks: [u32; 3],
}

#[derive(Clone, Copy)]
pub enum Button {
    Attack,
    Use,
    Pick,
}

pub trait InteractionWorld {
    /// Minecraft.pick before handleKeybinds: the tick's `hitResult`, which
    /// every click of the tick reads, even after one of them changed the
    /// block it names (a creative break, then a use that replaces it).
    fn pick_hit_result(&mut self);
    /// handleKeybinds' keys between the number keys and the clicks (the inventory key, swap
    /// hands, drop), which act before any attack, use or pick of the tick.
    fn handle_keys(&mut self) {}
    /// The tick's clicks are done: later picks follow the crosshair again.
    fn release_hit_result(&mut self);
    fn using_item(&self) -> bool;
    fn release_using(&mut self);
    fn start_attack(&mut self) -> bool;
    fn start_use(&mut self, held: bool);
    fn pick(&mut self);
    fn use_delay(&self) -> u32;
    fn continue_attack(&mut self, down: bool);
}

/// Minecraft.startUseItem dispatches one hit result per hand. A passing
/// entity interaction falls back to the item, without trying a block behind it.
pub trait TargetUseWorld {
    fn use_entity(&mut self) -> bool;
    fn use_block(&mut self) -> bool;
    fn use_item(&mut self);
}

pub fn use_on_target(world: &mut impl TargetUseWorld, entity: bool) {
    let consumed = if entity {
        world.use_entity()
    } else {
        world.use_block()
    };
    if !consumed {
        world.use_item();
    }
}

impl InteractionInput {
    pub fn press(&mut self, button: Button) {
        self.clicks[button as usize] = self.clicks[button as usize].saturating_add(1);
    }
    /// The clicks of `button` waiting for a tick.
    pub fn queued(&self, button: Button) -> u32 {
        self.clicks[button as usize]
    }
    /// Take the pending input, leaving empty counters for callbacks during the tick.
    /// The owner must merge callback input back after the tick.
    pub fn take_queued(&mut self) -> Self {
        std::mem::take(self)
    }
    /// Preserve clicks queued by callbacks while the owner temporarily takes this input.
    pub fn merge_queued(&mut self, queued: Self) {
        for (clicks, added) in self.clicks.iter_mut().zip(queued.clicks) {
            *clicks = clicks.saturating_add(added);
        }
    }
    pub fn tick(&mut self, world: &mut impl InteractionWorld, attack_down: bool, use_down: bool) {
        world.pick_hit_result();
        world.handle_keys();
        let mut instant = false;
        if world.using_item() {
            if !use_down {
                world.release_using();
            }
            // Vanilla drains all three click counters while using an item,
            // including the tick that releases it.
            self.clicks = [0; 3];
        } else {
            // Drain every pending click in vanilla `while (key.consumeClick())` order.
            let [attack, use_item, pick] = std::mem::take(&mut self.clicks);
            for _ in 0..attack {
                instant |= world.start_attack();
            }
            for _ in 0..use_item {
                world.start_use(false);
            }
            for _ in 0..pick {
                world.pick();
            }
        }
        if use_down && world.use_delay() == 0 && !world.using_item() {
            world.start_use(true);
        }
        world.continue_attack(!instant && attack_down);
        world.release_hit_result();
    }
}

/// The sentence of `Minecraft.handleKeybinds` that follows a key which opened
/// a screen: `Gui.setScreen` has already run `KeyMapping.releaseAll()`, so
/// `key.use` reads up and the same call releases an active use
/// (`player.isUsingItem() && !keyUse.isDown()` → `MultiPlayerGameMode.releaseUsingItem`).
/// Only the screens opened from the keybinds do this (the inventory, chat and
/// advancements); the pause menu, server containers, mount inventories and
/// dialogs release on the first tick after closing instead, as vanilla does.
pub fn release_on_screen_open(world: &mut impl InteractionWorld) {
    if world.using_item() {
        world.release_using();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct World {
        using: bool,
        destroying: bool,
        delay: u32,
        begin_using: bool,
        events: Vec<&'static str>,
        use_attempts: Vec<bool>,
        callback_input: Option<InteractionInput>,
    }
    impl InteractionWorld for World {
        fn pick_hit_result(&mut self) {
            if let Some(input) = &mut self.callback_input {
                for _ in 0..1025 {
                    input.press(Button::Attack);
                }
            }
        }
        fn release_hit_result(&mut self) {}
        fn using_item(&self) -> bool {
            self.using
        }
        fn release_using(&mut self) {
            self.events.push("release");
            self.using = false;
        }
        fn start_attack(&mut self) -> bool {
            self.events.push("attack");
            false
        }
        fn start_use(&mut self, held: bool) {
            self.use_attempts.push(held);
            if !self.destroying {
                self.events.push("use");
                self.delay = 4;
                self.using = self.begin_using;
            }
        }
        fn pick(&mut self) {
            self.events.push("pick");
        }
        fn use_delay(&self) -> u32 {
            self.delay
        }
        fn continue_attack(&mut self, _: bool) {
            self.events.push("continue");
        }
    }

    /// Exercise the same take, callback, merge cycle as Remote without game resources:
    /// callbacks that enqueue more than 1024 clicks are neither capped nor split across ticks.
    #[test]
    fn callback_burst_survives_to_next_tick_without_cap() {
        let mut world = World {
            callback_input: Some(InteractionInput::default()),
            ..Default::default()
        };
        for tick in 0..2 {
            let mut input = world.callback_input.as_mut().unwrap().take_queued();
            world.events.clear();
            input.tick(&mut world, false, false);
            let queued = world.callback_input.take().unwrap();
            input.merge_queued(queued);
            world.callback_input = Some(input);
            assert_eq!(
                world
                    .events
                    .iter()
                    .filter(|&&event| event == "attack")
                    .count(),
                if tick == 0 { 0 } else { 1025 }
            );
        }
    }

    #[test]
    fn quick_clicks_wait_for_a_tick_and_follow_keybind_order() {
        let mut input = InteractionInput::default();
        let mut world = World::default();
        input.press(Button::Use);
        input.press(Button::Pick);
        input.press(Button::Attack);
        assert!(world.events.is_empty());
        // Both edges can arrive between ticks; the press still acts once.
        input.tick(&mut world, false, false);
        assert_eq!(world.events, ["attack", "use", "pick", "continue"]);
        world.events.clear();
        input.tick(&mut world, false, false);
        assert_eq!(
            world.events,
            ["continue"],
            "no spurious release or repeated click"
        );
    }

    #[test]
    fn releasing_actual_item_use_discards_clicks_for_that_tick() {
        let mut input = InteractionInput::default();
        let mut world = World {
            using: true,
            ..Default::default()
        };
        for button in [Button::Attack, Button::Use, Button::Pick] {
            input.press(button);
        }
        input.tick(&mut world, true, false);
        assert_eq!(world.events, ["release", "continue"]);
    }

    #[test]
    fn holding_use_respects_the_delay_and_active_digging() {
        let mut input = InteractionInput::default();
        let mut world = World {
            destroying: true,
            ..Default::default()
        };
        input.press(Button::Use);
        input.tick(&mut world, true, true);
        assert_eq!(world.events, ["continue"]);
        world.destroying = false;
        world.events.clear();
        input.tick(&mut world, false, true);
        assert_eq!(world.events, ["use", "continue"]);
        world.events.clear();
        input.tick(&mut world, false, true);
        assert_eq!(world.events, ["continue"]);
        assert_eq!(world.use_attempts, [false, true, true]);
    }

    #[test]
    fn a_quick_use_click_releases_on_the_following_tick() {
        let mut input = InteractionInput::default();
        let mut world = World {
            begin_using: true,
            ..Default::default()
        };
        input.press(Button::Use);
        input.tick(&mut world, false, false);
        assert_eq!(world.events, ["use", "continue"]);
        assert!(world.using);
        world.events.clear();
        input.tick(&mut world, false, false);
        assert_eq!(world.events, ["release", "continue"]);
    }

    /// A floating block in front of the crosshair, nothing behind it, and a
    /// block item in the hand of a creative player.
    #[derive(Default)]
    struct Column {
        block: bool,
        hit: Option<Option<()>>,
        placed: bool,
    }
    impl Column {
        /// Minecraft.hitResult: the tick's pick, or a fresh one outside it.
        fn target(&self) -> Option<()> {
            self.hit.unwrap_or(self.block.then_some(()))
        }
    }
    impl InteractionWorld for Column {
        fn pick_hit_result(&mut self) {
            self.hit = Some(self.target());
        }
        fn release_hit_result(&mut self) {
            self.hit = None;
        }
        fn using_item(&self) -> bool {
            false
        }
        fn release_using(&mut self) {}
        fn start_attack(&mut self) -> bool {
            // MultiPlayerGameMode.startDestroyBlock in creative: the
            // predicted break leaves air.
            if self.target().is_some() {
                self.block = false;
            }
            !self.block
        }
        fn start_use(&mut self, _held: bool) {
            // BlockItem.place: the clicked block, now air, can be replaced.
            if self.target().is_some() && !self.block {
                self.block = true;
                self.placed = true;
            }
        }
        fn pick(&mut self) {}
        fn use_delay(&self) -> u32 {
            0
        }
        fn continue_attack(&mut self, _: bool) {}
    }

    #[test]
    fn attack_and_use_in_one_tick_share_the_hit_result() {
        // #3006: both buttons at once replace the block the crosshair is on.
        let mut input = InteractionInput::default();
        let mut world = Column {
            block: true,
            ..Default::default()
        };
        input.press(Button::Attack);
        input.press(Button::Use);
        input.tick(&mut world, false, false);
        assert!(world.placed, "the use acts on the block the attack broke");
        assert!(world.block);
        assert!(world.hit.is_none(), "the next pick follows the crosshair");
    }

    /// Click counts do not cap at 1024 and saturate cleanly at `u32::MAX`.
    #[test]
    fn click_counts_do_not_cap_at_1024_and_saturate_at_max() {
        let mut input = InteractionInput::default();
        for _ in 0..1100 {
            for button in [Button::Attack, Button::Use, Button::Pick] {
                input.press(button);
            }
        }
        assert_eq!(input.clicks, [1100; 3]);
        input.clicks[Button::Attack as usize] = u32::MAX - 1;
        input.press(Button::Attack);
        input.press(Button::Attack);
        assert_eq!(input.queued(Button::Attack), u32::MAX);
    }

    /// Every queued click drains in one tick in vanilla `while (key.consumeClick())` order.
    #[test]
    fn a_long_burst_drains_all_clicks_in_one_tick() {
        let mut input = InteractionInput::default();
        let mut world = World::default();
        use Button::{Attack, Pick, Use};
        for button in [Attack, Attack, Attack, Use, Use, Pick] {
            input.press(button);
        }
        input.tick(&mut world, false, false);
        assert_eq!(
            world.events,
            [
                "attack", "attack", "attack", "use", "use", "pick", "continue"
            ]
        );
        for _ in 0..150 {
            input.press(Button::Attack);
        }
        for _ in 0..70 {
            input.press(Button::Use);
        }
        world.events.clear();
        input.tick(&mut world, false, false);
        let count = |event| world.events.iter().filter(|e| **e == event).count();
        assert_eq!([count("attack"), count("use")], [150, 70]);
        world.events.clear();
        input.tick(&mut world, false, false);
        assert_eq!(world.events, ["continue"]);
    }
}
