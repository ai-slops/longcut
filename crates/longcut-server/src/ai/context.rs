use std::collections::HashSet;

use crate::model::{Project, Source, SourceRole};

use super::AiScope;

pub(super) fn scoped_sources<'a>(project: &'a Project, scope: &AiScope) -> Vec<&'a Source> {
    let allowed = match scope {
        AiScope::Source { source_id, .. } => HashSet::from([source_id.as_str()]),
        AiScope::Scenes { scene_ids } => {
            let selected = scene_ids.iter().map(String::as_str).collect::<HashSet<_>>();
            let mut allowed = project
                .scenes
                .iter()
                .filter(|scene| selected.contains(scene.id.as_str()))
                .flat_map(|scene| {
                    std::iter::once(scene.source_id.as_str())
                        .chain(scene.camera.iter().map(|camera| camera.source_id.as_str()))
                })
                .collect::<HashSet<_>>();
            allowed.extend(
                project
                    .sources
                    .iter()
                    .filter(|source| matches!(&source.role, SourceRole::Camera))
                    .map(|source| source.id.as_str()),
            );
            allowed
        }
    };
    project
        .sources
        .iter()
        .filter(|source| allowed.contains(source.id.as_str()))
        .collect()
}
