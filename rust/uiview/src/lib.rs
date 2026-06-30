// meridian-uiview: platform-neutral core for the Meridian proto-driven
// UI framework.
//
// What it provides:
//   * prost-generated Rust types for meridian.ui.v1 (PanelDescriptor +
//     friends). Accessible at `meridian_uiview::proto::*`.
//   * `ProtoPaths` — field-path accessor over prost Message instances.
//     Drives TableColumn.field_path and FieldBinding.row_field
//     resolution.
//   * `RequestBuilder` — turns an RpcCall + runtime context into a
//     serde_json::Value request the host can submit over gRPC-Web /
//     gRPC / whatever.
//
// Two consumers:
//   * `meridian-tui` — native Rust ratatui renderer. Uses these
//     helpers directly.
//   * The TS web renderer — imports a wasm-bindgen wrapper compiled
//     from this crate's `wasm` feature. The DOM lives on the JS
//     side; this crate handles all proto-walking + request building.
//
// All types here are platform-neutral: no JavaFX, no DOM, no terminal
// dependencies. Renderers layer those on top.

/// prost-generated types for meridian.ui.v1.
///
/// Two compilation paths, same emitted module:
///   * **Bazel** — `//rust/uiview:uiview_proto_rust` (rules_rust_prost)
///     ships the codegen as a sibling crate `uiview_proto`, which
///     this re-exports via the `bazel_proto` cargo feature.
///   * **cargo (default)** — `build.rs` runs `prost_build` over
///     `../../proto/uiview.proto` and emits `meridian.ui.v1.rs`
///     into OUT_DIR; the `include!` below pulls it in.
///
/// The `cargo` path lets consumers (e.g. fastverk/botnoc) depend
/// on meridian-uiview via a `path = "..."` dep without needing the
/// full Bazel build of meridian.
pub mod proto {
    #[cfg(feature = "bazel_proto")]
    pub use uiview_proto::meridian::ui::v1::*;

    #[cfg(not(feature = "bazel_proto"))]
    include!(concat!(env!("OUT_DIR"), "/meridian.ui.v1.rs"));
}

mod paths;
mod request;
mod render;

pub use paths::ProtoPaths;
pub use render::{
    format_cell, format_value, render_gallery, render_table, RenderedCard, RenderedRow,
};
pub use request::{Context, RequestBuilder};

// Re-export the prost crate so downstream consumers can decode our
// generated message types (e.g. `PanelBundle`) without introducing a
// second prost instance from their own crate universe. Bazel's
// isolated `@crates::prost` would otherwise produce trait-distinct
// `Message` impls and `PanelBundle::decode` would not resolve.
pub use prost;

#[cfg(target_arch = "wasm32")]
mod wasm_api;
