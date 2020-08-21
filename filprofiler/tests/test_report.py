"""Unit tests for filprofiler._report."""

from os.path import dirname, join
from filprofiler._report import replace_code_references

FILE_PATH = join(dirname(__file__), "_replacementcode.py")


def test_replace_code_references():
    """
    replace_code_references() replaces special text with the Python code line.
    """
    s = f"""<title>hello</title>
    <title>TB@@{FILE_PATH}:1@@TB</title><more>TB@@{FILE_PATH}:4@@TB</more>"""
    assert (
        replace_code_references(s)
        == f'''<title>hello</title>
    <title>&#160;&#160;&#160;&#160;"""A file used by test_report."""</title><more>TB@@{FILE_PATH}:4@@TB</more>'''
    )


def test_replace_code_references_encoded():
    """
    replace_code_references() replaces special text with the Python code line,
    handling HTML-encoded paths.
    """
    file_path = "".join(f"&#{ord(c)};" for c in FILE_PATH)
    s = f"<title>TB@@{file_path}:1@@TB</title>"
    assert (
        replace_code_references(s)
        == f'''<title>&#160;&#160;&#160;&#160;"""A file used by test_report."""</title>'''
    )


def test_replace_code_references_quoting():
    """
    replace_code_references() XML-quotes the strings it inserts, and turns
    spaces at start to &nbsp;.
    """
    s = f"<title>TB@@{FILE_PATH}:5@@TB</title>"
    assert (
        replace_code_references(s)
        == "<title>&#160;&#160;&#160;&#160;return 1 &lt; 2</title>"
    )


def test_replace_code_references_unknown_file():
    """
    replace_code_references() inserts empty string for unknown files and lines
    that don't exist.
    """
    s = f"<title>TB@@nosuchfile.py:10000@@TB</title><title>TB@@{FILE_PATH}:10000@@TB</title>"
    assert (
        replace_code_references(s)
        == "<title>&#160;&#160;&#160;&#160;</title><title>&#160;&#160;&#160;&#160;</title>"
    )
