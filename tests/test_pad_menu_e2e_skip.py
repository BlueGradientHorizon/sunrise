"""pad_menu_e2e reports a case the window can't run as skipped, apart from passes and failures."""

import contextlib
import io
import sys
import unittest

# pad_menu_e2e puts tools/ first on sys.path, where same-named parity scripts would shadow tests/ at discovery.
_path = list(sys.path)
try:
    from tools import pad_menu_e2e
finally:
    sys.path[:] = _path


def passes(args, name):
    return []


def fails(args, name):
    return ['the focus went nowhere']


def skips(args, name):
    raise pad_menu_e2e.Skipped('every page fits at [468.0, 509.0] GUI px: the list never scrolls in this window')


CASES = {'passes': passes, 'fails': fails, 'skips': skips}


def run(names):
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        status = pad_menu_e2e.run_cases(CASES, names, 1, None)
    return status, out.getvalue().splitlines()


class SkippedCases(unittest.TestCase):
    def test_a_skip_beside_a_pass_is_counted_apart_and_the_run_passes(self):
        status, lines = run(['passes', 'skips'])
        self.assertEqual(status, 0)
        self.assertIn('ok   passes', lines)
        self.assertIn('skip skips: every page fits at [468.0, 509.0] GUI px: the list never scrolls in this window',
                      lines)
        self.assertEqual(lines[-1], '1 ok, 0 failed, 1 skipped (skips)')

    def test_a_failure_beside_a_skip_fails_the_run(self):
        status, lines = run(['skips', 'fails'])
        self.assertEqual(status, 1)
        self.assertIn('FAIL fails: the focus went nowhere', lines)
        self.assertEqual(lines[-1], '0 ok, 1 failed, 1 skipped (skips)')

    def test_a_run_where_every_case_skipped_does_not_pass(self):
        status, lines = run(['skips'])
        self.assertEqual(status, 2)
        self.assertEqual(lines[-2:], ['0 ok, 0 failed, 1 skipped (skips)', 'nothing ran'])


if __name__ == '__main__':
    unittest.main()
