#!/bin/bash
set -euo pipefail
mkdir -p dist
rm -f filprofiler/fil-python

#docker run -u "$(id -u):$(id -g)" -v "${PWD}:/src" -w /src bitnami/python:3.6-debian-9 wheels/build-wheel.sh 3.6m
docker run -u "$(id -u):$(id -g)" -v "${PWD}:/src" -v "${HOME}/.cache/pip:/.cache/pip" -w /src bitnami/python:3.7-debian-9 wheels/build-wheel.sh 3.7m
docker run -u "$(id -u):$(id -g)" -v "${PWD}:/src" -v "${HOME}/.cache/pip:/.cache/pip" -w /src bitnami/python:3.8-debian-9 wheels/build-wheel.sh 3.8

pip3 install auditwheel
#auditwheel repair -w dist/ dist/filprofiler*36*whl
auditwheel repair -w dist/ dist/filprofiler*37*whl
auditwheel repair -w dist/ dist/filprofiler*38*whl
