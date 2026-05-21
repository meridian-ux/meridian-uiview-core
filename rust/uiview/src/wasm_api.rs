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
use crate::proto::PanelDescriptor;
use crate::render::render_table;
use crate::request::{Context, RequestBuilder};
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
