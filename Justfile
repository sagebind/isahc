version = `grep -m1 -e '^version' < Cargo.toml | sed 's/[^"]*"\(.*\)"/\1/'`

publish:
    cargo test
    git tag {{version}}
    git push origin {{version}}
    cargo publish
