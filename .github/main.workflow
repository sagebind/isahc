workflow "main" {
  on = "push"
  resolves = ["test-stable", "test-nightly", "examples"]
}

workflow "release" {
  on = "release"
  resolves = ["publish"]
}

action "test-stable" {
  uses = "docker://rust:1.36"
  args = "cargo test"
}

action "test-nightly" {
  uses = "docker://rustlang/rust:nightly"
  args = "cargo test --features nightly"
}

action "examples" {
  uses = "docker://rust:1.36"
  args = "cargo run --release --example simple"
}

action "release-published" {
  uses = "dschep/filter-event-action@master"
  args = "event.action == 'published'"
}

action "publish" {
  needs = ["release-published"]
  uses = "docker://rust:1.36"
  args = ".github/publish.sh"
  secrets = ["CARGO_TOKEN"]
}
