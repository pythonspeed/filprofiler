#!/bin/bash
set -euo pipefail

export PATH=/opt/python/cp36-cp36m/bin/:$PATH
make -e "PYTHON_VERSION=$1"
pip install -e .[test]
make test-python
pip wheel . -w /tmp/wheel
rm filprofiler/fil-python
