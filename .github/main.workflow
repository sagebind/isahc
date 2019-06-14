workflow "main" {
  on = "push"
  resolves = ["test", "examples"]
}

workflow "release" {
  on = "release"
  resolves = ["publish"]
}

action "test" {
  uses = "docker://rust"
  args = "cargo test"
}

action "examples" {
  uses = "docker://rust"
  args = "cargo run --release --example simple"
}

action "publish" {
  needs = ["test", "examples"]
  uses = "docker://rust"
  args = ".github/publish.sh"
  secrets = ["CARGO_TOKEN"]
}
