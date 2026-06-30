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
    let vendor_root = proto_root.join("vendor");

    // Proto sources, matching proto/BUILD.bazel's `proto_library(srcs = [...])`.
    // v0.2.0 split the single uiview.proto into one file per concept; this
    // list must stay in lock-step with the Bazel BUILD or prost-build won't
    // pick up new messages.
    let srcs = [
        "rpc.proto",
        "form.proto",
        "table.proto",
        "gallery.proto",
        "lro.proto",
        "prompt.proto",
        "llm_prompt.proto",
        "panel.proto",
    ];
    let proto_files: Vec<PathBuf> = srcs.iter().map(|n| proto_root.join(n)).collect();

    for p in &proto_files {
        println!("cargo:rerun-if-changed={}", p.display());
    }
    println!(
        "cargo:rerun-if-changed={}",
        vendor_root
            .join("google/api/field_behavior.proto")
            .display()
    );

    // The .protos use `import "proto/<name>.proto"` form (matching
    // Bazel's workspace-relative resolution). So protoc's include
    // path needs to be the workspace root (one level above proto/),
    // not proto/ itself. Vendor stays as a separate include for the
    // google/api/field_behavior import.
    let workspace_root = proto_root
        .parent()
        .expect("proto/ must have a parent")
        .to_path_buf();

    prost_build::Config::new()
        .compile_protos(&proto_files, &[workspace_root, vendor_root])?;

    Ok(())
}
