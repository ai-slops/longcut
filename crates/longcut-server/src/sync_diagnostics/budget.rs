use std::collections::BTreeMap;

use longcut_session_contract::model::TimeMapSegment;

use crate::model::SourceSessionRecord;

use super::dto::CalculationLimits;

const MAX_ITEMS: usize = 20_000;
const MAX_WORK: usize = 1_000_000;

pub(super) fn measure(
    record: &SourceSessionRecord,
    segments: Option<&[TimeMapSegment]>,
) -> CalculationLimits {
    let segment_count = segments.map_or(0, <[TimeMapSegment]>::len);
    let mut segments_by_stream: BTreeMap<(&str, u32), usize> = BTreeMap::new();
    for segment in segments.unwrap_or_default() {
        let count = segments_by_stream
            .entry((
                &segment.media_ref.asset_id.0,
                segment.media_ref.stream_index,
            ))
            .or_default();
        *count = count.saturating_add(1);
    }
    let mut binding_count = 0_usize;
    let mut emitted_segments = 0_usize;
    for binding in record
        .package
        .registry
        .sources
        .iter()
        .flat_map(|source| &source.media_bindings)
    {
        binding_count = binding_count.saturating_add(1);
        emitted_segments = emitted_segments.saturating_add(
            segments_by_stream
                .get(&(binding.asset_id.0.as_str(), binding.stream_index))
                .copied()
                .unwrap_or(0),
        );
    }
    let range_output_bound = if segments.is_some() {
        segment_count
            .saturating_mul(2)
            .saturating_add(segment_count.saturating_add(1))
    } else {
        0
    };
    let item_count = record
        .package
        .registry
        .sources
        .len()
        .saturating_add(binding_count)
        .saturating_add(emitted_segments)
        .saturating_add(range_output_bound);
    let work = binding_count
        .saturating_mul(binding_count)
        .saturating_add(segment_count.saturating_mul(segment_count));
    CalculationLimits {
        max_items: MAX_ITEMS.to_string(),
        max_work: MAX_WORK.to_string(),
        observed_items: item_count.to_string(),
        observed_work: work.to_string(),
        exceeded: item_count > MAX_ITEMS || work > MAX_WORK,
    }
}
