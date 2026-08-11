//! Guards the committed descriptor set that the cargo build generates from.
//!
//! `proto/gen/meridian_ui_v1.binpb` is a checked-in copy of what
//! `@meridian_schemas//proto:uiview_proto` produces. A copy can rot, and when it
//! does the symptom is not a build failure — it is cargo and Bazel silently
//! generating *different* Rust types from the same crate. That is the same class
//! of bug as meridian-k8s's vendored proto tree, which drifted from 0.5.0 while
//! canonical moved on and lost `Slot.sub_view` without anything noticing.
//!
//! What this can and cannot check, stated honestly:
//!   * ✅ the set decodes, is self-contained, and carries every file the
//!     `uiview_proto` proto_library declares;
//!   * ✅ the fields this crate's code actually reads are present;
//!   * ❌ byte-freshness against meridian-schemas HEAD. That comparison needs the
//!     source protos, which live in another repository — so it belongs in a Bazel
//!     test against `@meridian_schemas//proto:uiview_proto`, the way
//!     meridian-proto's `//proto/gen:descriptor_freshness_test` does it. Owed;
//!     see tools/regen_descriptors.sh.

use prost::Message as _;
use prost_types::FileDescriptorSet;

const FDS: &[u8] = include_bytes!("../proto/gen/meridian_ui_v1.binpb");

/// The 27 files of `//proto:uiview_proto`. Listed so that adding a proto upstream
/// without regenerating fails here rather than at some renderer months later.
const EXPECTED: &[&str] = &[
    "affordance", "catalog", "choice", "command_palette", "connect_flow",
    "conversation", "copy_value", "form", "gallery", "grammar", "layout_service",
    "llm_prompt", "lro", "media", "nav_tree", "panel", "prompt", "rpc", "shell",
    "snippet", "stat", "steps", "stream", "table", "terminal", "value", "view",
];

fn fds() -> FileDescriptorSet {
    FileDescriptorSet::decode(FDS).expect("committed descriptor set does not decode")
}

#[test]
fn carries_every_file_the_proto_library_declares() {
    let set = fds();
    let names: Vec<&str> = set.file.iter().filter_map(|f| f.name.as_deref()).collect();
    for want in EXPECTED {
        let path = format!("proto/{want}.proto");
        assert!(
            names.contains(&path.as_str()),
            "{path} missing from the descriptor set — regenerate with tools/regen_descriptors.sh"
        );
    }
    assert_eq!(
        EXPECTED.len(),
        27,
        "the uiview_proto file list changed; update EXPECTED and regenerate"
    );
}

#[test]
fn is_self_contained() {
    // --include_imports inlines every transitive dependency. If it did not, prost
    // would fail at build time with an unresolved type; asserting it here names
    // the cause instead of leaving a confusing codegen error.
    let set = fds();
    let present: Vec<&str> = set.file.iter().filter_map(|f| f.name.as_deref()).collect();
    for f in &set.file {
        for dep in &f.dependency {
            assert!(
                present.contains(&dep.as_str()),
                "{} imports {dep}, which is not in the set — regenerate with --include_imports",
                f.name.as_deref().unwrap_or("?")
            );
        }
    }
}

#[test]
fn carries_the_fields_this_crate_reads() {
    // A spot-check on the specific additions that separate the schema versions
    // this crate has been pinned to. TableColumn.value_display arrived in 0.22.0;
    // the StatPanel populate/previous_field/display_field trio came later. If the
    // set were regenerated from an older schemas, the crate would still compile —
    // it just would not see these — so compilation alone is not a version check.
    let set = fds();
    let field_names = |msg: &str| -> Vec<String> {
        set.file
            .iter()
            .flat_map(|f| f.message_type.iter())
            .find(|m| m.name.as_deref() == Some(msg))
            .unwrap_or_else(|| panic!("message {msg} not found in the descriptor set"))
            .field
            .iter()
            .filter_map(|f| f.name.clone())
            .collect()
    };

    assert!(
        field_names("TableColumn").iter().any(|n| n == "value_display"),
        "TableColumn.value_display absent — the set predates schemas 0.22.0"
    );
    let stat = field_names("StatPanel");
    for want in ["populate", "previous_field", "display_field"] {
        assert!(stat.iter().any(|n| n == want), "StatPanel.{want} absent");
    }
}
