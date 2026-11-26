"""Unit tests for tools/datafix_worlds.py: the Java runtime choice, the bot's configuration phase, the server stop."""
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
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


if __name__ == '__main__':
    unittest.main()
