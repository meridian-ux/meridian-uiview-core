use crate::paths::ProtoPaths;
use crate::proto::{ColumnFormat, GalleryPanel, TableColumn, TablePanel};
use serde_json::Value;

// One rendered row in the table. Each element of `cells` corresponds
// 1:1 to the column at the same index in the TablePanel's columns
// list. `raw` carries the source JSON object (so row actions can
// resolve row.field_path bindings against it).
pub struct RenderedRow {
    pub raw: Value,
    pub cells: Vec<String>,
}

/// Renders the `rows_field` of a JSON response into a sequence of
/// `RenderedRow`. Each row is paired with its rendered column cells
/// formatted per the TableColumn.format.
///
/// Mirrors the per-cell formatting in
/// meridian.ui.javafx.DescribedTableCard.renderValue, so the JavaFX
/// and TUI / web outputs match.
pub fn render_table(response: &Value, table: &TablePanel) -> Vec<RenderedRow> {
    let rows = ProtoPaths::rows(response, &table.rows_field);
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let cells: Vec<String> = table
            .columns
            .iter()
            .map(|col| format_cell(ProtoPaths::get(row, &col.field_path), col))
            .collect();
        out.push(RenderedRow {
            raw: row.clone(),
            cells,
        });
    }
    out
}

// One rendered card in a gallery. `raw` carries the source JSON object (so
// click/navigation can resolve against it); the slots are pre-formatted strings
// the host drops into the card chrome. `icon` is a key the host maps to a glyph.
pub struct RenderedCard {
    pub raw: Value,
    pub title: String,
    pub subtitle: String,
    pub icon: String,
    pub status: String,
    pub href: String,
    pub action_label: String,
}

/// Renders the `rows_field` of a JSON response into a sequence of `RenderedCard`,
/// mapping each row's fields to card slots per the GalleryPanel's CardSpec.
pub fn render_gallery(response: &Value, gallery: &GalleryPanel) -> Vec<RenderedCard> {
    let rows = ProtoPaths::rows(response, &gallery.rows_field);
    let card = gallery.card.clone().unwrap_or_default();
    // Read a dotted path as a display string; empty path or null -> "".
    let slot = |row: &Value, path: &str| -> String {
        if path.is_empty() {
            return String::new();
        }
        match ProtoPaths::get(row, path) {
            Value::String(s) => s.clone(),
            Value::Null => String::new(),
            v => v.to_string(),
        }
    };
    rows.iter()
        .map(|row| RenderedCard {
            title: slot(row, &card.title_field),
            subtitle: slot(row, &card.subtitle_field),
            icon: slot(row, &card.icon_field),
            status: slot(row, &card.status_field),
            href: slot(row, &card.href_field),
            action_label: slot(row, &card.action_label_field),
            raw: (*row).clone(),
        })
        .collect()
}

/// Formats one JSON value per a TableColumn's format directive.
pub fn format_cell(value: &Value, column: &TableColumn) -> String {
    let format = ColumnFormat::try_from(column.format).unwrap_or(ColumnFormat::Unspecified);
    format_value(value, format)
}

/// Standalone formatter — also used by wasm wrappers that want to
/// format a single value without a TableColumn handy.
pub fn format_value(value: &Value, format: ColumnFormat) -> String {
    if value.is_null() {
        return String::new();
    }
    match format {
        ColumnFormat::Float2dp => value
            .as_f64()
            .map(|n| format!("{:.2}", n))
            .unwrap_or_else(|| value.to_string()),
        ColumnFormat::Integer => value
            .as_i64()
            .map(|n| n.to_string())
            .unwrap_or_else(|| value.to_string()),
        ColumnFormat::EnumName | ColumnFormat::String | ColumnFormat::Unspecified => {
            match value {
                Value::String(s) => s.clone(),
                _ => value.to_string(),
            }
        }
        ColumnFormat::StringList => match value {
            Value::Array(items) => items
                .iter()
                .map(|item| match item {
                    Value::String(s) => s.clone(),
                    _ => item.to_string(),
                })
                .collect::<Vec<_>>()
                .join(", "),
            _ => value.to_string(),
        },
        ColumnFormat::Timestamp => match value {
            Value::String(s) => s.clone(),
            _ => value.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn formats_float_two_dp() {
        assert_eq!(format_value(&json!(0.954), ColumnFormat::Float2dp), "0.95");
    }

    #[test]
    fn formats_string_list() {
        assert_eq!(
            format_value(&json!(["a", "b", "c"]), ColumnFormat::StringList),
            "a, b, c",
        );
    }

    #[test]
    fn render_table_maps_rows_to_cells() {
        let response = json!({
            "claims": [
                {"confidence": 0.95, "text": "fast-setting"},
                {"confidence": 0.78, "text": "non-shrink"},
            ]
        });
        let table = TablePanel {
            populate: None,
            rows_field: "claims".into(),
            item_noun: "claims".into(),
            placeholder: String::new(),
            columns: vec![
                TableColumn {
                    header: "confidence".into(),
                    field_path: "confidence".into(),
                    format: ColumnFormat::Float2dp as i32,
                    pref_width: 0,
                },
                TableColumn {
                    header: "claim".into(),
                    field_path: "text".into(),
                    format: ColumnFormat::String as i32,
                    pref_width: 0,
                },
            ],
            actions: vec![],
        };
        let rendered = render_table(&response, &table);
        assert_eq!(rendered.len(), 2);
        assert_eq!(rendered[0].cells, vec!["0.95", "fast-setting"]);
        assert_eq!(rendered[1].cells, vec!["0.78", "non-shrink"]);
    }
}
