#!/usr/bin/env python3
"""One-shot debt 56w3 migration: type the vanilla HUD anchors and remote entry calls."""

import re
import subprocess
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = "crates/sunrise-client/src"
GAME = f"{SRC}/game"
HUD = f"{SRC}/gui_layers.rs"
CHAT = f"{GAME}/interface/chat.rs"
MOUNTS = f"{GAME}/mobs/mounts.rs"
HUD_TEST = f"{GAME}/modding/gui_layer_tests.rs"
HUD_HELPERS = f"{GAME}/native_hooks/shared/world_helpers.rs"
REMOTE = f"{GAME}/network/remote"
NATIVE = f"{GAME}/native_hooks"
SWITCH = f"{GAME}/network/viewed_worlds/switch.rs"

# Registration order, not the order of the calls in Game::hud_layers.
VANILLA_IDS = {
    variant: "minecraft:" + re.sub(r"(?<!^)(?=[A-Z])", "_", variant).lower()
    for variant in (
        "Crosshair", "Hotbar", "Effects", "BossOverlay", "VehicleHealth",
        "ContextualInfoBar", "PlayerHealth", "SelectedItemName", "SleepOverlay",
        "OverlayMessage", "Title", "ScoreboardSidebar", "SubtitleOverlay", "Chat", "TabList",
    )
}
HUD_CALLS = {
    f"{SRC}/game.rs": (
        "Crosshair", "Hotbar", "Effects", "BossOverlay", "PlayerHealth", "ContextualInfoBar",
        "SelectedItemName", "SleepOverlay", "OverlayMessage", "Title", "ScoreboardSidebar",
        "SubtitleOverlay", "SubtitleOverlay", "Chat", "TabList",
    ),
    CHAT: ("Chat",),
    MOUNTS: ("VehicleHealth",),
}
VANILLA_TEST_CALLS = ("Hotbar", "Effects", "Chat", "SleepOverlay")

# The old second arguments in call order: L for false (login), R for true (respawn).
REMOTE_CALLS = {
    f"{REMOTE}/mod.rs": "LL",
    SWITCH: "RR",
    f"{NATIVE}/block_entity_on_load_routes_tests.rs": "L",
    f"{NATIVE}/block_entity_on_load_tests.rs": "L",
    f"{NATIVE}/client_chunk_hooks_tests.rs": "LL",
    f"{NATIVE}/level_tick_tests.rs": "LL",
    f"{NATIVE}/on_block_state_change_tests.rs": "L",
    f"{REMOTE}/attachment_query_tests.rs": "LL",
    f"{REMOTE}/client_block_changed_tests.rs": "L",
    f"{REMOTE}/client_camera_tests.rs": "L",
    f"{REMOTE}/client_epochs_tests.rs": "LRL",
    f"{REMOTE}/client_handles_tests.rs": "LLLLLLR",
    f"{REMOTE}/client_menu_tests.rs": "RL",
    f"{REMOTE}/client_player_reads_tests.rs": "L",
    f"{REMOTE}/dimension_transition_tests.rs": "LRR",
    f"{REMOTE}/particle_group_tests.rs": "L",
    f"{REMOTE}/placement_prediction_tests.rs": "L",
    f"{REMOTE}/predicted_block_notice_tests.rs": "L",
}

DIRECT_CALL = re.compile(r"(?:\.|::)\s*(gui_layer|draw_mod_gui_layers|enter_remote_dimension)\s*\(")
GUI_CALL = re.compile(r'(\.gui_layer\(\s*)"([^"\n]+)"(?=,)')
TEST_CALL = re.compile(r'(\bvanilla\(game, &mut batch, )"([^"\n]+)"(?=\);)')
REMOTE_CALL = re.compile(
    r"(\.enter_remote_dimension\(\s*&"
    r"(?:spawn|p\.spawn|second|other|other_level|overworld\(\)|SpawnInfo \{[^{}]*\}),\s*)"
    r"(false|true)(?=,\s*(?:reason|(?:sunrise_client::level_loading::)?Reason::Other),?\s*\))"
)


def main():
    # Discover all tracked Rust references, including calls outside the planned files.
    candidates = subprocess.check_output(
        ["git", "grep", "-lwz", "-e", "gui_layer", "-e", "draw_mod_gui_layers",
         "-e", "enter_remote_dimension", "--", "*.rs"],
        cwd=ROOT, text=True, encoding="utf-8",
    ).strip("\0").split("\0")
    sources = {name: (ROOT / name).read_text(encoding="utf-8") for name in candidates}
    expected = {
        "gui_layer": {**{path: len(calls) for path, calls in HUD_CALLS.items()}, HUD_TEST: 1},
        "draw_mod_gui_layers": {f"{SRC}/game.rs": 2, HUD_TEST: 2, HUD_HELPERS: 1},
        "enter_remote_dimension": {path: len(calls) for path, calls in REMOTE_CALLS.items()},
    }
    for path in sorted(sources.keys() | {p for paths in expected.values() for p in paths}):
        calls = Counter(DIRECT_CALL.findall(sources.get(path, "")))
        planned = Counter({name: paths[path] for name, paths in expected.items() if path in paths})
        if calls != planned:
            raise SystemExit(f"{path}: expected {dict(planned)}, found {dict(calls)}")

    sources[HUD] = (ROOT / HUD).read_text(encoding="utf-8")
    edits = sources.copy()

    def replace(path, old, new, count=1):
        actual = edits[path].count(old)
        if actual != count:
            raise SystemExit(f"{path}: expected {count} of {old!r}, found {actual}")
        edits[path] = edits[path].replace(old, new)

    def rewrite_calls(path, pattern, old, new):
        actual = [m[2] for m in pattern.finditer(edits[path])]
        if actual != old:
            raise SystemExit(f"{path}: expected call arguments {old!r}, found {actual!r}")
        replacements = iter(new)
        edits[path] = pattern.sub(lambda m: m[1] + next(replacements), edits[path])

    variants = "".join(f"    {v},\n" for v in VANILLA_IDS)
    ids = "".join(f'    "{id_}",\n' for id_ in VANILLA_IDS.values())
    anchors = "".join(f"        Self::{v},\n" for v in VANILLA_IDS)
    arms = "".join(f'            Self::{v} => "{id_}",\n' for v, id_ in VANILLA_IDS.items())
    size = len(VANILLA_IDS)
    replace(HUD,
        "/// Vanilla's HUD layers in the order Sunrise draws them (`VanillaGuiLayers`, without the camera\n"
        "/// overlays and the debug screen, which the HUD pass does not draw): the ids `gui_layer_pre` fires.\n"
        f"pub const VANILLA: [&str; {size}] = [\n{ids}];",
        "/// Vanilla HUD anchors used by the engine's HUD pass (no camera overlays or debug screen).\n"
        f"#[derive(Clone, Copy)]\npub enum VanillaGuiLayer {{\n{variants}}}\n\n"
        "impl VanillaGuiLayer {\n"
        "    /// Vanilla anchors in the layer registrar's bottom-to-top order.\n"
        f"    pub const ALL: [Self; {size}] = [\n{anchors}    ];\n\n"
        "    /// The vanilla layer ID reported to HUD observers.\n"
        f"    pub const fn id(self) -> &'static str {{\n        match self {{\n{arms}        }}\n    }}\n}}",
    )
    replace(HUD, "[`VANILLA`]", "[`VanillaGuiLayer::ALL`]")
    replace(HUD, "VANILLA.iter().map(|&id| id.to_owned()).collect()",
        "VanillaGuiLayer::ALL.iter().map(|layer| layer.id().to_owned()).collect()")
    replace(HUD, ".filter(|id| !VANILLA.contains(id))\n    }",
        ".filter(|id| !VanillaGuiLayer::ALL.iter().any(|layer| layer.id() == *id))\n    }")
    replace(HUD, "VANILLA.len()", "VanillaGuiLayer::ALL.len()", count=2)
    replace(HUD,
        "let vanilla: Vec<&str> = order.ids().filter(|id| VANILLA.contains(id)).collect();\n"
        "        assert_eq!(vanilla, VANILLA);",
        "let expected = VanillaGuiLayer::ALL.map(VanillaGuiLayer::id);\n"
        "        let vanilla: Vec<&str> = order.ids().filter(|id| expected.contains(id)).collect();\n"
        "        assert_eq!(vanilla, expected);",
    )
    replace(HUD, "let mods: Vec<&str> = order.ids().filter(|id| !VANILLA.contains(id)).collect();",
        "let vanilla = VanillaGuiLayer::ALL.map(VanillaGuiLayer::id);\n"
        "        let mods: Vec<&str> = order.ids().filter(|id| !vanilla.contains(id)).collect();")
    replace(HUD, "order.ids().eq(VANILLA)", "order.ids().eq(VanillaGuiLayer::ALL.map(VanillaGuiLayer::id))")
    if re.search(r"\bVANILLA\b", edits[HUD]):
        raise SystemExit(f"{HUD}: old HUD anchor list still referenced")

    layer_import = "use sunrise_client::gui_layers::VanillaGuiLayer;\n"
    replace(HUD_HELPERS, "use super::*;\n", "use super::*;\n" + layer_import)
    replace(HUD_HELPERS, "        layer: &str,\n        batch: &mut HudBatch,\n        draw:",
        "        layer: VanillaGuiLayer,\n        batch: &mut HudBatch,\n        draw:")
    replace(HUD_HELPERS, "self.draw_mod_gui_layers(Some(layer), batch);",
        "self.draw_mod_gui_layers(Some(layer), batch);\n        let layer = layer.id();")
    replace(HUD_HELPERS, "until: Option<&str>,", "until: Option<VanillaGuiLayer>,")
    replace(HUD_HELPERS, "Some(layer) => match order.position(layer) {",
        "Some(layer) => match order.position(layer.id()) {")

    for path, layers in HUD_CALLS.items():
        rewrite_calls(path, GUI_CALL, [VANILLA_IDS[v] for v in layers],
            [f"VanillaGuiLayer::{v}" for v in layers])
    game_rs = f"{SRC}/game.rs"
    replace(game_rs, "    fn hud_layers(&mut self, size: [u32; 2]) -> HudBatch {\n",
        "    fn hud_layers(&mut self, size: [u32; 2]) -> HudBatch {\n        " + layer_import + "\n")
    replace(game_rs, '.draw_mod_gui_layers(Some("minecraft:chat"), &mut batch);',
        ".draw_mod_gui_layers(Some(VanillaGuiLayer::Chat), &mut batch);")
    flush = ".draw_mod_gui_layers(None, &mut batch);"
    replace(game_rs, flush, flush)
    replace(HUD_TEST, flush, flush, count=2)
    for path in (CHAT, MOUNTS):
        replace(path, "    hud::HudBatch,\n", "    gui_layers::VanillaGuiLayer,\n    hud::HudBatch,\n")
    replace(HUD_TEST, "use sunrise_client::hud::HudBatch;",
        "use sunrise_client::{gui_layers::VanillaGuiLayer, hud::HudBatch};")
    replace(HUD_TEST,
        "fn vanilla(game: &mut super::Game<'_>, batch: &mut HudBatch, id: &'static str) {\n"
        '    game.gui_layer(id, batch, |_, _| log(format!("vanilla {id}")));',
        "fn vanilla(game: &mut super::Game<'_>, batch: &mut HudBatch, layer: VanillaGuiLayer) {\n"
        '    game.gui_layer(layer, batch, |_, _| log(format!("vanilla {}", layer.id())));',
    )
    # Count the helper's declaration and every call, even an unexpected non-literal one.
    if len(re.findall(r"\bvanilla\s*\(", edits[HUD_TEST])) != 1 + len(VANILLA_TEST_CALLS):
        raise SystemExit(f"{HUD_TEST}: unexpected number of vanilla helper references")
    rewrite_calls(HUD_TEST, TEST_CALL, [VANILLA_IDS[v] for v in VANILLA_TEST_CALLS],
        [f"VanillaGuiLayer::{v}" for v in VANILLA_TEST_CALLS])

    remote_mod = f"{REMOTE}/mod.rs"
    replace(remote_mod, "mod chat_queue;\n", "mod chat_queue;\n\n"
        "/// The packet route that starts a remote level entry.\n"
        "pub(in crate::game) enum RemoteDimensionEntry {\n    Login,\n    Respawn,\n}\n")
    replace(remote_mod, "        keep_world: bool,\n", "        entry: RemoteDimensionEntry,\n")
    replace(remote_mod, "let same = keep_world && r.heard_level.as_deref() == Some(spawn.dimension.as_str());",
        "let same = matches!(entry, RemoteDimensionEntry::Respawn)\n"
        "            && r.heard_level.as_deref() == Some(spawn.dimension.as_str());")

    # These fixtures do not already import the remote module's items.
    for path, anchor in {
        remote_mod: "    fn spawn_menu_test(game: &mut super::Game<'_>) -> sunrise_protocol::packets::play::SpawnInfo {\n",
        f"{NATIVE}/block_entity_on_load_routes_tests.rs": "        native_entity_refs::tests::{run_fresh_cases, with_game_role},\n    };\n",
        f"{NATIVE}/block_entity_on_load_tests.rs": "        fixture_root, run_fresh_cases, with_game, with_game_at,\n    };\n",
        f"{NATIVE}/level_tick_tests.rs": "        native_entity_refs::tests::with_client_game, native_lifecycle::NativeLifecycle,\n    };\n",
        f"{REMOTE}/client_epochs_tests.rs": "        native_lifecycle::NativeLifecycle,\n    };\n",
        f"{REMOTE}/client_menu_tests.rs": "    use sunrise_client::level_loading::Reason;\n",
        f"{REMOTE}/dimension_transition_tests.rs": "    use crate::game::native_entity_refs::tests::with_game;\n",
        f"{REMOTE}/client_player_reads_tests.rs": "    use crate::game::{execution_role::ExecutionRole, native_entity_refs::tests::with_game};\n",
    }.items():
        replace(path, anchor, anchor + "    use crate::game::remote::RemoteDimensionEntry;\n")
    for path, anchor in {
        f"{NATIVE}/client_chunk_hooks_tests.rs": "use crate::game::native_entity_refs::tests::{run_fresh_cases, with_game_role};\n",
        f"{NATIVE}/on_block_state_change_tests.rs": "use crate::game::native_entity_refs::tests::with_game;\n",
        f"{REMOTE}/client_camera_tests.rs": "    Game, native_entity_refs::tests::with_client_game, native_lifecycle::NativeLifecycle,\n};\n",
        f"{REMOTE}/particle_group_tests.rs": "    native_hooks::particle_count_tests::fill,\n};\n",
        SWITCH: "\nuse crate::game::Game;\n",
    }.items():
        replace(path, anchor, anchor + "use crate::game::remote::RemoteDimensionEntry;\n")
    handles_doc = "/// A client names the level it shows by the server's key, even where `Dim` has\n"
    replace(f"{REMOTE}/client_handles_tests.rs", handles_doc,
        "use crate::game::remote::RemoteDimensionEntry;\n\n" + handles_doc)

    for path, calls in REMOTE_CALLS.items():
        rewrite_calls(path, REMOTE_CALL, ["false" if c == "L" else "true" for c in calls],
            ["RemoteDimensionEntry::Login" if c == "L" else "RemoteDimensionEntry::Respawn" for c in calls])

    changed = sorted(path for path, source in edits.items() if source != sources.get(path))
    for path in changed:
        (ROOT / path).write_text(edits[path], encoding="utf-8")
    subprocess.run(["rustfmt", "--edition", "2024", "--config", "skip_children=true", *changed],
        cwd=ROOT, check=True)


if __name__ == "__main__":
    main()
