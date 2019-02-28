#!/bin/sh
set -eu

cargo login ${CARGO_TOKEN}
cargo publish
