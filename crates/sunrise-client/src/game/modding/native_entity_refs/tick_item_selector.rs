impl Game<'_> {
    fn level_entity_candidates(&self, level: LevelRef) -> impl Iterator<Item = EntityRef> + '_ {
        let mut after = None;
        std::iter::from_fn(move || {
            // Copy the key and release the residency borrow before resolution or a visitor.
            let key = self
                .resident_entities
                .table
                .borrow_mut()
                .level_entity_candidate_after(level.dimension(), after)?;
            after = Some(key);
            Some(key.1.get())
        })
        .filter(move |id| {
            !self
                .persist
                .removals
                .iter()
                .chain(self.persist.destroying_vehicles.iter())
                .any(|target| matches!(*target, Target::Boat(n) | Target::Cart(n) if n == *id))
        })
        .map(move |id| EntityRef::new(level.epoch(), id))
    }
}
