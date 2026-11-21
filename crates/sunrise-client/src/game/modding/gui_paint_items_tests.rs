#![cfg(test)]
//! Real dispatch through a mod's GUI canvas and the engine's hotbar painter.
use mod_api::{
    ClientHost, ComponentHoverEvent, GuiCanvas, GuiDrawError, Hooks, ItemBarEvent,
    ItemDecorationsEvent, ItemDisplayContext, ItemModelLayer, ItemModelUpdateEvent, ItemPart,
    ModEntry, RegisterItemModelsEvent, RegistrationContext, client::hud_layout::PoseOp,
};
use mod_platform::testing::with_mods;
use std::sync::{Arc, Mutex};
use sunrise_assets::item_model::ItemModelNode;
use sunrise_client::{
    hud::{HudBatch, HudVertex},
    hud_layout::{self, HotbarInputs},
    items::ItemStack,
    text::Text,
};

const MODEL: &str = "painter:icon";
const ITEM: &str = "minecraft:paper";
const BAR: u32 = 0xFF00_FF00;
type Answers = Arc<Mutex<Vec<Result<(), GuiDrawError>>>>;
type Resolves = Arc<Mutex<Vec<(i32, u32, ItemDisplayContext)>>>;

struct Painter {
    answers: Answers,
    resolves: Resolves,
}

#[mod_api::__subscriptions]
impl Hooks for Painter {
    fn register(&self, r: &mut dyn mod_api::Registrar, _: &RegistrationContext) {
        let key = r.item_key(ITEM);
        r.item_part(key, Decorator);
    }

    fn register_item_models(
        &self,
        _: &RegistrationContext,
        event: &mut RegisterItemModelsEvent<'_>,
    ) {
        event.models.push(MODEL.into());
    }

    fn item_model_update(&self, _: &dyn ClientHost, event: &mut ItemModelUpdateEvent<'_>) {
        if event.kind == MODEL {
            self.resolves
                .lock()
                .unwrap()
                .push((event.seed, event.stack.count(), event.display));
            event.layers.push(ItemModelLayer::new("paper"));
        }
    }

    fn component_hover(
        &self,
        host: &dyn ClientHost,
        event: &mut ComponentHoverEvent<'_>,
    ) -> mod_api::Flow {
        let stack = host
            .local_player()
            .and_then(|player| player.inventory_slots(host))
            .and_then(|slots| slots.into_iter().find(|(slot, _)| *slot == 0));
        if let Some((_, stack)) = stack {
            let refuse = event.x == 50;
            let answer = event.draw.draw_gui(&mut |canvas| {
                canvas.scope(&[PoseOp::Translate(20.0, 30.0)], None, &mut |parent| {
                    parent.scope(
                        &[
                            PoseOp::Translate(8.0, 12.0),
                            PoseOp::Scale(0.5, 1.5),
                            PoseOp::Translate(-8.0, -12.0),
                        ],
                        None,
                        &mut |model: &mut dyn GuiCanvas| model.item(&stack, [0.0, 0.0], 37),
                    )?;
                    parent.item_decorations(&stack, [0.0, 0.0], None)
                })?;
                if refuse {
                    canvas.item(&stack, [f32::NAN, 0.0], 99)?;
                }
                Ok(())
            });
            self.answers.lock().unwrap().push(answer);
        }
        mod_api::Flow::Cancel
    }
}

#[derive(Clone)]
struct Decorator;
#[mod_api::sunrise_part]
impl ItemPart for Decorator {
    fn item_bar(&self, _: &dyn ClientHost, event: &mut ItemBarEvent<'_>) {
        *event.bar = Some((5, BAR));
    }

    fn item_decorations(&self, _: &dyn ClientHost, event: &mut ItemDecorationsEvent<'_>) {
        event
            .draw
            .fill([event.x - 2.0, event.y + 2.0, 1.0, 2.0], 0xFFFF_0000)
            .unwrap();
    }
}

#[test]
#[ignore = "daily"]
fn scoped_and_hotbar_items_keep_the_seed_and_draw_decorations_outside_the_squeeze() {
    use crate::game::{
        Game, gui_paint_items::HotbarItems, native_client_session::tests::logged_client,
        native_entity_refs::tests::with_client_game,
    };

    let answers = Answers::default();
    let resolves = Resolves::default();
    let mods: mod_api::Mods = vec![ModEntry::new(
        "painter",
        Box::new(Painter {
            answers: answers.clone(),
            resolves: resolves.clone(),
        }),
    )]
    .into();
    crate::game::api_values::install_parts(&mods).unwrap();
    with_mods(mods, || {
        let mut types = Vec::new();
        Game::native_register_item_models(&RegistrationContext::new(), &mut types);
        assert_eq!(types, [MODEL]);
        with_client_game(|game| {
            let _server = logged_client(game);
            let definition = Arc::get_mut(&mut game.pack.item_renderer)
                .expect("the game's item renderer is shared")
                .models
                .definitions
                .get_mut(ITEM)
                .unwrap();
            let paper = std::mem::replace(&mut definition.model, ItemModelNode::Empty);
            definition.model = ItemModelNode::Registered {
                kind: MODEL.into(),
                transformation: None,
                config: Arc::new(sunrise_core::nbt::Tag::Compound(Default::default())),
                models: vec![("paper".into(), paper)],
            };
            // This count is legal to draw, but exceeds paper's resident stack limit.
            game.inventory[0] = Some(ItemStack::new(ITEM, 65));
            game.offhand = Some(ItemStack::new(ITEM, 65));
            let hover = Text::from(serde_json::json!({
                "text": "x", "hover_event": { "action": "show_text", "value": "hi" }
            }));
            let style = &hover.spans[0].style;
            let mut canvas = HudBatch::scratch([320, 240]);
            game.component_hover_effect(&mut canvas, style, (40.0, 30.0));
            let committed = bytemuck::cast_slice::<_, u8>(&canvas.vertices).to_vec();
            game.component_hover_effect(&mut canvas, style, (50.0, 30.0));
            assert_eq!(
                bytemuck::cast_slice::<_, u8>(&canvas.vertices),
                committed.as_slice()
            );

            let mut hotbar = HudBatch::scratch([320, 240]);
            let inputs = HotbarInputs {
                selected: 0,
                right_handed: true,
                offhand: true,
                slots: [Some(0.0), None, None, None, None, None, None, None, None],
                offhand_pop: 5.0,
                attack: None,
            };
            let draws = hud_layout::hotbar(320, 240, &inputs);
            let atlas = &game.gui.atlas;
            hud_layout::paint_items(&mut hotbar, atlas, &draws, &mut HotbarItems { game });

            // Paper's flat GUI model covers its 16-pixel square. Its bounds identify the model
            // suffix independently of its face order. The bar, text and marker use the parent.
            let check = |vertices: &[HudVertex], [x, y]: [f32; 2], bounds: [[f32; 2]; 2]| {
                let bar = vertices
                    .iter()
                    .position(|v| v.color == BAR)
                    .expect("no bar")
                    - 6;
                let mut min = [f32::INFINITY; 2];
                let mut max = [f32::NEG_INFINITY; 2];
                for v in &vertices[..bar] {
                    for axis in 0..2 {
                        min[axis] = min[axis].min(v.pos[axis]);
                        max[axis] = max[axis].max(v.pos[axis]);
                    }
                }
                assert_eq!([min, max], bounds, "model bounds");

                let mut expected = HudBatch::scratch([320, 240]);
                expected.fill(atlas, x + 2.0, y + 13.0, 13.0, 2.0, 0xFF00_0000);
                expected.fill(atlas, x + 2.0, y + 13.0, 5.0, 1.0, BAR);
                let width = atlas.font.width("65") as i32;
                expected.text(atlas, "65", (17 - width) as f32, 9.0, u32::MAX, true);
                for v in &mut expected.vertices[12..] {
                    v.pos[0] += x;
                    v.pos[1] += y;
                }
                assert!(expected.vertices.len() > 12, "count has no glyphs");
                expected.fill(atlas, x - 2.0, y + 2.0, 1.0, 2.0, 0xFF00_00FF);
                let end = bar + expected.vertices.len();
                assert_eq!(
                    bytemuck::cast_slice::<_, u8>(&vertices[bar..end]),
                    bytemuck::cast_slice::<_, u8>(&expected.vertices),
                    "bar, count and custom marker in parent coordinates",
                );
                end
            };
            let end = check(&canvas.vertices, [20.0, 30.0], [[24.0, 24.0], [32.0, 48.0]]);
            assert_eq!(end, canvas.vertices.len());
            // Three background quads precede the main-hand and offhand draws, in that order.
            let items = &hotbar.vertices[18..];
            let end = check(items, [72.0, 221.0], [[72.0, 221.0], [88.0, 237.0]]);
            let offhand = &items[end..];
            let end = check(offhand, [43.0, 221.0], [[47.0, 215.0], [55.0, 239.0]]);
            assert_eq!(end, offhand.len());

            game.inventory[0] = None;
            game.component_hover_effect(&mut canvas, style, (40.0, 30.0));
            assert_eq!(
                bytemuck::cast_slice::<_, u8>(&canvas.vertices),
                committed.as_slice()
            );
        });
    });
    assert_eq!(
        *answers.lock().unwrap(),
        [Ok(()), Err(GuiDrawError::NotFinite)]
    );
    assert_eq!(
        *resolves.lock().unwrap(),
        [37, 37, 1, 10].map(|seed| (seed, 65, ItemDisplayContext::Gui))
    );
}
