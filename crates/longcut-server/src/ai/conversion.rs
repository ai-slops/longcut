use std::collections::HashSet;

use super::{AiOperation, AiProposal};
use crate::model::{Operation, OperationType};

pub fn into_operations(proposal: &AiProposal) -> (Vec<Operation>, Vec<String>) {
    let mut evidence = Vec::new();
    let mut seen_evidence = HashSet::new();
    let operations = proposal
        .operations
        .iter()
        .map(|operation| {
            for id in operation.evidence_ids() {
                if seen_evidence.insert(id.clone()) {
                    evidence.push(id.clone());
                }
            }
            let id = uuid::Uuid::new_v4().to_string();
            match operation {
                AiOperation::InsertScene {
                    source_id,
                    in_frame,
                    out_frame,
                    title,
                    rationale,
                    ..
                } => Operation {
                    id,
                    scene_id: uuid::Uuid::new_v4().to_string(),
                    kind: OperationType::InsertScene,
                    camera: None,
                    in_frame: Some(in_frame.clone()),
                    out_frame: Some(out_frame.clone()),
                    source_id: Some(source_id.clone()),
                    title: Some(title.clone()),
                    purpose: None,
                    caption: None,
                    rationale: rationale.clone(),
                },
                AiOperation::TrimScene {
                    scene_id,
                    in_frame,
                    out_frame,
                    rationale,
                    ..
                } => Operation {
                    id,
                    scene_id: scene_id.clone(),
                    kind: OperationType::TrimScene,
                    camera: None,
                    in_frame: Some(in_frame.clone()),
                    out_frame: Some(out_frame.clone()),
                    source_id: None,
                    title: None,
                    purpose: None,
                    caption: None,
                    rationale: rationale.clone(),
                },
                AiOperation::SetCamera {
                    scene_id,
                    camera,
                    rationale,
                    ..
                } => Operation {
                    id,
                    scene_id: scene_id.clone(),
                    kind: OperationType::SetCamera,
                    camera: camera.clone(),
                    in_frame: None,
                    out_frame: None,
                    source_id: None,
                    title: None,
                    purpose: None,
                    caption: None,
                    rationale: rationale.clone(),
                },
                AiOperation::UpdateScene {
                    scene_id,
                    title,
                    purpose,
                    caption,
                    rationale,
                    ..
                } => Operation {
                    id,
                    scene_id: scene_id.clone(),
                    kind: OperationType::UpdateScene,
                    camera: None,
                    in_frame: None,
                    out_frame: None,
                    source_id: None,
                    title: Some(title.clone()),
                    purpose: Some(purpose.clone()),
                    caption: Some(caption.clone()),
                    rationale: rationale.clone(),
                },
                AiOperation::DeleteScene {
                    scene_id,
                    rationale,
                    ..
                } => Operation {
                    id,
                    scene_id: scene_id.clone(),
                    kind: OperationType::DeleteScene,
                    camera: None,
                    in_frame: None,
                    out_frame: None,
                    source_id: None,
                    title: None,
                    purpose: None,
                    caption: None,
                    rationale: rationale.clone(),
                },
            }
        })
        .collect();
    (operations, evidence)
}
