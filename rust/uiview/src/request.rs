use crate::proto::field_binding::Source;
use crate::proto::{ContextSource, FieldBinding, NestedBinding, RpcCall};
use serde_json::{Map, Value};
use std::collections::HashMap;

/// Runtime context the FieldBinding sources can pull from at call time.
///
/// Mirrors meridian.ui.descriptors.RequestBuilder.Context on the Java
/// side. The renderer populates it from whatever host-side state it
/// holds (the active resource path, the UI's representative Identity,
/// form input values, the selected row).
#[derive(Default, Clone)]
pub struct Context {
    /// The active resource path (CONTEXT_SOURCE.CURRENT_RESOURCE_PATH).
    pub current_resource_path: Option<String>,
    /// The UI's representative Identity proto, serialized as JSON.
    pub ui_identity: Option<Value>,
    /// The selected row (for RowAction bindings).
    pub selected_row: Option<Value>,
    /// Form field values keyed by field_id.
    pub form_values: HashMap<String, Value>,
}

/// Assembles a JSON request from an RpcCall's FieldBindings + runtime
/// context. Mirrors meridian.ui.descriptors.RequestBuilder on the
/// Java side; uses serde_json instead of prost reflection.
///
/// The host serializes the returned Value to proto bytes via its
/// own marshaling layer (tonic + a serde-aware codec, gRPC-Web JSON
/// mode, whatever).
pub struct RequestBuilder;

impl RequestBuilder {
    pub fn build(call: &RpcCall, ctx: &Context) -> Value {
        let mut root = Map::new();
        for binding in &call.bindings {
            Self::apply_binding(&mut root, binding, ctx);
        }
        Value::Object(root)
    }

    fn apply_binding(root: &mut Map<String, Value>, binding: &FieldBinding, ctx: &Context) {
        if binding.request_field.is_empty() {
            return;
        }
        let value = match binding.source.as_ref() {
            Some(Source::Context(code)) => Self::resolve_context(*code, ctx),
            Some(Source::RowField(p)) => ctx
                .selected_row
                .as_ref()
                .map(|r| crate::ProtoPaths::get(r, p).clone())
                .unwrap_or(Value::Null),
            Some(Source::FormField(id)) => ctx
                .form_values
                .get(id)
                .cloned()
                .unwrap_or(Value::Null),
            Some(Source::Literal(s)) => Value::String(s.clone()),
            Some(Source::Nested(nested)) => Self::build_nested(nested, ctx),
            None => return,
        };
        if value.is_null() {
            return;
        }
        Self::set_path(root, &binding.request_field, value);
    }

    fn build_nested(nested: &NestedBinding, ctx: &Context) -> Value {
        let mut inner = Map::new();
        for child in &nested.fields {
            Self::apply_binding(&mut inner, child, ctx);
        }
        Value::Object(inner)
    }

    fn resolve_context(code: i32, ctx: &Context) -> Value {
        match ContextSource::try_from(code).unwrap_or(ContextSource::Unspecified) {
            ContextSource::CurrentResourcePath => ctx
                .current_resource_path
                .as_ref()
                .map(|s| Value::String(s.clone()))
                .unwrap_or(Value::Null),
            ContextSource::UiIdentity => ctx.ui_identity.clone().unwrap_or(Value::Null),
            ContextSource::Unspecified => Value::Null,
        }
    }

    fn set_path(root: &mut Map<String, Value>, path: &str, value: Value) {
        let segments: Vec<&str> = path.split('.').collect();
        if segments.is_empty() {
            return;
        }
        let mut current = root;
        for segment in &segments[..segments.len() - 1] {
            let entry = current
                .entry((*segment).to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            if !entry.is_object() {
                *entry = Value::Object(Map::new());
            }
            current = entry.as_object_mut().unwrap();
        }
        current.insert(segments.last().unwrap().to_string(), value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fb(request_field: &str, source: Source) -> FieldBinding {
        FieldBinding {
            request_field: request_field.into(),
            source: Some(source),
        }
    }

    #[test]
    fn context_binding_resolves() {
        let mut ctx = Context::default();
        ctx.current_resource_path = Some("/tmp/x.pdf".into());
        let call = RpcCall {
            service: "p.v1.S".into(),
            method: "M".into(),
            bindings: vec![fb(
                "pdf_path",
                Source::Context(ContextSource::CurrentResourcePath as i32),
            )],
        };
        assert_eq!(
            RequestBuilder::build(&call, &ctx),
            json!({"pdf_path": "/tmp/x.pdf"}),
        );
    }

    #[test]
    fn nested_binding_builds_sub_object() {
        let mut ctx = Context::default();
        let mut forms = HashMap::new();
        forms.insert("secs".to_string(), json!(300));
        ctx.form_values = forms;
        let call = RpcCall {
            service: "p.v1.S".into(),
            method: "M".into(),
            bindings: vec![fb(
                "max_duration",
                Source::Nested(NestedBinding {
                    fields: vec![fb("seconds", Source::FormField("secs".into()))],
                }),
            )],
        };
        assert_eq!(
            RequestBuilder::build(&call, &ctx),
            json!({"max_duration": {"seconds": 300}}),
        );
    }
}
