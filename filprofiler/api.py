"""
Public API for Fil.

In order to use this you need to run your program with:

    $ filprofiler python yourprogram.py

instead of:

    $ python yourprogram.py
"""

from contextlib import contextmanager, AbstractContextManager
from typing import Union
from pathlib import Path

from ._tracer import start_tracing, stop_tracing, disable_thread_pools


@contextmanager
def profile(path: Union[str, Path]) -> AbstractContextManager:
    """
    Context manager that profiles memory and dumps the result to the given
    path.
    """
    start_tracing(path)
    with disable_thread_pools():
        try:
            yield
        finally:
            stop_tracing(path)


__all__ = ["profile"]
