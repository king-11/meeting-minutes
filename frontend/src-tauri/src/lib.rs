use serde::Deserialize;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

pub mod api;
pub mod audio;
pub mod audio_monitor;
pub mod console_utils;
pub mod global_shortcut;
pub mod ollama;
pub mod transcription;
pub mod tray;
pub mod utils;
pub mod window_manager;

use crate::utils::resample_audio;
use audio::{default_input_device, default_output_device, AudioStream};
use log::{debug as log_debug, error as log_error, info as log_info};
use tauri::{AppHandle, Emitter, Listener, Runtime, WindowEvent};

// Import from transcription module
use transcription::{AudioChunk, AudioQueue, TranscriptionStatus, TranscriptionWorker};

static RECORDING_FLAG: AtomicBool = AtomicBool::new(false);
static CHUNK_ID_COUNTER: AtomicU64 = AtomicU64::new(0);
static mut MIC_BUFFER: Option<Arc<Mutex<Vec<f32>>>> = None;
static mut SYSTEM_BUFFER: Option<Arc<Mutex<Vec<f32>>>> = None;
static mut AUDIO_CHUNK_QUEUE: Option<Arc<AudioQueue>> = None;
static mut MIC_STREAM: Option<Arc<AudioStream>> = None;
static mut SYSTEM_STREAM: Option<Arc<AudioStream>> = None;
static mut IS_RUNNING: Option<Arc<AtomicBool>> = None;
static mut RECORDING_START_TIME: Option<std::time::Instant> = None;
static mut TRANSCRIPTION_WORKERS: Option<Vec<tokio::task::JoinHandle<()>>> = None;
static mut AUDIO_COLLECTION_TASK: Option<tokio::task::JoinHandle<()>> = None;
static mut ERROR_EVENT_EMITTED: bool = false;
use std::sync::LazyLock;

static LAST_TRANSCRIPTION_ACTIVITY: LazyLock<Arc<AtomicU64>> =
    LazyLock::new(|| Arc::new(AtomicU64::new(0)));
static ACTIVE_WORKERS: LazyLock<Arc<AtomicU64>> = LazyLock::new(|| Arc::new(AtomicU64::new(0)));

// Audio configuration constants
const CHUNK_DURATION_MS: u32 = 30000; // 30 seconds per chunk for better sentence processing
const WHISPER_SAMPLE_RATE: u32 = 16000; // Whisper's required sample rate
const MIN_CHUNK_DURATION_MS: u32 = 2000; // Minimum duration before sending chunk
const MIN_RECORDING_DURATION_MS: u64 = 2000; // 2 seconds minimum
const MAX_AUDIO_QUEUE_SIZE: usize = 10; // Maximum number of chunks in queue

#[derive(Debug, Deserialize)]
struct RecordingArgs {
    save_path: String,
}

async fn audio_collection_task<R: Runtime>(
    mic_stream: Arc<AudioStream>,
    system_stream: Arc<AudioStream>,
    is_running: Arc<AtomicBool>,
    sample_rate: u32,
    recording_start_time: std::time::Instant,
    app_handle: AppHandle<R>,
    queue: Arc<AudioQueue>,
) -> Result<(), String> {
    log_info!("Audio collection task started");

    let mut mic_receiver = mic_stream.subscribe().await;
    let mut system_receiver = system_stream.subscribe().await;

    let chunk_samples = (WHISPER_SAMPLE_RATE as f32 * (CHUNK_DURATION_MS as f32 / 1000.0)) as usize;
    let min_samples =
        (WHISPER_SAMPLE_RATE as f32 * (MIN_CHUNK_DURATION_MS as f32 / 1000.0)) as usize;
    let mut current_chunk: Vec<f32> = Vec::with_capacity(chunk_samples);
    let mut last_chunk_time = std::time::Instant::now();
    let chunk_start_time = std::time::Instant::now();

    while is_running.load(Ordering::SeqCst) {
        // Collect audio samples
        let mut new_samples = Vec::new();
        let mut mic_samples = Vec::new();
        let mut system_samples = Vec::new();

        // Get microphone samples
        while let Ok(chunk) = mic_receiver.try_recv() {
            log_debug!("Received {} mic samples", chunk.len());

            // Calculate and emit audio levels if monitoring is active
            if audio_monitor::is_monitoring_active() {
                if let Err(e) = audio_monitor::process_audio_with_levels(&chunk, &app_handle) {
                    log_debug!("Failed to emit audio levels: {}", e);
                }
            }

            // Store mic samples in the global buffer for final recording
            unsafe {
                if let Some(buffer) = &MIC_BUFFER {
                    if let Ok(mut guard) = buffer.lock() {
                        guard.extend_from_slice(&chunk);
                    }
                }
            }

            mic_samples.extend(chunk);
        }

        // Get system audio samples
        while let Ok(chunk) = system_receiver.try_recv() {
            log_debug!("Received {} system samples", chunk.len());

            // Store system samples in the global buffer for final recording
            unsafe {
                if let Some(buffer) = &SYSTEM_BUFFER {
                    if let Ok(mut guard) = buffer.lock() {
                        guard.extend_from_slice(&chunk);
                    }
                }
            }

            system_samples.extend(chunk);
        }

        // Mix samples (80% mic, 20% system)
        let max_len = mic_samples.len().max(system_samples.len());
        for i in 0..max_len {
            let mic_sample = if i < mic_samples.len() {
                mic_samples[i]
            } else {
                0.0
            };
            let system_sample = if i < system_samples.len() {
                system_samples[i]
            } else {
                0.0
            };
            new_samples.push((mic_sample * 0.8) + (system_sample * 0.2));
        }

        // Add samples to current chunk
        for sample in new_samples {
            current_chunk.push(sample);
        }

        // Check if we should create a chunk
        let should_create_chunk = current_chunk.len() >= chunk_samples
            || (current_chunk.len() >= min_samples
                && last_chunk_time.elapsed() >= Duration::from_millis(CHUNK_DURATION_MS as u64));

        if should_create_chunk && !current_chunk.is_empty() {
            // Process chunk for Whisper API
            let whisper_samples = if sample_rate != WHISPER_SAMPLE_RATE {
                log_debug!(
                    "Resampling audio from {} to {}",
                    sample_rate,
                    WHISPER_SAMPLE_RATE
                );
                resample_audio(&current_chunk, sample_rate, WHISPER_SAMPLE_RATE)
            } else {
                current_chunk.clone()
            };

            // Create audio chunk
            let chunk_id = CHUNK_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
            let chunk_timestamp = chunk_start_time.elapsed().as_secs_f64();
            let audio_chunk = AudioChunk {
                samples: whisper_samples,
                timestamp: chunk_timestamp,
                chunk_id,
                start_time: std::time::Instant::now(),
                recording_start_time,
            };

            // Add to queue using new AudioQueue
            if let Some(dropped_chunk) = queue.push(audio_chunk) {
                let drop_count = queue.dropped_count();
                if drop_count == 1 {
                    let warning_message = format!("Transcription process is very slow. Audio chunk {} was dropped. Please choose a smaller model, or run whisper natively.", dropped_chunk.chunk_id);
                    log_info!("Emitting chunk-drop-warning event: {}", warning_message);

                    if let Err(e) = app_handle.emit("chunk-drop-warning", &warning_message) {
                        log_error!("Failed to emit chunk-drop-warning event: {}", e);
                    }
                }
            }

            // Reset for next chunk
            current_chunk.clear();
            last_chunk_time = std::time::Instant::now();
        }

        // Small sleep to prevent busy waiting
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Process any remaining audio in the buffer before exiting
    if !current_chunk.is_empty() {
        log_info!(
            "Processing final audio chunk with {} samples before exiting",
            current_chunk.len()
        );

        // Process chunk for Whisper API
        let whisper_samples = if sample_rate != WHISPER_SAMPLE_RATE {
            log_debug!(
                "Resampling final audio from {} to {}",
                sample_rate,
                WHISPER_SAMPLE_RATE
            );
            resample_audio(&current_chunk, sample_rate, WHISPER_SAMPLE_RATE)
        } else {
            current_chunk.clone()
        };

        // Create final audio chunk
        let chunk_id = CHUNK_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
        let chunk_timestamp = chunk_start_time.elapsed().as_secs_f64();
        let audio_chunk = AudioChunk {
            samples: whisper_samples,
            timestamp: chunk_timestamp,
            chunk_id,
            start_time: std::time::Instant::now(),
            recording_start_time,
        };

        // Add final chunk to queue
        queue.push(audio_chunk);
    }

    log_info!("Audio collection task ended");
    Ok(())
}

// Helper function to start transcription workers
async fn start_transcription_workers<R: Runtime>(
    app: AppHandle<R>,
    queue: Arc<AudioQueue>,
    is_running: Arc<AtomicBool>,
    num_workers: usize,
) -> Vec<tokio::task::JoinHandle<()>> {
    let mut handles = Vec::new();

    for worker_id in 0..num_workers {
        let worker = TranscriptionWorker::new(worker_id);
        let app_handle = app.clone();
        let queue_clone = queue.clone();
        let is_running_clone = is_running.clone();
        let active_workers_clone = ACTIVE_WORKERS.clone();
        let last_activity_clone = LAST_TRANSCRIPTION_ACTIVITY.clone();

        let handle = tokio::spawn(async move {
            worker
                .run(
                    app_handle,
                    queue_clone,
                    is_running_clone,
                    active_workers_clone,
                    last_activity_clone,
                )
                .await;
        });

        handles.push(handle);
    }

    handles
}

#[tauri::command]
async fn start_recording<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    log_info!("Attempting to start recording...");

    if is_recording() {
        log_error!("Recording already in progress");
        return Err("Recording already in progress".to_string());
    }

    // Start audio level monitoring
    audio_monitor::start_level_monitoring();

    // Show floating window and emit start event
    if let Err(e) = window_manager::show_floating_window(app.clone()).await {
        log_error!("Failed to show floating window: {}", e);
    }

    // Emit recording started events to floating window
    // Emit both events to support both UI and global shortcut triggers
    if let Err(e) = app.emit("start-recording-from-tray", ()) {
        log_error!("Failed to emit start-recording-from-tray event: {}", e);
    }
    if let Err(e) = app.emit("recording-started", ()) {
        log_error!("Failed to emit recording-started event: {}", e);
    }

    // Reset dropped chunk counter for new recording session (handled by AudioQueue)

    // Stop any existing tasks first
    unsafe {
        if let Some(task) = AUDIO_COLLECTION_TASK.take() {
            log_info!("Stopping existing audio collection task...");
            task.abort();
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        if let Some(mut workers) = TRANSCRIPTION_WORKERS.take() {
            log_info!("Stopping existing transcription workers...");
            for worker in workers.drain(..) {
                worker.abort();
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    // Initialize recording flag and buffers
    RECORDING_FLAG.store(true, Ordering::SeqCst);
    log_info!("Recording flag set to true");

    // Reset error event flag and activity tracking for new recording session
    unsafe {
        ERROR_EVENT_EMITTED = false;
    }

    // Reset transcription activity tracking
    LAST_TRANSCRIPTION_ACTIVITY.store(0, Ordering::SeqCst);
    ACTIVE_WORKERS.store(0, Ordering::SeqCst);

    // Store recording start time
    unsafe {
        RECORDING_START_TIME = Some(std::time::Instant::now());
    }

    // Initialize audio buffers and queue
    let queue = Arc::new(AudioQueue::new(MAX_AUDIO_QUEUE_SIZE));
    unsafe {
        MIC_BUFFER = Some(Arc::new(Mutex::new(Vec::new())));
        SYSTEM_BUFFER = Some(Arc::new(Mutex::new(Vec::new())));
        AUDIO_CHUNK_QUEUE = Some(queue.clone());
        log_info!("Initialized audio buffers and chunk queue");
    }

    // Get default devices
    let mic_device = Arc::new(default_input_device().map_err(|e| {
        log_error!("Failed to get default input device: {}", e);
        e.to_string()
    })?);

    let system_device = Arc::new(default_output_device().map_err(|e| {
        log_error!("Failed to get default output device: {}", e);
        e.to_string()
    })?);

    // Create audio streams
    let is_running = Arc::new(AtomicBool::new(true));

    // Create microphone stream
    let mic_stream = AudioStream::from_device(mic_device.clone(), is_running.clone())
        .await
        .map_err(|e| {
            log_error!("Failed to create microphone stream: {}", e);
            e.to_string()
        })?;
    let mic_stream = Arc::new(mic_stream);

    // Create system audio stream
    let system_stream = AudioStream::from_device(system_device.clone(), is_running.clone())
        .await
        .map_err(|e| {
            log_error!("Failed to create system stream: {}", e);
            e.to_string()
        })?;
    let system_stream = Arc::new(system_stream);

    unsafe {
        MIC_STREAM = Some(mic_stream.clone());
        SYSTEM_STREAM = Some(system_stream.clone());
        IS_RUNNING = Some(is_running.clone());
    }

    let device_config = mic_stream.device_config.clone();
    let sample_rate = device_config.sample_rate().0;
    let channels = device_config.channels();

    log_info!("Mic config: {} Hz, {} channels", sample_rate, channels);

    // Get recording start time for proper elapsed time calculation
    let recording_start_time =
        unsafe { RECORDING_START_TIME.unwrap_or_else(|| std::time::Instant::now()) };

    // Start audio collection task
    let audio_collection_handle = {
        let mic_stream_clone = mic_stream.clone();
        let system_stream_clone = system_stream.clone();
        let is_running_clone = is_running.clone();
        let app_handle_clone = app.clone();
        let queue_clone = queue.clone();
        tokio::spawn(async move {
            if let Err(e) = audio_collection_task(
                mic_stream_clone,
                system_stream_clone,
                is_running_clone,
                sample_rate,
                recording_start_time,
                app_handle_clone,
                queue_clone,
            )
            .await
            {
                log_error!("Audio collection task error: {}", e);
            }
        })
    };

    // Start transcription workers using the new module
    const NUM_WORKERS: usize = 3;
    let worker_handles =
        start_transcription_workers(app.clone(), queue.clone(), is_running.clone(), NUM_WORKERS)
            .await;

    // Store task handles globally
    unsafe {
        AUDIO_COLLECTION_TASK = Some(audio_collection_handle);
        TRANSCRIPTION_WORKERS = Some(worker_handles);
    }

    Ok(())
}

#[tauri::command]
async fn stop_recording<R: Runtime>(app: AppHandle<R>, args: RecordingArgs) -> Result<(), String> {
    log_info!("Attempting to stop recording...");

    // Only check recording state if we haven't already started stopping
    if !RECORDING_FLAG.load(Ordering::SeqCst) {
        log_info!("Recording is already stopped");
        return Ok(());
    }

    // Stop audio level monitoring
    audio_monitor::stop_level_monitoring();

    // Emit recording stopped events to floating window
    // Emit both events to support both UI and global shortcut triggers
    if let Err(e) = app.emit("stop-recording-from-tray", ()) {
        log_error!("Failed to emit stop-recording-from-tray event: {}", e);
    }
    if let Err(e) = app.emit("recording-stopped", ()) {
        log_error!("Failed to emit recording-stopped event: {}", e);
    }

    // Hide floating window after a delay to show confirmation
    let app_clone = app.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        if let Err(e) = window_manager::hide_floating_window(app_clone).await {
            log_error!("Failed to hide floating window: {}", e);
        }
    });

    // Check minimum recording duration
    let elapsed_ms = unsafe {
        RECORDING_START_TIME
            .map(|start| start.elapsed().as_millis() as u64)
            .unwrap_or(0)
    };

    if elapsed_ms < MIN_RECORDING_DURATION_MS {
        let remaining = MIN_RECORDING_DURATION_MS - elapsed_ms;
        log_info!(
            "Waiting for minimum recording duration ({} ms remaining)...",
            remaining
        );
        tokio::time::sleep(Duration::from_millis(remaining)).await;
    }

    // First set the recording flag to false to prevent new data from being processed
    RECORDING_FLAG.store(false, Ordering::SeqCst);
    log_info!("Recording flag set to false");

    unsafe {
        // Stop the running flag for audio streams first
        if let Some(is_running) = &IS_RUNNING {
            // Set running flag to false first to stop the tokio task
            is_running.store(false, Ordering::SeqCst);
            log_info!("Set recording flag to false, waiting for streams to stop...");

            // Wait for the audio collection task to finish adding its final chunk
            if let Some(task) = AUDIO_COLLECTION_TASK.take() {
                log_info!("Waiting for audio collection task to finish processing final buffer...");
                // Give it time to process and add final chunk
                tokio::time::sleep(Duration::from_millis(500)).await;
                // Then abort if it's still running
                task.abort();
                tokio::time::sleep(Duration::from_millis(100)).await;
                log_info!("Audio collection task has been stopped");
            }

            // Now wait for transcription workers to complete processing remaining chunks
            if TRANSCRIPTION_WORKERS.is_some() {
                log_info!("Waiting for transcription workers to complete...");

                // Wait for all workers to finish processing remaining chunks
                let mut wait_time = 0;
                const MAX_WAIT_TIME: u64 = 30000; // 30 seconds max
                const CHECK_INTERVAL: u64 = 100; // Check every 100ms

                while wait_time < MAX_WAIT_TIME {
                    let active_count = ACTIVE_WORKERS.load(Ordering::SeqCst);
                    let queue_size = if let Some(queue) = &AUDIO_CHUNK_QUEUE {
                        queue.len()
                    } else {
                        0
                    };

                    log_info!(
                        "Worker cleanup status: {} active workers, {} chunks in queue",
                        active_count,
                        queue_size
                    );

                    // If no active workers and queue is empty, we're done
                    if active_count == 0 && queue_size == 0 {
                        log_info!("All workers completed and queue is empty");
                        break;
                    }

                    tokio::time::sleep(Duration::from_millis(CHECK_INTERVAL)).await;
                    wait_time += CHECK_INTERVAL;
                }

                if wait_time >= MAX_WAIT_TIME {
                    log_error!(
                        "Transcription worker cleanup timeout after {} seconds",
                        MAX_WAIT_TIME / 1000
                    );
                }

                // Now stop the transcription workers
                if let Some(mut workers) = TRANSCRIPTION_WORKERS.take() {
                    log_info!("Stopping transcription workers...");
                    for worker in workers.drain(..) {
                        worker.abort();
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }

            // Give the tokio task time to finish and release its references
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Stop mic stream if it exists
            if let Some(mic_stream) = &MIC_STREAM {
                log_info!("Stopping microphone stream...");
                if let Err(e) = mic_stream.stop().await {
                    log_error!("Error stopping mic stream: {}", e);
                } else {
                    log_info!("Microphone stream stopped successfully");
                }
            }

            // Stop system stream if it exists
            if let Some(system_stream) = &SYSTEM_STREAM {
                log_info!("Stopping system stream...");
                if let Err(e) = system_stream.stop().await {
                    log_error!("Error stopping system stream: {}", e);
                } else {
                    log_info!("System stream stopped successfully");
                }
            }

            // Clear the stream references
            MIC_STREAM = None;
            SYSTEM_STREAM = None;
            IS_RUNNING = None;
            TRANSCRIPTION_WORKERS = None;
            // AUDIO_COLLECTION_TASK already taken and stopped above
            AUDIO_CHUNK_QUEUE = None;

            // Give streams time to fully clean up
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    // Get final buffers
    let mic_data = unsafe {
        if let Some(buffer) = &MIC_BUFFER {
            if let Ok(guard) = buffer.lock() {
                guard.clone()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    };

    let system_data = unsafe {
        if let Some(buffer) = &SYSTEM_BUFFER {
            if let Ok(guard) = buffer.lock() {
                guard.clone()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    };
    // Mix the audio and convert to 16-bit PCM
    let max_len = mic_data.len().max(system_data.len());
    let mut mixed_data = Vec::with_capacity(max_len);

    for i in 0..max_len {
        let mic_sample = if i < mic_data.len() { mic_data[i] } else { 0.0 };
        let system_sample = if i < system_data.len() { system_data[i] } else { 0.0 };
        mixed_data.push((mic_sample + system_sample) * 0.5);
    }

    if mixed_data.is_empty() {
        log_info!("No audio data captured, creating empty WAV file");
        // Create a minimal WAV file with silence
        mixed_data = vec![0.0; 1000]; // 1000 samples of silence
    }

    log_info!("Mixed {} audio samples", mixed_data.len());

    // Resample the audio to 16kHz for Whisper compatibility
    let original_sample_rate = 48000; // Assuming original sample rate is 48kHz
    if original_sample_rate != WHISPER_SAMPLE_RATE {
        log_info!("Resampling audio from {} Hz to {} Hz for Whisper compatibility",
                 original_sample_rate, WHISPER_SAMPLE_RATE);
        mixed_data = resample_audio(&mixed_data, original_sample_rate, WHISPER_SAMPLE_RATE);
        log_info!("Resampled to {} samples", mixed_data.len());
    }

    // Convert to 16-bit PCM samples
    let mut bytes = Vec::with_capacity(mixed_data.len() * 2);
    for &sample in mixed_data.iter() {
        let value = (sample.max(-1.0).min(1.0) * 32767.0) as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    log_info!("Converted to {} bytes of PCM data", bytes.len());

    // Create WAV header
    let data_size = bytes.len() as u32;
    let file_size = 36 + data_size;
    let sample_rate = WHISPER_SAMPLE_RATE; // Use Whisper's required sample rate (16000 Hz)
    let channels = 1u16; // Mono
    let bits_per_sample = 16u16;
    let block_align = channels * (bits_per_sample / 8);
    let byte_rate = sample_rate * block_align as u32;

    let mut wav_file = Vec::with_capacity(44 + bytes.len());

    // RIFF header
    wav_file.extend_from_slice(b"RIFF");
    wav_file.extend_from_slice(&file_size.to_le_bytes());
    wav_file.extend_from_slice(b"WAVE");

    // fmt chunk
    wav_file.extend_from_slice(b"fmt ");
    wav_file.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    wav_file.extend_from_slice(&1u16.to_le_bytes()); // audio format (PCM)
    wav_file.extend_from_slice(&channels.to_le_bytes()); // num channels
    wav_file.extend_from_slice(&sample_rate.to_le_bytes()); // sample rate
    wav_file.extend_from_slice(&byte_rate.to_le_bytes()); // byte rate
    wav_file.extend_from_slice(&block_align.to_le_bytes()); // block align
    wav_file.extend_from_slice(&bits_per_sample.to_le_bytes()); // bits per sample

    // data chunk
    wav_file.extend_from_slice(b"data");
    wav_file.extend_from_slice(&data_size.to_le_bytes());
    wav_file.extend_from_slice(&bytes);

    log_info!("Created WAV file with {} bytes total", wav_file.len());
    // Create the save directory if it doesn't exist
    if let Some(parent) = std::path::Path::new(&args.save_path).parent() {
        if !parent.exists() {
            log_info!("Creating directory: {:?}", parent);
            if let Err(e) = std::fs::create_dir_all(parent) {
                let err_msg = format!("Failed to create save directory: {}", e);
                log_error!("{}", err_msg);
                return Err(err_msg);
            }
        }
    }

    // Save the recording
    log_info!("Saving recording to: {}", args.save_path);
    match std::fs::write(&args.save_path, wav_file) {
        Ok(_) => log_info!("Successfully saved recording to: {}", args.save_path),
        Err(e) => {
            let err_msg = format!("Failed to save recording: {}", e);
            log_error!("{}", err_msg);
            return Err(err_msg);
        }
    }
    // Clean up
    unsafe {
        MIC_BUFFER = None;
        SYSTEM_BUFFER = None;
        MIC_STREAM = None;
        SYSTEM_STREAM = None;
        IS_RUNNING = None;
        RECORDING_START_TIME = None;
        TRANSCRIPTION_WORKERS = None;
        AUDIO_COLLECTION_TASK = None;
        AUDIO_CHUNK_QUEUE = None;
    }

    Ok(())
}

#[tauri::command]
fn is_recording() -> bool {
    RECORDING_FLAG.load(Ordering::SeqCst)
}

#[tauri::command]
async fn toggle_recording<R: Runtime>(app: AppHandle<R>) -> Result<bool, String> {
    if is_recording() {
        // Stop recording and save to default location
        // Get the downloads directory or app data directory
        let save_path = if let Some(download_dir) = dirs::download_dir() {
            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
            let filename = format!("recording_{}.wav", timestamp);
            download_dir.join(filename).to_string_lossy().to_string()
        } else {
            // Fallback to app data directory
            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
            format!("recording_{}.wav", timestamp)
        };

        log_info!("Saving recording to: {}", save_path);

        let args = RecordingArgs { save_path };
        stop_recording(app, args).await?;
        Ok(false)
    } else {
        // Start recording
        start_recording(app).await?;
        Ok(true)
    }
}

#[tauri::command]
fn get_transcription_status() -> TranscriptionStatus {
    let chunks_in_queue = unsafe {
        if let Some(queue) = &AUDIO_CHUNK_QUEUE {
            queue.len()
        } else {
            0
        }
    };

    let is_processing = ACTIVE_WORKERS.load(Ordering::SeqCst) > 0 || chunks_in_queue > 0;

    let last_activity_ms = LAST_TRANSCRIPTION_ACTIVITY.load(Ordering::SeqCst);
    let current_time_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let elapsed_since_activity = if last_activity_ms > 0 {
        current_time_ms.saturating_sub(last_activity_ms)
    } else {
        u64::MAX
    };

    TranscriptionStatus {
        chunks_in_queue,
        is_processing,
        last_activity_ms: elapsed_since_activity,
    }
}

#[tauri::command]
fn read_audio_file(file_path: String) -> Result<Vec<u8>, String> {
    match std::fs::read(&file_path) {
        Ok(data) => Ok(data),
        Err(e) => Err(format!("Failed to read audio file: {}", e)),
    }
}

#[tauri::command]
async fn save_transcript(file_path: String, content: String) -> Result<(), String> {
    log::info!("Saving transcript to: {}", file_path);

    // Ensure parent directory exists
    if let Some(parent) = std::path::Path::new(&file_path).parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }
    }

    // Write content to file
    std::fs::write(&file_path, content)
        .map_err(|e| format!("Failed to write transcript: {}", e))?;

    log::info!("Transcript saved successfully");
    Ok(())
}

pub fn run() {
    log::set_max_level(log::LevelFilter::Info);

    tauri::Builder::default()
        .setup(|app| {
            log::info!("Application setup complete");

            // Initialize system tray
            if let Err(e) = tray::create_tray(app.handle()) {
                log::error!("Failed to create system tray: {}", e);
            }

            // Register global shortcut
            if let Err(e) = global_shortcut::register_recording_shortcut(app.handle()) {
                log::error!("Failed to register global shortcut: {}", e);
            }

            // Listen for shortcut toggle event
            let app_handle = app.handle().clone();
            app.listen("toggle-recording-shortcut", move |_event| {
                let app_clone = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = toggle_recording(app_clone).await {
                        log::error!("Failed to toggle recording: {}", e);
                    }
                });
            });

            // Trigger microphone permission request on startup
            if let Err(e) = audio::core::trigger_audio_permission() {
                log::error!("Failed to trigger audio permission: {}", e);
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    // If this is the main window, hide it instead of closing
                    if window.label() == "main" {
                        log::info!("Main window close requested, hiding instead of closing");
                        api.prevent_close();
                        if let Err(e) = window.hide() {
                            log::error!("Failed to hide main window: {}", e);
                        }
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            start_recording,
            stop_recording,
            toggle_recording,
            is_recording,
            get_transcription_status,
            read_audio_file,
            save_transcript,
            ollama::get_ollama_models,
            api::api_get_meetings,
            api::api_search_transcripts,
            api::api_get_profile,
            api::api_save_profile,
            api::api_update_profile,
            api::api_get_model_config,
            api::api_save_model_config,
            api::api_get_api_key,
            api::api_get_transcript_config,
            api::api_save_transcript_config,
            api::api_get_transcript_api_key,
            api::api_delete_meeting,
            api::api_get_meeting,
            api::api_save_meeting_title,
            api::api_save_meeting_summary,
            api::api_get_summary,
            api::api_save_transcript,
            api::api_process_transcript,
            api::test_backend_connection,
            api::debug_backend_connection,
            api::open_external_url,
            console_utils::show_console,
            console_utils::hide_console,
            console_utils::toggle_console,
            window_manager::show_floating_window,
            window_manager::hide_floating_window,
            window_manager::save_window_position,
            window_manager::get_window_position,
            window_manager::toggle_recording_with_ui_feedback,
        ])
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
