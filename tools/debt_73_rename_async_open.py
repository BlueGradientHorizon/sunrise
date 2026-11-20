#!/usr/bin/env python3
"""Rename the existing push opener before adding the pull opener under its old name."""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OLD = re.compile(rb"\bopen_audio_stream\b")
NEW = re.compile(rb"\bopen_push_audio_stream\b")
EXPECTED = {
    "crates/sunrise-mod-vocab/src/host/audio.rs": 1,
    "crates/sunrise-mod-vocab/src/client/audio_stream.rs": 4,
    "crates/sunrise-client/src/game/modding/native_audio_streams.rs": 2,
    "crates/sunrise-client/src/game/interface/client_reload/mod.rs": 1,
    "crates/sunrise-client/src/game/modding/pack_finders/mod.rs": 1,
    "crates/sunrise-client-render/src/audio.rs": 2,
    "crates/sunrise-client-render/src/audio/streams.rs": 1,
    "crates/sunrise-hooks/src/sound.rs": 1,
    "docs/extras.md": 1,
    "docs/modding/src/how-to/client/audio-streams.md": 3,
}

# Validate all inputs before writing, so a rerun cannot rename a later pull opener.
updates = {}
for name, expected in EXPECTED.items():
    path = ROOT / name
    source = path.read_bytes()
    renamed, count = OLD.subn(b"open_push_audio_stream", source)
    if count != expected or NEW.search(source):
        raise SystemExit(
            f"{name}: expected {expected} old opener references and no push opener references; "
            "not applying the rename"
        )
    updates[path] = renamed

for path, renamed in updates.items():
    path.write_bytes(renamed)
