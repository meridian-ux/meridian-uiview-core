// wasm-bindgen surface for the TS web renderer.
//
// The web side imports this module via wasm-bindgen's generated bindings and
// uses the same proto-walking / request-building logic the Rust TUI uses. DOM
// rendering stays on the JS side; this crate handles the descriptor-driven
// plumbing once.
//
// Surface design: proto descriptors cross the boundary as protobuf BINARY
// (`prost::Message::decode`), not JSON. The TS side encodes them with
// protobuf-es `toBinary(...)`. This keeps the prost types free of serde derives
// — so meridian-schemas stays rust-free — and removes any camelCase/snake_case
// translation. RPC responses and runtime context are arbitrary JSON and still
// cross as JS values; rendered outputs (this crate's own structs) cross as JS
// values too.

use crate::paths::ProtoPaths;
use crate::proto::{GalleryPanel, PanelDescriptor, RpcCall, TablePanel};
use crate::render::{render_gallery, render_table};
use crate::request::{Context, RequestBuilder};
use prost::Message;
use serde_json::Value;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

/// Reads a field from a JSON-shaped object by dotted path. Mirrors
/// meridian.ui.descriptors.ProtoPaths.get. Used by the web renderer
/// to extract column cell values from response objects.
#[wasm_bindgen(js_name = "readPath")]
pub fn read_path(value: JsValue, path: &str) -> Result<JsValue, JsError> {
    let json: Value =
        serde_wasm_bindgen::from_value(value).map_err(|e| JsError::new(&e.to_string()))?;
    let result = ProtoPaths::get(&json, path).clone();
    serde_wasm_bindgen::to_value(&result).map_err(|e| JsError::new(&e.to_string()))
}

/// Renders a TablePanel against a JSON response value.
///
/// Inputs:
///   descriptor — a protobuf-binary `PanelDescriptor` (body.table populated),
///                produced by protobuf-es `toBinary(PanelDescriptorSchema, …)`.
///   response   — the JSON-shaped response from the populate RPC.
///
/// Output: an array of `{ raw, cells }` JS objects mirroring the Rust
/// `RenderedRow`.
#[wasm_bindgen(js_name = "renderTable")]
pub fn render_table_wasm(descriptor: &[u8], response: JsValue) -> Result<JsValue, JsError> {
    let d = PanelDescriptor::decode(descriptor).map_err(|e| JsError::new(&e.to_string()))?;
    let table = match d.body {
        Some(crate::proto::panel_descriptor::Body::Table(t)) => t,
        _ => return Err(JsError::new("descriptor body is not TABLE")),
    };
    let response_json: Value =
        serde_wasm_bindgen::from_value(response).map_err(|e| JsError::new(&e.to_string()))?;
    let rows = render_table(&response_json, &table);
    let serializable: Vec<RenderedRowJs> = rows
        .into_iter()
        .map(|r| RenderedRowJs {
            raw: r.raw,
            cells: r.cells,
        })
        .collect();
    serde_wasm_bindgen::to_value(&serializable).map_err(|e| JsError::new(&e.to_string()))
}

/// Builds the JSON request object for a TablePanel's populate RPC.
/// Hosts pass the runtime context as a JS object:
///
///   {
///     currentResourcePath: string | null,
///     uiIdentity: object | null,
///     selectedRow: object | null,
///     formValues: { [fieldId: string]: any }
///   }
///
/// `descriptor` is a protobuf-binary `PanelDescriptor`. The returned JS object
/// is the JSON-shaped request the host should submit through its RPC transport.
#[wasm_bindgen(js_name = "buildPopulateRequest")]
pub fn build_populate_request(
    descriptor: &[u8],
    context_value: JsValue,
) -> Result<JsValue, JsError> {
    let d = PanelDescriptor::decode(descriptor).map_err(|e| JsError::new(&e.to_string()))?;
    let table = match d.body {
        Some(crate::proto::panel_descriptor::Body::Table(t)) => t,
        _ => return Err(JsError::new("descriptor body is not TABLE")),
    };
    let populate = table
        .populate
        .ok_or_else(|| JsError::new("TablePanel has no populate RpcCall"))?;
    let ctx_in: ContextJs =
        serde_wasm_bindgen::from_value(context_value).map_err(|e| JsError::new(&e.to_string()))?;
    let request = RequestBuilder::build(&populate, &ctx_in.into_context());
    serde_wasm_bindgen::to_value(&request).map_err(|e| JsError::new(&e.to_string()))
}

/// Renders a GalleryPanel against a JSON response value. `gallery_panel` is a
/// protobuf-binary `GalleryPanel`. Returns an array of
/// `{ raw, title, subtitle, icon, status, href, action_label }` — one per row —
/// which the host drops into card chrome (resolving `icon` to a vector glyph).
#[wasm_bindgen(js_name = "renderGalleryPanel")]
pub fn render_gallery_panel_wasm(
    gallery_panel: &[u8],
    response: JsValue,
) -> Result<JsValue, JsError> {
    let gallery = GalleryPanel::decode(gallery_panel).map_err(|e| JsError::new(&e.to_string()))?;
    let response_json: Value =
        serde_wasm_bindgen::from_value(response).map_err(|e| JsError::new(&e.to_string()))?;
    let cards = render_gallery(&response_json, &gallery);
    let serializable: Vec<RenderedCardJs> = cards
        .into_iter()
        .map(|c| RenderedCardJs {
            raw: c.raw,
            title: c.title,
            subtitle: c.subtitle,
            icon: c.icon,
            status: c.status,
            href: c.href,
            action_label: c.action_label,
        })
        .collect();
    serde_wasm_bindgen::to_value(&serializable).map_err(|e| JsError::new(&e.to_string()))
}

#[derive(serde::Serialize)]
struct RenderedRowJs {
    raw: Value,
    cells: Vec<String>,
}

#[derive(serde::Serialize)]
struct RenderedCardJs {
    raw: Value,
    title: String,
    subtitle: String,
    icon: String,
    status: String,
    href: String,
    action_label: String,
}

#[derive(serde::Deserialize)]
struct ContextJs {
    #[serde(rename = "currentResourcePath")]
    current_resource_path: Option<String>,
    #[serde(rename = "uiIdentity")]
    ui_identity: Option<Value>,
    #[serde(rename = "selectedRow")]
    selected_row: Option<Value>,
    #[serde(rename = "formValues")]
    form_values: Option<HashMap<String, Value>>,
}

impl ContextJs {
    fn into_context(self) -> Context {
        Context {
            current_resource_path: self.current_resource_path,
            ui_identity: self.ui_identity,
            selected_row: self.selected_row,
            form_values: self.form_values.unwrap_or_default(),
            // Live Vega/panel signals are set by the JS host via the renderGrammar
            // handle, not passed through this construction context — start empty.
            signals: Default::default(),
        }
    }
}

// ----------------------------------------------------------------------------
// Generic primitives used by LRO + future panel shapes. The
// descriptor-specific helpers above are thin wrappers on top of these;
// hosts orchestrating multi-step flows (LRO start → poll → finalize)
// reach for these directly.
// ----------------------------------------------------------------------------

/// Builds the JSON request for any RpcCall, given a runtime context.
/// `rpc_call` is a protobuf-binary `RpcCall`. Same machinery TablePanel's
/// `populate` uses, exposed standalone so hosts can drive LroPanel.start,
/// LroPanel.finalize, and RowAction RPCs without each shape needing its own
/// wasm-bindgen wrapper.
#[wasm_bindgen(js_name = "buildRequest")]
pub fn build_request_wasm(rpc_call: &[u8], context_value: JsValue) -> Result<JsValue, JsError> {
    let call = RpcCall::decode(rpc_call).map_err(|e| JsError::new(&e.to_string()))?;
    let ctx_in: ContextJs =
        serde_wasm_bindgen::from_value(context_value).map_err(|e| JsError::new(&e.to_string()))?;
    let request = RequestBuilder::build(&call, &ctx_in.into_context());
    serde_wasm_bindgen::to_value(&request).map_err(|e| JsError::new(&e.to_string()))
}

/// Renders any TablePanel against any JSON response value. The existing
/// `renderTable` works on a full PanelDescriptor (extracting `descriptor.body.table`
/// internally); this variant takes a protobuf-binary `TablePanel` directly so
/// callers can render LroPanel.result with the same code path.
#[wasm_bindgen(js_name = "renderTablePanel")]
pub fn render_table_panel_wasm(table_panel: &[u8], response: JsValue) -> Result<JsValue, JsError> {
    let table = TablePanel::decode(table_panel).map_err(|e| JsError::new(&e.to_string()))?;
    let response_json: Value =
        serde_wasm_bindgen::from_value(response).map_err(|e| JsError::new(&e.to_string()))?;
    let rows = render_table(&response_json, &table);
    let serializable: Vec<RenderedRowJs> = rows
        .into_iter()
        .map(|r| RenderedRowJs {
            raw: r.raw,
            cells: r.cells,
        })
        .collect();
    serde_wasm_bindgen::to_value(&serializable).map_err(|e| JsError::new(&e.to_string()))
}

/// Convention-based LRO metadata formatter. Mirrors JavaFX's
/// `DescribedLroCard.renderMetadata`: extracts `state` (rendered as
/// `[STATE_NAME]`) and `status_message`, returning the concatenation.
/// Falls back to the raw JSON if neither field is present. Hosts
/// display this on a status line while polling WaitOperation.
#[wasm_bindgen(js_name = "formatLroMetadata")]
pub fn format_lro_metadata(metadata: JsValue) -> Result<String, JsError> {
    let value: Value =
        serde_wasm_bindgen::from_value(metadata).map_err(|e| JsError::new(&e.to_string()))?;
    let mut out = String::new();
    let state = ProtoPaths::get(&value, "state");
    if let Value::String(s) = state {
        out.push('[');
        out.push_str(s);
        out.push_str("] ");
    }
    let status = ProtoPaths::get(&value, "status_message");
    if let Value::String(s) = status {
        out.push_str(s);
    }
    if out.is_empty() {
        out = value.to_string();
    }
    Ok(out)
}

// NOTE: bundle decoding (formerly `decodePanelBundle`) now lives on the TS side —
// it uses protobuf-es `fromBinary(PanelBundleSchema, bytes)` to split a
// PanelBundle into its PanelDescriptors, then re-encodes each with `toBinary`
// and feeds them to the render functions above. Keeping (de)serialization in TS
// (which already has @savvifi/meridian-proto-ts) lets the prost types stay
// serde-free and the wasm own only the render/request logic.
