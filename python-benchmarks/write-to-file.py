#!/usr/bin/env python
"""Just write an argument to a file."""

import sys

url, path = sys.argv[1:]
with open(path, "w") as f:
    f.write(url)
