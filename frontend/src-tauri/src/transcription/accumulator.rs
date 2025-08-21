use super::types::{TranscriptSegment, TranscriptUpdate};
use crate::utils::format_timestamp;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

static SEQUENCE_COUNTER: AtomicU64 = AtomicU64::new(0);
const SENTENCE_TIMEOUT_MS: u64 = 1000; // Emit incomplete sentence after 1 second of silence

pub struct TranscriptAccumulator {
    pub current_sentence: String,
    pub sentence_start_time: u64,
    pub last_segment_time: Instant,
    pub current_chunk_id: u64,
    pub current_chunk_timestamp: f64,
    pub current_chunk_start_time: f64,
}

impl TranscriptAccumulator {
    pub fn new() -> Self {
        Self {
            current_sentence: String::new(),
            sentence_start_time: 0,
            last_segment_time: Instant::now(),
            current_chunk_id: 0,
            current_chunk_timestamp: 0.0,
            current_chunk_start_time: 0.0,
        }
    }

    pub fn set_chunk_context(&mut self, chunk_id: u64, timestamp: f64, recording_start_time: f64) {
        self.current_chunk_id = chunk_id;
        self.current_chunk_timestamp = timestamp;
        self.current_chunk_start_time = recording_start_time + timestamp;
    }

    pub fn add_segment(&mut self, segment: &TranscriptSegment) -> Option<TranscriptUpdate> {
        let text = segment.text.trim();
        if text.is_empty() {
            return None;
        }

        // Update timing
        self.last_segment_time = Instant::now();
        
        // If this is the start of a new sentence, record the timestamp
        if self.current_sentence.is_empty() {
            self.sentence_start_time = (segment.t0 * 1000.0) as u64;
        }

        // Add text to current sentence
        if !self.current_sentence.is_empty() && !self.current_sentence.ends_with(' ') {
            self.current_sentence.push(' ');
        }
        self.current_sentence.push_str(text);

        // Check if sentence is complete
        if self.is_sentence_complete() {
            let sequence_id = SEQUENCE_COUNTER.fetch_add(1, Ordering::SeqCst);
            let update = TranscriptUpdate {
                text: self.current_sentence.trim().to_string(),
                timestamp: format_timestamp(
                    self.current_chunk_start_time + (self.sentence_start_time as f64 / 1000.0)
                ),
                source: "Mixed Audio".to_string(),
                sequence_id,
                is_partial: false,
            };

            // Reset for next sentence
            self.current_sentence.clear();
            self.sentence_start_time = 0;

            Some(update)
        } else {
            None
        }
    }

    pub fn check_timeout(&mut self) -> Option<TranscriptUpdate> {
        if !self.current_sentence.is_empty() 
            && self.last_segment_time.elapsed().as_millis() as u64 > SENTENCE_TIMEOUT_MS {
            
            let sequence_id = SEQUENCE_COUNTER.fetch_add(1, Ordering::SeqCst);
            let update = TranscriptUpdate {
                text: self.current_sentence.trim().to_string(),
                timestamp: format_timestamp(
                    self.current_chunk_start_time + (self.sentence_start_time as f64 / 1000.0)
                ),
                source: "Mixed Audio".to_string(),
                sequence_id,
                is_partial: false,
            };

            // Reset for next sentence
            self.current_sentence.clear();
            self.sentence_start_time = 0;

            Some(update)
        } else {
            None
        }
    }

    pub fn create_partial_update(&mut self) -> TranscriptUpdate {
        let sequence_id = SEQUENCE_COUNTER.fetch_add(1, Ordering::SeqCst);
        TranscriptUpdate {
            text: self.current_sentence.trim().to_string(),
            timestamp: format_timestamp(
                self.current_chunk_start_time + (self.sentence_start_time as f64 / 1000.0)
            ),
            source: "Mixed Audio".to_string(),
            sequence_id,
            is_partial: true,
        }
    }

    fn is_sentence_complete(&self) -> bool {
        let trimmed = self.current_sentence.trim();
        trimmed.ends_with('.') 
            || trimmed.ends_with('!') 
            || trimmed.ends_with('?')
            || trimmed.ends_with(':')
            || trimmed.ends_with(';')
    }
}
