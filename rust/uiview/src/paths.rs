use serde_json::Value;

// Field-path accessor for JSON-shaped data.
//
// Why JSON instead of typed prost messages: prost-generated Rust
// types don't carry runtime descriptor info, so a generic
// "look up field `subject.claim.claim_text`" can't be done
// reflectively in pure prost. Hosts that talk gRPC marshal their
// responses to JSON before handing them to the renderer, which is
// what Connect-ES / grpc-web's JSON mode does natively anyway.
// The renderer never needs the strong types.
//
// Mirror of the Java meridian.ui.descriptors.ProtoPaths.
pub struct ProtoPaths;

impl ProtoPaths {
    /// Walks `value` along `path` (dot-separated proto field names)
    /// and returns the value at the leaf. Returns `Value::Null` when
    /// any intermediate field is absent or the leaf doesn't exist.
    ///
    /// Proto JSON convention: prost / Connect-ES / grpc-web emit
    /// snake_case field names. The path follows the same convention.
    pub fn get<'a>(value: &'a Value, path: &str) -> &'a Value {
        if path.is_empty() {
            return value;
        }
        let mut current = value;
        for segment in path.split('.') {
            match current {
                Value::Object(map) => match map.get(segment) {
                    Some(v) => current = v,
                    None => return &Value::Null,
                },
                _ => return &Value::Null,
            }
        }
        current
    }

    /// Returns the list of rows at `path`. Returns an empty Vec if
    /// the path doesn't resolve or the field isn't an array. The
    /// rows themselves are borrowed from `value`.
    pub fn rows<'a>(value: &'a Value, path: &str) -> Vec<&'a Value> {
        match Self::get(value, path) {
            Value::Array(rows) => rows.iter().collect(),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn walks_nested_paths() {
        let v = json!({
            "subject": {
                "claim": {
                    "claim_text": "CEMENT ALL sets in 15 minutes"
                }
            }
        });
        assert_eq!(
            ProtoPaths::get(&v, "subject.claim.claim_text"),
            &json!("CEMENT ALL sets in 15 minutes"),
        );
    }

    #[test]
    fn missing_path_returns_null() {
        let v = json!({"a": {"b": 1}});
        assert_eq!(ProtoPaths::get(&v, "a.c.d"), &Value::Null);
    }

    #[test]
    fn rows_returns_array_elements() {
        let v = json!({"claims": [{"text": "x"}, {"text": "y"}]});
        let rows = ProtoPaths::rows(&v, "claims");
        assert_eq!(rows.len(), 2);
    }
}
