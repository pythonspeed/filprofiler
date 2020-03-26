#!/bin/bash
set -euo pipefail

mkdir -p test
make -e "PYTHON_VERSION=$1" filprofiler/fil-python
python$1 -m venv /tmp/venv
. /tmp/venv/bin/activate
python -m pip install --no-cache-dir .[dev]
make test-python
python -m pip wheel --no-cache-dir . -w dist
rm filprofiler/fil-python
