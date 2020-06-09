"""Unit tests for filprofiler._report."""

from os.path import dirname, join
from filprofiler._report import replace_code_references

FILE_PATH = join(dirname(__file__), "_replacementcode.py")


def test_replace_code_references():
    """
    replace_code_references() replaces special text with the Python code line.
    """
    s = f"""<text>hello</text>
    <text>TB@@{FILE_PATH}:1@@TB</text><more>TB@@{FILE_PATH}:4@@TB</more>"""
    assert (
        replace_code_references(s)
        == '''<text>hello</text>
    <text>&#160;&#160;&#160;&#160;"""A file used by test_report."""</text><more>&#160;&#160;&#160;&#160;def i_am_function():</more>'''
    )


def test_replace_code_references_quoting():
    """
    replace_code_references() XML-quotes the strings it inserts, and turns
    spaces at start to &nbsp;.
    """
    s = f"<text>TB@@{FILE_PATH}:5@@TB</text>"
    assert (
        replace_code_references(s)
        == "<text>&#160;&#160;&#160;&#160;return 1 &lt; 2</text>"
    )


def test_replace_code_references_unknown_file():
    """
    replace_code_references() inserts empty string for unknown files and lines
    that don't exist.
    """
    s = f"<x>TB@@nosuchfile.py:10000@@TB</x><y>TB@@{FILE_PATH}:10000@@TB</y>"
    assert (
        replace_code_references(s)
        == "<x>&#160;&#160;&#160;&#160;</x><y>&#160;&#160;&#160;&#160;</y>"
    )
