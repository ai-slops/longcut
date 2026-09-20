use std::collections::HashSet;

use longcut_server::ai::proposal_schema;
use serde_json::Value;

#[test]
fn proposal_schema_uses_supported_strict_structured_output_subset() {
    // Given
    let schema = proposal_schema();
    // When
    let operations = &schema["properties"]["operations"]["items"]["anyOf"];
    // Then
    assert_eq!(operations.as_array().map(Vec::len), Some(5));
    assert_closed_objects(&schema);
    assert_forbidden_keywords_absent(&schema);
    for operation in operations.as_array().expect("operation variants") {
        assert_eq!(operation["properties"]["type"]["type"], "string");
        assert_eq!(
            operation["properties"]["type"]["enum"]
                .as_array()
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(operation["properties"]["evidence_ids"]["minItems"], 1);
    }
}

fn assert_closed_objects(value: &Value) {
    match value {
        Value::Object(object) => {
            if object.get("type") == Some(&Value::String("object".into())) {
                assert_eq!(
                    object.get("additionalProperties"),
                    Some(&Value::Bool(false))
                );
                let properties = object["properties"]
                    .as_object()
                    .expect("object properties")
                    .keys()
                    .map(String::as_str)
                    .collect::<HashSet<_>>();
                let required = object["required"]
                    .as_array()
                    .expect("required fields")
                    .iter()
                    .map(|field| field.as_str().expect("required field name"))
                    .collect::<HashSet<_>>();
                assert_eq!(properties, required);
            }
            for child in object.values() {
                assert_closed_objects(child);
            }
        }
        Value::Array(items) => {
            for item in items {
                assert_closed_objects(item);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn assert_forbidden_keywords_absent(value: &Value) {
    match value {
        Value::Object(object) => {
            for keyword in ["oneOf", "const", "uniqueItems"] {
                assert!(!object.contains_key(keyword), "unsupported {keyword}");
            }
            for child in object.values() {
                assert_forbidden_keywords_absent(child);
            }
        }
        Value::Array(items) => {
            for item in items {
                assert_forbidden_keywords_absent(item);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}
