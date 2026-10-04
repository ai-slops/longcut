use crate::model::Package;

pub(super) fn within_budget(package: &Package) -> bool {
    let streams = package
        .registry
        .assets
        .iter()
        .fold(0_usize, |count, asset| {
            count.saturating_add(asset.streams.len())
        });
    let bindings = package
        .registry
        .sources
        .iter()
        .fold(0_usize, |count, source| {
            count.saturating_add(source.media_bindings.len())
        });
    let channels = package
        .registry
        .sources
        .iter()
        .flat_map(|source| &source.media_bindings)
        .fold(0_usize, |count, binding| {
            count.saturating_add(binding.channel_indices.as_ref().map_or(0, Vec::len))
        });
    let segments = package
        .time_maps
        .as_ref()
        .map_or(0, |track| track.segments.len());
    let health = package
        .media_health
        .as_ref()
        .map_or(0, |track| track.intervals.len());
    let visuals = package.visual_composition.as_ref().map_or(0, |track| {
        track
            .intervals
            .iter()
            .fold(track.intervals.len(), |count, interval| {
                count.saturating_add(interval.contributions.len())
            })
    });
    let audio = package.audio_mix.as_ref().map_or(0, |track| {
        track
            .intervals
            .iter()
            .fold(track.intervals.len(), |count, interval| {
                count.saturating_add(interval.contributions.len())
            })
    });
    let total = [
        streams,
        bindings,
        channels,
        segments,
        health,
        visuals,
        audio,
        package.registry.assets.len(),
        package.registry.sources.len(),
    ]
    .into_iter()
    .fold(0_usize, usize::saturating_add);
    let work = segments
        .saturating_mul(segments)
        .saturating_add(
            bindings
                .saturating_mul(bindings)
                .saturating_mul(channels.max(1)),
        )
        .saturating_add(streams.saturating_mul(health))
        .saturating_add(segments.saturating_mul(health))
        .saturating_add(
            visuals
                .saturating_add(audio)
                .saturating_mul(bindings)
                .saturating_mul(channels.max(1)),
        );
    total <= 20_000 && work <= 1_000_000
}
