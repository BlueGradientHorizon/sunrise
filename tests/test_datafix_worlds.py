"""Unit tests for tools/datafix_worlds.py: the Java runtime choice, the bot's configuration phase, the server stop."""
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

from tools import datafix_worlds as worlds


class JavaRuntimeTests(unittest.TestCase):
    def java_for(self, component, platform_name, home_name):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp, 'java', home_name)
            (home / 'bin').mkdir(parents=True)
            (home / 'bin' / 'java').write_text('')
            pin = worlds.LOCK[platform_name]['java-runtime-gamma']
            (home / '.complete').write_text(pin['manifest_sha1'])
            with mock.patch.object(worlds, 'LOCAL', Path(temp)), mock.patch.object(worlds, 'platform', return_value=platform_name), \
                    mock.patch.object(worlds, 'version_meta', return_value={'javaVersion': {'component': component}}), \
                    mock.patch.object(worlds, 'install_runtime', side_effect=AssertionError('no download')):
                return worlds.java('x'), home

    def test_a_component_no_platform_pins_is_an_error(self):
        with self.assertRaisesRegex(ValueError, 'java-runtime-zeta'):
            self.java_for('java-runtime-zeta', 'linux', 'java-runtime-gamma-linux')

    def test_a_component_this_platform_lacks_uses_gamma_as_the_lock_says(self):
        exe, home = self.java_for('jre-legacy', 'mac-os-arm64', 'java-runtime-gamma-mac-os-arm64')
        self.assertEqual(exe, str(home / 'bin' / 'java'))


class UnpinnedPlatformTests(unittest.TestCase):
    def test_a_platform_the_lock_does_not_pin_is_refused_by_name_before_any_download(self):
        for plat in ('windows-x64', 'mac-os'):
            with self.subTest(plat), mock.patch.object(worlds, 'platform', return_value=plat), \
                    mock.patch.object(worlds, 'version_meta', return_value={'javaVersion': {'component': 'java-runtime-gamma'}}), \
                    mock.patch.object(worlds, 'install_runtime', side_effect=AssertionError('no download')), \
                    self.assertRaisesRegex(ValueError, f'platform {plat};'):
                worlds.java('x')

    def test_platform_names_what_mojang_calls_the_machine(self):
        for system, machine, expected in (('win32', 'AMD64', 'windows-x64'), ('win32', 'ARM64', 'windows-arm64'),
                                          ('darwin', 'x86_64', 'mac-os'), ('darwin', 'arm64', 'mac-os-arm64'), ('linux', 'x86_64', 'linux')):
            with self.subTest(system, machine=machine), mock.patch.object(worlds, 'sys', SimpleNamespace(platform=system)), \
                    mock.patch.object(worlds, 'host', SimpleNamespace(machine=lambda: machine)):
                self.assertEqual(worlds.platform(), expected)


class ServerStopTests(unittest.TestCase):
    def test_a_server_that_ignores_stop_is_killed_and_the_timeout_reaches_the_caller(self):
        server = worlds.Server.__new__(worlds.Server)
        server.process = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)'], stdin=subprocess.PIPE, text=True)
        self.addCleanup(server.process.kill)
        real_wait = server.process.wait
        calls = []

        def wait(timeout=None):
            calls.append(timeout)
            if timeout == 120:
                raise subprocess.TimeoutExpired('server', 120)
            return real_wait()
        server.process.wait = wait
        with self.assertRaises(subprocess.TimeoutExpired):
            server.stop()
        self.assertIsNotNone(server.process.poll())
        self.assertEqual(calls, [120, None])


class BuildCleanupTests(unittest.TestCase):
    """build() with the server, the bot and the scene replaced: what is closed when something fails."""

    def build(self, scene_error, stop_error):
        events = []

        class FakeBot:
            def __init__(self, port, protocol, name):
                events.append('joined')

            def leave(self):
                events.append('left')

        class FakeServer:
            def __init__(self, *args):
                pass

            def stop(self):
                events.append('stopped')
                if stop_error:
                    raise stop_error

        def scene(server, run, Bot, *args):
            bot = Bot(1, 2, 'x')
            if scene_error:
                raise scene_error
            return bot

        with tempfile.TemporaryDirectory() as temp, mock.patch.object(worlds, 'LOCAL', Path(temp)), \
                mock.patch.object(worlds, 'server_jar', return_value=(Path('server.jar'), 3700, 770)), \
                mock.patch.object(worlds, 'java', return_value='java'), mock.patch.object(worlds, 'Server', FakeServer), \
                mock.patch.object(worlds, 'Bot', FakeBot), mock.patch.object(worlds, 'build_scene', scene), \
                mock.patch.object(worlds, 'time', SimpleNamespace(sleep=lambda seconds: None)):
            try:
                worlds.build('1.21.5')
            except Exception as error:
                return events, error
        return events, None

    def test_a_scene_that_fails_still_closes_the_bot_and_stops_the_server(self):
        events, error = self.build(ValueError('scene failed'), None)
        self.assertEqual((events, str(error)), (['joined', 'left', 'stopped'], 'scene failed'))

    def test_a_stop_that_raises_reaches_the_caller_and_the_bot_is_closed(self):
        events, error = self.build(None, RuntimeError('stop failed'))
        self.assertEqual((events, str(error)), (['joined', 'left', 'stopped'], 'stop failed'))


if __name__ == '__main__':
    unittest.main()
