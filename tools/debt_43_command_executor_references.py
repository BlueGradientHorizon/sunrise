#!/usr/bin/env python3
"""Replace repeated prose references to the server command answer hook."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GAME = "crates/sunrise-client/src/game/"
VOCAB = "crates/sunrise-mod-vocab/src/"

for path, old, new in [
    (GAME + "modding/native_menus.rs", "`native_run_mod_command`", "a command executor"),
    (GAME + "native_hooks/base.rs", "`native_run_mod_command`", "a command executor"),
    (GAME + "network/lan_actions/respawn.rs", "as for native_run_mod_command", "as for a command executor"),
    (VOCAB + "hooks/container_opened.rs", "`run_mod_command`", "a command executor"),
    (VOCAB + "hooks/creative_slot.rs", "`run_mod_command`", "a command executor"),
    (VOCAB + "hooks/player_changed_dimension.rs", "`run_mod_command`", "a command executor"),
    (VOCAB + "hooks/vehicle_moved.rs", "`run_mod_command`", "a command executor"),
    (VOCAB + "host/services.rs", "`run_mod_command`", "a command executor"),
]:
    target = ROOT / path
    source = target.read_text()
    assert old in source, path
    target.write_text(source.replace(old, new))
