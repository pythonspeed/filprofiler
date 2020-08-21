"""
IPython magic, specifically for Jupyter, allowing memory profiling from inside
a Jupyter notebook.
"""

from pathlib import Path
from textwrap import indent
from contextlib import contextmanager

from IPython.core.magic import Magics, magics_class, cell_magic
from IPython.display import IFrame, display

from ._tracer import start_tracing, stop_tracing


# We use variable that is unlikely to conflict with user code.
TEMPLATE = """\
from filprofiler._ipython import run_with_profile as __arghbldsada__
with __arghbldsada__():
{}
del __arghbldsada__
"""


@magics_class
class FilMagics(Magics):
    """Magics for memory profiling."""

    @cell_magic
    def filprofile(self, line, cell):
        """Memory profile the code in the cell."""
        # We use a template that does the Fil setup inside the cell, rather
        # than here, so as to keep a whole pile of irrelevant IPython code
        # appearing as frames at the top of the memory profile flamegraph.
        self.shell.run_cell(TEMPLATE.format(indent(cell, "    ")))


@contextmanager
def run_with_profile():
    """Run some code under Fil, display result."""
    tempdir = "fil-result"
    start_tracing(tempdir)
    try:
        yield
    finally:
        index_html_path = stop_tracing(tempdir)
        svg_path = Path(index_html_path).parent / "peak-memory.svg"
        display(IFrame(svg_path, width="100%", height="600"))
