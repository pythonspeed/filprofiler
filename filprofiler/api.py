"""
Public API for Fil.

In order to use this you need to run your program with:

    $ filprofiler python yourprogram.py

instead of:

    $ python yourprogram.py
"""

from typing import Union, Callable, TypeVar
from pathlib import Path

from ._tracer import start_tracing, stop_tracing, disable_thread_pools


_T = TypeVar("_T")


def profile(code_to_profile: Callable[[], _T], path: Union[str, Path]) -> _T:
    """
    Context manager that profiles memory and dumps the result to the given
    path.
    """
    start_tracing(path)
    with disable_thread_pools():
        try:
            return code_to_profile()
        finally:
            stop_tracing(path)


__all__ = ["profile"]
