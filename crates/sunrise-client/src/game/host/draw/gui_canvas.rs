//! Direct mod GUI drawing on the hook's HUD batch.

use crate::game::api_values::TryIntoEngine;
use mod_api::{GuiBlitRegion, GuiCanvas, GuiDrawError, GuiSpriteRegion, Text};
use sunrise_client::{
    hud::{GuiSprite, HudAtlas, HudBatch},
    hud_layout::{
        Blend, IDENTITY, Pose, PoseOp, ResolvedGuiQuad, abgr, paint_fill_gradient, pose_apply,
        pose_point,
    },
};

pub(super) struct GameGuiCanvas<'a> {
    batch: &'a mut HudBatch,
    atlas: &'a HudAtlas,
    pose: Pose,
    /// Screen-space edges, not affected by future changes of pose.
    scissor: Option<[f32; 4]>,
}

impl GameGuiCanvas<'_> {
    pub(super) fn draw(
        batch: &mut HudBatch,
        atlas: &HudAtlas,
        draw: &mut dyn FnMut(&mut dyn GuiCanvas) -> Result<(), GuiDrawError>,
    ) -> Result<(), GuiDrawError> {
        Self::group(batch, atlas, IDENTITY, None, draw)
    }

    fn group(
        batch: &mut HudBatch,
        atlas: &HudAtlas,
        pose: Pose,
        scissor: Option<[f32; 4]>,
        draw: &mut dyn FnMut(&mut dyn GuiCanvas) -> Result<(), GuiDrawError>,
    ) -> Result<(), GuiDrawError> {
        batch.draw_scope(&atlas.font, |batch| {
            draw(&mut GameGuiCanvas {
                batch,
                atlas,
                pose,
                scissor,
            })
        })
    }

    /// An individual call can be refused even when the caller chooses to commit its group.
    fn paint(&mut self, emit: impl FnOnce(&mut HudBatch, &HudAtlas)) -> Result<(), GuiDrawError> {
        let (atlas, pose, scissor) = (self.atlas, self.pose, self.scissor);
        self.batch.draw_scope(&atlas.font, |batch| {
            let start = batch.vertices.len();
            emit(batch, atlas);
            let mut needs_clip =
                scissor.is_some_and(|[left, top, right, bottom]| left == right || top == bottom);
            for vertex in &mut batch.vertices[start..] {
                if pose != IDENTITY {
                    vertex.pos = pose_point(pose, vertex.pos);
                }
                let [x, y] = vertex.pos;
                if !vertex.pos.into_iter().chain(vertex.uv).all(f32::is_finite) {
                    return Err(GuiDrawError::NotFinite);
                }
                if let Some([left, top, right, bottom]) = scissor {
                    needs_clip |= x < left || y < top || x > right || y > bottom;
                }
            }
            if let Some([left, top, right, bottom]) = scissor
                && needs_clip
                && start < batch.vertices.len()
            {
                batch.clip_since(start, left, top, right, bottom);
                if !batch.vertices[start..]
                    .iter()
                    .all(|v| v.pos.into_iter().chain(v.uv).all(f32::is_finite))
                {
                    return Err(GuiDrawError::NotFinite);
                }
            }
            Ok(())
        })
    }
}

/// The opposite corner of a finite `[x, y, w, h]`, including finite endpoint arithmetic.
fn rect_end([x, y, w, h]: [f32; 4]) -> Result<[f32; 2], GuiDrawError> {
    let end = [x + w, y + h];
    if [x, y].into_iter().chain(end).all(f32::is_finite) {
        Ok(end)
    } else {
        Err(GuiDrawError::NotFinite)
    }
}

impl GuiCanvas for GameGuiCanvas<'_> {
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
        Self::group(self.batch, self.atlas, pose, clip, draw)
    }

    fn fill(&mut self, rect @ [x, y, ..]: [f32; 4], argb: u32) -> Result<(), GuiDrawError> {
        let [x1, y1] = rect_end(rect)?;
        let (left, right) = (x.min(x1), x.max(x1));
        let (top, bottom) = (y.min(y1), y.max(y1));
        self.paint(|batch, atlas| {
            batch.fill_quad(
                atlas,
                [[left, top], [right, top], [right, bottom], [left, bottom]],
                abgr(argb),
            );
        })
    }

    fn fill_gradient(&mut self, rect: [f32; 4], top: u32, bottom: u32) -> Result<(), GuiDrawError> {
        rect_end(rect)?;
        self.paint(|batch, atlas| paint_fill_gradient(batch, atlas, rect, top, bottom))
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
        self.paint(|batch, atlas| {
            batch.text_styled(atlas, &text, x, y, abgr(argb), shadow);
        })
    }

    fn blit_sprite(
        &mut self,
        name: &str,
        dst: [f32; 4],
        region: Option<GuiSpriteRegion>,
        argb: u32,
        blend: Blend,
    ) -> Result<(), GuiDrawError> {
        rect_end(dst)?;
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
        self.paint(|batch, atlas| quad.paint_sprite(batch, atlas, name, region, blend))
    }

    fn atlas_sprite(&mut self, name: &str, dst: [f32; 4], argb: u32) -> Result<(), GuiDrawError> {
        rect_end(dst)?;
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
        self.paint(|batch, _| quad.paint(batch))
    }

    fn blit(
        &mut self,
        texture: &str,
        dst: [f32; 4],
        GuiBlitRegion { uv, src, tex }: GuiBlitRegion,
        argb: u32,
        blend: Blend,
    ) -> Result<(), GuiDrawError> {
        rect_end(dst)?;
        if !uv.into_iter().all(f32::is_finite) {
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
        self.paint(|batch, _| quad.paint_blit(batch, uv, src, tex, blend))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hud::{Blend as HudBlend, Rect};
    use crate::resource_source::ResourceSource;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[test]
    fn gui_groups_clip_posed_leaves_and_restore_vertices_and_blends_on_abort() {
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
        GameGuiCanvas::draw(&mut batch, &atlas, &mut |root| {
            crosshair(root, [2.0, 3.0, 2.0, 2.0])
        })
        .unwrap();

        GameGuiCanvas::draw(&mut batch, &atlas, &mut |root| {
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

                    // Both discarded scopes append with the existing blend, extending its run.
                    assert_eq!(
                        parent.scope(&[], None, &mut |child| {
                            crosshair(child, [0.0, 0.0, 1.0, 1.0])?;
                            Err(GuiDrawError::Abort)
                        }),
                        Err(GuiDrawError::Abort)
                    );
                    let unwind = catch_unwind(AssertUnwindSafe(|| {
                        parent.scope(&[PoseOp::Translate(1.0, 0.0)], None, &mut |child| {
                            crosshair(child, [0.0, 0.0, 1.0, 1.0])?;
                            panic!("discard on unwind")
                        })
                    }));
                    assert!(unwind.is_err());

                    // Catch a leaf refusal instead of discarding its entire scope.
                    parent.scope(&[PoseOp::Scale(2.0, 1.0)], None, &mut |child| {
                        assert_eq!(
                            crosshair(child, [f32::MAX, 0.0, 1.0, 1.0]),
                            Err(GuiDrawError::NotFinite)
                        );
                        Ok(())
                    })?;
                    parent.fill_gradient([2.0, 2.0, 1.0, 1.0], 0x80AA_1234, 0xFF00_0000)
                },
            )?;
            root.fill([30.0, 40.0, 2.0, 2.0], 0xFFFF_FFFF)
        })
        .unwrap();
        assert_eq!(batch.vertices.len(), 24);
        assert_eq!(batch.vertices[0].pos, [2.0, 3.0]);
        assert_eq!(batch.vertices[6].pos, [12.0, 22.0]);
        assert_eq!(batch.vertices[8].pos, [15.0, 25.0]);
        assert_eq!(batch.vertices[6].uv, [0.265625, 0.15625]);
        assert_eq!(batch.vertices[6].color, 0x8033_2211);
        assert_eq!(batch.vertices[12].pos, [12.75, 22.25]);
        assert_eq!(batch.vertices[12].color, 0x8034_12AA);
        assert_eq!(batch.vertices[14].color, 0xFF00_0000);
        assert_eq!(batch.vertices[18].pos, [30.0, 40.0]);
        assert_eq!(batch.blends, [(0..12, HudBlend::Invert)]);

        let before = bytemuck::cast_slice::<_, u8>(&batch.vertices).to_vec();
        assert_eq!(
            GameGuiCanvas::draw(&mut batch, &atlas, &mut |root| {
                root.scope(&[], None, &mut |child| {
                    crosshair(child, [4.0, 7.0, 1.0, 1.0])
                })?;
                Err(GuiDrawError::Abort)
            }),
            Err(GuiDrawError::Abort)
        );
        assert_eq!(
            bytemuck::cast_slice::<_, u8>(&batch.vertices),
            before.as_slice()
        );
        assert_eq!(batch.blends, [(0..12, HudBlend::Invert)]);
    }

    #[test]
    fn gui_scissors_are_empty_on_reflection_inversion_and_disjoint_intersection() {
        let atlas = HudAtlas::build(&ResourceSource::directory(
            "/nonexistent/sunrise-test-assets".into(),
        ))
        .unwrap();
        let mut batch = HudBatch::scratch([320, 240]);
        GameGuiCanvas::draw(&mut batch, &atlas, &mut |root| {
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
}
