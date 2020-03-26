.SHELLFLAGS := -eu -o pipefail -c
SHELL := bash
.DELETE_ON_ERROR:
MAKEFLAGS += --warn-undefined-variables
MAKEFLAGS += --no-builtin-rules

.PHONY: build
build: filprofiler/_filpreload.so
	pip install -e .

PYTHON_VERSION := 3.8

filprofiler/_filpreload.so: filprofiler/_filpreload.c target/release/libpymemprofile_api.a
	gcc -c -std=c11 -fPIC $(shell python$(PYTHON_VERSION)-config --cflags) -fno-omit-frame-pointer filprofiler/_filpreload.c
	mv -f _filpreload.o filprofiler/
	gcc -shared -ldl -lpthread -lc -lm -o $@ filprofiler/_filpreload.o target/release/libpymemprofile_api.a

target/release/libpymemprofile_api.a: Cargo.lock memapi/Cargo.toml memapi/src/*.rs
	cargo build --release

venv:
	python3 -m venv venv/
	venv/bin/pip install -e .[dev]

.PHONY: test
test:
	make test-rust
	make test-python

.PHONY: test-rust
test-rust:
	env RUST_BACKTRACE=1 cargo test

.PHONY: test-python
test-python: build
	cythonize -3 -i python-benchmarks/pymalloc.pyx
	env RUST_BACKTRACE=1 py.test

.PHONY: docker-image
docker-image:
	docker build -t manylinux-rust -f wheels/Dockerfile.build .

.PHONY: wheel
wheel:
	docker run -u $(shell id -u):$(shell id -g) -v $(PWD):/src manylinux-rust /src/wheels/build-wheels.sh

.PHONY: clean
clean:
	rm -f filprofiler/fil-python
	rm -rf target
	rm -rf filprofiler/*.so
	python setup.py clean
