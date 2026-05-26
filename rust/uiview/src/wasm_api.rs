// wasm-bindgen surface for the TS web renderer.
//
// The web side imports this module via wasm-pack's generated bindings
// and uses the same proto-walking / request-building logic the Rust
// TUI uses. DOM rendering stays on the JS side; this crate handles
// the descriptor-driven plumbing once.
//
// Surface design: descriptors flow across the boundary as JSON-shaped
// objects (not bytes), because TS hosts construct them by hand or
// fetch them as JSON from a config service. Internally we use the
// typed prost::PanelDescriptor via serde's JSON round-trip — the
// build.rs adds serde derives to every generated type. Responses and
// request outputs also flow as JSON.

use crate::paths::ProtoPaths;
use crate::proto::{PanelBundle, PanelDescriptor, RpcCall, TablePanel};
use crate::render::render_table;
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
///   descriptor — a JSON-shaped PanelDescriptor (body.table populated).
///                Snake_case keys, matching proto3 JSON.
///   response   — the JSON-shaped response from the populate RPC.
///
/// Output: an array of `{ raw, cells }` JS objects mirroring the Rust
/// `RenderedRow`.
#[wasm_bindgen(js_name = "renderTable")]
pub fn render_table_wasm(
    descriptor: JsValue,
    response: JsValue,
) -> Result<JsValue, JsError> {
    let d: PanelDescriptor =
        serde_wasm_bindgen::from_value(descriptor).map_err(|e| JsError::new(&e.to_string()))?;
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
/// The returned JS object is the JSON-shaped request the host should
/// submit through its RPC transport.
#[wasm_bindgen(js_name = "buildPopulateRequest")]
pub fn build_populate_request(
    descriptor: JsValue,
    context_value: JsValue,
) -> Result<JsValue, JsError> {
    let d: PanelDescriptor =
        serde_wasm_bindgen::from_value(descriptor).map_err(|e| JsError::new(&e.to_string()))?;
    let table = match d.body {
        Some(crate::proto::panel_descriptor::Body::Table(t)) => t,
        _ => return Err(JsError::new("descriptor body is not TABLE")),
    };
    let populate = table
        .populate
        .ok_or_else(|| JsError::new("TablePanel has no populate RpcCall"))?;
    let ctx_in: ContextJs =
        serde_wasm_bindgen::from_value(context_value).map_err(|e| JsError::new(&e.to_string()))?;
    let ctx = Context {
        current_resource_path: ctx_in.current_resource_path,
        ui_identity: ctx_in.ui_identity,
        selected_row: ctx_in.selected_row,
        form_values: ctx_in.form_values.unwrap_or_default(),
    };
    let request = RequestBuilder::build(&populate, &ctx);
    serde_wasm_bindgen::to_value(&request).map_err(|e| JsError::new(&e.to_string()))
}

#[derive(serde::Serialize)]
struct RenderedRowJs {
    raw: Value,
    cells: Vec<String>,
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
/// Same machinery TablePanel's `populate` uses, exposed standalone so
/// hosts can drive LroPanel.start, LroPanel.finalize, and RowAction
/// RPCs without each shape needing its own wasm-bindgen wrapper.
#[wasm_bindgen(js_name = "buildRequest")]
pub fn build_request_wasm(
    rpc_call: JsValue,
    context_value: JsValue,
) -> Result<JsValue, JsError> {
    let call: RpcCall =
        serde_wasm_bindgen::from_value(rpc_call).map_err(|e| JsError::new(&e.to_string()))?;
    let ctx_in: ContextJs = serde_wasm_bindgen::from_value(context_value)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let request = RequestBuilder::build(&call, &ctx_in.into_context());
    serde_wasm_bindgen::to_value(&request).map_err(|e| JsError::new(&e.to_string()))
}

/// Renders any TablePanel against any JSON response value. The
/// existing `renderTable` works on a full PanelDescriptor (extracting
/// `descriptor.body.table` internally); this variant takes a
/// `TablePanel` directly so callers can render LroPanel.result with
/// the same code path.
#[wasm_bindgen(js_name = "renderTablePanel")]
pub fn render_table_panel_wasm(
    table_panel: JsValue,
    response: JsValue,
) -> Result<JsValue, JsError> {
    let table: TablePanel = serde_wasm_bindgen::from_value(table_panel)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let response_json: Value = serde_wasm_bindgen::from_value(response)
        .map_err(|e| JsError::new(&e.to_string()))?;
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

/// Decode a wire-encoded `meridian.ui.v1.PanelBundle` (the output of
/// `meridian_panel_bundle`) into a JS-shaped object. Callers pass the
/// raw bytes (e.g. fetched from `/api/panels.binpb`); the returned
/// object has the proto3-JSON snake_case shape every other wasm_api
/// function consumes, so its `.panels` entries can be passed straight
/// into `renderTablePanel`. This removes the need for a separate JS
/// proto-decoder library (protobuf-es, etc.).
#[wasm_bindgen(js_name = "decodePanelBundle")]
pub fn decode_panel_bundle(bytes: &[u8]) -> Result<JsValue, JsError> {
    let bundle =
        PanelBundle::decode(bytes).map_err(|e| JsError::new(&e.to_string()))?;
    serde_wasm_bindgen::to_value(&bundle).map_err(|e| JsError::new(&e.to_string()))
}
