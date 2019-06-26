workflow "main" {
  on = "push"
  resolves = ["test", "examples"]
}

workflow "release" {
  on = "release"
  resolves = ["publish"]
}

action "test" {
  uses = "docker://rustlang/rust:nightly"
  args = "cargo test"
}

action "examples" {
  uses = "docker://rustlang/rust:nightly"
  args = "cargo run --release --example simple"
}

action "release-published" {
  uses = "dschep/filter-event-action@master"
  args = "event.action == 'published'"
}

action "publish" {
  needs = ["release-published"]
  uses = "docker://rustlang/rust:nightly"
  args = ".github/publish.sh"
  secrets = ["CARGO_TOKEN"]
}
