#!/usr/bin/env python3
"""Move consumers of the protocol-owned profile property to core's storage record."""
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
for name in (
    "sunrise-client-render/src/auth.rs",
    "sunrise-client/src/entity_nbt.rs",
    "sunrise-client/src/game/modding/native_command_reads.rs",
    "sunrise-client/src/game/world/integrated.rs",
    "sunrise-client/src/lan/conn.rs",
    "sunrise-client/src/lan/guest.rs",
    "sunrise-client/src/lan/hub.rs",
    "sunrise-client/src/lan/mod.rs",
    "sunrise-client/tests/lan_bot.rs",
    "sunrise-mod-vocab/src/entity/types.rs",
    "sunrise-protocol/src/entity_data.rs",
    "sunrise-protocol/src/packets/play/players.rs",
):
    path = ROOT / "crates" / name
    old = path.read_text()
    new = re.sub(r"(?:crate::packets::login::)?\bProperty::(read|write)\(", r"PropertyCodec::\1(", old)
    new = new.replace("        use crate::packets::login::Property;\n", "")
    new = new.replace("    packets::login::Property,", "    packets::login::PropertyCodec,")
    new = re.sub(r"(?:(?:sunrise_protocol|crate)::packets::)?\blogin::Property\b", "sunrise_core::profile::AccountProperty", new)
    if name.endswith(("/native_command_reads.rs", "/entity/types.rs", "/entity_data.rs", "/play/players.rs")):
        new = re.sub(r"\bProperty\b", "AccountProperty", new)
    if new == old:
        raise SystemExit(f"no property use to move: {name}")
    path.write_text(new)
