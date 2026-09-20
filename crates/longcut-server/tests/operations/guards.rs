use longcut_server::{error::AppError, model::*};

use super::support::*;

#[test]
fn change_set_when_more_than_twenty_four_operations_is_rejected() {
    // Given
    let mut project = fixture_project();
    let mut history = history();
    let operations = (0..25)
        .map(|index| Operation {
            source_id: Some("source".into()),
            in_frame: Some("0".into()),
            out_frame: Some("1".into()),
            title: Some(format!("Scene {index}")),
            ..operation(
                OperationType::InsertScene,
                &format!("scene-{index}"),
                &format!("operation-{index}"),
            )
        })
        .collect();

    // When
    let result = try_create_change_set(&mut project, &mut history, operations);

    // Then
    assert!(matches!(result, Err(AppError::Invalid(_))));
    assert!(project.change_sets.is_empty());
    assert_eq!(project.scenes.len(), 1);
}

#[test]
fn change_set_when_operations_repeat_a_scene_property_is_rejected() {
    // Given
    let mut project = fixture_project();
    let mut history = history();
    let first = Operation {
        title: Some("First".into()),
        purpose: Some(String::new()),
        caption: Some(String::new()),
        ..operation(OperationType::UpdateScene, "scene", "first")
    };
    let second = Operation {
        title: Some("Second".into()),
        purpose: Some(String::new()),
        caption: Some(String::new()),
        ..operation(OperationType::UpdateScene, "scene", "second")
    };

    // When
    let result = try_create_change_set(&mut project, &mut history, vec![first, second]);

    // Then
    assert!(matches!(result, Err(AppError::Invalid(_))));
    assert!(project.change_sets.is_empty());
}

#[test]
fn insert_scene_when_an_earlier_operation_deletes_the_same_id_is_rejected() {
    // Given
    let mut project = fixture_project();
    let mut history = history();
    let insert = Operation {
        source_id: Some("source".into()),
        in_frame: Some("0".into()),
        out_frame: Some("20".into()),
        title: Some("Replacement".into()),
        ..operation(OperationType::InsertScene, "scene", "insert")
    };

    // When
    let result = try_create_change_set(
        &mut project,
        &mut history,
        vec![
            operation(OperationType::DeleteScene, "scene", "delete"),
            insert,
        ],
    );

    // Then
    assert!(matches!(result, Err(AppError::Conflict(_))));
    assert!(project.change_sets.is_empty());
    assert_eq!(project.scenes[0].title, "Scene");
}

#[test]
fn insert_scene_when_id_exists_or_source_is_not_visual_is_rejected_at_creation() {
    // Given
    let mut project = fixture_project();
    let mut history = history();
    let duplicate = Operation {
        source_id: Some("source".into()),
        in_frame: Some("0".into()),
        out_frame: Some("20".into()),
        title: Some("Duplicate".into()),
        ..operation(OperationType::InsertScene, "scene", "duplicate")
    };

    // When
    let duplicate_result = try_create_change_set(&mut project, &mut history, vec![duplicate]);
    let mut audio = project.sources[0].clone();
    audio.id = "audio".into();
    audio.width = 0;
    audio.height = 0;
    project.sources.push(audio);
    let audio_insert = Operation {
        source_id: Some("audio".into()),
        in_frame: Some("0".into()),
        out_frame: Some("20".into()),
        title: Some("Audio".into()),
        ..operation(OperationType::InsertScene, "new-scene", "audio")
    };
    let audio_result = try_create_change_set(&mut project, &mut history, vec![audio_insert]);

    // Then
    assert!(matches!(duplicate_result, Err(AppError::Conflict(_))));
    assert!(matches!(audio_result, Err(AppError::Invalid(_))));
    assert!(project.change_sets.is_empty());
    assert_eq!(project.scenes.len(), 1);
}

#[test]
fn insert_scene_when_range_exceeds_source_is_rejected_at_creation() {
    // Given
    let mut project = fixture_project();
    let mut history = history();
    let insert = Operation {
        source_id: Some("source".into()),
        in_frame: Some("90".into()),
        out_frame: Some("101".into()),
        title: Some("Outside".into()),
        ..operation(OperationType::InsertScene, "new-scene", "insert")
    };

    // When
    let result = try_create_change_set(&mut project, &mut history, vec![insert]);

    // Then
    assert!(matches!(result, Err(AppError::Invalid(_))));
    assert!(project.change_sets.is_empty());
    assert_eq!(project.scenes.len(), 1);
}

#[test]
fn update_scene_when_metadata_changed_after_creation_is_rejected() {
    // Given
    let mut project = fixture_project();
    let mut history = history();
    let update = Operation {
        title: Some("Proposed".into()),
        purpose: Some("Purpose".into()),
        caption: Some("Caption".into()),
        ..operation(OperationType::UpdateScene, "scene", "update")
    };
    create_change_set(&mut project, &mut history, vec![update]);
    let change_set_id = project.change_sets[0].id.clone();
    project.scenes[0].title = "Manual override".into();

    // When
    let result = apply_operations(
        &mut project,
        &mut history,
        Approval {
            change_set_id: &change_set_id,
            operation_ids: &["update"],
        },
    );

    // Then
    assert!(matches!(result, Err(AppError::Conflict(_))));
    assert_eq!(project.scenes[0].title, "Manual override");
    assert_eq!(project.change_sets[0].status, ChangeSetStatus::Ready);
}

#[test]
fn delete_scene_when_target_changed_locked_or_deleted_is_rejected() {
    // Given
    let mut projects = [fixture_project(), fixture_project(), fixture_project()];
    let mut histories = [history(), history(), history()];
    for (project, history) in projects.iter_mut().zip(histories.iter_mut()) {
        create_change_set(
            project,
            history,
            vec![operation(OperationType::DeleteScene, "scene", "delete")],
        );
    }
    projects[0].scenes[0].caption = "manual".into();
    projects[1].scenes[0].locked = true;
    projects[2].scenes.clear();

    // When
    let results = projects
        .iter_mut()
        .zip(histories.iter_mut())
        .map(|(project, history)| {
            let change_set_id = project.change_sets[0].id.clone();
            apply_operations(
                project,
                history,
                Approval {
                    change_set_id: &change_set_id,
                    operation_ids: &["delete"],
                },
            )
        })
        .collect::<Vec<_>>();

    // Then
    assert!(
        results
            .iter()
            .all(|result| matches!(result, Err(AppError::Conflict(_))))
    );
    assert_eq!(projects[0].scenes.len(), 1);
    assert_eq!(projects[1].scenes.len(), 1);
}

#[test]
fn selected_operations_when_one_precondition_fails_apply_nothing() {
    // Given
    let mut project = fixture_project();
    project.scenes.push(Scene {
        id: "second".into(),
        title: "Second".into(),
        ..project.scenes[0].clone()
    });
    let mut history = history();
    let update = Operation {
        title: Some("Changed".into()),
        purpose: Some(String::new()),
        caption: Some(String::new()),
        ..operation(OperationType::UpdateScene, "scene", "update")
    };
    create_change_set(
        &mut project,
        &mut history,
        vec![
            update,
            operation(OperationType::DeleteScene, "second", "delete"),
        ],
    );
    let change_set_id = project.change_sets[0].id.clone();
    project.scenes[1].locked = true;

    // When
    let result = apply_operations(
        &mut project,
        &mut history,
        Approval {
            change_set_id: &change_set_id,
            operation_ids: &["update", "delete"],
        },
    );

    // Then
    assert!(matches!(result, Err(AppError::Conflict(_))));
    assert_eq!(project.scenes[0].title, "Scene");
    assert_eq!(project.scenes.len(), 2);
    assert!(project.change_sets[0].applied_operation_ids.is_empty());
}

#[test]
fn invalid_camera_is_rejected_during_change_set_creation() {
    // Given
    let mut project = fixture_project();
    let mut history = history();
    let invalid_camera = Operation {
        camera: Some(Camera {
            x: 0.75,
            width: 0.5,
            ..valid_camera()
        }),
        ..operation(OperationType::SetCamera, "scene", "camera")
    };

    // When
    let result = try_create_change_set(&mut project, &mut history, vec![invalid_camera]);

    // Then
    assert!(matches!(result, Err(AppError::Invalid(_))));
    assert!(project.change_sets.is_empty());
}
