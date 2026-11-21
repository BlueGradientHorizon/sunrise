#!/usr/bin/env python3
"""Mark the host forwarders that write Text to packets or persisted components."""
from pathlib import Path

host = Path(__file__).resolve().parents[1] / "crates/sunrise-mod-vocab/src/host"
boundaries = {
    "boss_bars": ("create_boss_bar", "boss_bar_set_name"),
    "displays": ("display_set_text",),
    "menus": ("open_menu",),
    "player_menus": ("player_open_mod_menu", "player_open_ender_chest"),
    "player_messages": ("player_tell", "player_action_bar", "player_tab_list"),
    "player_session": ("player_disconnect",),
    "scoreboard": (
        "add_objective", "objective_set_display_name", "team_set_display_name",
        "team_set_prefix", "team_set_suffix",
    ),
}
for area, methods in boundaries.items():
    path = host / f"{area}.rs"
    source = path.read_text()
    for method in methods:
        declaration = f"    fn {method}("
        marked = "    #[serialize_text]\n" + declaration
        if marked not in source:
            assert source.count(declaration) == 1, (area, method)
            source = source.replace(declaration, marked, 1)
    path.write_text(source)
