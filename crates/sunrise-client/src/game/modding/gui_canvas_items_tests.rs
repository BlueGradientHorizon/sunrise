#![cfg(test)]
//! Integration tests for `GuiCanvas` items, item decorations, sprites, blits and per-call refusals
//! through the game's generated dispatch.
use mod_api::client::hud_layout::{Blend, PoseOp};
use mod_api::{
    ClientHost, ComponentHoverEvent, Flow, GuiBlitRegion, GuiDrawError, GuiSpriteRegion, Hooks,
    Identifier, ItemDecorationsEvent, ItemPart, ItemStack, ModEntry, Registrar,
    RegistrationContext,
};
use mod_platform::testing::with_mods;
use std::sync::Mutex;
use sunrise_client::hud::{Blend as HudBlend, HudBatch, map_slot_sprite};
use sunrise_client::hud_layout::abgr;
use sunrise_client::items::ItemStack as EngineStack;
use sunrise_client::text::{Style, Text};

fn hover_style() -> Style {
    let text = Text::from(serde_json::json!({
        "text": "x", "hover_event": { "action": "show_text", "value": "hi" }
    }));
    text.spans[0].style.clone()
}

fn vertices_snapshot(batch: &HudBatch) -> Vec<([f32; 2], [f32; 2], u32)> {
    batch
        .vertices
        .iter()
        .map(|v| (v.pos, v.uv, v.color))
        .collect()
}

struct ItemSeedPainter {
    item: &'static str,
    seed: i32,
}

#[mod_api::__subscriptions]
impl Hooks for ItemSeedPainter {
    fn component_hover(&self, host: &dyn ClientHost, event: &mut ComponentHoverEvent<'_>) -> Flow {
        let key = host
            .item_key(&Identifier::parse(self.item).unwrap())
            .unwrap();
        let stack = ItemStack::of(key);
        event
            .draw
            .draw_gui(&mut |canvas| canvas.item(&stack, [10.0, 20.0], self.seed))
            .unwrap();
        Flow::Cancel
    }
}

#[test]
#[ignore = "daily"]
fn canvas_item_forwards_seed_and_skips_air() {
    let style = hover_style();
    let mut air_batch = HudBatch::scratch([320, 240]);
    let mut seed0_batch = HudBatch::scratch([320, 240]);
    let mut seed1_batch = HudBatch::scratch([320, 240]);
    let mut direct0 = HudBatch::scratch([320, 240]);
    let mut direct1 = HudBatch::scratch([320, 240]);

    crate::game::native_entity_refs::tests::with_client_game(|game| {
        let compass = EngineStack::new("minecraft:recovery_compass", 1);
        // Settle the random-spin wobbler for the current game_time once so every draw at this tick
        // shares the same base wobble and differs only by seed.
        let mut warmup = HudBatch::scratch([320, 240]);
        game.draw_item_model(&mut warmup, &compass, 10.0, 20.0, 0);

        game.draw_item_model(&mut direct0, &compass, 10.0, 20.0, 0);
        game.draw_item_model(&mut direct1, &compass, 10.0, 20.0, 1);

        let air_mods: mod_api::Mods = vec![ModEntry::new(
            "air_painter",
            Box::new(ItemSeedPainter {
                item: "minecraft:air",
                seed: 7,
            }),
        )]
        .into();
        with_mods(air_mods, || {
            game.component_hover_effect(&mut air_batch, &style, (40.0, 30.0));
        });

        let seed0_mods: mod_api::Mods = vec![ModEntry::new(
            "seed0",
            Box::new(ItemSeedPainter {
                item: "minecraft:recovery_compass",
                seed: 0,
            }),
        )]
        .into();
        with_mods(seed0_mods, || {
            game.component_hover_effect(&mut seed0_batch, &style, (40.0, 30.0));
        });

        let seed1_mods: mod_api::Mods = vec![ModEntry::new(
            "seed1",
            Box::new(ItemSeedPainter {
                item: "minecraft:recovery_compass",
                seed: 1,
            }),
        )]
        .into();
        with_mods(seed1_mods, || {
            game.component_hover_effect(&mut seed1_batch, &style, (40.0, 30.0));
        });
    });

    assert!(air_batch.vertices.is_empty(), "air stack must draw nothing");
    assert!(!direct0.vertices.is_empty(), "compass model must draw");
    assert_eq!(vertices_snapshot(&seed0_batch), vertices_snapshot(&direct0));
    assert_eq!(vertices_snapshot(&seed1_batch), vertices_snapshot(&direct1));
    assert_ne!(
        seed0_batch
            .vertices
            .iter()
            .map(|v| v.uv)
            .collect::<Vec<_>>(),
        seed1_batch
            .vertices
            .iter()
            .map(|v| v.uv)
            .collect::<Vec<_>>(),
        "seed must reach the item model resolver"
    );
}

static DECORATOR_COORDS: Mutex<Vec<[f32; 2]>> = Mutex::new(Vec::new());

const DECORATOR_VISIBLE: u32 = 0xFF00_FF00;
const DECORATOR_CLIPPED: u32 = 0xFFFF_00FF;

#[derive(Clone)]
struct SwordDecorator;

#[mod_api::sunrise_part]
impl ItemPart for SwordDecorator {
    fn item_decorations(&self, _: &dyn ClientHost, event: &mut ItemDecorationsEvent<'_>) {
        DECORATOR_COORDS.lock().unwrap().push([event.x, event.y]);
        event
            .draw
            .fill([event.x + 1.0, event.y + 1.0, 2.0, 2.0], DECORATOR_VISIBLE)
            .unwrap();
        event
            .draw
            .fill(
                [event.x + 14.0, event.y + 14.0, 4.0, 4.0],
                DECORATOR_CLIPPED,
            )
            .unwrap();
    }
}

struct DecoratedItemMod {
    draw_decorations: bool,
}

#[mod_api::__subscriptions]
impl Hooks for DecoratedItemMod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let key = r.item_key("minecraft:iron_sword");
        r.item_part(key, SwordDecorator);
    }

    fn component_hover(&self, host: &dyn ClientHost, event: &mut ComponentHoverEvent<'_>) -> Flow {
        let sword_key = host
            .item_key(&Identifier::parse("minecraft:iron_sword").unwrap())
            .unwrap();
        let air_key = host
            .item_key(&Identifier::parse("minecraft:air").unwrap())
            .unwrap();
        let mut sword = ItemStack::new(sword_key, 5).unwrap();
        sword.set_damage(100);
        let air = ItemStack::of(air_key);
        let draw_decorations = self.draw_decorations;

        event
            .draw
            .draw_gui(&mut |canvas| {
                canvas.scope(
                    &[PoseOp::Translate(100.0, 50.0)],
                    Some([0.0, 0.0, 14.0, 16.0]),
                    &mut |scoped| {
                        // Squeezed model in a child scope (x2 width, x0.5 height around center).
                        scoped.scope(
                            &[
                                PoseOp::Translate(8.0, 8.0),
                                PoseOp::Scale(2.0, 0.5),
                                PoseOp::Translate(-8.0, -8.0),
                            ],
                            None,
                            &mut |squeezed| squeezed.item(&sword, [0.0, 0.0], 0),
                        )?;
                        if draw_decorations {
                            // Drawn outside the squeeze scope so decorations follow the enclosing
                            // pose and scissor, not the model squeeze.
                            scoped.item_decorations(&sword, [0.0, 0.0], None)?;
                            scoped.item_decorations(&air, [0.0, 0.0], None)?;
                        }
                        Ok(())
                    },
                )
            })
            .unwrap();
        Flow::Cancel
    }
}

#[test]
#[ignore = "daily"]
fn canvas_item_decorations_draws_vanilla_and_custom_under_enclosing_pose_and_scissor() {
    crate::game::in_own_process(
        "game::gui_canvas_items_tests::canvas_item_decorations_draws_vanilla_and_custom_under_enclosing_pose_and_scissor",
        canvas_item_decorations_isolated,
    );
}

fn canvas_item_decorations_isolated() {
    let style = hover_style();
    let model_only_mods: mod_api::Mods = vec![ModEntry::new(
        "decorated",
        Box::new(DecoratedItemMod {
            draw_decorations: false,
        }),
    )]
    .into();
    let combined_mods: mod_api::Mods = vec![ModEntry::new(
        "decorated",
        Box::new(DecoratedItemMod {
            draw_decorations: true,
        }),
    )]
    .into();

    let mut model_only = HudBatch::scratch([320, 240]);
    let mut combined = HudBatch::scratch([320, 240]);
    let mut vanilla_expected = HudBatch::scratch([320, 240]);

    with_mods(combined_mods.clone(), || {
        crate::game::api_values::install_parts(&combined_mods).unwrap();
        crate::game::native_entity_refs::tests::with_client_game(|game| {
            game.cooldowns.add("minecraft:iron_sword".into(), 20);
            let mut engine_sword = EngineStack::new("minecraft:iron_sword", 5);
            engine_sword.damage = 100;

            DECORATOR_COORDS.lock().unwrap().clear();
            game.component_hover_effect(&mut combined, &style, (40.0, 30.0));

            game.draw_vanilla_item_decorations(
                &mut vanilla_expected,
                &engine_sword,
                100.0,
                50.0,
                None,
            );
            vanilla_expected.clip_since(0, 100.0, 50.0, 114.0, 66.0);

            with_mods(model_only_mods, || {
                game.component_hover_effect(&mut model_only, &style, (40.0, 30.0));
            });
        });
    });

    assert_eq!(
        *DECORATOR_COORDS.lock().unwrap(),
        vec![[0.0, 0.0]],
        "decorator must run once in local coordinates for iron_sword and skip air"
    );

    // Model was squeezed vertically to [54.0, 62.0] around center y = 58.
    assert!(!model_only.vertices.is_empty());
    assert!(
        model_only
            .vertices
            .iter()
            .all(|v| v.pos[1] >= 53.99 && v.pos[1] <= 62.01),
        "squeezed model should stay within half-height band [54, 62]"
    );

    // Decorations follow the squeezed model in combined output.
    let model_len = model_only.vertices.len();
    assert_eq!(
        vertices_snapshot(&combined)[..model_len],
        vertices_snapshot(&model_only)[..]
    );
    let after_model = &combined.vertices[model_len..];
    // Vanilla decorations (bar + cooldown + count) are NOT squeezed and exceed the 3 quads of bar+cooldown.
    assert!(
        vanilla_expected.vertices.len() > 18,
        "vanilla decorations must include bar, cooldown and count"
    );
    assert!(
        vanilla_expected
            .vertices
            .iter()
            .any(|v| v.pos[1] >= 63.0 && v.pos[1] <= 66.0),
        "un-squeezed vanilla decorations must reach below the squeezed model band"
    );
    let vanilla_len = vanilla_expected.vertices.len();
    assert_eq!(
        after_model.len(),
        vanilla_len + 6,
        "expected vanilla decorations plus only the unclipped custom decorator quad"
    );
    assert_eq!(
        after_model[..vanilla_len]
            .iter()
            .map(|v| (v.pos, v.uv, v.color))
            .collect::<Vec<_>>(),
        vertices_snapshot(&vanilla_expected)
    );

    // The visible custom decorator quad sits at enclosing pose [101..103, 51..53] (unsqueezed).
    let custom = &after_model[vanilla_len..];
    assert_eq!(
        custom.iter().map(|v| (v.pos, v.color)).collect::<Vec<_>>(),
        vec![
            ([101.0, 51.0], abgr(DECORATOR_VISIBLE)),
            ([103.0, 51.0], abgr(DECORATOR_VISIBLE)),
            ([103.0, 53.0], abgr(DECORATOR_VISIBLE)),
            ([101.0, 51.0], abgr(DECORATOR_VISIBLE)),
            ([103.0, 53.0], abgr(DECORATOR_VISIBLE)),
            ([101.0, 53.0], abgr(DECORATOR_VISIBLE)),
        ]
    );
    assert!(
        combined
            .vertices
            .iter()
            .all(|v| v.color != abgr(DECORATOR_CLIPPED)),
        "custom decorator geometry outside the enclosing scissor must be clipped"
    );
}

struct SpriteAndBlitPainter;

#[mod_api::__subscriptions]
impl Hooks for SpriteAndBlitPainter {
    fn component_hover(&self, _: &dyn ClientHost, event: &mut ComponentHoverEvent<'_>) -> Flow {
        let texture = map_slot_sprite(0).unwrap();
        let full_region = GuiBlitRegion {
            uv: [0.0, 0.0],
            src: [8, 8],
            tex: [8, 8],
        };
        event
            .draw
            .draw_gui(&mut |canvas| {
                canvas.blit_sprite(
                    "hud/heart/full",
                    [10.0, 10.0, 9.0, 9.0],
                    None,
                    0xFFFF_FFFF,
                    Blend::Textured,
                )?;
                canvas.blit_sprite(
                    "hud/heart/full",
                    [20.0, 10.0, 5.0, 9.0],
                    Some(GuiSpriteRegion {
                        size: [9, 9],
                        offset: [0, 0],
                    }),
                    0xFFFF_FFFF,
                    Blend::Crosshair,
                )?;
                canvas.atlas_sprite("block/nether_portal", [30.0, 10.0, 8.0, 8.0], 0xFFFF_FFFF)?;
                canvas.blit(
                    texture,
                    [40.0, 10.0, 8.0, 8.0],
                    full_region,
                    0xFFFF_FFFF,
                    Blend::Vignette,
                )?;
                canvas.blit(
                    texture,
                    [50.0, 10.0, 8.0, 8.0],
                    full_region,
                    0xFFFF_FFFF,
                    Blend::Nausea,
                )?;

                // Every per-call refusal returns its error and emits no vertices.
                assert_eq!(
                    canvas.blit_sprite(
                        "test:unknown_sprite",
                        [60.0, 10.0, 8.0, 8.0],
                        None,
                        0xFFFF_FFFF,
                        Blend::Textured,
                    ),
                    Err(GuiDrawError::UnknownSprite)
                );
                assert_eq!(
                    canvas.atlas_sprite("test:unknown_atlas", [60.0, 10.0, 8.0, 8.0], 0xFFFF_FFFF,),
                    Err(GuiDrawError::UnknownSprite)
                );
                for blend in [Blend::Vignette, Blend::Nausea] {
                    assert_eq!(
                        canvas.blit_sprite(
                            "hud/heart/full",
                            [60.0, 10.0, 8.0, 8.0],
                            None,
                            0xFFFF_FFFF,
                            blend,
                        ),
                        Err(GuiDrawError::UnsupportedSpriteBlend)
                    );
                }
                assert_eq!(
                    canvas.blit_sprite(
                        "hud/heart/full",
                        [60.0, 10.0, 8.0, 8.0],
                        Some(GuiSpriteRegion {
                            size: [0, 9],
                            offset: [0, 0],
                        }),
                        0xFFFF_FFFF,
                        Blend::Textured,
                    ),
                    Err(GuiDrawError::BadSpriteRegion)
                );
                assert_eq!(
                    canvas.blit(
                        "test:unknown_texture",
                        [60.0, 10.0, 8.0, 8.0],
                        full_region,
                        0xFFFF_FFFF,
                        Blend::Textured,
                    ),
                    Err(GuiDrawError::UnknownTexture)
                );
                assert_eq!(
                    canvas.blit(
                        texture,
                        [60.0, 10.0, 8.0, 8.0],
                        GuiBlitRegion {
                            src: [0, 8],
                            ..full_region
                        },
                        0xFFFF_FFFF,
                        Blend::Textured,
                    ),
                    Err(GuiDrawError::BadBlitRegion)
                );
                assert_eq!(
                    canvas.blit(
                        texture,
                        [60.0, 10.0, 8.0, 8.0],
                        GuiBlitRegion {
                            tex: [8, 0],
                            ..full_region
                        },
                        0xFFFF_FFFF,
                        Blend::Textured,
                    ),
                    Err(GuiDrawError::BadBlitRegion)
                );
                assert_eq!(
                    canvas.blit_sprite(
                        "hud/heart/full",
                        [f32::NAN, 10.0, 8.0, 8.0],
                        None,
                        0xFFFF_FFFF,
                        Blend::Textured,
                    ),
                    Err(GuiDrawError::NotFinite)
                );
                assert_eq!(
                    canvas.blit(
                        texture,
                        [60.0, 10.0, 8.0, 8.0],
                        GuiBlitRegion {
                            uv: [f32::INFINITY, 0.0],
                            ..full_region
                        },
                        0xFFFF_FFFF,
                        Blend::Textured,
                    ),
                    Err(GuiDrawError::NotFinite)
                );
                Ok(())
            })
            .unwrap();
        Flow::Cancel
    }
}

#[test]
#[ignore = "daily"]
fn canvas_sprites_and_blits_draw_with_blends_and_refusals_keep_prior_draws() {
    let style = hover_style();
    let mods: mod_api::Mods = vec![ModEntry::new("blitter", Box::new(SpriteAndBlitPainter))].into();
    let mut batch = HudBatch::scratch([320, 240]);
    with_mods(mods, || {
        crate::game::native_entity_refs::tests::with_client_game(|game| {
            game.component_hover_effect(&mut batch, &style, (40.0, 30.0));
        });
    });

    // Exactly the 5 valid quads (30 vertices); none of the 10 refused calls added vertices.
    assert_eq!(batch.vertices.len(), 30);
    assert_eq!(batch.vertices[0].pos, [10.0, 10.0]);
    assert_eq!(batch.vertices[6].pos, [20.0, 10.0]);
    assert_eq!(batch.vertices[12].pos, [30.0, 10.0]);
    assert_eq!(batch.vertices[18].pos, [40.0, 10.0]);
    assert_eq!(batch.vertices[24].pos, [50.0, 10.0]);
    assert_eq!(
        batch.blends,
        [
            (6..12, HudBlend::Invert),
            (18..24, HudBlend::Vignette),
            (24..30, HudBlend::Additive),
        ]
    );
}
