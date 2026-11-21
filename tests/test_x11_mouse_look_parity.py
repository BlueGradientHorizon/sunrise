"""Vanilla 26.3 X11 mouse-look parity tests: recording fixture, committed 26.3 witness, raw IEEE-754
float-bit comparator, witness builder/verifier, and CLI exit codes."""
import contextlib
import hashlib
import io
import json
from pathlib import Path
import struct
import tempfile
import unittest

from tools import client_packet_diff as diff
from tools import client_packet_routes as routes
from tools import input_recording as recording
from tools import test_x11_mouse_look_parity as harness

ROOT = Path(__file__).resolve().parents[1]
RECORDING = ROOT / "tools/fixtures/26.3/x11_mouse_look_recording.jsonl"
WITNESS = ROOT / "tools/witness/26.3/x11_mouse_look.json"


def _load_witness():
    return json.loads(WITNESS.read_text(encoding="utf-8"))


def _varint(value):
    out = bytearray()
    v = value & 0xFFFFFFFF
    while True:
        b = v & 0x7F
        v >>= 7
        if v:
            out.append(b | 0x80)
        else:
            out.append(b)
            return bytes(out)


def _pos_long(x, y, z):
    # BlockPos.asLong: x in top 26 bits, z in next 26 bits, y in low 12 bits.
    v = ((x & 0x3FFFFFF) << 38) | ((z & 0x3FFFFFF) << 12) | (y & 0xFFF)
    return struct.pack(">Q", v)


def _c2s(name, payload=b""):
    return {"dir": "c2s", "state": "play", "name": name, "data": payload.hex()}


def _mark(number):
    text = f"wire-mark {number:02d}".encode()
    return _c2s("chat_command", _varint(len(text)) + text)


def _rot_bits(yaw_u32, pitch_u32, flags=1):
    return _c2s("move_player_rot", struct.pack(">IIB", yaw_u32, pitch_u32, flags))


def _pos_rot_bits(x, y, z, yaw_u32, pitch_u32, flags=1):
    return _c2s("move_player_pos_rot", struct.pack(">dddIIB", x, y, z, yaw_u32, pitch_u32, flags))


def _rot(yaw, pitch, flags=1):
    return _c2s("move_player_rot", struct.pack(">ffB", yaw, pitch, flags))


def _use_item(hand, sequence, yaw, pitch):
    return _c2s("use_item", _varint(hand) + _varint(sequence) + struct.pack(">ff", yaw, pitch))


def _use_item_on(hand, pos, face, cursor, inside, world_border, sequence):
    payload = (_varint(hand) + _pos_long(*pos) + _varint(face)
               + struct.pack(">fff??", cursor[0], cursor[1], cursor[2], inside, world_border) + _varint(sequence))
    return _c2s("use_item_on", payload)


def _player_action(action, pos, direction, sequence):
    return _c2s("player_action", _varint(action) + _pos_long(*pos) + struct.pack(">B", direction) + _varint(sequence))


def _write_run(runs_dir, folder_name, index, route_streams, *, client="vanilla", build="sha-v",
               witness=None, recording_digest=None, settings=None, course=None, server_jar=None,
               diagnostic=False, fixtures=None):
    w = witness if witness is not None else _load_witness()
    run_dir = Path(runs_dir) / folder_name
    run_dir.mkdir(parents=True, exist_ok=True)
    entries, played = [], []
    ticks = 0
    for mark_num, (route_name, tick_packets) in enumerate(route_streams, 1):
        entries.append(_mark(mark_num))
        for packets in tick_packets:
            entries.extend(packets)
            entries.append(_c2s("client_tick_end"))
            ticks += 1
        default_fix = w["routes"][route_name]["fixture"]
        played.append({"route": route_name, "mark": mark_num, "seconds": 1.0, "end_tick": ticks,
                       "fixture": (fixtures or {}).get(route_name, default_fix)})
    (run_dir / "capture.jsonl").write_text("".join(json.dumps(e) + "\n" for e in entries), encoding="utf-8")
    manifest = {
        "client": client, "index": index, "complete": True,
        "recording": recording_digest if recording_digest is not None else w["recording_digest"],
        "settings": settings if settings is not None else w["settings_digest"],
        "course": course if course is not None else w["course_digest"],
        "routes": [name for name, _ in route_streams],
        "server_jar": server_jar if server_jar is not None else w["vanilla_server_jar_sha256"],
        "client_build": build, "diagnostic": diagnostic,
        "mouse_scale": 0.5 if diagnostic else 1.0, "played": played,
    }
    (run_dir / "run.json").write_text(json.dumps(manifest), encoding="utf-8")
    return run_dir


def _witness_streams(witness=None, *, scale=1.0, pos_rot_on_look_y=False, extra_action=None):
    """Builds exact wire streams directly from the recorded vanilla 26.3 witness bits."""
    w = witness if witness is not None else _load_witness()
    streams = []
    for route_name in routes.MOUSE_LOOK:
        tick_packets = []
        for idx, rot in enumerate(w["routes"][route_name]["rotations"]):
            if scale == 1.0:
                yaw_u32 = int(rot["yaw_bits"], 16)
                pitch_u32 = int(rot["pitch_bits"], 16)
            else:
                yaw_u32 = struct.unpack(">I", struct.pack(">f", rot["yaw"] * scale))[0]
                pitch_u32 = struct.unpack(">I", struct.pack(">f", rot["pitch"] * scale))[0]
            if pos_rot_on_look_y and route_name == "look_y" and idx == 0:
                pkt = _pos_rot_bits(3.5, 101.0, 162.5, yaw_u32, pitch_u32)
            else:
                pkt = _rot_bits(yaw_u32, pitch_u32)
            tick_packets.append([pkt])
        if extra_action is not None and route_name == "regrab":
            tick_packets.append([extra_action])
        streams.append((route_name, tick_packets))
    return streams


class X11MouseLookParity263Tests(unittest.TestCase):
    def test_26_3_recording_fixture_matches_scripted_mouse_look_routes(self):
        header = json.loads(RECORDING.read_text(encoding="utf-8").splitlines()[0])
        self.assertEqual(header.get("minecraft"), "26.3")
        self.assertEqual(header.get("routes"), routes.MOUSE_LOOK)
        events = recording.read(RECORDING)
        parsed = recording.routes(events)
        self.assertEqual([name for name, _ in parsed], ["look_x", "look_y", "look_counts", "regrab"])
        self.assertEqual(events, routes.recording(routes.MOUSE_LOOK))
        # Course stays inside Minecraft's 256-chunk /forceload and 32,768-block /fill limits.
        x0, z0, x1, z1 = routes.course_bounds()
        self.assertLessEqual((x1 - x0 + 1) * (z1 - z0 + 1), 32768)

    def test_26_3_committed_witness_matches_recording_and_exact_float_bits(self):
        self.assertTrue(WITNESS.exists(), f"missing recorded 26.3 witness: {WITNESS}")
        witness = _load_witness()
        for key, expected in diff.LOOK_WITNESS_FORMAT.items():
            self.assertEqual(witness.get(key), expected)
        self.assertEqual(witness.get("recording"), harness.RECORDING_REL)
        file_sha = hashlib.sha256(RECORDING.read_bytes()).hexdigest()
        self.assertEqual(witness.get("recording_file_sha256"), file_sha)
        rec_digest = hashlib.sha256(json.dumps(recording.read(RECORDING), sort_keys=True).encode()).hexdigest()
        self.assertEqual(witness.get("recording_digest"), rec_digest)
        self.assertIn("25.", witness.get("java_version") or "")
        self.assertGreaterEqual(witness.get("runs_verified", 0), 2)
        self.assertEqual(list(witness.get("routes", {})), routes.MOUSE_LOOK)
        for route_name in routes.MOUSE_LOOK:
            entry = witness["routes"][route_name]
            self.assertTrue(entry["rotations"], f"{route_name} witness must contain rotations")
            for rot in entry["rotations"]:
                self.assertEqual(rot["yaw_bits"], diff.float_bits_hex(rot["yaw"]))
                self.assertEqual(rot["pitch_bits"], diff.float_bits_hex(rot["pitch"]))
        with tempfile.TemporaryDirectory() as folder:
            _write_run(folder, "sunrise-1", 1, _witness_streams(witness, pos_rot_on_look_y=True),
                       client="sunrise", build="s", witness=witness)
            rep = diff.compare_look_against_witness(diff.runs_of(Path(folder), "sunrise"), witness, "sunrise")
            self.assertTrue(rep["bit_exact"], diff.describe_look(rep))

    def test_26_3_raw_wire_float_bits_match_across_pos_rot_reminder_and_reject_one_ulp(self):
        witness = _load_witness()
        with tempfile.TemporaryDirectory() as folder:
            _write_run(folder, "vanilla-1", 1, _witness_streams(witness, pos_rot_on_look_y=True),
                       client="vanilla", build="v", witness=witness)
            _write_run(folder, "sunrise-1", 1, _witness_streams(witness, pos_rot_on_look_y=False),
                       client="sunrise", build="s", witness=witness)
            rep = diff.compare_look_runs(diff.runs_of(Path(folder), "vanilla"),
                                         diff.runs_of(Path(folder), "sunrise"), routes.MOUSE_LOOK)
            self.assertTrue(rep["bit_exact"], diff.describe_look(rep))
            self.assertEqual(rep["minecraft"], "26.3")
        # One ULP difference in a single witness yaw float must fail with zero tolerance.
        first_rot = witness["routes"]["look_x"]["rotations"][0]
        yaw_u32 = int(first_rot["yaw_bits"], 16)
        pitch_u32 = int(first_rot["pitch_bits"], 16)
        with tempfile.TemporaryDirectory() as folder:
            bad = _witness_streams(witness)
            bad[0][1][0] = [_rot_bits(yaw_u32 + 1, pitch_u32)]
            _write_run(folder, "sunrise-1", 1, bad, client="sunrise", build="s", witness=witness)
            rep = diff.compare_look_against_witness(diff.runs_of(Path(folder), "sunrise"), witness, "sunrise")
            self.assertFalse(rep["bit_exact"])
            self.assertEqual([d["route"] for d in rep["differences"]], ["look_x"])

    def test_26_3_double_turn_negative_zero_and_action_bits_fail_exact_comparison(self):
        witness = _load_witness()
        with tempfile.TemporaryDirectory() as folder:
            _write_run(folder, "sunrise-1", 1, _witness_streams(witness, scale=2.0),
                       client="sunrise", build="s", witness=witness)
            rep = diff.compare_look_against_witness(diff.runs_of(Path(folder), "sunrise"), witness, "sunrise")
            self.assertFalse(rep["bit_exact"])
            self.assertEqual([d["route"] for d in rep["differences"]], routes.MOUSE_LOOK)
        # Signed zero (-0.0 vs +0.0) differs in bit 31 and must fail raw bit comparison.
        with tempfile.TemporaryDirectory() as folder:
            bad = _witness_streams(witness)
            bad[0][1][-1] = [_rot(-0.0, 0.0)]
            _write_run(folder, "sunrise-1", 1, bad, client="sunrise", build="s", witness=witness)
            rep = diff.compare_look_against_witness(diff.runs_of(Path(folder), "sunrise"), witness, "sunrise")
            self.assertFalse(rep["bit_exact"])
            self.assertEqual([d["route"] for d in rep["differences"]], ["look_x"])
        # Look-dependent actions (use_item, use_item_on, player_action) compare exact fields and cursor float bits.
        regrab_last = witness["routes"]["regrab"]["rotations"][-1]
        yaw, pitch = regrab_last["yaw"], regrab_last["pitch"]
        one_ulp_pitch = struct.unpack(">f", struct.pack(">I", int(regrab_last["pitch_bits"], 16) + 1))[0]
        for label, act_a, act_b in (
            ("use_item", _use_item(0, 1, yaw, pitch), _use_item(0, 2, yaw, one_ulp_pitch)),
            ("use_item_on", _use_item_on(0, (3, 100, 165), 1, (0.5, 1.0, 0.25), False, False, 1),
             _use_item_on(0, (3, 100, 165), 1, (0.5, 1.0, 0.5), False, False, 1)),
            ("player_action", _player_action(0, (3, 101, 164), 2, 1), _player_action(0, (4, 101, 164), 2, 1)),
        ):
            with self.subTest(label), tempfile.TemporaryDirectory() as folder:
                _write_run(folder, "vanilla-1", 1, _witness_streams(witness, extra_action=act_a),
                           client="vanilla", build="v", witness=witness)
                _write_run(folder, "sunrise-1", 1, _witness_streams(witness, extra_action=act_b),
                           client="sunrise", build="s", witness=witness)
                rep = diff.compare_look_runs(diff.runs_of(Path(folder), "vanilla"),
                                             diff.runs_of(Path(folder), "sunrise"), routes.MOUSE_LOOK)
                self.assertFalse(rep["bit_exact"])
                self.assertEqual([d["route"] for d in rep["differences"]], ["regrab"])

    def test_26_3_missing_route_empty_rotations_diagnostic_and_fixture_mismatch_fail(self):
        witness = _load_witness()
        # Missing route fails.
        with tempfile.TemporaryDirectory() as folder:
            _write_run(folder, "vanilla-1", 1, _witness_streams(witness)[:3],
                       client="vanilla", build="v", witness=witness)
            _write_run(folder, "sunrise-1", 1, _witness_streams(witness)[:3],
                       client="sunrise", build="s", witness=witness)
            rep = diff.compare_look_runs(diff.runs_of(Path(folder), "vanilla"),
                                         diff.runs_of(Path(folder), "sunrise"), routes.MOUSE_LOOK)
            self.assertFalse(rep["bit_exact"])
            self.assertIn("route_missing", [p["kind"] for p in rep["problems"]])
        # Route with no rotation packets fails.
        with tempfile.TemporaryDirectory() as folder:
            empty = _witness_streams(witness)
            empty[3] = ("regrab", [[_c2s("move_player_status_only", b"\x01")]])
            _write_run(folder, "vanilla-1", 1, empty, client="vanilla", build="v", witness=witness)
            _write_run(folder, "sunrise-1", 1, empty, client="sunrise", build="s", witness=witness)
            rep = diff.compare_look_runs(diff.runs_of(Path(folder), "vanilla"),
                                         diff.runs_of(Path(folder), "sunrise"), routes.MOUSE_LOOK)
            self.assertFalse(rep["bit_exact"])
            self.assertIn("no_rotations", [p["kind"] for p in rep["problems"]])
        # Diagnostic (--mouse-scale != 1) and starting fixture differences fail.
        with tempfile.TemporaryDirectory() as folder:
            _write_run(folder, "sunrise-1", 1, _witness_streams(witness), client="sunrise", build="s",
                       witness=witness, diagnostic=True,
                       fixtures={"look_x": {"inventory": "other", "mode": "0", "effects": []}})
            rep = diff.compare_look_against_witness(diff.runs_of(Path(folder), "sunrise"), witness, "sunrise")
            self.assertFalse(rep["bit_exact"])
            kinds = {p["kind"] for p in rep["problems"]}
            self.assertIn("diagnostic", kinds)
            self.assertIn("fixture_differs", kinds)

    def test_26_3_witness_record_verify_roundtrip_and_cli_exit_codes(self):
        witness = _load_witness()
        with tempfile.TemporaryDirectory() as folder:
            runs = Path(folder) / "runs"
            _write_run(runs, "vanilla-1", 1, _witness_streams(witness, pos_rot_on_look_y=True),
                       client="vanilla", build="v", witness=witness)
            _write_run(runs, "vanilla-2", 2, _witness_streams(witness, pos_rot_on_look_y=False),
                       client="vanilla", build="v", witness=witness)
            rebuilt = diff.build_look_witness(
                diff.runs_of(runs, "vanilla"), routes.MOUSE_LOOK,
                recording_path=witness["recording"],
                recording_file_sha256=witness["recording_file_sha256"],
                java_version=witness["java_version"],
            )
            self.assertEqual(rebuilt["minecraft"], "26.3")
            self.assertEqual(rebuilt["runs_verified"], 2)
            self.assertEqual(rebuilt["java_version"], witness["java_version"])
            witness_path = Path(folder) / "witness-26.3.json"
            witness_path.write_text(json.dumps(rebuilt, indent=2), encoding="utf-8")

            _write_run(runs, "sunrise-1", 1, _witness_streams(witness), client="sunrise", build="s", witness=witness)
            _write_run(runs, "sunrise-2", 2, _witness_streams(witness), client="sunrise", build="s", witness=witness)
            with contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(harness.main(["--compare", str(runs), "--a", "vanilla", "--b", "sunrise"]), 0)
                self.assertEqual(harness.main(["--compare-witness", str(runs), "--client", "sunrise",
                                               "--witness", str(witness_path)]), 0)
                self.assertEqual(harness.main(["--compare-witness", str(runs), "--client", "sunrise",
                                               "--witness", str(WITNESS)]), 0)

            _write_run(runs, "bad-1", 1, _witness_streams(witness, scale=2.0),
                       client="sunrise", build="s", witness=witness)
            with contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(harness.main(["--compare-witness", str(runs), "--client", "bad",
                                               "--witness", str(WITNESS)]), 1)

            # Disagreeing vanilla runs are refused when recording a witness.
            _write_run(runs, "flaky-1", 1, _witness_streams(witness, scale=1.0),
                       client="vanilla", build="v", witness=witness)
            _write_run(runs, "flaky-2", 2, _witness_streams(witness, scale=2.0),
                       client="vanilla", build="v", witness=witness)
            with self.assertRaises(ValueError):
                diff.build_look_witness(diff.runs_of(runs, "flaky"), routes.MOUSE_LOOK)


if __name__ == "__main__":
    unittest.main()
