use super::Cue;
use crate::error::{AppError, AppResult};

pub(super) fn cues(text: &str, rate: u32, duration: u64) -> AppResult<Vec<Cue>> {
    if text.len() > 2 * 1024 * 1024 {
        return Err(AppError::Invalid("transcript exceeds 2 MiB".into()));
    }
    let normalized = text
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let normalized = normalized
        .lines()
        .map(|line| if line.trim().is_empty() { "" } else { line })
        .collect::<Vec<_>>()
        .join("\n");
    let mut cues = Vec::new();
    for block in normalized
        .split("\n\n")
        .filter(|block| !block.trim().is_empty())
    {
        let lines: Vec<_> = block.trim_matches('\n').lines().collect();
        if lines.first().is_some_and(|line| {
            *line == "WEBVTT"
                || line.starts_with("WEBVTT ")
                || *line == "NOTE"
                || line.starts_with("NOTE ")
                || *line == "STYLE"
                || *line == "REGION"
        }) {
            continue;
        }
        let position = lines
            .iter()
            .position(|line| line.contains(" --> "))
            .ok_or_else(|| AppError::Invalid("subtitle block has no timing line".into()))?;
        if position > 1 {
            return Err(AppError::Invalid("invalid subtitle cue header".into()));
        }
        let (start, end) = lines[position]
            .split_once(" --> ")
            .ok_or_else(|| AppError::Invalid("invalid subtitle timing".into()))?;
        let end = end
            .split_whitespace()
            .next()
            .ok_or_else(|| AppError::Invalid("missing subtitle end".into()))?;
        let start_ms = timestamp(start)?;
        let end_ms = timestamp(end)?;
        if start_ms >= end_ms {
            return Err(AppError::Invalid("subtitle ranges must increase".into()));
        }
        let start_frame = start_ms
            .checked_mul(u64::from(rate))
            .ok_or_else(|| AppError::Invalid("subtitle timestamp overflow".into()))?
            / 1000;
        let end_frame = end_ms
            .checked_mul(u64::from(rate))
            .and_then(|value| value.checked_add(999))
            .ok_or_else(|| AppError::Invalid("subtitle timestamp overflow".into()))?
            / 1000;
        let body = lines[position + 1..].join("\n");
        if body.trim().is_empty() || body.len() > 16 * 1024 || end_frame > duration {
            return Err(AppError::Invalid(
                "empty/oversized cue or timing outside the source".into(),
            ));
        }
        cues.push(Cue {
            id: uuid::Uuid::new_v4().to_string(),
            start_frame: start_frame.to_string(),
            end_frame: end_frame.to_string(),
            text: body,
            words: None,
        });
        if cues.len() > 20_000 {
            return Err(AppError::Invalid("transcript exceeds 20000 cues".into()));
        }
    }
    if cues.is_empty() {
        return Err(AppError::Invalid("transcript contains no cues".into()));
    }
    Ok(cues)
}

fn timestamp(value: &str) -> AppResult<u64> {
    let invalid =
        || AppError::Invalid("expected HH:MM:SS.mmm or MM:SS.mmm subtitle timestamp".into());
    let normalized = value.trim().replace(',', ".");
    let (time, fraction) = normalized.split_once('.').ok_or_else(invalid)?;
    if fraction.len() != 3 || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid());
    }
    let parts: Vec<_> = time.split(':').collect();
    if !matches!(parts.len(), 2 | 3)
        || parts
            .iter()
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(invalid());
    }
    let numbers = parts
        .into_iter()
        .map(|part| part.parse::<u64>().map_err(|_| invalid()))
        .collect::<AppResult<Vec<_>>>()?;
    let (hours, minutes, seconds) = match numbers.as_slice() {
        [minutes, seconds] => (0, *minutes, *seconds),
        [hours, minutes, seconds] => (*hours, *minutes, *seconds),
        _ => return Err(invalid()),
    };
    if hours > 9999 || minutes >= 60 || seconds >= 60 {
        return Err(invalid());
    }
    Ok(((hours * 60 + minutes) * 60 + seconds) * 1000
        + fraction.parse::<u64>().map_err(|_| invalid())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn korean_srt_and_vtt_preserve_text_and_explicit_frame_rounding() -> AppResult<()> {
        let srt = cues(
            "1\r\n00:00:00,001 --> 00:00:01,001\r\n첫 번째 실패\r\n원인 설명\r\n",
            30,
            300,
        )?;
        assert_eq!(srt[0].start_frame, "0");
        assert_eq!(srt[0].end_frame, "31");
        assert_eq!(srt[0].text, "첫 번째 실패\n원인 설명");
        assert!(srt[0].words.is_none());
        let vtt = cues(
            "WEBVTT\n\n00:01.000 --> 00:02.000 align:start\n결과 확인",
            60,
            600,
        )?;
        assert_eq!(vtt[0].start_frame, "60");
        assert_eq!(vtt[0].end_frame, "120");
        Ok(())
    }

    #[test]
    fn malformed_and_out_of_source_cues_are_rejected() {
        for text in [
            "1\n00:00:02,000 --> 00:00:01,000\nwrong",
            "00:99.000 --> 01:00.000\nwrong",
            "00:00.000 --> 00:20.000\nlong",
            "no timing",
            "00:00.000 --> 00:01.000",
        ] {
            assert!(cues(text, 30, 300).is_err());
        }
    }
}
