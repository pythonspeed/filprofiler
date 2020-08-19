"""
IPython magic, specifically for Jupyter, allowing memory profiling from inside
a Jupyter notebook.
"""

from tempfile import mkdtemp
from pathlib import Path

from IPython.core.magic import Magics, magics_class, cell_magic
from IPython.display import SVG

from ._tracer import start_tracing, stop_tracing


@magics_class
class FilMagics(Magics):
    """Magics for memory profiling."""

    @cell_magic
    def filprofile(self, line, cell):
        """Memory profile the code in the cell."""
        tempdir = mkdtemp()
        start_tracing(tempdir)
        self.shell.run_cell(cell)
        index_html_path = stop_tracing(tempdir)
        svg_path = Path(index_html_path).parent / "peak-memory.svg"
        self.shell.display(SVG(filename=svg_path))
