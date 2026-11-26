#[allow(unused_macros)]
macro_rules! server_world_items {
    ($s:ident) => {
        match (&$s.server_items, $s.side.world_tables.0.as_ref()) {
            (Some(items), _) => &**items,
            (None, Some(t)) => &t.items,
            (None, None) => $s.res.items(),
        }
    };
}

#[allow(unused_macros)]
macro_rules! server_world_recipes {
    ($s:ident) => {
        match $s.side.world_tables.0.as_ref() {
            Some(t) => &t.recipes,
            None => $s.res.recipes(),
        }
    };
}

#[allow(unused_macros)]
macro_rules! server_world_stations {
    ($s:ident) => {
        match $s.side.world_tables.0.as_ref() {
            Some(t) => &t.stations,
            None => $s.res.stations(),
        }
    };
}

/// Server-side `game_side!` using direct field borrows on `self.side` so sibling mutable borrows
/// of unrelated `ServerState` fields (e.g. `pending_sounds`, `ordered.lava_random`) stay disjoint.
#[allow(unused_macros)]
macro_rules! server_game_side {
    ($s:expr) => {{
        let level = $s.native_lifecycle.epoch().and_then(|epoch| {
            let key = $s.dim.key();
            let id = mod_api::__private::dimension_id(key)?;
            Some(mod_api::LevelRef::with_generation(
                epoch,
                key,
                $s.side.levels.0.runtime.generation_of(id),
            ))
        });
        let player_uuid = $s.side.lan_acting_uuid;
        let session_lock = $s.side.session_lock.0.is_some();
        $crate::game_side!(
            $s,
            (player_uuid, level, session_lock),
            $s.level.enabled_features.as_arc(),
            session_lock.then(|| $s.res.items())
        )
    }};
}
