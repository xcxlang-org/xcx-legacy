#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub fn fixture_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(relative)
}

pub fn fixture_source(relative: &str) -> String {
    std::fs::read_to_string(fixture_path(relative)).expect("fixture must be readable")
}
