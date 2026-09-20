use serde_json::{Value, json};

pub fn proposal_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["title", "summary", "warnings", "operations"],
        "properties": {
            "title": {"type": "string"},
            "summary": {"type": "string"},
            "warnings": {"type": "array", "items": {"type": "string"}},
            "operations": {
                "type": "array", "maxItems": 24,
                "items": {"anyOf": [
                    operation("insert_scene", json!({
                        "source_id": {"type": "string"}, "in_frame": {"type": "string"},
                        "out_frame": {"type": "string"}, "title": {"type": "string"}
                    }), &["source_id", "in_frame", "out_frame", "title"]),
                    operation("trim_scene", json!({
                        "scene_id": {"type": "string"}, "in_frame": {"type": "string"},
                        "out_frame": {"type": "string"}
                    }), &["scene_id", "in_frame", "out_frame"]),
                    operation("set_camera", json!({
                        "scene_id": {"type": "string"},
                        "camera": {"anyOf": [{"type": "null"}, camera_schema()]}
                    }), &["scene_id", "camera"]),
                    operation("update_scene", json!({
                        "scene_id": {"type": "string"}, "title": {"type": "string"},
                        "purpose": {"type": "string"}, "caption": {"type": "string"}
                    }), &["scene_id", "title", "purpose", "caption"]),
                    operation("delete_scene", json!({
                        "scene_id": {"type": "string"}
                    }), &["scene_id"])
                ]}
            }
        }
    })
}

fn operation(kind: &str, fields: Value, required: &[&str]) -> Value {
    let mut properties = fields.as_object().cloned().unwrap_or_default();
    properties.insert("type".into(), json!({"type": "string", "enum": [kind]}));
    properties.insert("rationale".into(), json!({"type": "string"}));
    properties.insert(
        "evidence_ids".into(),
        json!({"type": "array", "items": {"type": "string"}, "minItems": 1}),
    );
    let mut required_fields = vec!["type", "rationale", "evidence_ids"];
    required_fields.extend_from_slice(required);
    json!({
        "type": "object", "additionalProperties": false,
        "required": required_fields, "properties": properties
    })
}

fn camera_schema() -> Value {
    json!({
        "type": "object", "additionalProperties": false,
        "required": ["source_id", "in_frame", "x", "y", "width", "height", "opacity"],
        "properties": {
            "source_id": {"type": "string"}, "in_frame": {"type": "string"},
            "x": {"type": "number"}, "y": {"type": "number"},
            "width": {"type": "number"}, "height": {"type": "number"},
            "opacity": {"type": "number"}
        }
    })
}
