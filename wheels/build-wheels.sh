#!/bin/bash
set -euo pipefail
mkdir /tmp/home
mkdir /tmp/wheel
export HOME=/tmp/home
cd /src
mkdir -p dist
PATH=/opt/python/cp38-cp38/bin/:$PATH make clean

wheels/build-wheel.sh 3.8
wheels/build-wheel.sh 3.6m
wheels/build-wheel.sh 3.7m

auditwheel repair -w dist/ /tmp/wheel/filprofiler*36*whl
auditwheel repair -w dist/ /tmp/wheel/filprofiler*37*whl
auditwheel repair -w dist/ /tmp/wheel/filprofiler*38*whl
