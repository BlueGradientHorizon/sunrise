#!/usr/bin/env python3
"""Migrate signatures and direct callers for borrowed profile and session text."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
V = Path("crates/sunrise-mod-vocab/src")
G = Path("crates/sunrise-client/src/game")
changes = {}


def replace(path, before, after, count=1):
    if path not in changes:
        changes[path] = (ROOT / path).read_text()
    found = changes[path].count(before)
    if found != count:
        raise SystemExit(f"{path}: expected {count} matches for {before!r}, found {found}")
    changes[path] = changes[path].replace(before, after)


for path in (V / "host/player_session.rs", G / "modding/native_command_reads.rs"):
    replace(path, "-> Option<GameProfile>", "-> Option<GameProfile<'_>>")

for path in (
    V / "host/services.rs",
    G / "host/services.rs",
    G / "modding/native_client_session.rs",
):
    replace(path, "-> Option<ClientServer>", "-> Option<ClientServer<'_>>")
    method = "native_client_language" if path.parent.name == "modding" else "client_language"
    signature = f"fn {method}(&self) -> Option<"
    replace(path, signature + "String>", signature + "Arc<str>>")

for path, anchor in (
    (G / "host/services.rs", "use mod_api::{prelude::*, *};"),
    (G / "modding/native_client_session.rs", "use mod_api::{BlockBreakingState, BlockPos, ClientFrameStats, ClientServer, PlayerRef};"),
):
    replace(path, anchor, anchor + "\nuse std::sync::Arc;")

replace(
    Path("crates/sunrise-mod-host/examples/native_waypoints.rs"),
    "waypoint_path(&server.save_folder?,",
    "waypoint_path(server.save_folder?,",
    2,
)
integrated = G / "world/integrated.rs"
replace(integrated, "local.name.as_deref()", "local.name")
replace(integrated, "local.save_folder.as_deref()", "local.save_folder")
replace(
    integrated,
    """            assert_eq!(
                game.client_session()
                    .client_server()
                    .unwrap()
                    .name
                    .as_deref(),
                Some("Renamed world")
            );
            let renamed = game.client_session().client_server().unwrap();
            assert_eq!(renamed.save_folder, local.save_folder);""",
    """            let renamed = game.client_session().client_server().unwrap();
            assert_eq!(renamed.name, Some("Renamed world"));
            assert_eq!(renamed.save_folder, Some("first_world"));""",
)

for path, source in changes.items():
    (ROOT / path).write_text(source)
