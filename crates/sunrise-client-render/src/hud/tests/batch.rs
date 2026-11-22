use super::super::*;

#[test]
fn clip_since_keeps_the_visible_part_of_a_straddling_triangle() {
    let mut batch = HudBatch::scratch([320, 240]);
    let start = batch.vertices.len();
    // A right triangle (a quad `a, b, c, c`) whose centre lies outside the rect.
    batch.quad(
        [[0.0, 0.0], [16.0, 0.0], [0.0, 16.0], [0.0, 16.0]],
        [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [0.0, 1.0]],
        u32::MAX,
    );
    batch.clip_since(start, 0.0, 0.0, 16.0, 4.0);
    assert!(!batch.vertices.is_empty());
    assert!(batch.vertices.iter().all(|v| v.pos[1] <= 4.0));
    let area: f32 = batch
        .vertices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|t| {
            ((t[1].pos[0] - t[0].pos[0]) * (t[2].pos[1] - t[0].pos[1])
                - (t[2].pos[0] - t[0].pos[0]) * (t[1].pos[1] - t[0].pos[1]))
                .abs()
                / 2.0
        })
        .sum();
    // Rows 0..4 of the triangle: 16 * 16 / 2 - 12 * 12 / 2.
    assert!((area - 56.0).abs() < 1e-3, "{area}");
}

#[test]
fn clip_since_moves_blend_ranges_with_their_vertices() {
    let mut batch = HudBatch::scratch([320, 240]);
    let start = batch.vertices.len();
    let tri = |batch: &mut HudBatch, y: f32| {
        batch.quad(
            [[0.0, y], [8.0, y], [0.0, y + 8.0], [0.0, y + 8.0]],
            [[0.0; 2]; 4],
            u32::MAX,
        );
    };
    tri(&mut batch, 100.0); // outside, dropped
    let foil = batch.vertices.len();
    tri(&mut batch, 0.0); // straddles the bottom edge
    batch.blend_since(foil, Blend::Additive);
    batch.clip_since(start, 0.0, 0.0, 16.0, 4.0);
    assert_eq!(
        batch.blends,
        [(0..batch.vertices.len() as u32, Blend::Additive)]
    );
}

#[test]
fn clip_since_keeps_the_depth_of_an_item_or_scene_face() {
    let mut batch = HudBatch::scratch([320, 240]);
    let start = batch.vertices.len();
    // Two axis-aligned model faces at different depths, as an item model or a
    // `draw_scene` picture leaves them after `depth_since`.
    let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    batch.quad_3d(
        [
            [0.0, 0.0, 1.0],
            [8.0, 0.0, 1.0],
            [8.0, 8.0, 1.0],
            [0.0, 8.0, 1.0],
        ],
        uv,
        u32::MAX,
    );
    batch.quad_3d(
        [
            [2.0, 2.0, 3.0],
            [6.0, 2.0, 3.0],
            [6.0, 6.0, 5.0],
            [2.0, 6.0, 5.0],
        ],
        uv,
        u32::MAX,
    );
    batch.depth_since(start, [0.0, 0.0, 320.0, 240.0]);
    let before = batch.vertices.clone();
    assert!(before.iter().any(|v| v.depth > 0.0));
    // A clip containing both faces leaves every attribute as it was.
    batch.clip_since(start, 0.0, 0.0, 16.0, 16.0);
    assert_eq!(batch.vertices.len(), before.len());
    for (v, w) in batch.vertices.iter().zip(&before) {
        assert_eq!((v.pos, v.color, v.text), (w.pos, w.color, w.text));
        let near = |a: f32, b: f32| (a - b).abs() < 1e-6;
        assert!(near(v.depth, w.depth), "depth {} vs {}", v.depth, w.depth);
        assert!(near(v.uv[0], w.uv[0]) && near(v.uv[1], w.uv[1]));
    }
    // A clip cutting the sloped face keeps its depth across the cut.
    batch.clip_since(start, 0.0, 0.0, 16.0, 4.0);
    let sloped: Vec<_> = batch.vertices[6..]
        .iter()
        .map(|v| (v.pos[1], v.depth))
        .collect();
    let at = |y: f32| before[6..].iter().find(|v| v.pos[1] == y).unwrap().depth;
    for (y, depth) in sloped {
        let want = at(2.0) + (at(6.0) - at(2.0)) * (y - 2.0) / 4.0;
        assert!((depth - want).abs() < 1e-6, "y {y}: {depth} vs {want}");
    }
}

#[test]
fn clip_since_remaps_only_the_scoped_part_of_a_coalesced_blend() {
    let mut batch = HudBatch::scratch([320, 240]);
    let quad = |batch: &mut HudBatch, x: f32, y: f32| {
        batch.quad(
            [[x, y], [x + 4.0, y], [x + 4.0, y + 4.0], [x, y + 4.0]],
            [[0.0; 2]; 4],
            u32::MAX,
        );
    };
    quad(&mut batch, 32.0, 32.0);
    batch.invert_since(0);
    let start = batch.vertices.len();
    quad(&mut batch, -8.0, 0.0); // outside
    quad(&mut batch, 0.0, 0.0); // cut at x = 2
    batch.invert_since(start);
    assert_eq!(batch.blends, [(0..18, Blend::Invert)]);

    batch.clip_since(start, 2.0, 0.0, 8.0, 8.0);
    let positions: Vec<_> = batch.vertices.iter().map(|v| v.pos).collect();
    assert_eq!(
        positions,
        [
            [32.0, 32.0],
            [36.0, 32.0],
            [36.0, 36.0],
            [32.0, 32.0],
            [36.0, 36.0],
            [32.0, 36.0],
            [2.0, 0.0],
            [4.0, 0.0],
            [4.0, 4.0],
            [2.0, 0.0],
            [4.0, 4.0],
            [2.0, 4.0],
        ]
    );
    assert_eq!(batch.blends, [(0..12, Blend::Invert)]);

    let start = batch.vertices.len();
    quad(&mut batch, 0.0, 0.0);
    batch.invert_since(start);
    batch.clip_since(start, 2.0, 0.0, 2.0, 8.0); // empty scissor
    quad(&mut batch, 16.0, 16.0);
    assert_eq!(
        batch.runs(),
        [(0..12, Blend::Invert), (12..18, Blend::Alpha)]
    );
    assert_eq!(batch.vertices[12].pos, [16.0, 16.0]);
}

#[test]
fn draw_scope_rollback_restores_the_prior_batch_tail() {
    let font = crate::font::Font::metrics(&ResourceSource::indexed(Default::default()));
    let mut batch = HudBatch::scratch([320, 240]);
    let corners = [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]];
    let portal = EndPortalFill {
        game_time: 0.5,
        ..Default::default()
    };
    batch.quad(corners, [[0.0; 2]; 4], u32::MAX);
    batch.invert_since(0);
    batch.end_portal = portal;
    batch.glint_alpha = 0.5;
    batch.blur_at = 6;
    let profile = Arc::new(serde_json::json!({"name":"baseline"}));
    batch.inline_profiles.push(profile.clone());
    assert_eq!(
        batch.draw_scope(&font, |batch| {
            batch.quad(corners, [[0.0; 2]; 4], u32::MAX);
            batch.invert_since(6);
            batch.quad(corners, [[0.0; 2]; 4], u32::MAX);
            batch.blend_since(12, Blend::Additive);
            batch.end_portal = EndPortalFill::default();
            batch.glint_alpha = 0.25;
            batch.blur_at = 18;
            batch.pictures.push(Arc::new(pictures::Picture {
                id: 1,
                width: 1,
                height: 1,
                pixels: vec![255; 4],
                flags: TextureFlags::default(),
            }));
            batch.patches.push(crate::font::Patch {
                x: 4,
                y: 0,
                width: 1,
                height: 1,
                rgba: vec![255; 4],
            });
            batch.image_patches.push(dynamic_textures::ImagePatch {
                origin: [0, 4],
                size: [1, 1],
                rgba: Arc::new([255; 4]),
            });
            batch
                .inline_profiles
                .push(Arc::new(serde_json::json!({"name":"scoped"})));
            Err::<(), _>("abort")
        }),
        Err("abort")
    );
    batch.quad(corners.map(|[x, y]| [x + 8.0, y]), [[0.0; 2]; 4], u32::MAX);
    assert_eq!(batch.runs(), [(0..6, Blend::Invert), (6..12, Blend::Alpha)]);
    assert_eq!(
        batch.vertices[..6]
            .iter()
            .map(|v| v.pos)
            .collect::<Vec<_>>(),
        [
            [0.0, 0.0],
            [4.0, 0.0],
            [4.0, 4.0],
            [0.0, 0.0],
            [4.0, 4.0],
            [0.0, 4.0]
        ]
    );
    assert_eq!(batch.vertices[6].pos, [8.0, 0.0]);
    assert_eq!(
        (batch.end_portal, batch.glint_alpha, batch.blur_at),
        (portal, 0.5, 6)
    );
    assert!(batch.pictures.is_empty());
    assert!(batch.patches.is_empty());
    assert!(batch.image_patches.is_empty());
    assert_eq!(batch.inline_profiles, [profile]);
}
