"""Tests for filprofiler._script."""

from .._script import PARSER
from .._utils import _parse_glibc_version


def test_command_line_past_run():
    """
    When doing `fil-profile run`, all command-line arguments after the script
    can be passed on.
    """

    def passthrough_args(*args):
        args = ["run"] + list(args)
        return PARSER.parse_args(args).rest

    assert passthrough_args("script.py", "-o", "123") == ["script.py", "-o", "123"]
    assert passthrough_args("script.py", "-o", "123", "-m", "xxx") == [
        "script.py",
        "-o",
        "123",
        "-m",
        "xxx",
    ]
    assert passthrough_args("script.py", "--xxx=1", "-o", "2") == [
        "script.py",
        "--xxx=1",
        "-o",
        "2",
    ]
    assert passthrough_args("-m", "package", "-o", "123") == [
        "-m",
        "package",
        "-o",
        "123",
    ]


def test_parse_glibc_version():
    """
    Bad glibc versions can be handled (#277).
    """
    assert _parse_glibc_version(b"2.3") == (2, 3)
    assert _parse_glibc_version(b"2.12") == (2, 12)
    # For unparseable versions, just returned super-low version so we fallback
    # to LD_PRELOAD.
    assert _parse_glibc_version(b"2.3-blahblah") == (1, 1)
    assert _parse_glibc_version(b"blahblah") == (1, 1)
