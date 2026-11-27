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

# Clientbound play packets whose wire payloads can carry wall-clock values.
WALL_CLOCK_PACKETS: tuple[tuple[str, int, str], ...] = (
    ("debug_sample", 30, "server tick timing samples (dedicated_server_tick_time)"),
    ("keep_alive", 45, "elapsed milliseconds since join sent by the network thread"),
    ("pong_response", 63, "echoes the client PingRequest wall-clock timestamp"),
    ("player_chat", 66, "epoch-millisecond chat message timestamp"),
    ("player_info_update", 71, "smoothed keep-alive RTT in UPDATE_LATENCY"),
    ("update_advancements", 133, "epoch-millisecond criterion grant timestamps"),
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
    player: str
    conn: int
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


def parse_capture_file(path: Path) -> ConnectionCapture:
    player: str | None = None
    conn: int | None = None
    packets: list[PacketRecord] = []
    with path.open(encoding="utf-8") as fh:
        for lineno, raw in enumerate(fh, start=1):
            line = raw.rstrip("\r\n")
            if not line:
                continue
            if line.startswith("#"):
                if player is None:
                    parts = line.split("\t")
                    if len(parts) != 3 or parts[0] != "# v1":
                        raise ValueError(f"{path}:{lineno}: invalid capture header: {line!r}")
                    conn = int(parts[1])
                    player = parts[2]
                    if not player:
                        raise ValueError(f"{path}:{lineno}: empty player name in header")
                continue
            if player is None or conn is None:
                raise ValueError(f"{path}:{lineno}: packet record before header")
            parts = line.split("\t")
            if len(parts) != 5:
                raise ValueError(f"{path}:{lineno}: expected 5 tab-separated fields, got {len(parts)}")
            packets.append(
                PacketRecord(
                    seq=int(parts[0]),
                    tick=int(parts[1]),
                    packet_id=int(parts[2]),
                    length=int(parts[3]),
                    digest=parts[4],
                )
            )
    if player is None or conn is None:
        raise ValueError(f"{path}: missing capture header")
    return ConnectionCapture(player=player, conn=conn, path=path, packets=packets)


def load_directory(directory: Path) -> dict[str, list[ConnectionCapture]]:
    """Load all *.pkt files from `directory`, grouped by player name and sorted by conn id."""
    if not directory.is_dir():
        raise ValueError(f"not a directory: {directory}")
    grouped: dict[str, list[ConnectionCapture]] = defaultdict(list)
    for path in sorted(directory.glob("*.pkt")):
        if path.is_file():
            cap = parse_capture_file(path)
            grouped[cap.player].append(cap)
    for caps in grouped.values():
        caps.sort(key=lambda c: (c.conn, c.path.name))
    return dict(grouped)


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
) -> tuple[str, bool]:
    a_filt = [p for p in a_packets if p.packet_id not in ignored_ids]
    b_filt = [p for p in b_packets if p.packet_id not in ignored_ids]
    a_skipped = len(a_packets) - len(a_filt)
    b_skipped = len(b_packets) - len(b_filt)

    for ra, rb in zip(a_filt, b_filt):
        if (ra.tick, ra.packet_id, ra.length, ra.digest) != (
            rb.tick,
            rb.packet_id,
            rb.length,
            rb.digest,
        ):
            seq_s = _fmt_pair(str(ra.seq), str(rb.seq))
            tick_s = _fmt_pair(str(ra.tick), str(rb.tick))
            id_s = _fmt_pair(packet_label(ra.packet_id, names), packet_label(rb.packet_id, names))
            return (
                f"{label}: DIFFERS seq={seq_s} tick={tick_s} id={id_s} "
                f"len={ra.length} vs {rb.length} hash={ra.digest} vs {rb.digest}",
                False,
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
        )

    suffix = f" ({a_skipped}/{b_skipped} ignored)" if (a_skipped or b_skipped) else ""
    return f"{label}: identical ({len(a_filt)} packets){suffix}", True


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

    players = sorted(set(a_map) | set(b_map))
    if not players:
        return ["no packet recordings found", "DIFFERENT"], False

    report: list[str] = []
    all_same = True
    for player in players:
        a_list = a_map.get(player, [])
        b_list = b_map.get(player, [])
        count = max(len(a_list), len(b_list))
        for idx in range(count):
            label = player if count == 1 else f"{player} #{idx + 1}"
            if idx >= len(a_list):
                all_same = False
                report.append(f"{label}: DIFFERS only in B ({len(b_list[idx].packets)} packets)")
            elif idx >= len(b_list):
                all_same = False
                report.append(f"{label}: DIFFERS only in A ({len(a_list[idx].packets)} packets)")
            else:
                line, same = compare_connection(
                    label, a_list[idx].packets, b_list[idx].packets, ignored, pkt_names
                )
                report.append(line)
                all_same = all_same and same

    report.append("IDENTICAL" if all_same else "DIFFERENT")
    return report, all_same


def build_parser() -> argparse.ArgumentParser:
    wall_lines = "\n".join(
        f"  {name:<20} (id {pid:>3}): {desc}" for name, pid, desc in WALL_CLOCK_PACKETS
    )
    epilog = (
        "Play clientbound packets carrying wall-clock values (none ignored by default):\n"
        f"{wall_lines}\n\n"
        "Example:\n"
        "  packet_capture_compare.py run_a run_b --ignore keep_alive,pong_response"
    )
    parser = argparse.ArgumentParser(
        description=(
            "Compare two per-connection outgoing play packet capture directories "
            "(matched by player name) and print the first difference per connection."
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
