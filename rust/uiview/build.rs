use std::env;
use std::path::PathBuf;

// Builds Rust types for meridian.ui.v1 from the proto source in
// ../../proto/uiview.proto. The generated module lives at
// OUT_DIR/meridian.ui.v1.rs and is included via src/proto.rs.
//
// `field_behavior.proto` is imported by uiview.proto for the
// REQUIRED / OPTIONAL annotations; prost-build needs the
// googleapis dep on its include path. We rely on a vendored
// copy under proto_third_party/ so the build doesn't need
// network access.
fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace = manifest_dir.parent().unwrap();
    let repo = workspace.parent().unwrap();

    let proto = repo.join("proto/uiview.proto");
    let proto_include = repo.join("proto");
    let third_party = manifest_dir.join("proto_third_party");

    println!("cargo:rerun-if-changed={}", proto.display());
    println!(
        "cargo:rerun-if-changed={}",
        third_party
            .join("google/api/field_behavior.proto")
            .display()
    );

    prost_build::Config::new()
        .compile_protos(
            &[proto.to_str().unwrap()],
            &[
                proto_include.to_str().unwrap(),
                third_party.to_str().unwrap(),
            ],
        )
        .expect("prost-build failed");
}
