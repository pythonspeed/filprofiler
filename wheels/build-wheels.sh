#!/bin/bash
set -euo pipefail

mkdir /tmp/home
mkdir /tmp/wheel
export HOME=/tmp/home

curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain stable -y
export PATH="$HOME/.cargo/bin:$PATH"

cd /src
mkdir -p dist


rm -f filprofiler/_filpreload.o
rm -f filprofiler/_filpreload*.so
rm -f filprofiler/_filpreload*.dylib
rm -rf build

for PYBIN in /opt/python/cp{37,38,39,310}*/bin; do
    touch filpreload/src/_filpreload.c  # force rebuild of Python code with new interpreter
    export PYO3_PYTHON="$PYBIN/python"
    "${PYBIN}/pip" install -U setuptools wheel setuptools-rust pip
    "${PYBIN}/python" -m pip wheel -w /tmp/wheel --use-feature=in-tree-build .
done

auditwheel repair --plat manylinux2010_x86_64 -w dist/ /tmp/wheel/filprofiler*cp37*whl
auditwheel repair --plat manylinux2010_x86_64 -w dist/ /tmp/wheel/filprofiler*cp38*whl
auditwheel repair --plat manylinux2010_x86_64 -w dist/ /tmp/wheel/filprofiler*cp39*whl
auditwheel repair --plat manylinux2010_x86_64 -w dist/ /tmp/wheel/filprofiler*cp310*whl

