"""
Public API for Fil.

In order to use this you need to run your program with:

    $ filprofiler python yourprogram.py

instead of:

    $ python yourprogram.py
"""

# Design invariant: this should be importable without causing exceptions, even
# if Fil won't work. As such, all imports of ._tracer should not happen at
# module level.

from typing import Union, Callable, TypeVar
from pathlib import Path

_T = TypeVar("_T")


def profile(code_to_profile: Callable[[], _T], path: Union[str, Path]) -> _T:
    """
    Context manager that profiles memory and dumps the result to the given
    path.
    """
    from ._tracer import (
        start_tracing,
        stop_tracing,
        disable_thread_pools,
        check_if_fil_preloaded,
    )

    # First, make sure Fil library was preloaded. If not, we want to get a nice
    # error message.
    check_if_fil_preloaded()

    # Next, do what we're supposed to do:
    start_tracing(path)
    with disable_thread_pools():
        try:
            return code_to_profile()
        finally:
            stop_tracing(path)


__all__ = ["profile"]
