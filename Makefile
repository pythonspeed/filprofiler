.SHELLFLAGS := -eu -o pipefail -c
SHELL := bash
.DELETE_ON_ERROR:
MAKEFLAGS += --warn-undefined-variables
MAKEFLAGS += --no-builtin-rules

.PHONY: build
build: filprofiler/_filpreload.so filprofiler/libpymemprofile_api.so

filprofiler/_filpreload.so: filprofiler/_filpreload.c
	gcc -std=c11 -D_FORTIFY_SOURCE=2 -fasynchronous-unwind-tables -fstack-clash-protection -fstack-protector -Werror=format-security -Werror=implicit-function-declaration -O2 -shared -ldl -g -fPIC -fvisibility=hidden -Wall -o $@ $<

filprofiler/libpymemprofile_api.so: Cargo.lock memapi/Cargo.toml memapi/src/*.rs
	rm -f filprofiler/libymemprofile_api.so
	cargo build
	cp -f target/debug/libpymemprofile_api.so filprofiler/

test: build
	env RUST_BACKTRACE=1 PYTHONMALLOC=malloc LD_PRELOAD=./libpymemprofile_preload.so python3.8 example.py
