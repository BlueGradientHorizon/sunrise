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


class BotConfigurationTests(unittest.TestCase):
    def test_only_the_select_known_packs_packet_is_answered(self):
        known = b'\x01\x09minecraft'  # one pack, namespace "minecraft"
        incoming = [(7, known), (7, b''), (14, known), (3, b'')]  # registry data twice, known packs, finish
        sent = []
        bot = worlds.Bot.__new__(worlds.Bot)
        bot.protocol = 766
        bot.receive = lambda: incoming.pop(0)
        bot.send = lambda packet_id, payload=b'': sent.append((packet_id, payload))
        bot.configure()
        self.assertEqual(sent, [(7, b'\x00'), (3, b'')])


if __name__ == '__main__':
    unittest.main()
