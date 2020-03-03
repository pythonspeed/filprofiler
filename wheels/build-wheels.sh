#!/bin/bash
set -euo pipefail
mkdir /tmp/home
mkdir /tmp/wheel
export HOME=/tmp/home
cd /src
mkdir -p dist
PATH=/opt/python/cp38-cp38/bin/:$PATH make clean
make filprofiler/libpymemprofile_api.so

PATH=/opt/python/cp36-cp36m/bin/:$PATH make filprofiler/_filpreload.so
/opt/python/cp36-cp36m/bin/pip wheel . -w /tmp/wheel
rm -f filprofiler/_filpreload.so

PATH=/opt/python/cp37-cp37m/bin/:$PATH make filprofiler/_filpreload.so
/opt/python/cp37-cp37m/bin/pip wheel . -w /tmp/wheel
rm -f filprofiler/_filpreload.so

PATH=/opt/python/cp38-cp38m/bin/:$PATH make filprofiler/_filpreload.so
/opt/python/cp38-cp38/bin/pip wheel . -w /tmp/wheel
rm -f filprofiler/_filpreload.so

auditwheel repair -w dist/ /tmp/wheel/filprofiler*36*whl
auditwheel repair -w dist/ /tmp/wheel/filprofiler*37*whl
auditwheel repair -w dist/ /tmp/wheel/filprofiler*38*whl
