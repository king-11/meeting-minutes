use super::accumulator::TranscriptAccumulator;
use super::queue::AudioQueue;
use super::types::*;
use log::{debug, error, info};
use reqwest::multipart::{Form, Part};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Runtime};

const GRACE_PERIOD_MS: u64 = 2000;
const WORKER_POLL_MS: u64 = 50;
const MAX_RETRIES: u32 = 3;
const TRANSCRIPT_SERVER_URL: &str = "http://127.0.0.1:8178";

#[derive(Debug, Clone)]
enum WorkerState {
    Running,
    GracePeriod { started_at: Instant },
    Stopping,
}

pub struct TranscriptionWorker {
    id: usize,
    client: reqwest::Client,
    stream_url: String,
    state: WorkerState,
    accumulator: TranscriptAccumulator,
}

impl TranscriptionWorker {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            client: reqwest::Client::new(),
            stream_url: format!("{}/stream", TRANSCRIPT_SERVER_URL),
            state: WorkerState::Running,
            accumulator: TranscriptAccumulator::new(),
        }
    }

    pub async fn run<R: Runtime>(
        mut self,
        app_handle: AppHandle<R>,
        queue: Arc<AudioQueue>,
        is_running: Arc<AtomicBool>,
        active_workers: Arc<AtomicU64>,
        last_activity: Arc<AtomicU64>,
    ) {
        active_workers.fetch_add(1, Ordering::SeqCst);
        info!("Worker {} started", self.id);

        loop {
            // Update state based on recording status
            self.update_state(&is_running);

            // Check if we should exit
            if self.should_exit(&queue).await {
                break;
            }

            // Process any timeout transcripts
            self.flush_timeout_transcripts(&app_handle).await;

            // Try to get and process a chunk
            if let Some(chunk) = queue.pop() {
                // Update activity timestamp
                last_activity.store(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                    Ordering::SeqCst,
                );

                if let Err(should_stop) = self.process_chunk(chunk, &app_handle).await {
                    if should_stop {
                        error!("Worker {}: Critical error, stopping", self.id);
                        break;
                    }
                }
            } else {
                // No chunk available, wait
                self.idle_wait().await;
            }
        }

        // Cleanup
        self.cleanup(&app_handle).await;
        active_workers.fetch_sub(1, Ordering::SeqCst);
        info!("Worker {} stopped", self.id);
    }

    fn update_state(&mut self, is_running: &Arc<AtomicBool>) {
        if !is_running.load(Ordering::SeqCst) {
            match self.state {
                WorkerState::Running => {
                    info!("Worker {}: Entering grace period", self.id);
                    self.state = WorkerState::GracePeriod {
                        started_at: Instant::now(),
                    };
                }
                _ => {}
            }
        }
    }

    async fn should_exit(&mut self, queue: &Arc<AudioQueue>) -> bool {
        match &self.state {
            WorkerState::Running => false,
            WorkerState::GracePeriod { started_at } => {
                if started_at.elapsed() >= Duration::from_millis(GRACE_PERIOD_MS) {
                    if queue.is_empty() {
                        info!("Worker {}: Grace period expired, queue empty, exiting", self.id);
                        self.state = WorkerState::Stopping;
                        true
                    } else {
                        info!(
                            "Worker {}: Grace period expired but queue has {} chunks, continuing",
                            self.id,
                            queue.len()
                        );
                        // Reset grace period
                        self.state = WorkerState::GracePeriod {
                            started_at: Instant::now(),
                        };
                        false
                    }
                } else {
                    let remaining = GRACE_PERIOD_MS - started_at.elapsed().as_millis() as u64;
                    debug!(
                        "Worker {}: In grace period, {} ms remaining",
                        self.id, remaining
                    );
                    false
                }
            }
            WorkerState::Stopping => true,
        }
    }

    async fn process_chunk<R: Runtime>(
        &mut self,
        chunk: AudioChunk,
        app_handle: &AppHandle<R>,
    ) -> Result<(), bool> {
        info!(
            "Worker {}: Processing chunk {} with {} samples",
            self.id,
            chunk.chunk_id,
            chunk.samples.len()
        );

        // Set context for accumulator
        self.accumulator.set_chunk_context(
            chunk.chunk_id,
            chunk.timestamp,
            chunk.recording_start_time,
        );

        // Send to whisper
        match self.send_to_whisper(chunk.samples).await {
            Ok(response) => {
                info!(
                    "Worker {}: Received {} segments for chunk {}",
                    self.id,
                    response.segments.len(),
                    chunk.chunk_id
                );
                self.process_response(response, app_handle).await;
                Ok(())
            }
            Err(e) => {
                error!(
                    "Worker {}: Failed to transcribe chunk {}: {}",
                    self.id, chunk.chunk_id, e
                );
                // Return Ok(()) for non-critical errors, Err(true) for critical ones
                if e.contains("Failed to connect") || e.contains("Connection refused") {
                    self.emit_error(
                        "Transcription service is not available. Please check if the server is running.",
                        app_handle,
                    ).await;
                    Err(true) // Critical error
                } else {
                    Ok(()) // Non-critical, continue
                }
            }
        }
    }

    async fn send_to_whisper(&self, samples: Vec<f32>) -> Result<TranscriptResponse, String> {
        debug!("Worker {}: Sending {} samples to whisper", self.id, samples.len());

        // Convert samples to bytes
        let bytes: Vec<u8> = samples
            .iter()
            .flat_map(|&sample| {
                let clamped = sample.max(-1.0).min(1.0);
                clamped.to_le_bytes().to_vec()
            })
            .collect();

        // Retry logic
        let mut retry_count = 0;
        let mut last_error = String::new();

        while retry_count <= MAX_RETRIES {
            if retry_count > 0 {
                info!("Worker {}: Retry attempt {}/{}", self.id, retry_count, MAX_RETRIES);
                tokio::time::sleep(Duration::from_millis(500 * retry_count as u64)).await;
            }

            let form = Form::new()
                .text("sample_rate", "16000")
                .text("channels", "1")
                .text("sample_format", "f32")
                .part(
                    "audio",
                    Part::bytes(bytes.clone())
                        .file_name("audio.raw")
                        .mime_str("audio/raw")
                        .unwrap(),
                );

            match self.client.post(&self.stream_url).multipart(form).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        match response.json::<TranscriptResponse>().await {
                            Ok(transcript) => return Ok(transcript),
                            Err(e) => {
                                last_error = format!("Failed to parse response: {}", e);
                            }
                        }
                    } else {
                        last_error = format!("Server error: {}", response.status());
                    }
                }
                Err(e) => {
                    last_error = e.to_string();
                }
            }

            retry_count += 1;
        }

        Err(format!(
            "Failed after {} retries. Last error: {}",
            MAX_RETRIES, last_error
        ))
    }

    async fn process_response<R: Runtime>(
        &mut self,
        response: TranscriptResponse,
        app_handle: &AppHandle<R>,
    ) {
        for segment in response.segments {
            if let Some(update) = self.accumulator.add_segment(&segment) {
                self.emit_transcript(update, app_handle).await;
            }
        }
    }

    async fn emit_transcript<R: Runtime>(
        &self,
        update: TranscriptUpdate,
        app_handle: &AppHandle<R>,
    ) {
        debug!(
            "Worker {}: Emitting transcript with seq {}",
            self.id, update.sequence_id
        );
        if let Err(e) = app_handle.emit("transcript-update", &update) {
            error!("Worker {}: Failed to emit transcript: {}", self.id, e);
        }
    }

    async fn emit_error<R: Runtime>(&self, message: &str, app_handle: &AppHandle<R>) {
        if let Err(e) = app_handle.emit("transcript-error", message) {
            error!("Worker {}: Failed to emit error: {}", self.id, e);
        }
    }

    async fn flush_timeout_transcripts<R: Runtime>(&mut self, app_handle: &AppHandle<R>) {
        if let Some(update) = self.accumulator.check_timeout() {
            self.emit_transcript(update, app_handle).await;
        }
    }

    async fn idle_wait(&self) {
        tokio::time::sleep(Duration::from_millis(WORKER_POLL_MS)).await;
    }

    async fn cleanup<R: Runtime>(&mut self, app_handle: &AppHandle<R>) {
        // Flush any remaining transcripts
        if let Some(update) = self.accumulator.check_timeout() {
            info!("Worker {}: Emitting final timeout transcript", self.id);
            self.emit_transcript(update, app_handle).await;
        }

        // Flush partial sentences
        if !self.accumulator.current_sentence.is_empty() {
            info!("Worker {}: Flushing final partial sentence", self.id);
            let update = self.accumulator.create_partial_update();
            self.emit_transcript(update, app_handle).await;
        }

        // Emit completion event if this was the last worker
        // (This will be handled by the caller checking active_workers)
    }
}