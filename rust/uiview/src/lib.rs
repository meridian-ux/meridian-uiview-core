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

pub mod proto {
    // prost-generated types from proto/uiview.proto. Build script
    // (build.rs) runs protoc and writes the module to
    // OUT_DIR/meridian.ui.v1.rs.
    include!(concat!(env!("OUT_DIR"), "/meridian.ui.v1.rs"));
}

mod paths;
mod request;
mod render;

pub use paths::ProtoPaths;
pub use render::{format_cell, format_value, render_table, RenderedRow};
pub use request::{Context, RequestBuilder};

#[cfg(feature = "wasm")]
mod wasm_api;
