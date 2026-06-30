# meridian-uiview-core

The shared, platform-neutral **Rust core** of the meridian design language.

- **prost types** for `meridian.ui.v1` (`PanelDescriptor` & friends) and
  `meridian.theme.v1` (`Theme`), generated from the
  [meridian-schemas](https://github.com/meridian-ux/meridian-schemas) proto
  contracts via `rules_rust_prost` (consumed cross-repo as
  `@meridian_schemas//proto:…`).
- **`ProtoPaths`** — field-path accessor over prost messages (drives
  `TableColumn.field_path` / `FieldBinding.row_field`).
- **`RequestBuilder`** — turns an `RpcCall` + runtime context into a request.
- **render / scoring helpers** (`render_table`, `render_gallery`, `format_cell`, …).
- **`uiview_wasm`** — the wasm-bindgen bundle the web renderer loads.

Two downstream consumers, each in its own module so renderers never depend on
each other:

```
                       meridian-uiview-core  (this repo, rust)
                        ├── //rust/uiview          (native lib)   ─▶ meridian-tui
                        ├── //rust/uiview:theme_proto             ─▶ meridian-tui
                        └── //rust/uiview:uiview_wasm  (wasm)      ─▶ meridian-web
```

## Build

```bash
bazel build //rust/uiview:uiview //rust/uiview:uiview_wasm //rust/uiview:theme_proto
bazel test  //rust/uiview:uiview_test
```

The authoritative build is Bazel. The cargo `build.rs` path (proto codegen for
plain `cargo`) needs the proto sources locally and is not wired in the standalone
repo; use the Bazel `bazel_proto` feature (the default here).
