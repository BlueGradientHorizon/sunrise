#!/usr/bin/env python3
"""Add per-opening overflow-report state to each LAN Menu initializer."""

import re
import subprocess
from pathlib import Path

paths = subprocess.check_output(
    ["git", "grep", "-l", "blockless_data: Vec::new(),", "--", "crates/sunrise-client/src"],
    text=True,
).splitlines()
for name in paths:
    path = Path(name)
    original = path.read_text()
    updated = re.sub(
        r"(?m)^([ ]*)(sent_data: (?:Vec::new\(\)|vec!\[\]),\n)(\1blockless_data:)",
        r"\1\2\1reported_unsent_data_len: None,\n\3",
        original,
    )
    if updated != original:
        path.write_text(updated)
