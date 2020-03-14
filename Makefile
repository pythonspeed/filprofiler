.SHELLFLAGS := -eu -o pipefail -c
SHELL := bash
.DELETE_ON_ERROR:
MAKEFLAGS += --warn-undefined-variables
MAKEFLAGS += --no-builtin-rules

.PHONY: build
build: filprofiler/fil-python build_ext

.PHONY: build_ext
build_ext: filprofiler/libpymemprofile_api.so
	env CFLAGS=-fno-omit-frame-pointer python3.8 setup.py build_ext --inplace

filprofiler/fil-python: filprofiler/_filpreload.c target/release/libpymemprofile_api.a
	gcc -std=c11 $(shell python3.8-config --cflags --ldflags) -lpython3.8 -export-dynamic -flto -o $@ $< ./target/release/libpymemprofile_api.a

target/release/libpymemprofile_api.a: Cargo.lock memapi/Cargo.toml memapi/src/*.rs
	cargo build --release

test: build
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
	rm -rf target
	rm -rf filprofiler/*.so
	python setup.py clean
