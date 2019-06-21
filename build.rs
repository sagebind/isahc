use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    fs::write(out_dir.join("features.txt"), get_feature_string())?;

    Ok(())
}

/// Generate a "feature string" for the crate features currently enabled.
fn get_feature_string() -> String {
    env::vars()
        .filter(|(name, _)| name.starts_with("CARGO_FEATURE_"))
        .filter(|(_, value)| value == "1")
        .map(|(name, _)| name.trim_start_matches("CARGO_FEATURE_").to_lowercase())
        .collect::<Vec<String>>()
        .join(",")
}
