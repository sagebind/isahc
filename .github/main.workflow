workflow "Main" {
  on = "push"
  resolves = ["Test"]
}

action "Test" {
  uses = "docker://rust"
  args = "cargo test"
}
