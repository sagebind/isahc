//! A simple example that prints the version of isahc and libcurl being used.
//!
//! This example is useful to run on various systems when troubleshooting
//! version-related issues.

fn main() {
    println!("version: {}", isahc::version());
}
