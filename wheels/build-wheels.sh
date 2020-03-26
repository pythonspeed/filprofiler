#!/bin/bash
set -euo pipefail
mkdir /tmp/home
mkdir /tmp/wheel
export HOME=/tmp/home
cd /src
mkdir -p dist
make target/release/libpymemprofile_api.a

rm -f filprofiler/_filpreload.so

PATH=/opt/python/cp36-cp36m/bin/:$PATH make -e PYTHON_VERSION=3.6 filprofiler/_filpreload.so
/opt/python/cp36-cp36m/bin/python3 setup.py bdist_wheel -d /tmp/wheel
rm -f filprofiler/_filpreload.so

PATH=/opt/python/cp37-cp37m/bin/:$PATH make -e PYTHON_VERSION=3.7 filprofiler/_filpreload.so
/opt/python/cp37-cp37m/bin/python3 setup.py bdist_wheel -d /tmp/wheel
rm -f filprofiler/_filpreload.so

PATH=/opt/python/cp38-cp38/bin/:$PATH make -e PYTHON_VERSION=3.8 filprofiler/_filpreload.so
/opt/python/cp38-cp38/bin/python3 setup.py bdist_wheel -d /tmp/wheel
rm -f filprofiler/_filpreload.so

auditwheel addtag -w dist/ /tmp/wheel/filprofiler*36*whl
auditwheel addtag -w dist/ /tmp/wheel/filprofiler*37*whl
auditwheel addtag -w dist/ /tmp/wheel/filprofiler*38*whl
