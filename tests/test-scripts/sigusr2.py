"""Send SIGUSR2 to this process to trigger dumps."""

import os
import signal
import time
import numpy

data1 = numpy.ones((1024, 1024, 20), dtype=numpy.uint8)
os.kill(os.getpid(), signal.SIGUSR2)
time.sleep(0.5)
data2 = numpy.ones((1024, 1024, 50), dtype=numpy.uint8)
