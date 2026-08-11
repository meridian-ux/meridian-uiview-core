//! prost types for `meridian.theme.v1`.
//!
//! This crate exists to give the **cargo** build what
//! `rust_prost_library(name = "theme_proto")` gives the Bazel build — under the
//! same crate name AND the same module path, so a consumer writes
//!
//! ```ignore
//! pub use theme_proto::meridian::theme::v1::*;
//! ```
//!
//! and never learns which build system produced it. That exact line is
//! `meridian-tui/rust/tui/src/theme.rs`, and it is a large part of why the TUI
//! could not be built by cargo: the crate it names had no cargo producer.
//!
//! Under Bazel this file is not compiled at all — `//rust/uiview:theme_proto` is
//! the `rust_prost_library`, and `meridian-tui`'s BUILD deps point at that target
//! directly. So the two producers can never both be present in one graph.
//!
//! The `meridian::theme::v1` nesting is written out by hand because that is what
//! `rust_prost_library` emits and what the consumer's `use` path already assumes.
//! prost-build names its output file after the proto package but generates the
//! items at that file's top level, so without this wrapper the types would land
//! at the crate root and the shared `use` would only work under Bazel — the
//! silent, one-sided break this crate exists to prevent.

pub mod meridian {
    pub mod theme {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/meridian.theme.v1.rs"));
        }
    }
}
