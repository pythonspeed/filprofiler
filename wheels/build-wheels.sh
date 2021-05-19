#!/bin/bash
set -euo pipefail
mkdir /tmp/home
mkdir /tmp/wheel
export HOME=/tmp/home
cd /src
mkdir -p dist


rm -f filprofiler/_filpreload.o
rm -f filprofiler/_filpreload*.so
rm -f filprofiler/_filpreload*.dylib
rm -rf build

pip install setuptools_rust


for PYBIN in /opt/python/cp{36,37,38,39}*/bin; do
    "${PYBIN}/pip" install -U setuptools wheel setuptools-rust
    "${PYBIN}/python" setup.py bdist_wheel -d /tmp/wheel
done

auditwheel repair --plat manylinux2010_x86_64 -w dist/ /tmp/wheel/filprofiler*cp36*whl
auditwheel repair --plat manylinux2010_x86_64 -w dist/ /tmp/wheel/filprofiler*cp37*whl
auditwheel repair --plat manylinux2010_x86_64 -w dist/ /tmp/wheel/filprofiler*cp38*whl
auditwheel repair --plat manylinux2010_x86_64 -w dist/ /tmp/wheel/filprofiler*cp39*whl

