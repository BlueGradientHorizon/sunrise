//! The GUI atlas's animated sprites, frame by frame (vanilla's or a Polytone `mode`'s).

impl super::HudRenderer {
    /// Shows each animated sprite's frame: its mcmeta's, or what its Polytone `mode` picks.
    pub(crate) fn animate(
        &mut self,
        uploads: &mut crate::gfx::upload::Uploads,
        encoder: &mut wgpu::CommandEncoder,
        tick: u64,
        animator: &crate::gfx::animate::SpriteAnimator,
    ) {
        let inputs = animator.mode_inputs();
        let inputs = Some(&inputs);
        let Some(texture) = &self.atlas_texture else {
            return;
        };
        for (rect, animation, previous) in &mut self.sprite_animations {
            // `SpriteContents.AnimationState`: the frame from the mcmeta's
            // list, blended into the next one by the sub-frame's thousandths
            // when the sprite interpolates; or what a Polytone `mode` picks.
            let blend = animation.blend_with(tick, inputs);
            let state = (blend.frame, blend.next, blend.millis);
            if state == *previous {
                continue;
            }
            let (w, h) = rect.texels((texture.width(), texture.height()));
            let pixels = if blend.millis == 0 {
                animation.resized_pixels(blend.frame, w, h)
            } else {
                let current = animation.resized_pixels(blend.frame, w, h);
                let next = animation.resized_pixels(blend.next, w, h);
                let millis = blend.millis;
                current
                    .iter()
                    .zip(next)
                    .map(|(&a, b)| {
                        ((u32::from(a) * (1000 - millis) + u32::from(b) * millis + 500) / 1000)
                            as u8
                    })
                    .collect()
            };
            let x = (rect.u0 * texture.width() as f32).round() as u32;
            let y = (rect.v0 * texture.height() as f32).round() as u32;
            let has_border =
                x >= 1 && y >= 1 && x + w < texture.width() && y + h < texture.height();
            let (origin_x, origin_y, up_w, up_h, padded);
            let upload_bytes = if has_border {
                origin_x = x - 1;
                origin_y = y - 1;
                up_w = w + 2;
                up_h = h + 2;
                padded = super::clamp_border_pixels(&pixels, w, h);
                padded.as_slice()
            } else {
                origin_x = x;
                origin_y = y;
                up_w = w;
                up_h = h;
                pixels.as_slice()
            };
            uploads.texture(
                encoder,
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: origin_x,
                        y: origin_y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                up_w,
                up_h,
                upload_bytes,
            );
            *previous = state;
        }
    }
}
