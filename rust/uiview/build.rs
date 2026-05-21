use std::env;
use std::path::PathBuf;

// Builds Rust types for meridian.ui.v1 from the proto source in
// ../../proto/uiview.proto.
//
// Generated types derive `serde::Serialize` + `serde::Deserialize` so
// JSON-shaped descriptor objects from the TS / wasm boundary
// round-trip into typed prost messages without a manual encoder.
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

    let mut config = prost_build::Config::new();
    // Add serde derives to every generated type so TS hosts can
    // pass JSON-shaped descriptors / requests / responses across
    // the wasm boundary and have them deserialize to typed prost
    // messages.
    config.type_attribute(
        ".",
        "#[derive(::serde::Serialize, ::serde::Deserialize)]",
    );
    // Skip the field-behavior extension on FieldOptions; serde
    // doesn't need to round-trip the descriptor proto itself.
    config
        .compile_protos(
            &[proto.to_str().unwrap()],
            &[
                proto_include.to_str().unwrap(),
                third_party.to_str().unwrap(),
            ],
        )
        .expect("prost-build failed");
}
