workflow "main" {
  on = "push"
  resolves = ["test"]
}

workflow "release" {
  on = "release"
  resolves = ["publish"]
}

action "test" {
  uses = "docker://rust"
  args = "cargo test"
}

action "publish" {
  needs = "test"
  uses = "docker://rust"
  args = ".github/publish.sh"
  secrets = ["CARGO_TOKEN"]
}
