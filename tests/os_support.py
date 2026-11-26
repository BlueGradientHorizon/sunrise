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


def needs_symlinks(test):
    """Skips `test` (a test method or class) where this process may not create a symbolic link."""
    return unittest.skipUnless(can_symlink(), "this system does not allow creating symbolic links")(test)


# A CI workflow step's bash script is written for a Linux runner's tools and paths: on Windows `bash` is WSL's (it
# cannot see `C:\` paths) or Git Bash, so a test that runs one cannot pass there for any fault of the code.
runs_linux_runner_step = unittest.skipUnless(os.name == "posix", "runs a Linux runner's workflow step")
