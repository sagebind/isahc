workflow "main" {
  on = "push"
  resolves = ["test-stable", "test-nightly", "examples"]
}

workflow "release" {
  on = "release"
  resolves = ["publish"]
}

action "checkout-submodules" {
  uses = "textbook/git-checkout-submodule-action@master"
}

action "test-stable" {
  needs = ["checkout-submodules"]
  uses = "docker://rust:1.36"
  args = "cargo test --features psl"
}

action "test-nightly" {
  needs = ["checkout-submodules"]
  uses = "docker://rustlang/rust:nightly"
  args = "cargo test --features psl,nightly"
}

action "examples" {
  needs = ["checkout-submodules"]
  uses = "docker://rust:1.36"
  args = "cargo run --release --example simple"
}

action "release-published" {
  uses = "dschep/filter-event-action@master"
  args = "event.action == 'published'"
}

action "publish" {
  needs = ["checkout-submodules", "release-published"]
  uses = "docker://rust:1.36"
  args = ".github/publish.sh"
  secrets = ["CARGO_TOKEN"]
}
