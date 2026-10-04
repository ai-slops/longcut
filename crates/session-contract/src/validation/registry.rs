use std::collections::{BTreeMap, BTreeSet};

use crate::{Report, model::*};

pub(super) struct Index<'a> {
    pub sources: BTreeMap<SourceId, &'a Source>,
    pub streams: BTreeMap<MediaRef, &'a Stream>,
}

impl Index<'_> {
    pub fn has_kind(&self, source: &Source, kind: StreamKind) -> bool {
        source.media_bindings.iter().any(|binding| {
            self.streams
                .get(&MediaRef {
                    asset_id: binding.asset_id.clone(),
                    stream_index: binding.stream_index,
                })
                .is_some_and(|stream| stream.kind == kind)
        })
    }
}

impl Index<'_> {
    pub fn independent_kind(
        &self,
        source: &Source,
        program: Option<&Source>,
        kind: StreamKind,
    ) -> bool {
        source.media_bindings.iter().any(|binding| {
            if !self
                .streams
                .get(&MediaRef {
                    asset_id: binding.asset_id.clone(),
                    stream_index: binding.stream_index,
                })
                .is_some_and(|stream| stream.kind == kind)
            {
                return false;
            }
            let Some(program) = program else {
                return true;
            };
            source.source_id != program.source_id
                && !program
                    .media_bindings
                    .iter()
                    .any(|other| bindings_overlap(binding, other))
        })
    }
}

fn bindings_overlap(left: &Binding, right: &Binding) -> bool {
    left.asset_id == right.asset_id
        && left.stream_index == right.stream_index
        && match (&left.channel_indices, &right.channel_indices) {
            (Some(left), Some(right)) => left.iter().any(|channel| right.contains(channel)),
            _ => true,
        }
}

pub(super) fn index<'a>(package: &'a Package, report: &mut Report) -> Index<'a> {
    let mut index = Index {
        sources: BTreeMap::new(),
        streams: BTreeMap::new(),
    };
    let mut assets = BTreeSet::new();
    for (position, asset) in package.registry.assets.iter().enumerate() {
        let path = format!("sources.json/assets/{position}");
        if asset.asset_id.0.trim().is_empty() || !assets.insert(&asset.asset_id) {
            report.error(
                "DUPLICATE_OR_EMPTY_ID",
                &path,
                "asset IDs must be nonempty and unique",
            );
        }
        if !crate::loader::safe_relative(&asset.uri) {
            report.error(
                "UNSAFE_PATH",
                format!("{path}/uri"),
                "only literal package-relative media paths are supported",
            );
        }
        if asset.size_bytes.is_some_and(|size| size.get() < 0) {
            report.error("INVALID_SIZE", &path, "size must be nonnegative");
        }
        if asset.streams.is_empty() {
            report.error("EMPTY_STREAMS", &path, "asset must declare streams");
        }
        for stream in &asset.streams {
            let key = MediaRef {
                asset_id: asset.asset_id.clone(),
                stream_index: stream.stream_index,
            };
            if index.streams.insert(key, stream).is_some() {
                report.error(
                    "DUPLICATE_STREAM",
                    &path,
                    "stream indexes must be unique per asset",
                );
            }
            if stream.codec.trim().is_empty()
                || [
                    stream.width,
                    stream.height,
                    stream.sample_rate,
                    stream.channels,
                ]
                .into_iter()
                .flatten()
                .any(|value| value == 0)
            {
                report.error(
                    "INVALID_STREAM",
                    &path,
                    "codec and provided dimensions/sample/channel counts must be valid",
                );
            }
            for ratio in [stream.frame_rate.as_ref(), stream.time_base.as_ref()]
                .into_iter()
                .flatten()
            {
                if ratio.time_base().is_err() {
                    report.error("INVALID_RATE", &path, "rates must be positive rationals");
                }
            }
        }
    }
    if package.registry.sources.is_empty() {
        report.error(
            "EMPTY_SOURCES",
            "sources.json/sources",
            "at least one source is required",
        );
    }
    for (position, source) in package.registry.sources.iter().enumerate() {
        let path = format!("sources.json/sources/{position}");
        if source.source_id.0.trim().is_empty()
            || index
                .sources
                .insert(source.source_id.clone(), source)
                .is_some()
        {
            report.error(
                "DUPLICATE_OR_EMPTY_ID",
                &path,
                "source IDs must be nonempty and unique",
            );
        }
        if source.semantic_role.trim().is_empty() || source.media_bindings.is_empty() {
            report.error(
                "INVALID_SOURCE",
                &path,
                "role and media bindings are required",
            );
        }
        for (offset, binding) in source.media_bindings.iter().enumerate() {
            let binding_path = format!("{path}/media_bindings/{offset}");
            let key = MediaRef {
                asset_id: binding.asset_id.clone(),
                stream_index: binding.stream_index,
            };
            let Some(stream) = index.streams.get(&key) else {
                report.error(
                    "UNKNOWN_STREAM",
                    &binding_path,
                    "binding must resolve to an asset stream",
                );
                continue;
            };
            if source.media_bindings[..offset]
                .iter()
                .any(|other| bindings_overlap(binding, other))
            {
                report.error("DUPLICATE_BINDING", &binding_path, "duplicate binding");
            }
            if let Some(channels) = &binding.channel_indices
                && (stream.kind != StreamKind::Audio
                    || channels.is_empty()
                    || channels.iter().collect::<BTreeSet<_>>().len() != channels.len()
                    || !stream
                        .channels
                        .is_some_and(|count| channels.iter().all(|channel| *channel < count)))
            {
                report.error(
                    "INVALID_CHANNELS",
                    &binding_path,
                    "channel selection requires unique valid audio channel indexes",
                );
            }
        }
        let video = index.has_kind(source, StreamKind::Video);
        let audio = index.has_kind(source, StreamKind::Audio);
        let matches = match source.kind {
            SourceKind::Video => video && !audio,
            SourceKind::Audio => audio && !video,
            SourceKind::Av => video && audio,
        };
        if !matches {
            report.error(
                "SOURCE_KIND_MISMATCH",
                &path,
                "source kind must match its bound stream kinds",
            );
        }
    }
    let program = package
        .manifest
        .program_reference_source_id
        .as_ref()
        .and_then(|id| index.sources.get(id).copied());
    match (&package.manifest.program_reference_source_id, program) {
        (Some(_), None) => report.error(
            "UNKNOWN_PROGRAM_REFERENCE",
            "manifest.json/program_reference_source_id",
            "reference source must resolve",
        ),
        (None, _) => report.warning(
            "PROGRAM_REFERENCE_MISSING",
            "manifest.json",
            "actual broadcast reference is unavailable",
        ),
        (Some(_), Some(source)) => {
            report.capabilities.program_reference = index.has_kind(source, StreamKind::Video)
                && index.has_kind(source, StreamKind::Audio);
            if !report.capabilities.program_reference {
                report.warning(
                    "PROGRAM_REFERENCE_INCOMPLETE",
                    "sources.json",
                    "reference must provide both broadcast video and audio",
                );
            }
        }
    }
    for source in index.sources.values() {
        report.capabilities.independent_video_sources |=
            index.independent_kind(source, program, StreamKind::Video);
        report.capabilities.independent_audio_sources |=
            index.independent_kind(source, program, StreamKind::Audio);
    }
    if !report.capabilities.independent_video_sources
        && !report.capabilities.independent_audio_sources
    {
        report.warning(
            "INDEPENDENT_SOURCES_MISSING",
            "sources.json",
            "no independent editing source is declared",
        );
    }
    index
}
