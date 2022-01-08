use std::{env, error::Error};

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rustc-env=ISAHC_FEATURES={}", get_feature_string());

    Ok(())
}

/// Generate a "feature string" for the crate features currently enabled.
fn get_feature_string() -> String {
    env::vars()
        .filter(|(name, _)| name.starts_with("CARGO_FEATURE_"))
        .filter(|(_, value)| value == "1")
        .map(|(name, _)| name.trim_start_matches("CARGO_FEATURE_").to_lowercase())
        .map(|name| name.replace('_', "-"))
        .collect::<Vec<String>>()
        .join(",")
}
