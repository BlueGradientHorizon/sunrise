"""Behavior tests for join_announce, join_probe, net_capture hooks, and Game launch parameters."""
import argparse
import contextlib
import io
import json
import os
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path

from tools import join_announce, join_probe, net_capture, server_pack_e2e


NAMES = {
    ('handshake', 'serverbound'): ['intention'],
    ('login', 'clientbound'): ['login_disconnect', 'hello', 'login_finished', 'login_compression', 'custom_query'],
    ('login', 'serverbound'): ['hello', 'key', 'custom_query_answer', 'login_acknowledged'],
    ('configuration', 'clientbound'): ['custom_payload', 'disconnect', 'finish_configuration', 'keep_alive',
                                       'ping', 'store_cookie', 'cookie_request', 'select_known_packs', 'transfer'],
    ('configuration', 'serverbound'): ['client_information', 'cookie_response', 'custom_payload',
                                       'finish_configuration', 'keep_alive', 'pong', 'select_known_packs'],
}


class FrameReader:
    def __init__(self, sock):
        self.sock = sock
        self.buf = bytearray()

    def recv(self, threshold):
        while True:
            try:
                length, pos = net_capture.read_varint(self.buf, 0)
            except IndexError:
                length = pos = None
            if length is not None and len(self.buf) >= pos + length:
                body = bytes(self.buf[pos:pos + length])
                del self.buf[:pos + length]
                if threshold >= 0:
                    import zlib
                    size, p = net_capture.read_varint(body, 0)
                    body = zlib.decompress(body[p:]) if size else body[p:]
                pid, p = net_capture.read_varint(body, 0)
                return pid, body[p:]
            chunk = self.sock.recv(4096)
            if not chunk:
                raise EOFError('connection closed before full frame')
            self.buf.extend(chunk)


def free_local_port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        return sock.getsockname()[1]


class NetCaptureProbeHookTests(unittest.TestCase):
    def test_hooks_swallow_replies_inject_after_compression_and_record_errors(self):
        with tempfile.TemporaryDirectory() as directory:
            capture = Path(directory) / 'capture.jsonl'
            capture.touch()
            upstream_received = []
            upstream_ready = threading.Event()
            upstream_port = free_local_port()

            def run_upstream():
                with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as srv:
                    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
                    srv.bind(('127.0.0.1', upstream_port))
                    srv.listen(2)
                    srv.settimeout(5)
                    upstream_ready.set()
                    # Connection 1: normal handshake -> login_compression -> client answers injected custom_query
                    # (swallowed) -> client sends login_acknowledged (forwarded).
                    conn, _ = srv.accept()
                    with conn:
                        conn.settimeout(5)
                        reader = FrameReader(conn)
                        upstream_received.append(('c1_handshake', reader.recv(-1)))
                        # Send login_compression (id 3, threshold=16).
                        conn.sendall(join_probe.frame(-1, join_probe.varint(3) + join_probe.varint(16)))
                        # Expect ONLY login_acknowledged (id 3) compressed; custom_query_answer (id 2) must be swallowed.
                        upstream_received.append(('c1_after', reader.recv(16)))
                    # Connection 2: server sends corrupt zlib after compression to verify error event logging.
                    conn2, _ = srv.accept()
                    with conn2:
                        conn2.settimeout(5)
                        reader2 = FrameReader(conn2)
                        reader2.recv(-1)
                        conn2.sendall(join_probe.frame(-1, join_probe.varint(3) + join_probe.varint(1)))
                        bad_body = join_probe.varint(10) + b'not-zlib-bytes'
                        conn2.sendall(join_probe.varint(len(bad_body)) + bad_body)
                        time.sleep(0.1)

            t = threading.Thread(target=run_upstream, daemon=True)
            t.start()
            upstream_ready.wait(5)

            listen_port = free_local_port()
            steps = [(('login', 'login_compression'),
                      [('login', 'custom_query', join_probe.varint(7) + join_probe.string('probe:q') + b'hi')])]
            swallow_hook, after_forward = join_probe.make_hooks(steps, NAMES, pause=0)
            stop = net_capture.start_background(listen_port, f'127.0.0.1:{upstream_port}', capture, full=True,
                                                swallow=swallow_hook, after_forward=after_forward, names=NAMES)
            try:
                # Connection 1: handshake (intent=2 login) -> recv login_compression -> recv injected custom_query
                # framed under the new threshold 16 -> send custom_query_answer (swallowed) -> send login_acknowledged.
                with socket.create_connection(('127.0.0.1', listen_port), timeout=5) as cli:
                    cli.settimeout(5)
                    cli_reader = FrameReader(cli)
                    intention = (join_probe.varint(777) + join_probe.string('127.0.0.1')
                                 + struct.pack('>H', listen_port) + join_probe.varint(2))
                    cli.sendall(join_probe.frame(-1, join_probe.varint(0) + intention))
                    comp_id, comp_body = cli_reader.recv(-1)
                    self.assertEqual((comp_id, comp_body), (3, join_probe.varint(16)))
                    # Injected packet must arrive framed with threshold=16.
                    q_id, q_body = cli_reader.recv(16)
                    self.assertEqual(q_id, 4)
                    self.assertEqual(q_body, join_probe.varint(7) + join_probe.string('probe:q') + b'hi')
                    # Send custom_query_answer (id 2, swallowed) then login_acknowledged (id 3, forwarded).
                    cli.sendall(join_probe.frame(16, join_probe.varint(2) + join_probe.varint(7) + b'\x00'))
                    cli.sendall(join_probe.frame(16, join_probe.varint(3)))
                # Connection 2: trigger a zlib error on the clientbound pipe.
                with socket.create_connection(('127.0.0.1', listen_port), timeout=5) as cli2:
                    cli2.settimeout(5)
                    cli2_reader = FrameReader(cli2)
                    intention = (join_probe.varint(777) + join_probe.string('127.0.0.1')
                                 + struct.pack('>H', listen_port) + join_probe.varint(2))
                    cli2.sendall(join_probe.frame(-1, join_probe.varint(0) + intention))
                    cli2_reader.recv(-1)
                    time.sleep(0.2)
                t.join(5)
                time.sleep(0.1)
            finally:
                stop.set()
                time.sleep(0.6)

            self.assertEqual(upstream_received[1], ('c1_after', (3, b'')))
            entries = net_capture.load(capture)
            c1 = [e for e in entries if e.get('conn') == 1]
            self.assertTrue(any(e.get('injected') and e.get('name') == 'custom_query' for e in c1))
            self.assertTrue(any(e.get('dir') == 'c2s' and e.get('name') == 'custom_query_answer' for e in c1))
            c2_errors = [e for e in entries if e.get('conn') == 2 and e.get('event') == 'error']
            self.assertTrue(c2_errors, f'expected proxy error event in {entries}')


class JoinProbeTests(unittest.TestCase):
    def test_sign_protocol_ids_reads_prepared_reports_and_rejects_missing(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'facts.json').write_text(json.dumps({
                'states': [
                    ['minecraft:air', False, False, 'Block{minecraft:air}', {'id': 'minecraft:air', 'properties': {}}],
                    ['minecraft:oak_sign', False, False, 'rot1', {'id': 'minecraft:oak_sign', 'properties': {'rotation': '1', 'waterlogged': 'false'}}],
                    ['minecraft:oak_sign', False, False, 'rot0w', {'id': 'minecraft:oak_sign', 'properties': {'rotation': '0', 'waterlogged': 'true'}}],
                    ['minecraft:oak_sign', False, False, 'rot0', {'id': 'minecraft:oak_sign', 'properties': {'rotation': '0', 'waterlogged': 'false'}}],
                ],
            }), encoding='utf-8')
            (root / 'registries.json').write_text(json.dumps({
                'minecraft:block_entity_type': {'entries': {'minecraft:chest': {'protocol_id': 1},
                                                            'minecraft:sign': {'protocol_id': 9}}},
            }), encoding='utf-8')
            self.assertEqual(join_probe.sign_protocol_ids(root), (3, 9))

            (root / 'registries.json').write_text(json.dumps({'minecraft:block_entity_type': {'entries': {}}}),
                                                  encoding='utf-8')
            with self.assertRaises(ValueError):
                join_probe.sign_protocol_ids(root)

    def test_diff_pairs_by_probe_without_shifting_and_normalizes_transfer_port(self):
        with tempfile.TemporaryDirectory() as directory:
            a, b = Path(directory) / 'a', Path(directory) / 'b'
            a.mkdir()
            b.mkdir()
            ping1 = struct.pack('>i', 0x1234).hex()
            ping2 = struct.pack('>i', 0x5678).hex()
            t_a = (join_probe.string('127.0.0.1') + join_probe.varint(40001)).hex()
            t_b = (join_probe.string('127.0.0.1') + join_probe.varint(49999)).hex()
            # Recording b is missing the first ping probe; the second ping and transfer must still align cleanly.
            (a / 'all.json').write_text(json.dumps([
                {'dir': 's2c', 'state': 'configuration', 'name': 'ping', 'data': ping1, 'injected': True},
                {'dir': 'c2s', 'state': 'configuration', 'name': 'pong', 'data': ping1},
                {'dir': 's2c', 'state': 'play', 'name': 'ping', 'data': ping2, 'injected': True},
                {'dir': 'c2s', 'state': 'play', 'name': 'pong', 'data': ping2},
                {'dir': 's2c', 'state': 'configuration', 'name': 'transfer', 'data': t_a, 'injected': True},
            ]), encoding='utf-8')
            (b / 'all.json').write_text(json.dumps([
                {'dir': 's2c', 'state': 'play', 'name': 'ping', 'data': ping2, 'injected': True},
                {'dir': 'c2s', 'state': 'play', 'name': 'pong', 'data': ping2},
                {'dir': 's2c', 'state': 'configuration', 'name': 'transfer', 'data': t_b, 'injected': True},
            ]), encoding='utf-8')
            out = io.StringIO()
            with contextlib.redirect_stdout(out), self.assertRaises(SystemExit) as cm:
                join_probe.diff(argparse.Namespace(a=str(a), b=str(b), md=None))
            self.assertEqual(cm.exception.code, 1)
            table = out.getvalue()
            self.assertIn(f'| configuration/ping {ping1} | <- configuration/pong {ping1} | (missing probe) |', table)
            self.assertIn(f'| play/ping {ping2} | <- play/pong {ping2} | (same) |', table)
            self.assertIn('| configuration/transfer host=127.0.0.1 port=(proxy port) | nothing | (same) |', table)
            self.assertIn('| probe count | 3 | 2 |', table)

            # Matching transfer runs with different ports exit 0.
            (a / 'all.json').write_text(json.dumps([
                {'dir': 's2c', 'state': 'configuration', 'name': 'transfer', 'data': t_a, 'injected': True},
            ]), encoding='utf-8')
            (b / 'all.json').write_text(json.dumps([
                {'dir': 's2c', 'state': 'configuration', 'name': 'transfer', 'data': t_b, 'injected': True},
            ]), encoding='utf-8')
            out = io.StringIO()
            with contextlib.redirect_stdout(out), self.assertRaises(SystemExit) as cm:
                join_probe.diff(argparse.Namespace(a=str(a), b=str(b), md=None))
            self.assertEqual(cm.exception.code, 0)
            self.assertIn('(same)', out.getvalue())

    def test_describe_decodes_multibyte_varint_channel(self):
        long_channel = 'probe:' + 'a' * 140
        body = join_probe.string(long_channel) + b'\xde\xad'
        entry = {'state': 'configuration', 'name': 'custom_payload', 'data': body.hex()}
        self.assertEqual(join_probe.describe(entry), f'configuration/custom_payload channel={long_channel} body=dead')


class JoinAnnounceTests(unittest.TestCase):
    def test_describe_uses_varint_for_long_intention_address_and_channel(self):
        long_host = 'h' * 140 + '.example'
        intention = join_probe.varint(777) + join_probe.string(long_host) + struct.pack('>H', 25565) + join_probe.varint(2)
        self.assertEqual(
            join_announce.describe({'state': 'handshake', 'id': 0, 'name': 'intention', 'data': intention.hex()}),
            f'handshake/intention (id 0) protocol=777 address={long_host}',
        )
        long_channel = 'mod:' + 'c' * 140
        payload = join_probe.string(long_channel) + b'\x01\x02'
        self.assertEqual(
            join_announce.describe({'state': 'configuration', 'id': 2, 'name': 'custom_payload', 'data': payload.hex()}),
            f'configuration/custom_payload (id 2) channel={long_channel} body=0102',
        )

    @unittest.skipIf(os.name == 'nt', 'Xvfb -displayfd pipe uses POSIX select')
    def test_read_display_reads_number_and_refuses_dead_empty_or_stalled_xvfb(self):
        read_fd, write_fd = os.pipe()
        os.write(write_fd, b'42\n')
        os.close(write_fd)
        proc = subprocess.Popen([sys.executable, '-c', 'pass'])
        try:
            self.assertEqual(join_announce._read_display(read_fd, proc, timeout=2.0), ':42')
        finally:
            os.close(read_fd)
            proc.wait()

        # Immediate exit with no output must not hang or return ':'.
        read_fd, write_fd = os.pipe()
        proc = subprocess.Popen([sys.executable, '-c', 'import sys; sys.exit(1)'], pass_fds=(write_fd,))
        os.close(write_fd)
        try:
            with self.assertRaises(RuntimeError):
                join_announce._read_display(read_fd, proc, timeout=2.0)
        finally:
            os.close(read_fd)
            proc.wait()

        # Silent process must time out rather than block forever.
        read_fd, write_fd = os.pipe()
        proc = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(10)'], pass_fds=(write_fd,))
        os.close(write_fd)
        try:
            with self.assertRaises(RuntimeError):
                join_announce._read_display(read_fd, proc, timeout=0.25)
        finally:
            os.close(read_fd)
            join_announce.stop_process(proc)

    def test_output_folder_refuses_strays_and_cleans_owned_files(self):
        with tempfile.TemporaryDirectory() as directory:
            out = Path(directory) / 'out'
            out.mkdir()
            (out / 'capture.jsonl').write_text('old\n', encoding='utf-8')
            cleaned = join_announce.output_folder(out, ('capture.jsonl', 'serverbound.json'))
            self.assertEqual(cleaned, out)
            self.assertFalse((out / 'capture.jsonl').exists())

            (out / 'foreign.txt').write_text('keep\n', encoding='utf-8')
            with self.assertRaises(SystemExit):
                join_announce.output_folder(out, ('capture.jsonl',))
            self.assertTrue((out / 'foreign.txt').exists())

    def test_diff_dedups_play_packets_and_reports_exit_code(self):
        with tempfile.TemporaryDirectory() as directory:
            a, b = Path(directory) / 'a', Path(directory) / 'b'
            a.mkdir()
            b.mkdir()
            packets_a = [
                {'state': 'play', 'id': 1, 'name': 'client_tick_end', 'data': ''},
                {'state': 'play', 'id': 1, 'name': 'client_tick_end', 'data': ''},
            ]
            packets_b = [
                {'state': 'play', 'id': 1, 'name': 'client_tick_end', 'data': ''},
            ]
            (a / 'serverbound.json').write_text(json.dumps(packets_a), encoding='utf-8')
            (b / 'serverbound.json').write_text(json.dumps(packets_b), encoding='utf-8')
            out = io.StringIO()
            with contextlib.redirect_stdout(out), self.assertRaises(SystemExit) as cm:
                join_announce.diff(argparse.Namespace(a=str(a), b=str(b), md=None))
            self.assertEqual(cm.exception.code, 0)

    @unittest.skipIf(os.name == 'nt', 'stub sunrise-ctl shell script uses POSIX sh')
    def test_game_accepts_headless_mode_and_extra_env(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            bins = root / 'bins'
            bins.mkdir()
            recorded = root / 'recorded.json'
            ctl = bins / 'sunrise-ctl'
            ctl.write_text(
                f'#!/usr/bin/env python3\n'
                f'import json, os, sys\n'
                f'with open({str(recorded)!r}, "w", encoding="utf-8") as fh:\n'
                f'    fh.write(json.dumps({{"argv": sys.argv[1:], '
                f'"telemetry": os.environ.get("SUNRISE_TELEMETRY_URL"), '
                f'"home": os.environ.get("SUNRISE_HOME")}}))\n'
                f'print(json.dumps({{"pid": 0}}))\n',
                encoding='utf-8',
            )
            ctl.chmod(0o755)
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                game = join_announce.headless_game(bins, root / 'res', root / 'work')
            info = json.loads(recorded.read_text(encoding='utf-8'))
            self.assertIn('--headless', info['argv'])
            self.assertNotIn('--offscreen', info['argv'])
            self.assertEqual(info['telemetry'], 'off')
            self.assertEqual(info['home'], str(root / 'work' / 'home'))
            self.assertEqual(game.log, root / 'work' / 'home' / 'logs' / 'latest.log')


if __name__ == '__main__':
    unittest.main()
