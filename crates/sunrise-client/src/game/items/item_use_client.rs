#[allow(unused_imports)]
use super::{Game, shield::OFFHAND_SLOT};
#[cfg(test)]
use crate::game::server_side::item_use::ItemUseState;
use crate::game::server_side::item_use::hand_of;
use sunrise_client::items::{ItemInfo, ItemStack};

impl Game<'_, crate::game::client_side::ClientState> {
    /// What `Item.finishUsingItem` leaves in the hand on a server's client: a
    /// stack that is not a consumable stays as it is (a spyglass); a
    /// consumable loses one (`ItemStack.consume`), and once that empties the
    /// hand, its `use_remainder` takes the hand
    /// (`UseRemainder.convertIntoRemainder`). Neither in creative mode. A
    /// remainder of a stack that is not emptied goes to the inventory on the
    /// server and arrives with its inventory update.
    pub(in crate::game::client_side) fn finish_result(
        &self,
        stack: &ItemStack,
        info: &ItemInfo,
    ) -> Option<ItemStack> {
        if self.creative || self.consumable(stack).is_none() {
            return Some(stack.clone());
        }
        let mut left = stack.clone();
        left.count = left.count.saturating_sub(1);
        if left.count > 0 {
            return Some(left);
        }
        sunrise_client::item_sync::use_remainder(stack, Some(info))
    }
    /// `LivingEntityUseItemEvent.Finish` on a server's client, whose
    /// `completeUsingItem` (`handleEntityEvent(9)`) ran for `stack` in `slot`.
    /// `ItemStack.finishUsingItem` runs first, as on the client: the use
    /// cooldown, then the hand holds `finish_result`. A mod's result is put in
    /// the slot; the server's inventory update replaces either. Finishing a use
    /// sends no creative packet in vanilla (its inventory-slot edits go out
    /// through the open creative screen's `CreativeInventoryListener`), so the
    /// slot counts as synced for this client's creative sync.
    pub(in crate::game) fn remote_use_item_finished(
        &mut self,
        slot: usize,
        stack: &ItemStack,
        info: &ItemInfo,
    ) {
        self.client_apply_use_cooldown(stack);
        let mut result = self.finish_result(stack, info);
        let hand = hand_of(Some((slot, 0))).unwrap_or(self.hand);
        self.native_living_entity_use_item_finish(stack, None, hand, 0, &mut result);
        let before = self.creative_stacks();
        if !self.creative && stack.count == 1 && self.consumable(stack).is_some() {
            if self.stack_in(slot) != Some(stack) {
                return;
            }
            *self.stack_in_mut(slot) = None;
            self.player_item_destroyed(stack, Some(hand));
        }
        *self.stack_in_mut(slot) = result.filter(|s| s.count > 0);
        self.remote_synced_since(&before);
    }
    /// Client-side `UseCooldown.apply`: updates local cooldowns until server sync confirms or clears.
    pub(in crate::game) fn client_apply_use_cooldown(&mut self, stack: &ItemStack) {
        let default = self
            .res
            .items()
            .get(&stack.item)
            .and_then(|i| i.use_cooldown.clone());
        let Some(component) = sunrise_client::cooldowns::component(stack, default.as_ref()) else {
            return;
        };
        let group = sunrise_client::cooldowns::group(stack, default.as_ref());
        self.cooldowns.add(group, component.ticks());
    }
    pub(in crate::game) fn client_consume_held_slot(&mut self, amount: u32) {
        let cell = self.held_mut();
        if let Some(stack) = cell.as_mut() {
            stack.count = stack.count.saturating_sub(amount);
            if stack.count == 0 {
                *cell = None;
            }
        }
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    use sunrise_core::use_animation::UseAnimation;

    pub(in crate::game) fn stand_in_first_person(game: &mut crate::game::ClientGame<'_>) {
        game.player = Some(
            sunrise_client::motion::PlayerMotion::new(
                game.motion,
                sunrise_core::geometry::Vec3::ZERO,
            )
            .unwrap(),
        );
        game.tick_player_model();
    }

    #[test]
    #[ignore = "daily"]
    fn default_first_person_arm_obeys_its_own_sleeve_option() {
        use crate::game::native_entity_refs::tests::{run_fresh_cases, with_client_game};
        use sunrise_client::options_file::V;

        if run_fresh_cases(
            "game::client_side::item_use_client::tests::default_first_person_arm_obeys_its_own_sleeve_option",
            "SUNRISE_HAND_SLEEVE_CASE",
            &["left-hidden"],
        ) {
            return;
        }
        with_client_game(|game| {
            stand_in_first_person(game);
            game.side.settings.main_hand_left = true;
            let left = game.hand_geometry(0.0).0;
            game.side.settings.main_hand_left = false;
            let right = game.hand_geometry(0.0).0;

            game.side
                .settings
                .file
                .set("modelPart_left_sleeve", V::Bool(false));
            game.tick_player_model();
            let right_hidden_left = game.hand_geometry(0.0).0;
            game.side.settings.main_hand_left = true;
            let left_hidden_left = game.hand_geometry(0.0).0;

            assert_eq!(
                (left.len(), right.len(), left_hidden_left.len()),
                (48, 48, 24)
            );
            assert_eq!(
                bytemuck::cast_slice::<_, u8>(&left[..24]),
                bytemuck::cast_slice::<_, u8>(&left_hidden_left)
            );
            assert_eq!(
                bytemuck::cast_slice::<_, u8>(&right),
                bytemuck::cast_slice::<_, u8>(&right_hidden_left)
            );
        });
    }

    /// `render_hand` draws in a hand that holds an item, where `render_arm`
    /// never fires. What the mod drew is in the hand's vertices, in a run of
    /// its own ahead of vanilla's, and it stays when the mod leaves vanilla's
    /// hand out. The pose the mod sets moves vanilla's hand, not what it
    /// drew. The empty off hand is asked too.
    #[test]
    #[ignore = "daily"]
    fn a_mod_draws_in_a_hand_that_holds_an_item() {
        use mod_api::{ClientHost, Flow, Hand, Hooks, ModEntry, Mods, RenderHandEvent};
        use std::sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        };

        // One quad a block wide, a block in front of the eyes.
        const MARK: [[f32; 3]; 4] = [
            [0.0, 1.0, -1.0],
            [0.0, 0.0, -1.0],
            [1.0, 0.0, -1.0],
            [1.0, 1.0, -1.0],
        ];
        struct Mod {
            cancel: Arc<AtomicBool>,
            shift: Arc<AtomicBool>,
            asked: Arc<Mutex<Vec<(Hand, bool)>>>,
        }
        #[mod_api::__subscriptions]
        impl Hooks for Mod {
            fn render_hand(&self, _: &dyn ClientHost, event: &mut RenderHandEvent<'_, '_>) -> Flow {
                let holds_an_item = event.render.stack.is_some();
                self.asked
                    .lock()
                    .unwrap()
                    .push((event.render.hand, holds_an_item));
                if self.shift.load(Ordering::Relaxed) {
                    event.render.translation[0] += 1.0;
                }
                // Draw in the hand that holds the sword only.
                if holds_an_item {
                    let uv = [0.0, 0.0, 16.0, 16.0];
                    assert!(event.draw.quad("block/stone", MARK, uv, 0xFF_FF_FF));
                }
                if self.cancel.load(Ordering::Relaxed) {
                    Flow::Cancel
                } else {
                    Flow::Continue
                }
            }
        }
        fn hold_a_sword(game: &mut crate::game::ClientGame<'_>) {
            game.player = Some(
                sunrise_client::motion::PlayerMotion::new(
                    game.motion,
                    sunrise_core::geometry::Vec3::ZERO,
                )
                .unwrap(),
            );
            game.inventory[game.selected] = Some(sunrise_client::items::ItemStack::new(
                "minecraft:iron_sword",
                1,
            ));
            game.tick_player_model();
            game.side.visible_hands[0].1 = game.inventory[game.selected].clone();
        }

        let cancel = Arc::new(AtomicBool::new(false));
        let shift = Arc::new(AtomicBool::new(false));
        let asked = Arc::new(Mutex::new(Vec::new()));
        let hooks = Mod {
            cancel: cancel.clone(),
            shift: shift.clone(),
            asked: asked.clone(),
        };
        let mods: Mods = vec![ModEntry::new("hand", Box::new(hooks))].into();
        crate::game::api_values::install_parts(&mods).unwrap();
        let mut drawn = None;
        mod_platform::testing::with_mods(mods, || {
            super::super::native_entity_refs::tests::with_client_game(|game| {
                hold_a_sword(game);
                let kept = game.hand_geometry(0.0);
                shift.store(true, Ordering::Relaxed);
                let posed = game.hand_geometry(0.0).0;
                shift.store(false, Ordering::Relaxed);
                cancel.store(true, Ordering::Relaxed);
                drawn = Some((kept, posed, game.hand_geometry(0.0)));
            });
        });
        let ((kept, kept_runs), posed, (alone, alone_runs)) = drawn.unwrap();
        // Each frame both hands were asked, the empty off hand included.
        let frame = [(Hand::Main, true), (Hand::Off, false)];
        assert_eq!(*asked.lock().unwrap(), [frame, frame, frame].concat());
        // Cancelled: only the mod's quad, in a run with no rendered item id.
        assert!(!alone.is_empty());
        assert_eq!(alone_runs, [(0, 0)]);
        // The pose the mod set is not under its draw: the quad stays, and
        // vanilla's hand moves by the block asked along the view's x.
        assert_eq!(posed.len(), kept.len());
        let (quad, hand) = posed.split_at(alone.len());
        assert!(quad.iter().zip(&alone).all(|(a, b)| a.pos == b.pos));
        for (moved, still) in hand.iter().zip(&kept[alone.len()..]) {
            let by = (0..3)
                .map(|i| (moved.pos[i] - still.pos[i]).powi(2))
                .sum::<f32>()
                .sqrt();
            assert!((by - 1.0).abs() < 1e-4, "moved by {by}");
        }

        // The unmodified comparison owns a new resource graph and Parts snapshot.
        crate::game::api_values::install_parts(&[]).unwrap();
        super::super::native_entity_refs::tests::with_client_game(|game| {
            hold_a_sword(game);
            let (vanilla, vanilla_runs) = game.hand_geometry(0.0);
            assert!(!vanilla.is_empty());
            // Kept: the same quad first, then vanilla's hand exactly as without a mod.
            assert_eq!(kept.len(), alone.len() + vanilla.len());
            assert!(kept.iter().zip(&alone).all(|(a, b)| a.pos == b.pos));
            let hand = &kept[alone.len()..];
            assert!(hand.iter().zip(&vanilla).all(|(a, b)| a.pos == b.pos));
            let shifted = vanilla_runs
                .iter()
                .map(|&(start, id)| (start + alone.len() as u32, id));
            let runs = std::iter::once((0, 0)).chain(shifted).collect::<Vec<_>>();
            assert_eq!(kept_runs, runs);
        });
    }

    #[test]
    #[ignore = "daily"]
    fn item_animation_dispatches_vanilla_parts_and_refuses_invalid_spears() {
        use mod_api::{Hooks, Host, ItemPart, ModEntry, Mods, Registrar, UseAnimationEvent};
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        #[derive(Clone)]
        struct Part(Arc<AtomicUsize>);
        #[mod_api::sunrise_part]
        impl ItemPart for Part {
            fn use_animation(&self, _: &dyn Host, event: &mut UseAnimationEvent<'_>) {
                self.0.fetch_add(1, Ordering::Relaxed);
                let expected = match mod_api::__private::item_key_id(event.stack.item()).unwrap() {
                    "minecraft:bow" => UseAnimation::Bow,
                    "minecraft:crossbow" => UseAnimation::Crossbow,
                    "minecraft:trident" => UseAnimation::Trident,
                    "minecraft:brush" => UseAnimation::Brush,
                    "minecraft:wooden_spear"
                        if event.stack.components().is_none_or(|d| {
                            d.as_nbt()
                                .get(&"!minecraft:kinetic_weapon".into())
                                .is_none()
                        }) =>
                    {
                        UseAnimation::Spear
                    }
                    "minecraft:stick"
                        if event.stack.components().is_some_and(|d| {
                            d.as_nbt().get(&"minecraft:kinetic_weapon".into()).is_some()
                        }) =>
                    {
                        UseAnimation::Spear
                    }
                    _ => UseAnimation::None,
                };
                assert_eq!(*event.animation, expected);
                *event.animation = if mod_api::__private::item_key_id(event.stack.item()).unwrap()
                    == "minecraft:stick"
                    || event.stack.components().is_some_and(|d| {
                        d.as_nbt()
                            .get(&"!minecraft:kinetic_weapon".into())
                            .is_some()
                    }) {
                    UseAnimation::Spear
                } else {
                    UseAnimation::Drink
                };
            }
        }
        struct Mod(Arc<AtomicUsize>);
        #[mod_api::__subscriptions]
        impl Hooks for Mod {
            fn register(&self, r: &mut dyn Registrar, _: &mod_api::RegistrationContext) {
                for id in [
                    "minecraft:bow",
                    "minecraft:crossbow",
                    "minecraft:trident",
                    "minecraft:brush",
                    "minecraft:wooden_spear",
                    "minecraft:stick",
                ] {
                    let item = r.item_key(id);
                    r.item_part(item, Part(self.0.clone()));
                }
            }
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let mut overridden_geometry = None;
        let mods: Mods = vec![ModEntry::new("animation", Box::new(Mod(calls.clone())))].into();
        crate::game::api_values::install_parts(&mods).unwrap();
        mod_platform::testing::with_mods(mods, || {
            super::super::native_entity_refs::tests::with_client_game(|game| {
                for id in [
                    "minecraft:bow",
                    "minecraft:crossbow",
                    "minecraft:trident",
                    "minecraft:brush",
                    "minecraft:wooden_spear",
                ] {
                    assert_eq!(
                        game.item_use_animation(&sunrise_client::items::ItemStack::new(id, 1)),
                        UseAnimation::Drink
                    );
                }
                assert_eq!(
                    game.item_use_animation(&sunrise_client::items::ItemStack::new(
                        "minecraft:stick",
                        1
                    )),
                    UseAnimation::None
                );
                let mut patched = sunrise_client::items::ItemStack::new("minecraft:bow", 1);
                patched.data_mut().set_other(
                    "minecraft:consumable",
                    Some(sunrise_core::nbt::Tag::compound([(
                        "animation",
                        sunrise_core::nbt::Tag::String("drink".into()),
                    )])),
                );
                assert_eq!(game.item_use_animation(&patched), UseAnimation::Drink);
                use sunrise_core::nbt::Tag;
                let mut kinetic_stick = sunrise_client::items::ItemStack::new("minecraft:stick", 1);
                kinetic_stick.data_mut().set_other(
                    "minecraft:kinetic_weapon",
                    Some(Tag::compound([("delay_ticks", Tag::Int(30))])),
                );
                assert!(matches!(
                    game.item_use_state(&kinetic_stick),
                    super::ItemUseState::Spear(_)
                ));
                let mut removed =
                    sunrise_client::items::ItemStack::new("minecraft:wooden_spear", 1);
                removed.data_mut().set_other(
                    "!minecraft:kinetic_weapon",
                    Some(Tag::Compound(Default::default())),
                );
                assert_eq!(game.item_use_animation(&removed), UseAnimation::None);
                assert_eq!(calls.load(Ordering::Relaxed), 9);
                game.player = Some(
                    sunrise_client::motion::PlayerMotion::new(
                        game.motion,
                        sunrise_core::geometry::Vec3::ZERO,
                    )
                    .unwrap(),
                );
                game.inventory[game.selected] =
                    Some(sunrise_client::items::ItemStack::new("minecraft:bow", 1));
                game.using = Some((game.selected, 0));
                game.tick_player_model();
                assert_eq!(
                    game.side.player_model.as_ref().unwrap().extra.use_animation,
                    UseAnimation::Drink
                );
                assert_eq!(calls.load(Ordering::Relaxed), 10);
                game.side.visible_hands[0].1 = game.inventory[game.selected].clone();
                game.use_duration = 72_000;
                let (overridden, _) = game.hand_geometry(0.0);
                assert_eq!(calls.load(Ordering::Relaxed), 11);
                // A scoped spear override on a component-patched ordinary item
                // must reach hand geometry without the old whitelist panic.
                game.inventory[game.selected] = Some(kinetic_stick);
                game.side.visible_hands[0].1 = game.inventory[game.selected].clone();
                let (kinetic_geometry, _) = game.hand_geometry(0.0);
                assert!(!kinetic_geometry.is_empty());
                overridden_geometry = Some(overridden);
            });
        });
        // The unmodified comparison owns a new resource graph and Parts snapshot.
        crate::game::api_values::install_parts(&[]).unwrap();
        super::super::native_entity_refs::tests::with_client_game(|game| {
            game.player = Some(
                sunrise_client::motion::PlayerMotion::new(
                    game.motion,
                    sunrise_core::geometry::Vec3::ZERO,
                )
                .unwrap(),
            );
            game.inventory[game.selected] =
                Some(sunrise_client::items::ItemStack::new("minecraft:bow", 1));
            game.using = Some((game.selected, 0));
            game.tick_player_model();
            game.side.visible_hands[0].1 = game.inventory[game.selected].clone();
            game.use_duration = 72_000;
            let (vanilla, _) = game.hand_geometry(0.0);
            let overridden = overridden_geometry.unwrap();
            assert!(!overridden.is_empty());
            assert_eq!(overridden.len(), vanilla.len());
            assert!(overridden.iter().zip(&vanilla).any(|(a, b)| a.pos != b.pos));
        });
    }

    #[test]
    #[ignore = "daily"]
    fn render_hand_submits_both_skinned_arms_in_draw_order() {
        use crate::game::native_entity_refs::tests::{run_fresh_cases, with_client_game};
        use glam::{Mat4, Quat, Vec3};
        use mod_api::{
            ClientHost, Flow, Hand, Hooks, MainHand, ModEntry, RenderArmEvent, RenderHandEvent,
        };
        use std::sync::{
            Arc,
            atomic::{AtomicU8, Ordering::Relaxed},
        };
        use sunrise_client::{hud::Rect, items::ItemStack};

        if run_fresh_cases(
            "game::client_side::item_use_client::tests::render_hand_submits_both_skinned_arms_in_draw_order",
            "SUNRISE_HAND_ARM_CASE",
            &["wide", "slim"],
        ) {
            return;
        }
        let slim = std::env::var("SUNRISE_HAND_ARM_CASE").unwrap() == "slim";
        const MOVES: [[f32; 3]; 2] = [[0.75, -0.5, -1.0], [-0.25, 0.5, -2.0]];
        const LIGHT: [u32; 2] = [0x30a0, 0x60b0];
        const SCALE: [f32; 3] = [5e-7, 1e-8, 1e-8];
        #[derive(Default)]
        struct State {
            mode: AtomicU8,
            default_arms: AtomicU8,
        }
        struct Mod(Arc<State>);
        #[mod_api::__subscriptions]
        impl Hooks for Mod {
            fn render_arm(&self, _: &dyn ClientHost, _: &mut RenderArmEvent<'_>) -> Flow {
                self.0.default_arms.fetch_add(1, Relaxed);
                Flow::Continue
            }
            fn render_hand(&self, _: &dyn ClientHost, event: &mut RenderHandEvent<'_, '_>) -> Flow {
                let mode = self.0.mode.load(Relaxed);
                if event.render.hand != Hand::Main || mode == 0 {
                    return Flow::Continue;
                }
                let original_light = event.draw.light();
                for (i, arm) in [MainHand::Left, MainHand::Right].into_iter().enumerate() {
                    event.draw.push();
                    event.draw.translate(MOVES[i]);
                    if mode == 3 && i == 0 {
                        event.draw.scale(SCALE);
                        event.draw.rotate_about([0.0, 0.0, 1.0], 90.0, [0.0; 3]);
                    }
                    event.draw.set_light(LIGHT[i]);
                    event.draw.render_arm(arm);
                    event.draw.pop();
                }
                event.draw.set_light(original_light);
                if mode == 2 {
                    Flow::Cancel
                } else {
                    Flow::Continue
                }
            }
        }
        let state = Arc::new(State::default());
        mod_platform::testing::with_mods(
            vec![ModEntry::new("arms", Box::new(Mod(state.clone())))].into(),
            || {
                with_client_game(|game| {
                    stand_in_first_person(game);
                    for (i, (slot, item)) in [
                        (game.selected, "minecraft:iron_sword"),
                        (super::OFFHAND_SLOT, "minecraft:stick"),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let stack = ItemStack::new(item, 1);
                        *game.stack_in_mut(slot) = Some(stack.clone());
                        game.side.visible_hands[i] = (slot, Some(stack));
                    }
                    let atlas = Rect {
                        u0: 0.25,
                        v0: 0.125,
                        u1: 0.75,
                        v1: 0.375,
                        w: 64,
                        h: 64,
                    };
                    game.side
                        .gui
                        .atlas
                        .sprites
                        .insert("arms:fixture_skin".into(), atlas);
                    let avatar = game.side.player_model.as_mut().unwrap();
                    avatar.skin = Some("arms:fixture_skin".into());
                    avatar.slim = slim;
                    let (vanilla, runs) = game.hand_geometry(0.0);
                    let [_, (offhand_at, offhand_id)] = runs[..] else {
                        panic!("expected both item runs: {runs:?}")
                    };
                    assert!(offhand_at > 0 && (offhand_at as usize) < vanilla.len());
                    state.mode.store(1, Relaxed);
                    let (added, added_runs) = game.hand_geometry(0.0);
                    state.mode.store(2, Relaxed);
                    let (replaced, replaced_runs) = game.hand_geometry(0.0);
                    assert_eq!(state.default_arms.load(Relaxed), 0);

                    // Six quads for the arm and six for the sleeve. Check each layer's first
                    // corner from PlayerModel's cube origins/texture offsets, including 3px arms.
                    let width = if slim { 3.0 } else { 4.0 };
                    let quads = 6 * 4;
                    let arm_vertices = quads * 2;
                    let submitted = arm_vertices * 2;
                    let view = game.side.sway.to_world(0.0, &game.side.view);
                    for (i, (shoulder, roll, x, base_u, sleeve_u, v)) in [
                        (
                            5.0,
                            -0.1,
                            width - 1.0,
                            32.0 + 4.0 + width,
                            48.0 + 4.0 + width,
                            48.0,
                        ),
                        (-5.0, 0.1, 1.0, 40.0 + 4.0 + width, 40.0 + 4.0 + width, 16.0),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let world = view
                            * Mat4::from_translation(Vec3::from_array(MOVES[i]))
                            * Mat4::from_rotation_translation(
                                Quat::from_rotation_z(roll),
                                Vec3::new(shoulder / 16.0, 2.0 / 16.0, 0.0),
                            );
                        let arm = &added[i * arm_vertices..(i + 1) * arm_vertices];
                        for (vertex, inflate, u, v) in [
                            (arm[0], 0.0, base_u, v),
                            (
                                arm[quads],
                                0.25,
                                sleeve_u,
                                if i == 0 { v } else { v + 16.0 },
                            ),
                        ] {
                            let p = Vec3::new(x + inflate, -2.0 - inflate, 2.0 + inflate) / 16.0;
                            assert_eq!(
                                vertex.pos.map(f32::to_bits),
                                world.transform_point3(p).to_array().map(f32::to_bits)
                            );
                            assert_eq!(vertex.uv, [0.25 + u / 128.0, 0.125 + v / 256.0]);
                        }
                        assert!(
                            arm.iter()
                                .all(|vertex| vertex.light == LIGHT[i] && vertex.layer == 0)
                        );
                    }
                    assert_eq!(
                        &added_runs[..],
                        &[
                            (0, 0),
                            (submitted as u32, 0),
                            (submitted as u32 + offhand_at, offhand_id)
                        ]
                    );
                    assert_eq!(
                        &replaced_runs[..],
                        &[(0, 0), (submitted as u32, offhand_id)]
                    );
                    assert_eq!(
                        bytemuck::cast_slice::<_, u8>(&added[submitted..]),
                        bytemuck::cast_slice::<_, u8>(&vanilla)
                    );
                    assert_eq!(
                        bytemuck::cast_slice::<_, u8>(&replaced[..submitted]),
                        bytemuck::cast_slice::<_, u8>(&added[..submitted])
                    );
                    assert_eq!(
                        bytemuck::cast_slice::<_, u8>(&replaced[submitted..]),
                        bytemuck::cast_slice::<_, u8>(&vanilla[offhand_at as usize..])
                    );

                    state.mode.store(3, Relaxed);
                    let (scaled, _) = game.hand_geometry(0.0);
                    // ModelPart.Cube's first polygon faces DOWN. The left arm's
                    // -0.1 roll followed by the listener's quarter turn sends -Y
                    // toward +X and -Y. Scale normals by the reciprocal axes.
                    let normal = view
                        .transform_vector3(Vec3::new(
                            sunrise_core::jmath::cos(0.1) as f32 / SCALE[0],
                            -(sunrise_core::jmath::sin(0.1) as f32) / SCALE[1],
                            0.0,
                        ))
                        .normalize();
                    let packed = normal.to_array().map(|n| (n * 127.0) as i8);
                    assert!(scaled[..4].iter().all(|v| v.normal[..3] == packed));
                    assert_eq!(
                        bytemuck::cast_slice::<_, u8>(&scaled[arm_vertices..submitted]),
                        bytemuck::cast_slice::<_, u8>(&added[arm_vertices..submitted]),
                        "pop restores the unscaled normal pose for the second arm"
                    );

                    game.side.gui.atlas.sprites.remove("arms:fixture_skin");
                    for missing_avatar in [false, true] {
                        if missing_avatar {
                            game.side.player_model = None;
                        }
                        state.mode.store(0, Relaxed);
                        let (items, item_runs) = game.hand_geometry(0.0);
                        state.mode.store(1, Relaxed);
                        let (drawn, drawn_runs) = game.hand_geometry(0.0);
                        assert_eq!(drawn_runs, item_runs);
                        assert_eq!(
                            bytemuck::cast_slice::<_, u8>(&drawn),
                            bytemuck::cast_slice::<_, u8>(&items),
                            "no arm without the selected sprite or the avatar state"
                        );
                    }
                });
            },
        );
    }
}
