use longcut_server::model::*;

use super::support::*;

#[test]
fn shipped_trim_payload_without_extension_fields_remains_readable() {
    // Given
    let payload = serde_json::json!({
        "id": "trim",
        "scene_id": "scene",
        "type": "trim_scene",
        "camera": null,
        "in_frame": "10",
        "out_frame": "50",
        "rationale": "tighten"
    });

    // When
    let operation: Operation = serde_json::from_value(payload).expect("deserialize shipped trim");

    // Then
    assert_eq!(operation.kind, OperationType::TrimScene);
    assert_eq!(operation.in_frame.as_deref(), Some("10"));
    assert!(operation.source_id.is_none());
    assert!(operation.title.is_none());
    assert!(operation.purpose.is_none());
    assert!(operation.caption.is_none());
}

#[test]
fn insert_scene_when_approved_retains_server_id_and_serialized_fields() {
    // Given
    let mut project = fixture_project();
    let mut history = history();
    let operation = Operation {
        source_id: Some("source".into()),
        in_frame: Some("10".into()),
        out_frame: Some("40".into()),
        title: Some("Opening".into()),
        ..operation(OperationType::InsertScene, "generated-scene", "insert")
    };
    let serialized = serde_json::to_value(&operation).expect("serialize operation");
    create_change_set(&mut project, &mut history, vec![operation]);
    let change_set_id = project.change_sets[0].id.clone();

    // When
    apply_operations(
        &mut project,
        &mut history,
        Approval {
            change_set_id: &change_set_id,
            operation_ids: &["insert"],
        },
    )
    .expect("apply insertion");

    // Then
    assert_eq!(serialized["type"], "insert_scene");
    assert_eq!(serialized["scene_id"], "generated-scene");
    assert_eq!(serialized["source_id"], "source");
    assert_eq!(serialized["title"], "Opening");
    assert!(serialized["camera"].is_null());
    assert!(serialized["purpose"].is_null());
    assert!(serialized["caption"].is_null());
    let inserted = &project.scenes[1];
    assert_eq!(inserted.id, "generated-scene");
    assert_eq!(inserted.source_id, "source");
    assert_eq!(inserted.in_frame, "10");
    assert_eq!(inserted.out_frame, "40");
    assert!(!inserted.manual);
}

#[test]
fn update_scene_when_approved_changes_only_metadata() {
    // Given
    let mut project = fixture_project();
    project.scenes[0].manual = true;
    project.scenes[0].camera = Some(valid_camera());
    let before = project.scenes[0].clone();
    let mut history = history();
    let update = Operation {
        title: Some("New title".into()),
        purpose: Some("New purpose".into()),
        caption: Some("New caption".into()),
        ..operation(OperationType::UpdateScene, "scene", "update")
    };
    create_change_set(&mut project, &mut history, vec![update]);
    let change_set_id = project.change_sets[0].id.clone();

    // When
    apply_operations(
        &mut project,
        &mut history,
        Approval {
            change_set_id: &change_set_id,
            operation_ids: &["update"],
        },
    )
    .expect("apply metadata update");

    // Then
    let updated = &project.scenes[0];
    assert_eq!(updated.title, "New title");
    assert_eq!(updated.purpose, "New purpose");
    assert_eq!(updated.caption, "New caption");
    assert_eq!(updated.source_id, before.source_id);
    assert_eq!(updated.in_frame, before.in_frame);
    assert_eq!(updated.out_frame, before.out_frame);
    assert_eq!(updated.camera, before.camera);
    assert!(updated.manual);
}

#[test]
fn independent_properties_can_be_partially_approved() {
    // Given
    let mut project = fixture_project();
    let mut history = history();
    let trim = Operation {
        in_frame: Some("10".into()),
        out_frame: Some("50".into()),
        ..operation(OperationType::TrimScene, "scene", "trim")
    };
    let update = Operation {
        title: Some("Changed".into()),
        purpose: Some("Purpose".into()),
        caption: Some("Caption".into()),
        ..operation(OperationType::UpdateScene, "scene", "update")
    };
    create_change_set(&mut project, &mut history, vec![trim, update]);
    let change_set_id = project.change_sets[0].id.clone();

    // When
    for operation_id in ["trim", "update"] {
        apply_operations(
            &mut project,
            &mut history,
            Approval {
                change_set_id: &change_set_id,
                operation_ids: &[operation_id],
            },
        )
        .expect("apply independent operation");
    }

    // Then
    assert_eq!(project.scenes[0].in_frame, "10");
    assert_eq!(project.scenes[0].title, "Changed");
    assert_eq!(project.change_sets[0].status, ChangeSetStatus::Applied);
}
