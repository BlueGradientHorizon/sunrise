#!/usr/bin/env python3
"""Remove draw-time configuration from special item renderers before baking it at load."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VOCAB = "crates/sunrise-mod-vocab/src/"
CLIENT = "crates/sunrise-client/src/game/modding/"


def replace(path, old, new, count=1):
    path = ROOT / path
    source = path.read_text()
    assert source.count(old) == count, path
    path.write_text(source.replace(old, new))


api = VOCAB + "client/special_model_renderers.rs"
replace(api, "ItemStack, Tag", "ItemStack")
replace(api, "        config: &Tag,\n", "")
replace(api, "            _: &Tag,\n", "")
engine = CLIENT + "native_special_renderers"
replace(engine + ".rs", "extract(host, &api_stack, config, context)", "extract(host, &api_stack, context)")
replace(engine + ".rs", "            _: &Tag,\n", "")
for name, count in [("cost_tests", 2), ("frame_tests", 1)]:
    replace(f"{engine}/{name}.rs", "        _: &sunrise_core::nbt::Tag,\n", "", count)
replace(VOCAB + "hooks/register_special_model_renderer.rs", "        _config: &Tag,\n", "")
replace("docs/modding/src/how-to/client/special-item-renderers.md", "        _config: &Tag,\n", "")
