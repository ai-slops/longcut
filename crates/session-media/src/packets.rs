use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader};

use crate::{
    Options,
    probe::{self, Probe, ProbeError},
};

mod coverage;

#[derive(Debug)]
pub(crate) struct Envelope {
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub count: u64,
    pub complete: bool,
    pub ordered: bool,
    pub coverage_resolved: bool,
    pub gap_count: u64,
    pub overlap_count: u64,
    pub max_gap: Option<i64>,
    pub(crate) previous_pts: Option<i64>,
    pub(crate) frontier: Option<i64>,
    pub(crate) pending: std::collections::BinaryHeap<std::cmp::Reverse<(i64, i64)>>,
    pub(crate) last_emitted: Option<i64>,
}

impl Default for Envelope {
    fn default() -> Self {
        Self {
            start: None,
            end: None,
            count: 0,
            complete: true,
            ordered: true,
            coverage_resolved: true,
            gap_count: 0,
            overlap_count: 0,
            max_gap: None,
            previous_pts: None,
            frontier: None,
            pending: std::collections::BinaryHeap::new(),
            last_emitted: None,
        }
    }
}

pub(crate) type Scan = BTreeMap<u32, Envelope>;

pub(crate) async fn inspect(
    path: &Path,
    probe: &Probe,
    options: &Options,
) -> Result<Scan, ProbeError> {
    let mut command = probe::input_command(path, options);
    command.args([
        "-show_packets",
        "-show_entries",
        "packet=stream_index,pts,duration:packet_side_data=",
        "-of",
        "compact=p=0:nk=0",
    ]);
    let streams = probe.streams.iter().map(|stream| stream.index).collect();
    probe::run(command, |stdout| read(stdout, &streams), options).await
}

async fn read(reader: impl AsyncRead + Unpin, streams: &BTreeSet<u32>) -> Result<Scan, ProbeError> {
    let mut reader = BufReader::new(reader);
    let mut line = Vec::new();
    let mut result = Scan::new();
    loop {
        line.clear();
        if (&mut reader)
            .take(4097)
            .read_until(b'\n', &mut line)
            .await?
            == 0
        {
            break;
        }
        if line.len() > 4096 {
            return Err(ProbeError::OutputLimit);
        }
        let text = std::str::from_utf8(&line)
            .map_err(|error| ProbeError::Invalid(error.to_string()))?
            .trim();
        if text.is_empty() {
            continue;
        }
        let packet = parse(text)?;
        if !streams.contains(&packet.stream_index) {
            return Err(ProbeError::Invalid(
                "packet references an unknown stream".into(),
            ));
        }
        let entry = result.entry(packet.stream_index).or_default();
        entry.count = entry.count.checked_add(1).ok_or(ProbeError::OutputLimit)?;
        let Some(pts) = packet.pts else {
            entry.complete = false;
            continue;
        };
        if entry.previous_pts.is_some_and(|previous| pts < previous) {
            entry.ordered = false;
        }
        entry.previous_pts = Some(pts);
        entry.start = Some(entry.start.map_or(pts, |start| start.min(pts)));
        match packet
            .duration
            .filter(|duration| *duration > 0)
            .and_then(|duration| pts.checked_add(duration))
        {
            Some(end) => {
                entry.queue_coverage(pts, end)?;
                entry.end = Some(entry.end.map_or(end, |previous| previous.max(end)));
            }
            None => entry.complete = false,
        }
    }
    for envelope in result.values_mut() {
        envelope.finish_coverage()?;
    }
    Ok(result)
}

struct Packet {
    stream_index: u32,
    pts: Option<i64>,
    duration: Option<i64>,
}

fn parse(line: &str) -> Result<Packet, ProbeError> {
    let mut fields = [None; 3];
    for field in line.split('|').filter(|field| !field.is_empty()) {
        let (key, value) = field
            .split_once('=')
            .ok_or_else(|| ProbeError::Invalid("invalid packet record".into()))?;
        let index = match key {
            "stream_index" => 0,
            "pts" => 1,
            "duration" => 2,
            _ => return Err(ProbeError::Invalid("unexpected packet field".into())),
        };
        if fields[index].replace(value).is_some() {
            return Err(ProbeError::Invalid("duplicate packet field".into()));
        }
    }
    let timestamp = |value: Option<&str>| -> Result<Option<i64>, ProbeError> {
        match value {
            None | Some("N/A") => Ok(None),
            Some(value) => value
                .parse()
                .map(Some)
                .map_err(|_| ProbeError::Invalid("invalid packet integer".into())),
        }
    };
    Ok(Packet {
        stream_index: fields[0]
            .ok_or_else(|| ProbeError::Invalid("packet stream index missing".into()))?
            .parse()
            .map_err(|_| ProbeError::Invalid("invalid packet stream index".into()))?,
        pts: timestamp(fields[1])?,
        duration: timestamp(fields[2])?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reorder_negative_and_large_pts_preserve_exact_envelope() -> Result<(), ProbeError> {
        let scan = read(b"stream_index=0|pts=9007199254740993|duration=1\nstream_index=0|pts=-1024|duration=1024|\n\n".as_slice(), &BTreeSet::from([0])).await?;
        assert_eq!(scan[&0].start, Some(-1024));
        assert_eq!(scan[&0].end, Some(9_007_199_254_740_994));
        assert!(scan[&0].complete);
        Ok(())
    }

    #[tokio::test]
    async fn absent_or_unusable_timing_never_becomes_a_complete_envelope() -> Result<(), ProbeError>
    {
        for row in [
            "stream_index=0|pts=N/A|duration=1",
            "stream_index=0|pts=0",
            "stream_index=0|pts=0|duration=0",
            "stream_index=0|pts=9223372036854775807|duration=1",
        ] {
            let scan = read(row.as_bytes(), &BTreeSet::from([0])).await?;
            assert!(!scan[&0].complete);
        }
        Ok(())
    }

    #[tokio::test]
    async fn malformed_unknown_stream_and_oversized_records_fail() {
        for row in [
            "stream_index=0|pts=1.5|duration=1",
            "stream_index=0|pts=1|pts=2",
            "stream_index=1|pts=0|duration=1",
        ] {
            assert!(read(row.as_bytes(), &BTreeSet::from([0])).await.is_err());
        }
        assert!(matches!(
            read(vec![b'x'; 4097].as_slice(), &BTreeSet::from([0])).await,
            Err(ProbeError::OutputLimit)
        ));
    }

    #[tokio::test]
    async fn ordered_packet_gaps_and_overlaps_are_counted_without_buffering()
    -> Result<(), ProbeError> {
        // Given
        let rows = b"stream_index=0|pts=0|duration=10\nstream_index=0|pts=15|duration=10\nstream_index=0|pts=20|duration=10\n";

        // When
        let scan = read(rows.as_slice(), &BTreeSet::from([0])).await?;

        // Then
        assert_eq!(scan[&0].gap_count, 1);
        assert_eq!(scan[&0].overlap_count, 1);
        assert_eq!(scan[&0].max_gap, Some(5));
        assert!(scan[&0].ordered);
        Ok(())
    }

    #[tokio::test]
    async fn reordered_arrival_retains_its_diagnostic_and_resolves_small_windows()
    -> Result<(), ProbeError> {
        // Given / When
        let scan = read(
            b"stream_index=0|pts=10|duration=10\nstream_index=0|pts=0|duration=10\n".as_slice(),
            &BTreeSet::from([0]),
        )
        .await?;

        // Then
        assert!(!scan[&0].ordered);
        assert!(scan[&0].coverage_resolved);
        assert_eq!(scan[&0].gap_count, 0);
        Ok(())
    }
}
