use super::types::{TranscriptSegment, TranscriptUpdate};
use crate::utils::format_timestamp;
use log::info;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

// Global sequence counter for transcript ordering
static SEQUENCE_COUNTER: AtomicU64 = AtomicU64::new(0);

// Configuration
const SENTENCE_TIMEOUT_MS: u64 = 1000; // Emit incomplete sentence after 1 second of silence

#[derive(Debug)]
pub struct TranscriptAccumulator {
    pub current_sentence: String,
    sentence_start_time: f32,
    last_update_time: Instant,
    last_segment_hash: u64,
    current_chunk_id: u64,
    current_chunk_start_time: f64,
    recording_start_time: Option<Instant>,
}

impl TranscriptAccumulator {
    pub fn new() -> Self {
        Self {
            current_sentence: String::new(),
            sentence_start_time: 0.0,
            last_update_time: Instant::now(),
            last_segment_hash: 0,
            current_chunk_id: 0,
            current_chunk_start_time: 0.0,
            recording_start_time: None,
        }
    }

    pub fn set_chunk_context(
        &mut self,
        chunk_id: u64,
        chunk_start_time: f64,
        recording_start_time: Instant,
    ) {
        self.current_chunk_id = chunk_id;
        self.current_chunk_start_time = chunk_start_time;
        self.recording_start_time = Some(recording_start_time);
    }

    pub fn add_segment(&mut self, segment: &TranscriptSegment) -> Option<TranscriptUpdate> {
        info!("Processing new transcript segment: {:?}", segment);

        // Update the last update time
        self.last_update_time = Instant::now();

        // Clean up the text
        let clean_text = segment
            .text
            .replace("[BLANK_AUDIO]", "")
            .replace("[AUDIO OUT]", "")
            .trim()
            .to_string();

        if !clean_text.is_empty() {
            info!("Clean transcript text: {}", clean_text);
        }

        // Skip empty or very short segments
        if clean_text.is_empty() || (segment.t1 - segment.t0) < 1.0 {
            return None;
        }

        // Calculate hash to detect duplicates
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        segment.text.hash(&mut hasher);
        segment.t0.to_bits().hash(&mut hasher);
        segment.t1.to_bits().hash(&mut hasher);
        self.current_chunk_id.hash(&mut hasher);
        let segment_hash = hasher.finish();

        // Skip duplicates
        if segment_hash == self.last_segment_hash {
            info!("Skipping duplicate segment: {}", clean_text);
            return None;
        }
        self.last_segment_hash = segment_hash;

        // Track sentence start time
        if self.current_sentence.is_empty() {
            self.sentence_start_time = segment.t0;
        }

        // Add text with proper spacing
        if !self.current_sentence.is_empty() && !self.current_sentence.ends_with(' ') {
            self.current_sentence.push(' ');
        }
        self.current_sentence.push_str(&clean_text);

        // Check for sentence ending
        let has_sentence_ending = clean_text.ends_with('.')
            || clean_text.ends_with('?')
            || clean_text.ends_with('!')
            || clean_text.ends_with("...")
            || clean_text.ends_with(".\"")
            || clean_text.ends_with(".'");

        if has_sentence_ending {
            self.create_complete_update()
        } else {
            None
        }
    }

    pub fn check_timeout(&mut self) -> Option<TranscriptUpdate> {
        if !self.current_sentence.is_empty()
            && self.last_update_time.elapsed() > Duration::from_millis(SENTENCE_TIMEOUT_MS)
        {
            Some(self.create_partial_update())
        } else {
            None
        }
    }

    pub fn create_partial_update(&mut self) -> TranscriptUpdate {
        let sentence = std::mem::take(&mut self.current_sentence);
        let sequence_id = SEQUENCE_COUNTER.fetch_add(1, Ordering::SeqCst);

        let start_elapsed = self.calculate_elapsed_time(self.sentence_start_time);

        TranscriptUpdate {
            text: sentence.trim().to_string(),
            timestamp: format_timestamp(start_elapsed),
            source: "Mixed Audio".to_string(),
            sequence_id,
            chunk_start_time: self.current_chunk_start_time,
            is_partial: true,
        }
    }

    fn create_complete_update(&mut self) -> Option<TranscriptUpdate> {
        let sentence = std::mem::take(&mut self.current_sentence);
        let sequence_id = SEQUENCE_COUNTER.fetch_add(1, Ordering::SeqCst);

        let start_elapsed = self.calculate_elapsed_time(self.sentence_start_time);

        let update = TranscriptUpdate {
            text: sentence.trim().to_string(),
            timestamp: format_timestamp(start_elapsed),
            source: "Mixed Audio".to_string(),
            sequence_id,
            chunk_start_time: self.current_chunk_start_time,
            is_partial: false,
        };

        info!("Generated transcript update: {:?}", update);
        Some(update)
    }

    fn calculate_elapsed_time(&self, time_in_chunk: f32) -> f64 {
        let elapsed = self.current_chunk_start_time + (time_in_chunk as f64 / 1000.0);
        elapsed.max(0.0)
    }
}
