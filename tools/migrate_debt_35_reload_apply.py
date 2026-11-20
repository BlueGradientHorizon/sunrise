#!/usr/bin/env python3
"""Replay the reload callback signature and constructor migration (debt 35).

By default, update the engine's listener implementations and examples. Pass
--sample PATH to apply the same migration to a sunrise-machine-sample checkout.
"""
import argparse
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
PATHS = (
    "crates/sunrise-mod-vocab/src/client/client_reload.rs",
    "crates/sunrise-mod-vocab/src/exports/reload.rs",
    "crates/sunrise-mod-vocab/src/world/reload.rs",
    "crates/sunrise-mod-vocab/src/world/server_reload.rs",
    "crates/sunrise-mod-vocab/src/world/server_reload_tests.rs",
    "crates/sunrise-mod-vocab/src/hooks/notes.rs",
    "crates/sunrise-mod-vocab/src/loot_modifiers.rs",
    "crates/sunrise-mod-api/tests/client_reload_attribution.rs",
    "crates/sunrise-mod-api/tests/server_reload_attribution.rs",
    "crates/sunrise-client/src/game/interface/client_reload/client_reload_tests.rs",
    "crates/sunrise-client/src/game/world/server_reload/server_reload_tests.rs",
    "crates/sunrise-client/src/game/modding/mods/mod_inventory_tests.rs",
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sample", type=Path)
    args = parser.parse_args()
    paths = [args.sample / "src/tour/reload.rs"] if args.sample else [ROOT / p for p in PATHS]
    for path in paths:
        before = path.read_text(encoding="utf-8")
        after = re.sub(r"<dyn (?:crate::|mod_api::)?ReloadApply>::new", "Box::new", before)
        after = re.sub(
            r"Box<dyn (?:crate::|mod_api::)?ReloadApply>",
            "Box<dyn FnOnce() + Send + 'static>",
            after,
        )
        after = re.sub(r"\bReloadApply, ", "", after)
        after = after.replace(", ReloadApply,\n", ",\n")
        if path == ROOT / "crates/sunrise-mod-vocab/src/world/reload.rs" and "pub trait ReloadApply" in before:
            start = after.index("/// Infallible publication,")
            end = after.index("/// The stateless boundary listener", start)
            after = after[:start] + after[end:]
            after = after.replace("                apply.apply();", "                apply();")
        if path == ROOT / "crates/sunrise-mod-vocab/src/exports/reload.rs":
            after = after.replace("world::reload::{ReloadResourceError}", "world::reload::ReloadResourceError")
        if after != before:
            path.write_text(after, encoding="utf-8")


if __name__ == "__main__":
    main()
