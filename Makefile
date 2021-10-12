.SHELLFLAGS := -eu -o pipefail -c
SHELL := bash
.DELETE_ON_ERROR:
MAKEFLAGS += --warn-undefined-variables
MAKEFLAGS += --no-builtin-rules

.PHONY: build
build:
	pip install -e .
	python setup.py install_data

target/release/libfilpreload.so: Cargo.lock memapi/Cargo.toml memapi/src/*.rs filpreload/src/*.rs filpreload/src/*.c
	cd filpreload && cargo build --release

venv:
	python3 -m venv venv/
	venv/bin/pip install --upgrade pip setuptools setuptools-rust

.PHONY: test
test:
	make test-rust
	make test-python

.PHONY: test-rust
test-rust:
	cd memapi && env RUST_BACKTRACE=1 cargo test
	cd filpreload && env RUST_BACKTRACE=1 cargo test --no-default-features

.PHONY: test-python
test-python: build
	make test-python-no-deps
	env RUST_BACKTRACE=1 py.test -v filprofiler/tests/
	flake8 filprofiler/

.PHONY: test-python-no-deps
test-python-no-deps:
	cythonize -3 -i tests/test-scripts/pymalloc.pyx
	c++ -shared -fPIC -lpthread tests/test-scripts/cpp.cpp -o tests/test-scripts/cpp.so
	cc -shared -fPIC -lpthread tests/test-scripts/malloc_on_thread_exit.c -o tests/test-scripts/malloc_on_thread_exit.so
	cd tests/test-scripts && python -m numpy.f2py -c fortran.f90 -m fortran
	env RUST_BACKTRACE=1 py.test tests/

.PHONY: docker-image
docker-image:
	docker build -t manylinux-rust -f wheels/Dockerfile.build .

.PHONY: wheel
wheel:
	python setup.py bdist_wheel

.PHONY: manylinux-wheel
manylinux-wheel:
	docker run -u $(shell id -u):$(shell id -g) -v $(PWD):/src quay.io/pypa/manylinux2010_x86_64:latest /src/wheels/build-wheels.sh

.PHONY: clean
clean:
	rm -rf target
	rm -rf filprofiler/*.so
	rm -rf filprofiler/*.dylib
	python setup.py clean

.PHONY: licenses
licenses:
	cd memapi && cargo lichking check
	cd memapi && cargo lichking bundle --file ../filprofiler/licenses.txt || true
	cat extra-licenses/APSL.txt >> filprofiler/licenses.txt

data_kernelspec/kernel.json: generate-kernelspec.py
	rm -rf data_kernelspec
	python generate-kernelspec.py

.PHONY: benchmark
benchmark:
	make benchmarks/results/*.json
	python setup.py --version > benchmarks/results/version.txt
	git diff --word-diff benchmarks/results/

.PHONY: benchmarks/results/pystone.json
benchmarks/results/pystone.json:
	_RJEM_MALLOC_CONF=dirty_decay_ms:-1,muzzy_decay_ms:-1,abort_conf:true FIL_NO_REPORT=1 FIL_BENCHMARK=benchmarks/results/pystone.json fil-profile run benchmarks/pystone.py

.PHONY: benchmarks/results/lots-of-peaks.json
benchmarks/results/lots-of-peaks.json:
	_RJEM_MALLOC_CONF=dirty_decay_ms:-1,muzzy_decay_ms:-1,abort_conf:true FIL_NO_REPORT=1 FIL_BENCHMARK=benchmarks/results/lots-of-peaks.json fil-profile run benchmarks/lots-of-peaks.py

.PHONY: benchmarks/results/multithreading-1.json
benchmarks/results/multithreading-1.json:
	cythonize -3 -i benchmarks/pymalloc.pyx
	_RJEM_MALLOC_CONF=dirty_decay_ms:-1,muzzy_decay_ms:-1,abort_conf:true FIL_NO_REPORT=1 FIL_BENCHMARK=benchmarks/results/multithreading-1.json fil-profile run benchmarks/multithreading.py 1
