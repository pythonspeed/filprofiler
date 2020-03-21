.SHELLFLAGS := -eu -o pipefail -c
SHELL := bash
.DELETE_ON_ERROR:
MAKEFLAGS += --warn-undefined-variables
MAKEFLAGS += --no-builtin-rules

.PHONY: build
build: filprofiler/fil-python
	pip install -e .

filprofiler/fil-python: filprofiler/_filpreload.c target/release/libpymemprofile_api.a
	gcc -std=c11 -g $(shell python3.8-config --cflags --ldflags) -O3 -lpython3.8 -export-dynamic -flto -o $@ $< ./target/release/libpymemprofile_api.a

target/release/libpymemprofile_api.a: Cargo.lock memapi/Cargo.toml memapi/src/*.rs
	cargo build --release

venv:
	python3 -m venv venv/
	venv/bin/pip install -e .[dev]

test: build
	cythonize -3 -i python-benchmarks/pymalloc.pyx
	env RUST_BACKTRACE=1 cargo test
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
