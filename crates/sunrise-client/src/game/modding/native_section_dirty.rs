//! `ClientRenderClientHost::set_section_dirty` and `set_blocks_dirty`: a client mod asks the
//! mesher to rebuild loaded sections (`LevelExtractor.setSectionDirty`,
//! `ClientLevel.setSectionDirtyWithNeighbors`, `ClientLevel.setBlocksDirty`).
use super::Game;

impl Game<'_> {
    /// Queues section `(cx, section_y, cz)` (vanilla section coordinates) for remeshing when its
    /// column is loaded and the level holds that height. Whether it was queued.
    fn mark_section_dirty(&mut self, cx: i32, section_y: i32, cz: i32) -> bool {
        let layout = self.world.layout();
        let sy = section_y - (layout.min_y() >> 4);
        if sy < 0 || sy >= layout.sections() as i32 || !self.world.chunks.contains_key(&(cx, cz)) {
            return false;
        }
        self.world.dirty_sections.insert((cx, sy, cz));
        true
    }
    /// `LevelExtractor.setSectionDirty`, with `with_neighbors` as
    /// `ClientLevel.setSectionDirtyWithNeighbors`: false, marking nothing, for a section that
    /// is not loaded.
    pub(in crate::game) fn set_section_dirty(
        &mut self,
        section: [i32; 3],
        with_neighbors: bool,
    ) -> bool {
        let [x, y, z] = section;
        if !self.mark_section_dirty(x, y, z) {
            return false;
        }
        if with_neighbors {
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        self.mark_section_dirty(x + dx, y + dy, z + dz);
                    }
                }
            }
        }
        true
    }
    /// `ClientLevel.setBlocksDirty`: every loaded section holding a block of the box.
    pub(in crate::game) fn set_blocks_dirty(&mut self, min: [i32; 3], max: [i32; 3]) {
        let layout = self.world.layout();
        let lo = layout.min_y() >> 4;
        let hi = lo + layout.sections() as i32 - 1;
        let (x0, x1) = (min[0].min(max[0]) >> 4, min[0].max(max[0]) >> 4);
        let (y0, y1) = (
            (min[1].min(max[1]) >> 4).max(lo),
            (min[1].max(max[1]) >> 4).min(hi),
        );
        let (z0, z1) = (min[2].min(max[2]) >> 4, min[2].max(max[2]) >> 4);
        // The loaded columns, so a box of any size costs what is loaded.
        let columns: Vec<_> = self
            .world
            .chunks
            .keys()
            .copied()
            .filter(|&(cx, cz)| (x0..=x1).contains(&cx) && (z0..=z1).contains(&cz))
            .collect();
        for (cx, cz) in columns {
            for y in y0..=y1 {
                self.mark_section_dirty(cx, y, cz);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use mod_api::prelude::ClientRenderClientHost;

    /// Only loaded sections inside the level's height are queued, one section or a box of them,
    /// with or without the neighbours.
    #[test]
    #[ignore = "daily"]
    fn host_marks_only_loaded_sections_dirty() {
        use super::super::native_entity_refs::tests::with_client_game;

        with_client_game(|game| {
            game.world.chunks.clear();
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(sunrise_client::chunk::Chunk::empty(0, 0)),
            );
            let min_section = game.world.bottom_y() >> 4;
            let sections = game.world.layout().sections() as i32;
            // The sections of column (0, 0) queued, as vanilla section Ys.
            let marked = |game: &mut super::Game<'_>| {
                assert!(!game.world.dirty_sections.contains(&(1, 0, 0)));
                let marked: Vec<i32> = (0..sections)
                    .filter(|&sy| game.world.dirty_sections.contains(&(0, sy, 0)))
                    .map(|sy| sy + min_section)
                    .collect();
                game.world.dirty_sections.clear();
                marked
            };
            game.world.dirty_sections.clear();

            assert!(!ClientRenderClientHost::set_section_dirty(
                game,
                [1, 4, 0],
                false
            ));
            assert!(!ClientRenderClientHost::set_section_dirty(
                game,
                [0, min_section - 1, 0],
                true
            ));
            assert!(game.world.dirty_sections.is_empty());

            assert!(ClientRenderClientHost::set_section_dirty(
                game,
                [0, 4, 0],
                false
            ));
            assert_eq!(marked(game), vec![4]);

            assert!(ClientRenderClientHost::set_section_dirty(
                game,
                [0, 4, 0],
                true
            ));
            assert_eq!(marked(game), vec![3, 4, 5]);

            ClientRenderClientHost::set_blocks_dirty(game, [15, 80, 9], [-20, 60, 3]);
            assert_eq!(marked(game), vec![3, 4, 5]);

            ClientRenderClientHost::set_blocks_dirty(
                game,
                [-1_000_000, i32::MIN / 2, -1_000_000],
                [1_000_000, i32::MAX / 2, 1_000_000],
            );
            assert_eq!(marked(game).len(), sections as usize);
        });
    }
}
