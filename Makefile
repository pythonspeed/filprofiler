.SHELLFLAGS := -eu -o pipefail -c
SHELL := bash
.DELETE_ON_ERROR:
MAKEFLAGS += --warn-undefined-variables
MAKEFLAGS += --no-builtin-rules

.PHONY: build
build: filprofiler/_filpreload.so filprofiler/libpymemprofile_api.so build_ext

.PHONY: build_ext
build_ext: filprofiler/libpymemprofile_api.so
	python3.8 setup.py build_ext --inplace

filprofiler/_filpreload.so: filprofiler/_filpreload.c
	gcc -std=c11 -D_FORTIFY_SOURCE=2 -fasynchronous-unwind-tables -fstack-clash-protection -fstack-protector -Werror=format-security -Werror=implicit-function-declaration -O2 -shared -ldl -g -fPIC -fvisibility=hidden -Wall -o $@ $<

filprofiler/libpymemprofile_api.so: Cargo.lock memapi/Cargo.toml memapi/src/*.rs
	rm -f filprofiler/libymemprofile_api.so
	cargo build --release
	cp -f target/release/libpymemprofile_api.so filprofiler/

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
