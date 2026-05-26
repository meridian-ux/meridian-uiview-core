//! Cargo build script: compile meridian.ui.v1 proto via prost-build.
//!
//! The Bazel build of meridian-uiview pulls
//! `//rust/uiview:uiview_proto_rust` (rules_rust_prost over
//! //proto:uiview_proto). Under cargo we have no rules_rust_prost,
//! so we run prost-build directly here and `include!()` the
//! generated `meridian.ui.v1.rs` from lib.rs.
//!
//! The .proto imports `google/api/field_behavior.proto`; we vendor
//! a minimal stub under `proto/vendor/` so prost-build's include
//! path resolves it without a googleapis-proto dep.

use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("../../proto");
    let proto_file = proto_root.join("uiview.proto");
    let vendor_root = proto_root.join("vendor");

    // Re-run when any of these change.
    println!("cargo:rerun-if-changed={}", proto_file.display());
    println!(
        "cargo:rerun-if-changed={}",
        vendor_root
            .join("google/api/field_behavior.proto")
            .display()
    );

    prost_build::Config::new()
        .compile_protos(&[proto_file], &[proto_root, vendor_root])?;

    Ok(())
}
