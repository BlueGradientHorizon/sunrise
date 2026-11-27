#!/usr/bin/env python3
"""Compare two per-connection play packet capture directories (SUNRISE_PACKET_CAPTURE)."""
from __future__ import annotations

import argparse
import re
import sys
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
IDS_PATH = ROOT / "crates/sunrise-protocol/src/ids.rs"
HEX_DIGEST_RE = re.compile(r"^[0-9a-f]{32}$")

# Clientbound play packets whose wire payloads can carry wall-clock values.
WALL_CLOCK_PACKETS: tuple[tuple[str, int, str], ...] = (
    ("debug_sample", 30, "server tick timing samples (dedicated_server_tick_time)"),
    ("keep_alive", 45, "elapsed milliseconds since join sent by the network thread"),
    ("pong_response", 63, "echoes the client PingRequest wall-clock timestamp"),
    ("player_chat", 66, "epoch-millisecond chat message timestamp (also carries message/sender)"),
    (
        "player_info_update",
        71,
        "smoothed keep-alive RTT in UPDATE_LATENCY (also carries roster/gamemode/session)",
    ),
    (
        "update_advancements",
        133,
        "epoch-millisecond criterion grant timestamps (also carries criteria/completion)",
    ),
)


@dataclass(frozen=True)
class PacketRecord:
    seq: int
    tick: int
    packet_id: int
    length: int
    digest: str


@dataclass(frozen=True)
class ConnectionCapture:
    run_id: str
    player: str
    conn: int
    ordinal: int
    path: Path
    packets: list[PacketRecord]


def load_play_clientbound_names(path: Path = IDS_PATH) -> list[str]:
    """Return play::clientbound::NAMES indexed by packet id."""
    if not path.is_file():
        return []
    text = path.read_text(encoding="utf-8")
    play_block = re.search(r"pub mod play \{(.*?)\n\}", text, re.S)
    if not play_block:
        return []
    cb_block = re.search(
        r"pub mod clientbound \{.*?NAMES: &\[&str\] = &\[(.*?)\];",
        play_block.group(1),
        re.S,
    )
    if not cb_block:
        return []
    return re.findall(r'"([^"]+)"', cb_block.group(1))


def packet_label(packet_id: int, names: list[str]) -> str:
    if 0 <= packet_id < len(names):
        return f"{packet_id} ({names[packet_id]})"
    return f"{packet_id} (unknown)"


def _parse_int(text: str, field: str, path: Path, lineno: int) -> int:
    try:
        return int(text)
    except ValueError as err:
        raise ValueError(f"{path}:{lineno}: invalid {field}: {text!r}") from err


def parse_capture_file(path: Path) -> ConnectionCapture:
    raw_bytes = path.read_bytes()
    if not raw_bytes:
        raise ValueError(f"{path}: empty capture file")
    if not raw_bytes.endswith(b"\n"):
        raise ValueError(f"{path}: truncated file (missing trailing newline)")
    try:
        text = raw_bytes.decode("utf-8")
    except UnicodeDecodeError as err:
        raise ValueError(f"{path}: invalid UTF-8: {err}") from err

    lines = text.splitlines()
    if len(lines) < 2:
        raise ValueError(f"{path}: incomplete capture file (missing end marker)")

    header = lines[0].split("\t")
    if len(header) != 5 or header[0] != "# v2":
        raise ValueError(f"{path}:1: invalid capture header: {lines[0]!r}")
    run_id = header[1]
    conn = _parse_int(header[2], "conn", path, 1)
    ordinal = _parse_int(header[3], "ordinal", path, 1)
    player = header[4]
    if not run_id or conn < 0 or ordinal < 1 or not player:
        raise ValueError(f"{path}:1: invalid capture header fields: {lines[0]!r}")

    packets: list[PacketRecord] = []
    ended = False
    for lineno, line in enumerate(lines[1:], start=2):
        if ended:
            raise ValueError(f"{path}:{lineno}: trailing data after end marker")
        if line.startswith("# end\t"):
            parts = line.split("\t")
            if len(parts) < 3:
                raise ValueError(f"{path}:{lineno}: malformed end marker: {line!r}")
            declared_count = _parse_int(parts[1], "end count", path, lineno)
            status = parts[2]
            if status != "ok":
                reason = parts[3] if len(parts) > 3 else status
                raise ValueError(f"{path}:{lineno}: incomplete capture ({reason})")
            if len(parts) != 3:
                raise ValueError(f"{path}:{lineno}: malformed ok end marker: {line!r}")
            if declared_count != len(packets):
                raise ValueError(
                    f"{path}:{lineno}: end marker count {declared_count} != {len(packets)} rows"
                )
            ended = True
            continue
        if line.startswith("#") or not line:
            raise ValueError(f"{path}:{lineno}: unexpected line in capture body: {line!r}")
        parts = line.split("\t")
        if len(parts) != 5:
            raise ValueError(
                f"{path}:{lineno}: expected 5 tab-separated fields, got {len(parts)}"
            )
        seq = _parse_int(parts[0], "seq", path, lineno)
        tick = _parse_int(parts[1], "tick", path, lineno)
        packet_id = _parse_int(parts[2], "packet_id", path, lineno)
        length = _parse_int(parts[3], "length", path, lineno)
        digest = parts[4]
        if seq != len(packets):
            raise ValueError(
                f"{path}:{lineno}: noncontiguous sequence {seq} (expected {len(packets)})"
            )
        if packet_id < 0:
            raise ValueError(f"{path}:{lineno}: invalid packet id {packet_id}")
        if length < 0:
            raise ValueError(f"{path}:{lineno}: negative length {length}")
        if not HEX_DIGEST_RE.match(digest):
            raise ValueError(f"{path}:{lineno}: invalid digest {digest!r}")
        packets.append(
            PacketRecord(
                seq=seq,
                tick=tick,
                packet_id=packet_id,
                length=length,
                digest=digest,
            )
        )

    if not ended:
        raise ValueError(f"{path}: truncated capture (missing # end marker)")

    return ConnectionCapture(
        run_id=run_id,
        player=player,
        conn=conn,
        ordinal=ordinal,
        path=path,
        packets=packets,
    )


def load_directory(directory: Path) -> dict[tuple[str, int], ConnectionCapture]:
    """Validate manifest.tsv and every *.pkt file in `directory`, keyed by (player, ordinal)."""
    if not directory.is_dir():
        raise ValueError(f"not a directory: {directory}")
    manifest_path = directory / "manifest.tsv"
    if not manifest_path.is_file():
        raise ValueError(f"{directory}: incomplete run (missing manifest.tsv)")

    raw_bytes = manifest_path.read_bytes()
    if not raw_bytes or not raw_bytes.endswith(b"\n"):
        raise ValueError(f"{manifest_path}: truncated manifest (missing trailing newline)")
    try:
        lines = raw_bytes.decode("utf-8").splitlines()
    except UnicodeDecodeError as err:
        raise ValueError(f"{manifest_path}: invalid UTF-8: {err}") from err

    header = lines[0].split("\t")
    if len(header) != 3 or header[0] != "# manifest v1":
        raise ValueError(f"{manifest_path}:1: invalid manifest header: {lines[0]!r}")
    run_id, run_status = header[1], header[2]
    if not run_id:
        raise ValueError(f"{manifest_path}:1: empty run id")
    if run_status != "ok":
        raise ValueError(f"{manifest_path}:1: incomplete capture run (status={run_status})")

    captures: dict[tuple[str, int], ConnectionCapture] = {}
    expected_files: set[str] = set()
    for lineno, line in enumerate(lines[1:], start=2):
        parts = line.split("\t")
        if len(parts) != 6:
            raise ValueError(
                f"{manifest_path}:{lineno}: expected 6 tab-separated fields, got {len(parts)}"
            )
        conn = _parse_int(parts[0], "conn", manifest_path, lineno)
        ordinal = _parse_int(parts[1], "ordinal", manifest_path, lineno)
        player, filename, status = parts[2], parts[3], parts[4]
        declared_count = _parse_int(parts[5], "count", manifest_path, lineno)
        if conn < 0 or ordinal < 1 or not player:
            raise ValueError(f"{manifest_path}:{lineno}: invalid session identity")
        if status != "ok":
            raise ValueError(
                f"{manifest_path}:{lineno}: session {player} (conn={conn}) incomplete ({status})"
            )
        if not filename or filename == "-" or "/" in filename or "\\" in filename:
            raise ValueError(f"{manifest_path}:{lineno}: invalid session filename {filename!r}")
        if filename in expected_files:
            raise ValueError(f"{manifest_path}:{lineno}: duplicate filename {filename!r}")
        expected_files.add(filename)

        pkt_path = directory / filename
        if not pkt_path.is_file():
            raise ValueError(f"{manifest_path}:{lineno}: missing session file {filename}")
        cap = parse_capture_file(pkt_path)
        if (
            cap.run_id != run_id
            or cap.conn != conn
            or cap.ordinal != ordinal
            or cap.player != player
        ):
            raise ValueError(
                f"{pkt_path}: header ({cap.run_id}, conn={cap.conn}, ord={cap.ordinal}, "
                f"player={cap.player!r}) does not match manifest ({run_id}, conn={conn}, "
                f"ord={ordinal}, player={player!r})"
            )
        if len(cap.packets) != declared_count:
            raise ValueError(
                f"{pkt_path}: packet count {len(cap.packets)} != manifest count {declared_count}"
            )
        key = (player, ordinal)
        if key in captures:
            raise ValueError(f"{manifest_path}:{lineno}: duplicate session key {key!r}")
        captures[key] = cap

    actual_files = {p.name for p in directory.glob("*.pkt") if p.is_file()}
    extra_files = sorted(actual_files - expected_files)
    if extra_files:
        raise ValueError(f"{directory}: unlisted .pkt file(s) not in manifest: {', '.join(extra_files)}")

    return captures


def resolve_ignores(specs: list[str], names: list[str]) -> set[int]:
    name_to_id = {name: idx for idx, name in enumerate(names)}
    ignored: set[int] = set()
    for spec in specs:
        for raw in spec.split(","):
            token = raw.strip()
            if not token:
                continue
            if token.lstrip("-").isdigit():
                ignored.add(int(token))
            elif token in name_to_id:
                ignored.add(name_to_id[token])
            else:
                raise ValueError(f"unknown play clientbound packet name or id: {token!r}")
    return ignored


def _fmt_pair(a: str, b: str) -> str:
    return a if a == b else f"{a} vs {b}"


def compare_connection(
    label: str,
    a_packets: list[PacketRecord],
    b_packets: list[PacketRecord],
    ignored_ids: set[int],
    names: list[str],
) -> tuple[str, bool, int]:
    a_filt = [p for p in a_packets if p.packet_id not in ignored_ids]
    b_filt = [p for p in b_packets if p.packet_id not in ignored_ids]
    a_skipped = len(a_packets) - len(a_filt)
    b_skipped = len(b_packets) - len(b_filt)
    total_skipped = a_skipped + b_skipped

    for ra, rb in zip(a_filt, b_filt):
        if (ra.tick, ra.packet_id, ra.length, ra.digest) != (
            rb.tick,
            rb.packet_id,
            rb.length,
            rb.digest,
        ) or (not ignored_ids and ra.seq != rb.seq):
            seq_s = _fmt_pair(str(ra.seq), str(rb.seq))
            tick_s = _fmt_pair(str(ra.tick), str(rb.tick))
            id_s = _fmt_pair(packet_label(ra.packet_id, names), packet_label(rb.packet_id, names))
            return (
                f"{label}: DIFFERS seq={seq_s} tick={tick_s} id={id_s} "
                f"len={ra.length} vs {rb.length} hash={ra.digest} vs {rb.digest}",
                False,
                total_skipped,
            )

    if len(a_filt) != len(b_filt):
        if len(a_filt) > len(b_filt):
            extra = a_filt[len(b_filt)]
            side = "A"
            lens = f"{extra.length} vs <eof>"
            hashes = f"{extra.digest} vs <eof>"
        else:
            extra = b_filt[len(a_filt)]
            side = "B"
            lens = f"<eof> vs {extra.length}"
            hashes = f"<eof> vs {extra.digest}"
        return (
            f"{label}: DIFFERS count={len(a_filt)} vs {len(b_filt)} "
            f"(first extra in {side}: seq={extra.seq} tick={extra.tick} "
            f"id={packet_label(extra.packet_id, names)} len={lens} hash={hashes})",
            False,
            total_skipped,
        )

    if ignored_ids:
        return (
            f"{label}: retained packets identical ({len(a_filt)} kept, "
            f"{a_skipped}/{b_skipped} ignored)",
            True,
            total_skipped,
        )
    return f"{label}: identical ({len(a_filt)} packets)", True, 0


def compare_dirs(
    dir_a: Path,
    dir_b: Path,
    ignored_ids: set[int] | None = None,
    names: list[str] | None = None,
) -> tuple[list[str], bool]:
    ignored = ignored_ids or set()
    pkt_names = load_play_clientbound_names() if names is None else names
    a_map = load_directory(dir_a)
    b_map = load_directory(dir_b)

    keys = sorted(set(a_map) | set(b_map))
    if not keys:
        return ["no packet recordings found", "DIFFERENT"], False

    ordinals_per_player: dict[str, set[int]] = defaultdict(set)
    for player, ordinal in keys:
        ordinals_per_player[player].add(ordinal)

    report: list[str] = []
    all_same = True
    total_ignored = 0
    for player, ordinal in keys:
        multi = len(ordinals_per_player[player]) > 1 or ordinal > 1
        label = f"{player} #{ordinal}" if multi else player
        cap_a = a_map.get((player, ordinal))
        cap_b = b_map.get((player, ordinal))
        if cap_a is None:
            all_same = False
            report.append(f"{label}: DIFFERS only in B ({len(cap_b.packets)} packets)")
        elif cap_b is None:
            all_same = False
            report.append(f"{label}: DIFFERS only in A ({len(cap_a.packets)} packets)")
        else:
            line, same, skipped = compare_connection(
                label, cap_a.packets, cap_b.packets, ignored, pkt_names
            )
            report.append(line)
            all_same = all_same and same
            total_ignored += skipped

    if all_same:
        if ignored:
            report.append(
                f"IDENTICAL (retained only; {total_ignored} ignored packet(s) skipped)"
            )
        else:
            report.append("IDENTICAL")
    else:
        report.append("DIFFERENT")
    return report, all_same


def build_parser() -> argparse.ArgumentParser:
    wall_lines = "\n".join(
        f"  {name:<20} (id {pid:>3}): {desc}" for name, pid, desc in WALL_CLOCK_PACKETS
    )
    epilog = (
        "Play clientbound packets carrying wall-clock values (none ignored by default):\n"
        f"{wall_lines}\n\n"
        "Notes:\n"
        "  - --ignore skips entire packet IDs; a filtered match proves equality only of\n"
        "    retained records (not skipped fields such as roster, chat, or advancements).\n"
        "  - Exclude clock-dependent commands (e.g. /debug stop) and network-driven chunk\n"
        "    batching from parity workloads rather than broadly ignoring chat or chunks.\n\n"
        "Example:\n"
        "  packet_capture_compare.py run_a run_b --ignore keep_alive,pong_response"
    )
    parser = argparse.ArgumentParser(
        description=(
            "Compare two per-connection outgoing play packet capture directories "
            "(matched by player name and connection ordinal) and print the first "
            "difference per connection."
        ),
        epilog=epilog,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("a", type=Path, help="first capture directory")
    parser.add_argument("b", type=Path, help="second capture directory")
    parser.add_argument(
        "--ignore",
        action="append",
        default=[],
        metavar="PACKET",
        help=(
            "play clientbound packet name or numeric id to ignore (repeatable or "
            "comma-separated; off by default)"
        ),
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    names = load_play_clientbound_names()
    try:
        ignored = resolve_ignores(args.ignore, names)
        report, same = compare_dirs(args.a, args.b, ignored_ids=ignored, names=names)
    except ValueError as err:
        print(f"error: {err}", file=sys.stderr)
        return 2
    print("\n".join(report))
    return 0 if same else 1


if __name__ == "__main__":
    sys.exit(main())
