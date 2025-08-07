#!/usr/bin/env sh

set -o errexit -o nounset

for i in $(seq 100); do
  echo "

---------------- CYCLE $i BEGINS ----------------

"
  cargo test --test segfault -- --nocapture
done
