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

impl GameGuiCanvas<'_, '_> {
    pub(crate) fn draw(
        batch: &mut HudBatch,
        atlas: &HudAtlas,
        game: Option<&Game<'_>>,
        draw: &mut dyn FnMut(&mut dyn GuiCanvas) -> Result<(), GuiDrawError>,
    ) -> Result<(), GuiDrawError> {
        draw(&mut GameGuiCanvas {
            batch,
            atlas,
            game,
            pose: IDENTITY,
            scissor: None,
        })
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

    fn paint_emit(&mut self, emit: impl FnOnce(&mut HudBatch, &HudAtlas)) {
        let start = self.batch.vertices.len();
        emit(self.batch, self.atlas);
        if self.pose == IDENTITY && self.scissor.is_none() {
            return;
        }
        let mut needs_clip = self
            .scissor
            .is_some_and(|[left, top, right, bottom]| left == right || top == bottom);
        for vertex in &mut self.batch.vertices[start..] {
            if self.pose != IDENTITY {
                vertex.pos = pose_point(self.pose, vertex.pos);
            }
            if let Some([left, top, right, bottom]) = self.scissor {
                let [x, y] = vertex.pos;
                needs_clip |= x < left || y < top || x > right || y > bottom;
            }
        }
        if let Some([left, top, right, bottom]) = self.scissor
            && needs_clip
            && start < self.batch.vertices.len()
        {
            self.batch.clip_since(start, left, top, right, bottom);
        }
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
        let clip = if let Some(rect @ [x, y, w, h]) = scissor {
            let end = rect_end(rect)?;
            let [x, y] = pose_point(pose, [x, y]);
            let [right, bottom] = pose_point(pose, end);
            // ScreenRectangle transforms the two ordered corners, not an enclosing box;
            // it floors the origin and extents separately.
            let [left, top] = [x.floor(), y.floor()];
            let [width, height] = [(right - x).floor(), (bottom - y).floor()];
            let mut clip = [left, top, left + width, top + height];
            if !clip.into_iter().all(f32::is_finite) {
                return Err(GuiDrawError::NotFinite);
            }
            if let Some([x0, y0, x1, y1]) = self.scissor {
                clip = [
                    clip[0].max(x0),
                    clip[1].max(y0),
                    clip[2].min(x1),
                    clip[3].min(y1),
                ];
            }
            if w <= 0.0 || h <= 0.0 || clip[2] <= clip[0] || clip[3] <= clip[1] {
                clip = [0.0; 4];
            }
            Some(clip)
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
        });
        Ok(())
    }

    fn fill_gradient(&mut self, rect: [f32; 4], top: u32, bottom: u32) -> Result<(), GuiDrawError> {
        self.check_rect_geometry(rect)?;
        self.paint_emit(|batch, atlas| paint_fill_gradient(batch, atlas, rect, top, bottom));
        Ok(())
    }

    fn text(
        &mut self,
        text: &Text,
        at @ [x, y]: [f32; 2],
        argb: u32,
        shadow: bool,
    ) -> Result<(), GuiDrawError> {
        if !at.into_iter().all(f32::is_finite) {
            return Err(GuiDrawError::NotFinite);
        }
        let text = text.try_into_engine(&()).map_err(GuiDrawError::Text)?;
        let pad = if shadow { 1.0 } else { 0.0 };
        self.check_rect_geometry([x, y, self.atlas.font.width_text(&text) + pad, 9.0 + pad])?;
        self.paint_emit(|batch, atlas| {
            batch.text_styled(atlas, &text, x, y, abgr(argb), shadow);
        });
        Ok(())
    }

    fn item(&mut self, stack: &ItemStack, [x, y]: [f32; 2], seed: i32) -> Result<(), GuiDrawError> {
        let Some(stack) = drawable_stack(stack)? else {
            return Ok(());
        };
        self.check_rect_geometry([x, y, 16.0, 16.0])?;
        let game = self.game.ok_or(GuiDrawError::Unavailable)?;
        self.paint_emit(|batch, _| {
            game.draw_item_model(batch, &stack, x, y, seed);
        });
        Ok(())
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
        self.check_rect_geometry([x, y, 16.0, 16.0])?;
        if let Some(label) = count {
            let w = self.atlas.font.width(label);
            self.check_rect_geometry([x + 17.0 - w, y + 9.0, w + 1.0, 10.0])?;
        }
        let game = self.game.ok_or(GuiDrawError::Unavailable)?;
        self.paint_emit(|batch, _| {
            game.draw_vanilla_item_decorations(batch, &stack, x, y, count.map(str::to_owned));
        });
        // Custom decorators draw directly into this canvas, inheriting its pose and scissor.
        game.native_item_decorations(&stack, x, y, self);
        Ok(())
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
        let Some(GuiSprite::Present { rect, .. }) = self.atlas.gui_lookup(name) else {
            return Err(GuiDrawError::UnknownSprite);
        };
        let quad = ResolvedGuiQuad {
            rect,
            dst,
            color: argb,
        };
        self.paint_emit(|batch, atlas| quad.paint_sprite(batch, atlas, name, region, blend));
        Ok(())
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
        self.paint_emit(|batch, _| quad.paint(batch));
        Ok(())
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
        if src.into_iter().chain(tex).any(|extent| extent <= 0) {
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
        self.paint_emit(|batch, _| quad.paint_blit(batch, uv, src, tex, blend));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hud::{Blend as HudBlend, Rect};
    use crate::resource_source::ResourceSource;
    use std::panic::{AssertUnwindSafe, catch_unwind};

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
                    src: [4, -1],
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
}
