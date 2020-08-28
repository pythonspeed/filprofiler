"""Tests for trying to use IPython when you're not running via Fil."""

from IPython.core.interactiveshell import InteractiveShell
from IPython.utils.capture import capture_output
import sys


def test_helpful_error():
    """If you're not in Fil, you get an error message."""
    results = []
    sys.excepthook = results.append
    InteractiveShell.clear_instance()
    shell = InteractiveShell.instance()
    with capture_output() as results:
        shell.run_cell("%load_ext filprofiler")
    assert "UsageError" in results.stderr
    assert "Fil kernel" in results.stderr
    assert "Change Kernel" in results.stderr
    InteractiveShell.clear_instance()
