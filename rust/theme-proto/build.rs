//! Generate the `meridian.theme.v1` prost types for the **cargo** build.
//!
//! Same shape as `//rust/uiview/build.rs`, one descriptor set narrower. See that
//! file for the full reasoning; the short version:
//!
//!   * **Bazel** never runs this. `rust_prost_library(name = "theme_proto")` in
//!     `//rust/uiview/BUILD.bazel` generates over
//!     `@meridian_schemas//proto:theme_proto`, so Bazel stays the authority.
//!   * **cargo** (this file) reads the committed
//!     `proto/gen/meridian_theme_v1.binpb`, which is that proto_library's output
//!     checked in. No protoc, no include paths: `compile_fds` reads bytes.
//!
//! Why a SEPARATE crate rather than another module inside `meridian-uiview`:
//! because Bazel already makes it a separate crate, and the consumer's `use`
//! statement has to resolve identically under both builds.
//! `meridian-tui/src/theme.rs` writes
//! `pub use theme_proto::meridian::theme::v1::*` against the
//! `rust_prost_library`; folding these types into `meridian_uiview::theme` would
//! force that line to differ per build system, which is exactly the kind of
//! divergence the dual-build idiom exists to avoid.
//!
//! It is also the honest boundary. `theme.proto` is *style*; `panel.proto` and
//! friends are *semantics*. The BUILD comment on `theme_proto` already says so —
//! "SEPARATE crate (semantics ⊥ style)" — and a crate split is how that stays
//! true rather than aspirational.

use std::path::PathBuf;

use prost::Message as _;

/// The committed descriptor set — the cargo path's input.
const COMMITTED_FDS: &str = "proto/gen/meridian_theme_v1.binpb";

/// Escape hatch mirroring `MERIDIAN_UIVIEW_DESCRIPTOR_SET`: point this at a
/// freshly built set to codegen against something other than the committed bytes
/// (what `tools/regen_descriptors.sh` uses to verify itself).
const FDS_ENV: &str = "MERIDIAN_THEME_DESCRIPTOR_SET";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed={FDS_ENV}");
    println!("cargo:rerun-if-changed={COMMITTED_FDS}");

    let fds_path = match std::env::var_os(FDS_ENV) {
        Some(p) => PathBuf::from(p),
        None => PathBuf::from(COMMITTED_FDS),
    };

    if !fds_path.exists() {
        return Err(format!(
            "descriptor set {} not found — run tools/regen_descriptors.sh",
            fds_path.display()
        )
        .into());
    }

    let bytes = std::fs::read(&fds_path)
        .map_err(|e| format!("reading descriptor set {}: {e}", fds_path.display()))?;
    let fds = prost_types::FileDescriptorSet::decode(bytes.as_slice())
        .map_err(|e| format!("decoding descriptor set {}: {e}", fds_path.display()))?;

    // `compile_well_known_types()` deliberately NOT called — see uiview/build.rs.
    // theme.proto imports nothing today, so it is moot here, but leaving the two
    // generators configured identically means an import added upstream cannot
    // change behaviour between them.
    prost_build::Config::new().compile_fds(fds)?;

    Ok(())
}
