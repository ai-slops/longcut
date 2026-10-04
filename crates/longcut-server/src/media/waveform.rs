use std::{path::Path, process::Stdio, time::Duration};

use tokio::{io::AsyncReadExt, process::Command};
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};

use super::process;

const MAX_STORED_PEAKS: usize = 1024;
const MAX_FINAL_PEAKS: usize = 512;

pub(super) async fn extract(path: &Path, cancel: &CancellationToken) -> AppResult<Vec<f32>> {
    let mut command = Command::new("ffmpeg");
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-protocol_whitelist",
            "file,pipe",
            "-i",
        ])
        .arg(path)
        .args([
            "-vn", "-ac", "1", "-ar", "8000", "-threads", "1", "-f", "s16le", "-",
        ])
        .stdout(Stdio::piped());
    let mut running = process::spawn(&mut command, cancel).await?;
    let mut stdout = running.take_stdout()?;
    let samples = tokio::spawn(async move {
        let mut reducer = PeakReducer::new();
        let mut bytes = [0_u8; 8192];
        loop {
            let read = stdout.read(&mut bytes).await?;
            if read == 0 {
                return Ok::<Vec<f32>, std::io::Error>(reducer.finish());
            }
            reducer.push_bytes(&bytes[..read]);
        }
    });
    let process_result = running
        .wait("waveform", Duration::from_secs(120), cancel)
        .await;
    let peaks = samples.await.map_err(|error| {
        AppError::Internal(anyhow::anyhow!("waveform reader failed: {error}"))
    })??;
    process_result?;
    Ok(peaks)
}

struct PeakReducer {
    peaks: Vec<f32>,
    samples_per_peak: usize,
    current_peak: f32,
    current_samples: usize,
    pending_byte: Option<u8>,
    #[cfg(test)]
    largest_peak_count: usize,
}

impl PeakReducer {
    fn new() -> Self {
        Self {
            peaks: Vec::with_capacity(MAX_STORED_PEAKS),
            samples_per_peak: 1,
            current_peak: 0.0,
            current_samples: 0,
            pending_byte: None,
            #[cfg(test)]
            largest_peak_count: 0,
        }
    }

    fn push_bytes(&mut self, bytes: &[u8]) {
        let mut offset = 0;
        if let Some(low) = self.pending_byte.take() {
            if let Some(&high) = bytes.first() {
                self.push_sample(i16::from_le_bytes([low, high]));
                offset = 1;
            } else {
                self.pending_byte = Some(low);
                return;
            }
        }
        while offset + 1 < bytes.len() {
            self.push_sample(i16::from_le_bytes([bytes[offset], bytes[offset + 1]]));
            offset += 2;
        }
        if offset < bytes.len() {
            self.pending_byte = Some(bytes[offset]);
        }
    }

    fn push_sample(&mut self, sample: i16) {
        self.current_peak = self
            .current_peak
            .max(f32::from(sample.unsigned_abs()) / 32768.0);
        self.current_samples += 1;
        if self.current_samples == self.samples_per_peak {
            self.store_current_peak();
        }
    }

    fn store_current_peak(&mut self) {
        self.peaks.push(self.current_peak);
        self.current_peak = 0.0;
        self.current_samples = 0;
        #[cfg(test)]
        {
            self.largest_peak_count = self.largest_peak_count.max(self.peaks.len());
        }
        if self.peaks.len() == MAX_STORED_PEAKS {
            for index in 0..MAX_STORED_PEAKS / 2 {
                self.peaks[index] = self.peaks[index * 2].max(self.peaks[index * 2 + 1]);
            }
            self.peaks.truncate(MAX_STORED_PEAKS / 2);
            self.samples_per_peak = self.samples_per_peak.saturating_mul(2);
        }
    }

    fn finish(mut self) -> Vec<f32> {
        if self.current_samples > 0 {
            self.store_current_peak();
        }
        let group_size = self.peaks.len().div_ceil(MAX_FINAL_PEAKS).max(1);
        self.peaks
            .chunks(group_size)
            .map(|group| group.iter().copied().fold(0.0, f32::max))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_FINAL_PEAKS, MAX_STORED_PEAKS, PeakReducer};

    #[test]
    fn reducer_handles_odd_read_boundaries() {
        // Given
        let samples = [i16::MIN, -16384, 0, i16::MAX];
        let bytes: Vec<u8> = samples.into_iter().flat_map(i16::to_le_bytes).collect();
        let mut reducer = PeakReducer::new();

        // When
        for byte in bytes {
            reducer.push_bytes(&[byte]);
        }
        let peaks = reducer.finish();

        // Then
        assert_eq!(peaks, vec![1.0, 0.5, 0.0, f32::from(i16::MAX) / 32768.0]);
    }

    #[test]
    fn reducer_bounds_intermediate_and_final_peak_storage() {
        // Given
        let mut reducer = PeakReducer::new();
        let bytes = i16::MAX.to_le_bytes();

        // When
        for _ in 0..100_000 {
            reducer.push_bytes(&bytes);
        }
        let largest_peak_count = reducer.largest_peak_count;
        let peaks = reducer.finish();

        // Then
        assert!(largest_peak_count <= MAX_STORED_PEAKS);
        assert!(peaks.len() <= MAX_FINAL_PEAKS);
    }

    #[test]
    fn compaction_keeps_uniform_bucket_width_before_the_next_sample() {
        let mut reducer = PeakReducer::new();
        for _ in 0..MAX_STORED_PEAKS + 1 {
            reducer.push_sample(0);
        }
        assert_eq!(reducer.samples_per_peak, 2);
        assert_eq!(reducer.peaks.len(), MAX_STORED_PEAKS / 2);
        assert_eq!(reducer.current_samples, 1);
    }
}
