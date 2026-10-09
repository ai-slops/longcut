use std::path::Path;

use super::attempt_work_path;

#[test]
fn resumed_attempts_use_distinct_children_of_the_job_directory() {
    // Given
    let root = Path::new("asr-work");
    let job_id = "job-id";
    // When
    let first = attempt_work_path(root, job_id);
    let second = attempt_work_path(root, job_id);
    // Then
    assert_ne!(first, second);
    assert_eq!(first.parent(), Some(root.join(job_id).as_path()));
    assert_eq!(second.parent(), Some(root.join(job_id).as_path()));
}
