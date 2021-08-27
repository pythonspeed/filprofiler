#!/bin/bash
set -euo pipefail
wget https://github.com/rust-lang/mdBook/releases/download/v0.4.12/mdbook-v0.4.12-x86_64-unknown-linux-gnu.tar.gz
tar xvfz mdbook*.tar.gz
chmod +x mdbook
./mdbook build
