#!/bin/bash
set -euo pipefail
wget https://github.com/rust-lang/mdBook/releases/download/v0.4.12/mdbook-v0.4.12-x86_64-unknown-linux-gnu.tar.gz
tar xvfz mdbook*.tar.gz
chmod +x mdbook
wget https://github.com/badboy/mdbook-toc/releases/download/0.7.0/mdbook-toc-0.7.0-x86_64-unknown-linux-musl.tar.gz
tar xvfz mdbook-toc*.tar.gz
chmod +x mdbook-toc
export PATH=$PATH:$PWD
mdbook build
