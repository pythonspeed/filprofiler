import os, signal
import numpy

# This is peak:
x = numpy.ones((200 * 1024 * 1024), dtype=numpy.int8)
del x
# Below peak, but will be present when we run out of memory;
# we expect this to be dumped, not the deleted allocation:
x = numpy.ones((100 * 1024 * 1024), dtype=numpy.int8)

# Trigger a MemoryError:
toobig = numpy.ones((1024, 1024 * 1024, 1024 * 1024), dtype=numpy.int8)
