"""What the operating system running a test allows, for tests whose behaviour depends on it.

A test checks a capability here, never a platform name: Windows lets a process create a symbolic
link only with Developer Mode on or as an administrator, and macOS and Linux always do.
"""
import functools
import os
import tempfile
import unittest


@functools.cache
def can_symlink():
    """Whether this process may create a symbolic link."""
    with tempfile.TemporaryDirectory() as directory:
        try:
            os.symlink(directory, os.path.join(directory, "link"), target_is_directory=True)
        except OSError:
            return False
    return True


needs_runner_shell = unittest.skipUnless(os.name == "posix", "runs a Linux runner's workflow step")
"""Skips a test that runs a workflow step or a `tools/ci/*.sh` script with bash: those are Linux runner scripts.

Windows has no bash that sees its paths and carries the runner's tools (WSL's cannot read `C:\\` paths), so the
test cannot pass there for a reason that is no bug; macOS and Linux run it.
"""


def needs_symlinks(test):
    """Skips `test` (a test method or class) where this process may not create a symbolic link."""
    return unittest.skipUnless(can_symlink(), "this system does not allow creating symbolic links")(test)
