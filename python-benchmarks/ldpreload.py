"""Validate that LD_PRELOAD is capturing certain APIs."""

from ctypes import c_void_p

from filprofiler._tracer import preload

from pymalloc import pymalloc, pyfree


address = pymalloc(365)
assert preload.pymemprofile_get_allocation_size(c_void_p(address)) == 365
pyfree(address)
result = preload.pymemprofile_get_allocation_size(c_void_p(address))
print("SIZE AFTER FREE", result)
# Might have ended up with memory being reused, but if so it's not going to be
# exactly 365 bytes.
assert result != 365
