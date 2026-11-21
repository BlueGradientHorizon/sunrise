//! Direct mod GUI drawing on the hook's HUD batch.

use crate::game::{
    Game,
    api_values::{TryIntoEngine, stack_value},
};
use mod_api::{GuiBlitRegion, GuiCanvas, GuiDrawError, GuiSpriteRegion, ItemStack, Text};
use sunrise_client::{
    hud::{GuiSprite, HudAtlas, HudBatch},
    hud_layout::{
        Blend, IDENTITY, Pose, PoseOp, ResolvedGuiQuad, abgr, paint_fill_gradient, pose_apply,
        pose_point,
    },
    items::ItemStack as EngineStack,
};

pub(crate) struct GameGuiCanvas<'a, 'g> {
    batch: &'a mut HudBatch,
    atlas: &'a HudAtlas,
    /// Absent only for the renderer-only canvas fixture.
    game: Option<&'a Game<'g>>,
    pose: Pose,
    /// Screen-space edges, not affected by future changes of pose.
    scissor: Option<[f32; 4]>,
}

impl<'a, 'g> GameGuiCanvas<'a, 'g> {
    pub(crate) fn new(
        batch: &'a mut HudBatch,
        atlas: &'a HudAtlas,
        game: Option<&'a Game<'g>>,
        pose: Pose,
        scissor: Option<[f32; 4]>,
    ) -> Self {
        Self {
            batch,
            atlas,
            game,
            pose,
            scissor,
        }
    }
    pub(crate) fn draw(
        batch: &mut HudBatch,
        atlas: &HudAtlas,
        game: Option<&Game<'_>>,
        draw: &mut dyn FnMut(&mut dyn GuiCanvas) -> Result<(), GuiDrawError>,
    ) -> Result<(), GuiDrawError> {
        Self::draw_with_scissor(batch, atlas, game, None, draw)
    }

    pub(crate) fn draw_with_scissor(
        batch: &mut HudBatch,
        atlas: &HudAtlas,
        game: Option<&Game<'_>>,
        scissor: Option<[f32; 4]>,
        draw: &mut dyn FnMut(&mut dyn GuiCanvas) -> Result<(), GuiDrawError>,
    ) -> Result<(), GuiDrawError> {
        let _callback_scope = mod_api::__private::contain_gui_callbacks();
        draw(&mut GameGuiCanvas::new(
            batch, atlas, game, IDENTITY, scissor,
        ))
    }

    fn draw_text(
        &mut self,
        text: &crate::text::Text,
        [x, y]: [f32; 2],
        argb: u32,
        shadow: bool,
    ) -> Result<(), GuiDrawError> {
        if !x.is_finite() || !y.is_finite() {
            return Err(GuiDrawError::NotFinite);
        }
        let start = self.batch.vertices.len();
        let pose = self.pose;
        let ok =
            self.batch
                .try_text_styled_posed(self.atlas, text, x, y, abgr(argb), shadow, |point| {
                    if pose == IDENTITY {
                        point
                    } else {
                        pose_point(pose, point)
                    }
                });
        if !ok {
            return Err(GuiDrawError::NotFinite);
        }
        self.clip_emitted(start);
        Ok(())
    }

    pub(crate) fn vanilla_decorations(
        &mut self,
        game: &Game<'_>,
        stack: &EngineStack,
        [x, y]: [f32; 2],
        count: Option<String>,
    ) -> Result<(), GuiDrawError> {
        let layout = game
            .item_decoration_state(stack, count)
            .layout(&self.atlas.font);
        for ([dx, dy, w, h], color) in layout.fills.into_iter().flatten() {
            self.fill([x + dx as f32, y + dy as f32, w as f32, h as f32], color)?;
        }
        if let Some(([dx, dy], label)) = layout.count {
            self.draw_text(&label, [x + dx as f32, y + dy as f32], u32::MAX, true)?;
        }
        Ok(())
    }

    pub(crate) fn draw_decorations(
        &mut self,
        game: &Game<'_>,
        stack: &EngineStack,
        at: [f32; 2],
        count: Option<String>,
    ) -> Result<(), GuiDrawError> {
        let _callback_scope = mod_api::__private::contain_gui_callbacks();
        self.vanilla_decorations(game, stack, at, count)?;
        // NeoForge renders custom decorations after the vanilla decorations, in the same scope.
        game.native_item_decorations(stack, at[0], at[1], self);
        Ok(())
    }

    fn check_rect_geometry(&self, rect @ [x, y, ..]: [f32; 4]) -> Result<[f32; 2], GuiDrawError> {
        let [right, bottom] = rect_end(rect)?;
        if self.pose == IDENTITY
            || [[x, y], [right, y], [right, bottom], [x, bottom]]
                .into_iter()
                .flat_map(|point| pose_point(self.pose, point))
                .all(f32::is_finite)
        {
            Ok([right, bottom])
        } else {
            Err(GuiDrawError::NotFinite)
        }
    }

    fn clip_emitted(&mut self, start: usize) {
        if let Some(clip) = self.scissor {
            self.batch.scissor_since(start, clip);
        }
    }

    fn paint_emit(
        &mut self,
        emit: impl FnOnce(&mut HudBatch, &HudAtlas),
    ) -> Result<(), GuiDrawError> {
        let start = self.batch.vertices.len();
        emit(self.batch, self.atlas);
        if !self.batch.try_pose_since(start, self.pose) {
            return Err(GuiDrawError::NotFinite);
        }
        self.clip_emitted(start);
        Ok(())
    }
}

/// The opposite corner of a finite `[x, y, w, h]`, including finite endpoint arithmetic.
fn rect_end([x, y, w, h]: [f32; 4]) -> Result<[f32; 2], GuiDrawError> {
    let end = [x + w, y + h];
    if [x, y, w, h].into_iter().chain(end).all(f32::is_finite) {
        Ok(end)
    } else {
        Err(GuiDrawError::NotFinite)
    }
}

/// The API count is nonzero, but vanilla also treats air as an empty stack.
fn drawable_stack(stack: &ItemStack) -> Result<Option<EngineStack>, GuiDrawError> {
    let stack = stack_value(stack).map_err(GuiDrawError::Stack)?;
    Ok((&*stack.item != "minecraft:air").then_some(stack))
}

impl GuiCanvas for GameGuiCanvas<'_, '_> {
    fn size(&self) -> [f32; 2] {
        [self.batch.width, self.batch.height]
    }

    fn scope(
        &mut self,
        ops: &[PoseOp],
        scissor: Option<[f32; 4]>,
        draw: &mut dyn FnMut(&mut dyn GuiCanvas) -> Result<(), GuiDrawError>,
    ) -> Result<(), GuiDrawError> {
        let mut pose = self.pose;
        for &op in ops {
            let finite = match op {
                PoseOp::Translate(x, y) | PoseOp::Scale(x, y) => x.is_finite() && y.is_finite(),
                PoseOp::Rotate(angle) => angle.is_finite(),
            };
            if !finite {
                return Err(GuiDrawError::NotFinite);
            }
            pose_apply(&mut pose, op);
            if !pose.into_iter().all(f32::is_finite) {
                return Err(GuiDrawError::NotFinite);
            }
        }
        let clip = if let Some(rect) = scissor {
            Some(
                sunrise_client::ui::intersect_posed_scissor(self.batch, pose, rect, self.scissor)
                    .map_err(|_| GuiDrawError::NotFinite)?,
            )
        } else {
            self.scissor
        };
        draw(&mut GameGuiCanvas {
            batch: self.batch,
            atlas: self.atlas,
            game: self.game,
            pose,
            scissor: clip,
        })
    }

    fn fill(&mut self, rect @ [x, y, ..]: [f32; 4], argb: u32) -> Result<(), GuiDrawError> {
        let [x1, y1] = self.check_rect_geometry(rect)?;
        let (left, right) = (x.min(x1), x.max(x1));
        let (top, bottom) = (y.min(y1), y.max(y1));
        self.paint_emit(|batch, atlas| {
            batch.fill_quad(
                atlas,
                [[left, top], [right, top], [right, bottom], [left, bottom]],
                abgr(argb),
            );
        })
    }

    fn fill_gradient(&mut self, rect: [f32; 4], top: u32, bottom: u32) -> Result<(), GuiDrawError> {
        self.check_rect_geometry(rect)?;
        self.paint_emit(|batch, atlas| paint_fill_gradient(batch, atlas, rect, top, bottom))
    }

    fn text(
        &mut self,
        text: &Text,
        at: [f32; 2],
        argb: u32,
        shadow: bool,
    ) -> Result<(), GuiDrawError> {
        if argb >> 24 == 0 {
            return Ok(());
        }
        let text = text.try_into_engine(&()).map_err(GuiDrawError::Text)?;
        self.draw_text(&text, at, argb, shadow)
    }

    fn item(&mut self, stack: &ItemStack, [x, y]: [f32; 2], seed: i32) -> Result<(), GuiDrawError> {
        let Some(stack) = drawable_stack(stack)? else {
            return Ok(());
        };
        self.check_rect_geometry([x, y, 16.0, 16.0])?;
        let game = self.game.ok_or(GuiDrawError::Unavailable)?;
        self.paint_emit(|batch, _| {
            game.draw_item_model(batch, &stack, x, y, seed);
        })
    }

    fn item_decorations(
        &mut self,
        stack: &ItemStack,
        [x, y]: [f32; 2],
        count: Option<&str>,
    ) -> Result<(), GuiDrawError> {
        let Some(stack) = drawable_stack(stack)? else {
            return Ok(());
        };
        if !x.is_finite() || !y.is_finite() {
            return Err(GuiDrawError::NotFinite);
        }
        let game = self.game.ok_or(GuiDrawError::Unavailable)?;
        self.draw_decorations(game, &stack, [x, y], count.map(str::to_owned))
    }

    fn blit_sprite(
        &mut self,
        name: &str,
        dst: [f32; 4],
        region: Option<GuiSpriteRegion>,
        argb: u32,
        blend: Blend,
    ) -> Result<(), GuiDrawError> {
        self.check_rect_geometry(dst)?;
        if matches!(blend, Blend::Vignette | Blend::Nausea) {
            return Err(GuiDrawError::UnsupportedSpriteBlend);
        }
        let region = if let Some(GuiSpriteRegion {
            size: [w, h],
            offset: [u, v],
        }) = region
        {
            if w <= 0 || h <= 0 {
                return Err(GuiDrawError::BadSpriteRegion);
            }
            if !(u as f32 + dst[2]).is_finite() || !(v as f32 + dst[3]).is_finite() {
                return Err(GuiDrawError::NotFinite);
            }
            Some([w, h, u, v])
        } else {
            None
        };
        let Some(GuiSprite::Present { rect, scaling }) = self.atlas.gui_lookup(name) else {
            return Err(GuiDrawError::UnknownSprite);
        };
        if sunrise_client::ui::blit_gui_sprite(
            self.batch,
            self.atlas,
            name,
            sunrise_client::ui::GuiSpriteBlit {
                rect,
                scaling,
                dst,
                color: abgr(argb),
                region,
                blend,
            },
            self.pose,
            self.scissor,
        ) {
            Ok(())
        } else {
            Err(GuiDrawError::NotFinite)
        }
    }

    fn atlas_sprite(&mut self, name: &str, dst: [f32; 4], argb: u32) -> Result<(), GuiDrawError> {
        self.check_rect_geometry(dst)?;
        let rect = *self
            .atlas
            .sprites
            .get(name)
            .ok_or(GuiDrawError::UnknownSprite)?;
        let quad = ResolvedGuiQuad {
            rect,
            dst,
            color: argb,
        };
        self.paint_emit(|batch, _| quad.paint(batch))
    }

    fn blit(
        &mut self,
        texture: &str,
        dst: [f32; 4],
        GuiBlitRegion { uv, src, tex }: GuiBlitRegion,
        argb: u32,
        blend: Blend,
    ) -> Result<(), GuiDrawError> {
        self.check_rect_geometry(dst)?;
        if ![uv[0], uv[1], uv[0] + src[0] as f32, uv[1] + src[1] as f32]
            .into_iter()
            .all(f32::is_finite)
        {
            return Err(GuiDrawError::NotFinite);
        }
        if tex.into_iter().any(|extent| extent <= 0) {
            return Err(GuiDrawError::BadBlitRegion);
        }
        let rect = *self
            .atlas
            .blit_rect(texture)
            .ok_or(GuiDrawError::UnknownTexture)?;
        let quad = ResolvedGuiQuad {
            rect,
            dst,
            color: argb,
        };
        self.paint_emit(|batch, _| quad.paint_blit(batch, uv, src, tex, blend))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hud::{Blend as HudBlend, Rect};
    use crate::resource_source::ResourceSource;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[test]
    fn transparent_text_does_not_prepare_an_overflowing_glyph() {
        let atlas = HudAtlas::build(&ResourceSource::directory(
            "/nonexistent/sunrise-test-assets".into(),
        ))
        .unwrap();
        let mut batch = HudBatch::scratch([320, 240]);
        GameGuiCanvas::draw(&mut batch, &atlas, None, &mut |root| {
            root.scope(
                &[PoseOp::Scale(f32::MAX / 20.0, 1.0)],
                None,
                &mut |canvas| {
                    canvas.text(
                        &Text::literal("many visible glyphs"),
                        [0.0, 0.0],
                        0x00FF_FFFF,
                        true,
                    )
                },
            )
        })
        .unwrap();
        assert!(batch.vertices.is_empty());
        assert!(batch.inline_profiles.is_empty());
    }

    #[test]
    fn a_wide_nine_slice_only_visits_tiles_inside_the_scissor() {
        use sunrise_client::ui::{GuiSpriteScaling, NineSlice};
        let mut atlas = HudAtlas::build(&ResourceSource::directory(
            "/nonexistent/sunrise-test-assets".into(),
        ))
        .unwrap();
        atlas.gui.insert(
            "test:wide".into(),
            GuiSprite::Present {
                rect: Rect {
                    u0: 0.0,
                    v0: 0.0,
                    u1: 1.0,
                    v1: 1.0,
                    w: 3,
                    h: 3,
                },
                scaling: Some(GuiSpriteScaling::NineSlice(NineSlice {
                    width: 3,
                    height: 3,
                    border: [1; 4],
                    stretch_inner: false,
                })),
            },
        );
        let mut batch = HudBatch::scratch([320, 240]);
        GameGuiCanvas::draw(&mut batch, &atlas, None, &mut |root| {
            root.scope(&[], Some([10.0, 10.0, 3.0, 1.0]), &mut |canvas| {
                canvas.blit_sprite(
                    "test:wide",
                    [-20_000_000.0, 9.0, 32_000_000.0, 3.0],
                    None,
                    u32::MAX,
                    Blend::Textured,
                )
            })
        })
        .unwrap();
        assert_eq!(batch.vertices.len(), 18);
        for (i, quad) in batch.vertices.chunks_exact(6).enumerate() {
            assert_eq!(quad[0].pos, [10.0 + i as f32, 10.0]);
            assert_eq!(quad[0].uv, [1.0 / 3.0, 1.0 / 3.0]);
        }
    }

    fn tiled_status_pack(shift: bool) -> HudAtlas {
        use sunrise_assets::resource_source::Resource;
        let mut png = Vec::new();
        let mut encoder = png::Encoder::new(&mut png, 2, 2);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[255; 16])
            .unwrap();
        let mut files = std::collections::BTreeMap::from([
            ("assets/test/textures/gui/tile.png".into(), Resource::bytes(png)),
            ("assets/test/textures/gui/tile.png.mcmeta".into(), Resource::bytes(br#"{"gui":{"scaling":{"type":"tile","width":2,"height":2}}}"#.to_vec())),
            ("assets/minecraft/atlases/gui.json".into(), Resource::bytes(br#"{"sources":[{"type":"minecraft:single","resource":"test:gui/tile","sprite":"minecraft:hud/food_full"}]}"#.to_vec())),
        ]);
        if shift {
            files.insert(
                "assets/test/polytone/overlay_modifiers/shift.json".into(),
                Resource::bytes(br#"{"texture":"minecraft:hud/food_full","x_inc":4}"#.to_vec()),
            );
        }
        HudAtlas::build(&ResourceSource::indexed(files)).unwrap()
    }

    #[test]
    fn pack_tile_scaling_matches_in_hud_and_canvas_for_full_and_partial_blits() {
        let atlas = tiled_status_pack(false);
        let r = *atlas.gui_draw_rect("hud/food_full").unwrap();
        for (dst, region, expected) in [
            (
                [10.0, 10.0, 5.0, 2.0],
                None,
                vec![(10.0, r.u0), (12.0, r.u0), (14.0, r.u0)],
            ),
            (
                [10.0, 10.0, 3.0, 2.0],
                Some(GuiSpriteRegion {
                    size: [5, 2],
                    offset: [1, 0],
                }),
                vec![(10.0, (r.u0 + r.u1) / 2.0), (11.0, r.u0)],
            ),
        ] {
            let [x, y, w, h] = dst.map(|n| n as i32);
            let mut canvas_batch = HudBatch::scratch([320, 240]);
            GameGuiCanvas::draw(&mut canvas_batch, &atlas, None, &mut |canvas| {
                canvas.blit_sprite("hud/food_full", dst, region, u32::MAX, Blend::Crosshair)
            })
            .unwrap();
            let mut hud_batch = HudBatch::scratch([320, 240]);
            sunrise_client::hud_layout::paint(
                &mut hud_batch,
                &atlas,
                &[sunrise_client::hud_layout::GuiDraw::Sprite {
                    sprite: "hud/food_full".into(),
                    x,
                    y,
                    w,
                    h,
                    region: region.map(
                        |GuiSpriteRegion {
                             size: [sw, sh],
                             offset: [u, v],
                         }| [sw, sh, u, v],
                    ),
                    color: u32::MAX,
                    blend: Blend::Crosshair,
                }],
            );
            for batch in [canvas_batch, hud_batch] {
                assert_eq!(
                    batch.blends,
                    [(0..expected.len() as u32 * 6, HudBlend::Invert)]
                );
                assert_eq!(
                    batch
                        .vertices
                        .chunks_exact(6)
                        .map(|q| (q[0].pos[0], q[0].uv[0]))
                        .collect::<Vec<_>>(),
                    expected,
                );
            }
        }
    }

    #[test]
    fn extreme_tile_counts_clip_both_axes_before_submitting_repeats() {
        use sunrise_client::ui::GuiSpriteScaling;
        let mut atlas = HudAtlas::build(&ResourceSource::indexed(Default::default())).unwrap();
        atlas.gui.insert(
            "test:tile".into(),
            GuiSprite::Present {
                rect: Rect {
                    u0: 0.25,
                    v0: 0.25,
                    u1: 0.75,
                    v1: 0.75,
                    w: 2,
                    h: 2,
                },
                scaling: Some(GuiSpriteScaling::Tile([1, 1])),
            },
        );
        for (pose, dst, visible) in [
            (
                [PoseOp::Scale(1.0, 1e-20), PoseOp::Translate(0.0, 0.0)],
                [-2.0, 0.0, 1.0, 1e22],
                false,
            ),
            (
                [PoseOp::Translate(0.0, 0.0625), PoseOp::Scale(1.0, 0.25)],
                [0.0, 0.0, 1.0, 1e22],
                true,
            ),
        ] {
            let mut batch = HudBatch::scratch([320, 240]);
            GameGuiCanvas::draw(&mut batch, &atlas, None, &mut |root| {
                root.scope(&pose, None, &mut |canvas| {
                    canvas.blit_sprite("test:tile", dst, None, u32::MAX, Blend::Textured)
                })
            })
            .unwrap();
            assert_eq!(batch.vertices.len(), if visible { 6 } else { 0 });
            if let Some(v) = batch.vertices.first() {
                // This quad repeats this atlas subrect; the renderer supplies the phase at upload.
                assert_eq!(v.repeat_x[3], 1);
                assert_eq!((v.uv, v.glint_uv), ([0.25; 2], [0.75; 2]));
            }
        }
    }

    #[test]
    fn sprite_modifier_cannot_publish_non_finite_vertices_or_blends() {
        let atlas = tiled_status_pack(true);
        let mut batch = HudBatch::scratch([320, 240]);
        GameGuiCanvas::draw(&mut batch, &atlas, None, &mut |root| {
            root.blit_sprite(
                "hud/food_full",
                [0.0, 0.0, 1.0, 1.0],
                None,
                u32::MAX,
                Blend::Crosshair,
            )?;
            root.scope(&[PoseOp::Scale(f32::MAX / 2.0, 1.0)], None, &mut |canvas| {
                assert_eq!(
                    canvas.blit_sprite(
                        "hud/food_full",
                        [0.0, 0.0, 1.0, 1.0],
                        None,
                        u32::MAX,
                        Blend::Crosshair
                    ),
                    Err(GuiDrawError::NotFinite)
                );
                Ok(())
            })
        })
        .unwrap();
        assert_eq!(batch.vertices.len(), 6);
        assert_eq!(batch.vertices[0].pos, [4.0, 0.0]);
        assert_eq!(batch.blends, [(0..6, HudBlend::Invert)]);
    }

    #[test]
    fn posed_and_scissored_scopes_clip_leaves_and_restore_pose_and_scissor_on_err_and_unwind() {
        let mut atlas = HudAtlas::build(&ResourceSource::directory(
            "/nonexistent/sunrise-test-assets".into(),
        ))
        .unwrap();
        atlas.live.insert(
            "test:canvas".into(),
            Rect {
                u0: 0.25,
                v0: 0.125,
                u1: 0.75,
                v1: 0.625,
                w: 4,
                h: 4,
            },
        );
        let crosshair = |canvas: &mut dyn GuiCanvas, dst| {
            canvas.blit(
                "test:canvas",
                dst,
                GuiBlitRegion {
                    uv: [0.0, 0.0],
                    src: [4, 4],
                    tex: [4, 4],
                },
                0x8011_2233,
                Blend::Crosshair,
            )
        };
        let mut batch = HudBatch::scratch([320, 240]);
        GameGuiCanvas::draw(&mut batch, &atlas, None, &mut |root| {
            crosshair(root, [2.0, 3.0, 2.0, 2.0])
        })
        .unwrap();

        GameGuiCanvas::draw(&mut batch, &atlas, None, &mut |root| {
            assert_eq!(root.size(), [320.0, 240.0]);
            root.scope(
                &[PoseOp::Translate(10.75, 20.25)],
                Some([0.0, 0.0, 5.75, 5.75]),
                &mut |parent| {
                    parent.scope(
                        &[PoseOp::Translate(1.0, 1.0), PoseOp::Scale(2.0, 3.0)],
                        Some([0.25, 0.25, 3.0, 3.0]),
                        &mut |child| crosshair(child, [0.0, 0.0, 4.0, 4.0]),
                    )?;
                    for angle in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                        assert_eq!(
                            parent.scope(
                                &[PoseOp::Translate(2.0, 0.0), PoseOp::Rotate(angle)],
                                None,
                                &mut |_| panic!("a refused rotation must not invoke the callback"),
                            ),
                            Err(GuiDrawError::NotFinite),
                        );
                    }
                    assert_eq!(
                        parent.scope(
                            &[PoseOp::Scale(f32::MAX, 1.0), PoseOp::Scale(2.0, 1.0)],
                            None,
                            &mut |_| {
                                panic!("a scope with a non-finite composed pose must not run")
                            }
                        ),
                        Err(GuiDrawError::NotFinite)
                    );
                    assert_eq!(
                        parent.scope(&[], Some([f32::MAX, 0.0, f32::MAX, 1.0]), &mut |_| {
                            panic!("a scope with a non-finite clip edge must not run")
                        }),
                        Err(GuiDrawError::NotFinite)
                    );

                    // A child that errors keeps what it already emitted, while restoring parent
                    // pose and scissor.
                    assert_eq!(
                        parent.scope(
                            &[PoseOp::Translate(1.0, 0.0)],
                            Some([0.0, 0.0, 1.0, 1.0]),
                            &mut |child| {
                                crosshair(child, [0.0, 0.0, 1.0, 1.0])?;
                                child.blit(
                                    "test:missing",
                                    [0.0, 0.0, 1.0, 1.0],
                                    GuiBlitRegion {
                                        uv: [0.0, 0.0],
                                        src: [4, 4],
                                        tex: [4, 4],
                                    },
                                    u32::MAX,
                                    Blend::Textured,
                                )
                            }
                        ),
                        Err(GuiDrawError::UnknownTexture)
                    );
                    // A child that panics keeps its prior draw and still restores parent pose
                    // and scissor on unwind.
                    let unwind = catch_unwind(AssertUnwindSafe(|| {
                        parent.scope(
                            &[PoseOp::Translate(2.0, 0.0)],
                            Some([0.0, 0.0, 1.0, 1.0]),
                            &mut |child| {
                                crosshair(child, [0.0, 0.0, 1.0, 1.0])?;
                                panic!("restore parent pose and scissor on unwind")
                            },
                        )
                    }));
                    assert!(unwind.is_err());

                    // A refused leaf under a scaling pose emits no vertices.
                    parent.scope(&[PoseOp::Scale(2.0, 1.0)], None, &mut |child| {
                        assert_eq!(
                            crosshair(child, [f32::MAX, 0.0, 1.0, 1.0]),
                            Err(GuiDrawError::NotFinite)
                        );
                        Ok(())
                    })?;
                    // Parent pose (10.75, 20.25) and scissor [10, 20, 15, 25] are intact.
                    parent.fill_gradient([2.0, 2.0, 1.0, 1.0], 0x80AA_1234, 0xFF00_0000)
                },
            )?;
            // Root identity pose and unclipped scissor are intact.
            root.fill([30.0, 40.0, 2.0, 2.0], 0xFFFF_FFFF)
        })
        .unwrap();

        assert_eq!(batch.vertices.len(), 36);
        assert_eq!(batch.vertices[0].pos, [2.0, 3.0]);
        assert_eq!(batch.vertices[6].pos, [12.0, 22.0]);
        assert_eq!(batch.vertices[8].pos, [15.0, 25.0]);
        assert_eq!(batch.vertices[6].uv, [0.265625, 0.15625]);
        assert_eq!(batch.vertices[6].color, 0x8033_2211);
        // Kept draw from Err child (translated by +1 in x, clipped to [11, 20, 12, 21]).
        assert_eq!(batch.vertices[12].pos, [11.75, 20.25]);
        assert_eq!(batch.vertices[14].pos, [12.0, 21.0]);
        // Kept draw from panicking child (translated by +2 in x, clipped to [12, 20, 13, 21]).
        assert_eq!(batch.vertices[18].pos, [12.75, 20.25]);
        assert_eq!(batch.vertices[20].pos, [13.0, 21.0]);
        // Parent gradient after Err and unwind, still under (10.75, 20.25).
        assert_eq!(batch.vertices[24].pos, [12.75, 22.25]);
        assert_eq!(batch.vertices[24].color, 0x8034_12AA);
        assert_eq!(batch.vertices[26].color, 0xFF00_0000);
        // Root fill after scope exit.
        assert_eq!(batch.vertices[30].pos, [30.0, 40.0]);
        assert_eq!(batch.blends, [(0..24, HudBlend::Invert)]);
    }

    #[test]
    fn per_call_validation_emits_no_vertices_and_keeps_prior_draws() {
        const DST: [f32; 4] = [10.0, 20.0, 4.0, 4.0];
        const WHITE: u32 = u32::MAX;
        const REGION: GuiBlitRegion = GuiBlitRegion {
            uv: [0.0, 0.0],
            src: [4, 4],
            tex: [4, 4],
        };
        let mut atlas = HudAtlas::build(&ResourceSource::directory(
            "/nonexistent/sunrise-test-assets".into(),
        ))
        .unwrap();
        let rect = Rect {
            u0: 0.25,
            v0: 0.5,
            u1: 0.5,
            v1: 0.75,
            w: 4,
            h: 4,
        };
        atlas.gui.insert(
            "test:present".into(),
            GuiSprite::Present {
                rect,
                scaling: None,
            },
        );
        atlas.gui.insert("test:removed".into(), GuiSprite::Missing);
        atlas.sprites.insert("test:removed".into(), rect);
        atlas.live.insert("test:live".into(), rect);

        let mut batch = HudBatch::scratch([320, 240]);
        GameGuiCanvas::draw(&mut batch, &atlas, None, &mut |root| {
            root.atlas_sprite("test:removed", DST, WHITE)?;
            root.blit_sprite("test:present", DST, None, WHITE, Blend::Crosshair)
        })
        .unwrap();
        assert_eq!(batch.vertices.len(), 12);
        assert_eq!(batch.vertices[0].uv, [0.25, 0.5]);
        assert_eq!(batch.vertices[6].uv, [0.25, 0.5]);
        assert_eq!(batch.blends, [(6..12, HudBlend::Invert)]);
        let before = bytemuck::cast_slice::<_, u8>(&batch.vertices).to_vec();

        let mut refuses = |draw: &mut dyn FnMut(&mut dyn GuiCanvas) -> Result<(), GuiDrawError>,
                           expected| {
            assert_eq!(
                GameGuiCanvas::draw(&mut batch, &atlas, None, draw),
                Err(expected)
            );
            assert_eq!(
                bytemuck::cast_slice::<_, u8>(&batch.vertices),
                before.as_slice()
            );
            assert_eq!(batch.blends, [(6..12, HudBlend::Invert)]);
        };
        for name in [
            "test:unknown",
            "test:removed",
            "mob_effect/test_missing",
            "test:mob_effect/missing",
        ] {
            refuses(
                &mut |gui| gui.blit_sprite(name, DST, None, WHITE, Blend::Textured),
                GuiDrawError::UnknownSprite,
            );
        }
        refuses(
            &mut |gui| gui.atlas_sprite("test:unknown", DST, WHITE),
            GuiDrawError::UnknownSprite,
        );
        for blend in [Blend::Nausea, Blend::Vignette] {
            refuses(
                &mut |gui| gui.blit_sprite("test:present", DST, None, WHITE, blend),
                GuiDrawError::UnsupportedSpriteBlend,
            );
        }
        for (size, blend) in [
            ([0, 4], Blend::Textured),
            ([4, 0], Blend::Textured),
            ([-1, 4], Blend::Crosshair),
        ] {
            let region = Some(GuiSpriteRegion {
                size,
                offset: [0; 2],
            });
            refuses(
                &mut |gui| gui.blit_sprite("test:present", DST, region, WHITE, blend),
                GuiDrawError::BadSpriteRegion,
            );
        }
        refuses(
            &mut |gui| gui.blit("test:unknown", DST, REGION, WHITE, Blend::Vignette),
            GuiDrawError::UnknownTexture,
        );
        for (region, error) in [
            (
                GuiBlitRegion {
                    tex: [0, 4],
                    ..REGION
                },
                GuiDrawError::BadBlitRegion,
            ),
            (
                GuiBlitRegion {
                    tex: [4, -1],
                    ..REGION
                },
                GuiDrawError::BadBlitRegion,
            ),
            (
                GuiBlitRegion {
                    uv: [f32::NAN, 0.0],
                    ..REGION
                },
                GuiDrawError::NotFinite,
            ),
            (
                GuiBlitRegion {
                    uv: [f32::INFINITY, 0.0],
                    ..REGION
                },
                GuiDrawError::NotFinite,
            ),
        ] {
            refuses(
                &mut |gui| gui.blit("test:live", DST, region, WHITE, Blend::Textured),
                error,
            );
        }
        refuses(
            &mut |gui| gui.fill([f32::MAX, 0.0, f32::MAX, 1.0], WHITE),
            GuiDrawError::NotFinite,
        );
        refuses(
            &mut |gui| gui.fill_gradient([0.0, f32::NAN, 4.0, 4.0], WHITE, WHITE),
            GuiDrawError::NotFinite,
        );
        refuses(
            &mut |gui| gui.text(&Text::literal("hi"), [f32::INFINITY, 0.0], WHITE, false),
            GuiDrawError::NotFinite,
        );
    }

    #[test]
    fn scoped_text_keeps_style_and_prior_text_stays_when_later_text_refuses() {
        let atlas = HudAtlas::build(&ResourceSource::directory(
            "/nonexistent/sunrise-test-assets".into(),
        ))
        .unwrap();
        let mut styled = Text::literal("A");
        styled.style.color = Some("#112233".into());
        styled.style.underlined = Some(true);
        let too_deep = (0..512).fold(Text::literal("leaf"), |child, _| {
            Text::literal("").append(child)
        });
        let at = [10.0, 20.0];
        let mut batch = HudBatch::scratch([320, 240]);
        assert_eq!(
            GameGuiCanvas::draw(&mut batch, &atlas, None, &mut |root| {
                root.text(&styled, at, u32::MAX, false)?;
                root.text(&too_deep, at, u32::MAX, false)
            }),
            Err(GuiDrawError::Text(mod_api::text::TextFormError::TooDeep))
        );
        // The first draw succeeded and stays in the batch; the refused second draw emitted nothing.
        assert!(!batch.vertices.is_empty());
        assert!(
            batch
                .vertices
                .iter()
                .all(|vertex| vertex.color == 0xFF33_2211)
        );
        let count_after_first = batch.vertices.len();
        assert_eq!(
            GameGuiCanvas::draw(&mut batch, &atlas, None, &mut |root| {
                root.text(&too_deep, at, u32::MAX, false)
            }),
            Err(GuiDrawError::Text(mod_api::text::TextFormError::TooDeep))
        );
        assert_eq!(batch.vertices.len(), count_after_first);
    }

    #[test]
    fn gui_scissors_are_empty_on_reflection_inversion_and_disjoint_intersection() {
        let atlas = HudAtlas::build(&ResourceSource::directory(
            "/nonexistent/sunrise-test-assets".into(),
        ))
        .unwrap();
        let mut batch = HudBatch::scratch([320, 240]);
        GameGuiCanvas::draw(&mut batch, &atlas, None, &mut |root| {
            let mut over_scissor =
                |canvas: &mut dyn GuiCanvas| canvas.fill([-32.0, -32.0, 64.0, 64.0], 0xFFFF_FFFF);
            root.scope(
                &[PoseOp::Scale(-1.0, 1.0)],
                Some([1.0, 1.0, 4.0, 4.0]),
                &mut over_scissor,
            )?;
            root.scope(
                &[PoseOp::Scale(-1.0, 1.0)],
                Some([5.0, 1.0, -4.0, 4.0]),
                &mut over_scissor,
            )?;
            root.scope(&[], Some([0.0, 0.0, 4.0, 4.0]), &mut |parent| {
                parent.scope(&[], Some([6.0, 6.0, 4.0, 4.0]), &mut over_scissor)
            })?;
            // A singular, finite pose is allowed. A scissor with zero transformed width is empty.
            root.scope(
                &[PoseOp::Scale(0.0, 1.0)],
                Some([0.0, 0.0, 4.0, 4.0]),
                &mut over_scissor,
            )
        })
        .unwrap();
        assert!(batch.vertices.is_empty());
        assert!(batch.blends.is_empty());
    }

    #[test]
    fn deep_scope_nesting_exceeds_64_levels() {
        fn nest(canvas: &mut dyn GuiCanvas, depth: usize) -> Result<(), GuiDrawError> {
            if depth == 0 {
                return canvas.fill([0.0, 0.0, 4.0, 4.0], 0xFFFF_FFFF);
            }
            canvas.scope(
                &[PoseOp::Translate(1.0, 0.0)],
                Some([0.0, 0.0, 200.0, 200.0]),
                &mut |child| nest(child, depth - 1),
            )
        }

        let atlas = HudAtlas::build(&ResourceSource::directory(
            "/nonexistent/sunrise-test-assets".into(),
        ))
        .unwrap();
        let mut batch = HudBatch::scratch([320, 240]);
        GameGuiCanvas::draw(&mut batch, &atlas, None, &mut |root| nest(root, 100)).unwrap();
        assert_eq!(batch.vertices.len(), 6);
        assert_eq!(batch.vertices[0].pos, [100.0, 0.0]);
        assert_eq!(batch.vertices[2].pos, [104.0, 4.0]);
    }

    #[test]
    fn blit_allows_negative_and_zero_src_extents() {
        let mut atlas = HudAtlas::build(&ResourceSource::directory(
            "/nonexistent/sunrise-test-assets".into(),
        ))
        .unwrap();
        atlas.live.insert(
            "test:mirror".into(),
            Rect {
                u0: 0.25,
                v0: 0.5,
                u1: 0.75,
                v1: 1.0,
                w: 8,
                h: 8,
            },
        );
        let mut batch = HudBatch::scratch([320, 240]);
        GameGuiCanvas::draw(&mut batch, &atlas, None, &mut |root| {
            // Mirrored horizontally (`src[0] < 0`) and vertically (`src[1] < 0`).
            root.blit(
                "test:mirror",
                [0.0, 0.0, 8.0, 8.0],
                GuiBlitRegion {
                    uv: [8.0, 8.0],
                    src: [-8, -8],
                    tex: [8, 8],
                },
                u32::MAX,
                Blend::Textured,
            )?;
            // Zero `src[0]` samples a constant u coordinate across the quad.
            root.blit(
                "test:mirror",
                [10.0, 0.0, 4.0, 8.0],
                GuiBlitRegion {
                    uv: [4.0, 0.0],
                    src: [0, 8],
                    tex: [8, 8],
                },
                u32::MAX,
                Blend::Textured,
            )
        })
        .unwrap();
        assert_eq!(batch.vertices.len(), 12);
        // Mirrored quad: u goes 0.75 -> 0.25, v goes 1.0 -> 0.5.
        assert_eq!(batch.vertices[0].uv, [0.75, 1.0]);
        assert_eq!(batch.vertices[1].uv, [0.25, 1.0]);
        assert_eq!(batch.vertices[2].uv, [0.25, 0.5]);
        // Zero-width src: u stays at midpoint 0.5, v goes 0.5 -> 1.0.
        assert_eq!(batch.vertices[6].uv, [0.5, 0.5]);
        assert_eq!(batch.vertices[7].uv, [0.5, 0.5]);
        assert_eq!(batch.vertices[8].uv, [0.5, 1.0]);
    }

    #[test]
    fn blit_sprite_honours_nine_slice_scaling_full_and_partial() {
        let mut atlas = HudAtlas::build(&ResourceSource::directory(
            "/nonexistent/sunrise-test-assets".into(),
        ))
        .unwrap();
        let rect = Rect {
            u0: 0.0,
            v0: 0.0,
            u1: 0.5,
            v1: 0.5,
            w: 12,
            h: 12,
        };
        let scaling = sunrise_client::ui::NineSlice {
            width: 12,
            height: 12,
            border: [3, 3, 3, 3],
            stretch_inner: true,
        };
        atlas.gui.insert(
            "test:panel".into(),
            GuiSprite::Present {
                rect,
                scaling: Some(sunrise_client::ui::GuiSpriteScaling::NineSlice(scaling)),
            },
        );

        let mut full = HudBatch::scratch([320, 240]);
        GameGuiCanvas::draw(&mut full, &atlas, None, &mut |root| {
            root.blit_sprite(
                "test:panel",
                [10.0, 20.0, 24.0, 18.0],
                None,
                0xFFFF_FFFF,
                Blend::Crosshair,
            )
        })
        .unwrap();
        // Nine-slice emits 9 quads (54 vertices) instead of 1 stretched quad, all inverted.
        assert_eq!(full.vertices.len(), 54);
        assert_eq!(full.blends, [(0..54, HudBlend::Invert)]);

        // Partial nine-slice blits full sprite at (dst - offset, sw x sh) scissored to dst.
        let mut partial = HudBatch::scratch([320, 240]);
        GameGuiCanvas::draw(&mut partial, &atlas, None, &mut |root| {
            root.blit_sprite(
                "test:panel",
                [10.0, 20.0, 8.0, 6.0],
                Some(GuiSpriteRegion {
                    size: [24, 18],
                    offset: [4, 2],
                }),
                0xFFFF_FFFF,
                Blend::Textured,
            )
        })
        .unwrap();
        assert!(!partial.vertices.is_empty());
        for v in &partial.vertices {
            assert!(
                v.pos[0] >= 10.0 && v.pos[0] <= 18.0 && v.pos[1] >= 20.0 && v.pos[1] <= 26.0,
                "partial nine-slice vertex {:?} escaped dst scissor",
                v.pos
            );
        }

        // Non-positive nine-slice destination draws nothing.
        let mut empty = HudBatch::scratch([320, 240]);
        GameGuiCanvas::draw(&mut empty, &atlas, None, &mut |root| {
            root.blit_sprite(
                "test:panel",
                [10.0, 20.0, 0.0, 12.0],
                None,
                u32::MAX,
                Blend::Textured,
            )?;
            root.blit_sprite(
                "test:panel",
                [10.0, 20.0, -4.0, 12.0],
                None,
                u32::MAX,
                Blend::Textured,
            )
        })
        .unwrap();
        assert!(empty.vertices.is_empty());
    }

    #[test]
    fn text_rejects_actual_emitted_vertex_overflow_without_partial_vertices() {
        let atlas = HudAtlas::build(&ResourceSource::directory(
            "/nonexistent/sunrise-test-assets".into(),
        ))
        .unwrap();
        let mut styled = Text::literal("A");
        styled.style.underlined = Some(true);
        let mut batch = HudBatch::scratch([320, 240]);
        let res = GameGuiCanvas::draw(&mut batch, &atlas, None, &mut |root| {
            // At x = 0.0, `Scale(f32::MAX / 16.0, 1.0)` after `Translate(-f32::MAX, 0.0)` maps
            // `[0.0, 0.0]` and `[w, 0.0]` to finite values, while the underline's left edge at
            // `x - 1.0` overflows to `-inf`.
            root.scope(
                &[
                    PoseOp::Translate(-f32::MAX, 0.0),
                    PoseOp::Scale(f32::MAX / 16.0, 1.0),
                ],
                None,
                &mut |child| child.text(&styled, [0.0, 0.0], u32::MAX, false),
            )
        });
        assert_eq!(res, Err(GuiDrawError::NotFinite));
        assert!(batch.vertices.is_empty());
    }
}
