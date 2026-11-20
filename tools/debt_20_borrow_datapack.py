#!/usr/bin/env python3
"""Migrate level datapack reads from owned field vectors to scoped visitors."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EDITS = {
    "crates/sunrise-mod-vocab/src/host/level_datapacks.rs": [
        (
            """    /// The decoded fields of the entry a handle names, in the registry's schema order, as plain
    /// values: a copy, which carries no authority after the read. The handle is checked first
    /// (session, then generation), then the entry; the errors are those of
    /// [`level_datapack_handle`](Self::level_datapack_handle).
    #[access(WorldOnly)]
    fn level_datapack_read(
        &self,
        level: LevelRef,
        handle: &DatapackHandle,
    ) -> Result<Vec<(String, DatapackValue)>, DatapackReadError>;""",
            """    /// Borrow the published entry a handle names. `visit` runs once on success, without
    /// holding a registry lock, and never on refusal. `entry.values` is in schema order;
    /// retain a key or copy any values needed after `visit` returns.
    /// The handle is checked first (session, then generation), then the entry; registry and
    /// entry failures are the same as for [`LevelRef::datapack_handle`].
    #[access(WorldOnly)]
    fn level_datapack_read(
        &self,
        level: LevelRef,
        handle: &DatapackHandle,
        visit: &mut dyn FnMut(&DatapackEntry),
    ) -> Result<(), DatapackReadError>;""",
        ),
    ],
    "crates/sunrise-client/src/game/modding/native_datapack_refs.rs": [
        (
            """    /// The fields of the entry a handle names, after the handle's session and generation check.
    pub(super) fn level_datapack_read(
        &self,
        level: LevelRef,
        handle: &mod_api::DatapackHandle,
    ) -> Result<Vec<(String, mod_api::DatapackValue)>, mod_api::DatapackReadError> {
        let set = self
            .level_datapack_set(level)
            .ok_or(mod_api::DatapackReadError::UnknownRegistry)?;
        Ok(set.get(handle)?.values.clone())
    }""",
            """    /// Visit the entry a handle names, after the handle's session and generation check.
    pub(super) fn level_datapack_read(
        &self,
        level: LevelRef,
        handle: &mod_api::DatapackHandle,
        visit: &mut dyn FnMut(&mod_api::DatapackEntry),
    ) -> Result<(), mod_api::DatapackReadError> {
        let set = self
            .level_datapack_set(level)
            .ok_or(mod_api::DatapackReadError::UnknownRegistry)?;
        visit(set.get(handle)?);
        Ok(())
    }""",
        ),
        (
            """                    assert_eq!(
                        game.level_datapack_read(level, &handle),
                        Ok(vec![("power".into(), DatapackValue::Int(7))])
                    );""",
            """                    let mut visits = 0;
                    game.level_datapack_read(level, &handle, &mut |entry| {
                        assert_eq!(entry.values, [("power".to_owned(), DatapackValue::Int(7))]);
                        visits += 1;
                    })
                    .unwrap();
                    assert_eq!(visits, 1);""",
        ),
        (
            """                    assert_eq!(
                        game.level_datapack_read(level, &handle),
                        Err(E::StaleGeneration)
                    );""",
            """                    let mut visited_refusal = false;
                    let mut unexpected_visit = |_: &DatapackEntry| visited_refusal = true;
                    assert_eq!(
                        game.level_datapack_read(level, &handle, &mut unexpected_visit),
                        Err(E::StaleGeneration)
                    );""",
        ),
        (
            """                        game.level_datapack_read(level, &handle),
                        Err(E::WrongSession)
                    );""",
            """                        game.level_datapack_read(level, &handle, &mut unexpected_visit),
                        Err(E::WrongSession)
                    );
                    assert!(!visited_refusal);""",
        ),
    ],
    "crates/sunrise-client/src/game/world/server_reload/datapack_host_read_tests.rs": [
        (
            """    DatapackField, DatapackFieldKind, DatapackReadError as E, DatapackRegistrySync, DimensionKey,
    LevelRef, Registrar, RegistrationContext, WorldEpoch,""",
            """    DatapackEntry, DatapackField, DatapackFieldKind, DatapackReadError as E, DatapackRegistrySync,
    DimensionKey, LevelRef, Registrar, RegistrationContext, WorldEpoch,""",
        ),
        (
            """                assert_eq!(
                    level.datapack_read(game, &handle),
                    Ok(vec![("power".into(), DatapackValue::Int(7))])
                );""",
            """                let mut visits = 0;
                level
                    .datapack_read(game, &handle, &mut |entry| {
                        assert_eq!(entry.values, [("power".to_owned(), DatapackValue::Int(7))]);
                        visits += 1;
                    })
                    .unwrap();
                assert_eq!(visits, 1);""",
        ),
        (
            "                for refused in [",
            """                let mut visited_refusal = false;
                let mut unexpected_visit = |_: &DatapackEntry| visited_refusal = true;
                for refused in [""",
        ),
        (
            "refused.datapack_read(game, &handle),",
            "refused.datapack_read(game, &handle, &mut unexpected_visit),",
        ),
        (
            "                assert_eq!(level.datapack_read(game, &handle), Err(E::StaleGeneration));",
            """                assert_eq!(
                    level.datapack_read(game, &handle, &mut unexpected_visit),
                    Err(E::StaleGeneration)
                );""",
        ),
        (
            "                assert_eq!(level.datapack_read(game, &handle), Err(E::WrongSession));",
            """                assert_eq!(
                    level.datapack_read(game, &handle, &mut unexpected_visit),
                    Err(E::WrongSession)
                );""",
        ),
        (
            """                assert_eq!(
                    level.datapack_read(game, &client_handle),
                    Ok(vec![("power".into(), DatapackValue::Int(19))])
                );""",
            """                let mut visits = 0;
                level
                    .datapack_read(game, &client_handle, &mut |entry| {
                        assert_eq!(entry.values, [("power".to_owned(), DatapackValue::Int(19))]);
                        visits += 1;
                    })
                    .unwrap();
                assert_eq!(visits, 1);""",
        ),
        (
            """                    level.datapack_read(game, &client_handle),
                    Err(E::UnknownRegistry)
                );""",
            """                    level.datapack_read(game, &client_handle, &mut unexpected_visit),
                    Err(E::UnknownRegistry)
                );
                assert!(!visited_refusal);""",
        ),
    ],
}

if __name__ == "__main__":
    updates = {}
    for relative_path, edits in EDITS.items():
        path = ROOT / relative_path
        text = path.read_text(encoding="utf-8")
        for old, new in edits:
            matches = text.count(old)
            if matches != 1:
                raise SystemExit(f"{relative_path}: expected one match, found {matches} for {old!r}")
            text = text.replace(old, new, 1)
        updates[path] = text
    for path, text in updates.items():
        path.write_text(text, encoding="utf-8")
