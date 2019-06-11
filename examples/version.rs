//! A simple example that prints the version of chttp and libcurl being used.
//!
//! This example is useful to run on various systems when troubleshooting
//! version-related issues.

fn main() {
    println!("version: {}", chttp::version());
}
