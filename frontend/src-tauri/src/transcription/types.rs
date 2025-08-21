use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub samples: Vec<f32>,
    pub timestamp: f64,
    pub chunk_id: u64,
    pub start_time: Instant,
    pub recording_start_time: Instant,
}

#[derive(Debug, Serialize, Clone)]
pub struct TranscriptUpdate {
    pub text: String,
    pub timestamp: String,
    pub source: String,
    pub sequence_id: u64,
    pub chunk_start_time: f64,
    pub is_partial: bool,
}

#[derive(Debug, Deserialize)]
pub struct TranscriptSegment {
    pub text: String,
    pub t0: f32,
    pub t1: f32,
}

#[derive(Debug, Deserialize)]
pub struct TranscriptResponse {
    pub segments: Vec<TranscriptSegment>,
    pub buffer_size_ms: i32,
}

#[derive(Debug, Serialize, Clone)]
pub struct TranscriptionStatus {
    pub chunks_in_queue: usize,
    pub is_processing: bool,
    pub last_activity_ms: u64,
}