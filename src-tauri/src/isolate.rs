//! Isolated Voice: filters a recorded clip down to the primary user's
//! speech before it reaches Whisper. Runs once, post-capture, on the full
//! buffered clip — not live/streaming — matching the app's push-to-talk
//! (not continuous) architecture.
//!
//! Two paths, chosen by `apply` based on enrollment state:
//! - Enrolled: speaker-embedding similarity (see `voice_isolation.rs`).
//! - Not enrolled (or the embedding path is unavailable): the energy gate
//!   here alone. It can only suppress quiet background noise, not
//!   distinguish a second human voice at similar volume — Settings copy
//!   must say so; don't let a caller believe this is as strong as the
//!   enrolled path.

use tauri::{AppHandle, Manager};

use crate::voice_isolation::VoiceIsolationState;

/// ~20ms per window at 16kHz.
const WINDOW_SIZE: usize = 320;
/// ~150ms of padding kept on both edges of a voiced range so trailing
/// consonants (which trail off in energy before the word actually ends)
/// aren't clipped.
const HANGOVER_WINDOWS: usize = 8;

/// Enter threshold is higher than exit so a window that's borderline right
/// after speech starts/ends doesn't cause the gate to chatter open/closed.
const ENTER_RMS: f32 = 0.02;
const EXIT_RMS: f32 = 0.01;

/// A voiced range whose embedding's cosine similarity to the enrolled
/// profile falls below this gets masked out as "not the primary speaker".
/// Starting point only — cosine similarity between two same-speaker
/// WeSpeaker embeddings is typically well above this and different-speaker
/// pairs well below, but the real threshold needs empirical tuning during
/// manual verification (see the phase-2 plan).
const SIMILARITY_THRESHOLD: f32 = 0.5;

/// Below this, a voiced range is too short for a reliable standalone
/// embedding. It's still masked based on its own boundaries, but the
/// embedding used to score it is computed over a widened window of
/// surrounding audio for context.
const MIN_SCORE_SAMPLES: usize = 8_000; // 0.5s @ 16kHz

/// Returns `[start, end)` sample-index ranges classified as voiced, using
/// short-window RMS with hysteresis. Also the segmenter the embedding path
/// (`voice_isolation.rs`) scores against — written once, used by both.
pub(crate) fn energy_gate(samples: &[f32]) -> Vec<(usize, usize)> {
    if samples.is_empty() {
        return Vec::new();
    }

    let mut windows_voiced = Vec::with_capacity(samples.len() / WINDOW_SIZE + 1);
    let mut is_voiced = false;
    for chunk in samples.chunks(WINDOW_SIZE) {
        let rms = rms(chunk);
        is_voiced = if is_voiced {
            rms >= EXIT_RMS
        } else {
            rms >= ENTER_RMS
        };
        windows_voiced.push(is_voiced);
    }

    // Merge voiced windows into ranges, then pad each with hangover and
    // merge any ranges that now overlap as a result.
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < windows_voiced.len() {
        if windows_voiced[i] {
            let start_window = i;
            while i < windows_voiced.len() && windows_voiced[i] {
                i += 1;
            }
            let end_window = i;

            let start = start_window.saturating_sub(HANGOVER_WINDOWS) * WINDOW_SIZE;
            let end = ((end_window + HANGOVER_WINDOWS) * WINDOW_SIZE).min(samples.len());
            ranges.push((start, end));
        } else {
            i += 1;
        }
    }

    merge_overlapping(ranges)
}

fn rms(chunk: &[f32]) -> f32 {
    if chunk.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = chunk.iter().map(|s| s * s).sum();
    (sum_sq / chunk.len() as f32).sqrt()
}

fn merge_overlapping(mut ranges: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    ranges.sort_by_key(|&(start, _)| start);
    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(ranges.len());
    for (start, end) in ranges {
        if let Some(last) = merged.last_mut() {
            if start <= last.1 {
                last.1 = last.1.max(end);
                continue;
            }
        }
        merged.push((start, end));
    }
    merged
}

/// Zero-fills every sample NOT covered by `voiced`. Preserves total buffer
/// length (rather than trimming+concatenating) so whisper.cpp's
/// segment/timestamp handling still sees a continuous timeline, and a
/// fully-masked buffer degrades into the existing "no speech detected"
/// path in `recording.rs` instead of needing new error handling.
pub(crate) fn mask_unvoiced(samples: &mut [f32], voiced: &[(usize, usize)]) {
    let mut cursor = 0;
    for &(start, end) in voiced {
        let start = start.min(samples.len());
        let end = end.min(samples.len());
        if start > cursor {
            samples[cursor..start].fill(0.0);
        }
        cursor = cursor.max(end);
    }
    if cursor < samples.len() {
        samples[cursor..].fill(0.0);
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

/// Widens a too-short range with surrounding (mostly trailing) context,
/// purely so the embedding extractor has enough audio to work with —
/// callers still mask based on the original, un-widened range. Extends
/// forward from `start` first and only pulls the window backward if that
/// would run past `total_len`, which guarantees the target width is always
/// reached (short of the whole buffer itself being shorter than it).
fn widen_for_scoring(range: (usize, usize), total_len: usize) -> (usize, usize) {
    let (start, end) = range;
    let target = MIN_SCORE_SAMPLES.min(total_len);
    if end.saturating_sub(start) >= target {
        return range;
    }
    let new_end = (start + target).min(total_len);
    let new_start = new_end.saturating_sub(target);
    (new_start, new_end)
}

/// No-op passthrough if Isolated Voice is off. Otherwise masks out
/// non-primary-speaker audio: the embedding path if a voice is enrolled,
/// else the energy-gate-only heuristic.
pub fn apply(app: &AppHandle, samples: Vec<f32>) -> Vec<f32> {
    let cfg = crate::config::load(app);
    if !cfg.isolated_voice_enabled {
        return samples;
    }

    let voiced = energy_gate(&samples);
    let mut samples = samples;

    let vi_state = app.state::<VoiceIsolationState>();
    let voiced = match vi_state.enrolled_embedding() {
        Some(enrolled) => voiced
            .into_iter()
            .filter(|&range| {
                let scoring_range = widen_for_scoring(range, samples.len());
                match vi_state.compute_embedding(app, &samples, std::slice::from_ref(&scoring_range))
                {
                    Ok(embedding) => cosine_similarity(&embedding, &enrolled) >= SIMILARITY_THRESHOLD,
                    // Fail open: an embedding hiccup shouldn't silently
                    // drop real speech — worst case this segment just falls
                    // back to the energy-gate's judgment for this clip.
                    Err(err) => {
                        crate::applog!("isolate: embedding scoring failed, keeping segment: {err}");
                        true
                    }
                }
            })
            .collect(),
        None => voiced,
    };

    mask_unvoiced(&mut samples, &voiced);
    samples
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: u32 = 16_000;

    fn tone(len: usize, amplitude: f32) -> Vec<f32> {
        (0..len)
            .map(|i| amplitude * (i as f32 * 0.3).sin())
            .collect()
    }

    #[test]
    fn silence_produces_no_voiced_ranges() {
        let samples = vec![0.0; SAMPLE_RATE as usize];
        assert!(energy_gate(&samples).is_empty());
    }

    #[test]
    fn empty_input_does_not_panic() {
        assert!(energy_gate(&[]).is_empty());
    }

    #[test]
    fn constant_loud_tone_is_one_voiced_range_spanning_the_buffer() {
        let samples = tone(SAMPLE_RATE as usize, 0.5);
        let ranges = energy_gate(&samples);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].0, 0);
        assert_eq!(ranges[0].1, samples.len());
    }

    #[test]
    fn tone_silence_tone_produces_two_separated_ranges() {
        let loud = tone(8_000, 0.5);
        let quiet = vec![0.0; 8_000];
        let mut samples = loud.clone();
        samples.extend(&quiet);
        samples.extend(&loud);

        let ranges = energy_gate(&samples);
        assert_eq!(ranges.len(), 2);
        assert!(ranges[0].1 < ranges[1].0, "ranges should stay separated by the silence gap");
    }

    #[test]
    fn mask_unvoiced_zeroes_everything_outside_voiced_ranges() {
        let mut samples = vec![1.0f32; 100];
        mask_unvoiced(&mut samples, &[(20, 40), (60, 80)]);

        assert!(samples[0..20].iter().all(|&s| s == 0.0));
        assert!(samples[20..40].iter().all(|&s| s == 1.0));
        assert!(samples[40..60].iter().all(|&s| s == 0.0));
        assert!(samples[60..80].iter().all(|&s| s == 1.0));
        assert!(samples[80..100].iter().all(|&s| s == 0.0));
        assert_eq!(samples.len(), 100, "masking must not change buffer length");
    }

    #[test]
    fn mask_unvoiced_with_no_voiced_ranges_zeroes_everything() {
        let mut samples = vec![1.0f32; 50];
        mask_unvoiced(&mut samples, &[]);
        assert!(samples.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn apply_is_passthrough_when_disabled() {
        // isolated_voice_enabled defaults to false, and `apply` reads it
        // straight from config — a fresh, unconfigured app should never
        // silently start filtering audio.
        assert!(!crate::config::AppConfig::default().isolated_voice_enabled);
    }

    #[test]
    fn cosine_similarity_of_identical_vectors_is_one() {
        let v = vec![0.4, -0.2, 0.8, 0.1];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_of_orthogonal_vectors_is_zero() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!(cosine_similarity(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_of_opposite_vectors_is_negative_one() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![-1.0, -2.0, -3.0];
        assert!((cosine_similarity(&a, &b) + 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_of_near_identical_vectors_clears_the_accept_threshold() {
        // A stand-in for "same speaker, two different clips" — embeddings
        // won't be bit-identical, but a small perturbation should still
        // clear SIMILARITY_THRESHOLD.
        let a = vec![0.5, 0.5, 0.5, 0.5];
        let b = vec![0.52, 0.48, 0.51, 0.49];
        assert!(cosine_similarity(&a, &b) >= SIMILARITY_THRESHOLD);
    }

    #[test]
    fn cosine_similarity_of_dissimilar_vectors_falls_below_the_accept_threshold() {
        // A stand-in for "different speaker" — should read as clearly
        // rejected, not just barely under the threshold.
        let a = vec![1.0, 0.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0, 1.0];
        assert!(cosine_similarity(&a, &b) < SIMILARITY_THRESHOLD);
    }

    #[test]
    fn mismatched_length_vectors_are_treated_as_dissimilar_rather_than_panicking() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn widen_for_scoring_leaves_long_enough_ranges_untouched() {
        let range = (100, 100 + MIN_SCORE_SAMPLES);
        assert_eq!(widen_for_scoring(range, 1_000_000), range);
    }

    #[test]
    fn widen_for_scoring_pads_short_ranges_up_to_the_minimum_without_exceeding_bounds() {
        let (start, end) = widen_for_scoring((10, 20), 1_000_000);
        assert!(end - start >= MIN_SCORE_SAMPLES);
        assert!(start <= 10);
        assert!(end >= 20);
    }

    #[test]
    fn widen_for_scoring_clamps_to_buffer_bounds_near_the_edges() {
        let total_len = 5_000;
        let (start, end) = widen_for_scoring((0, 100), total_len);
        assert_eq!(start, 0);
        assert!(end <= total_len);
    }
}
