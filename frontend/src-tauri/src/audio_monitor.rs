use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Runtime};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioLevels {
    pub rms: f32,
    pub peak: f32,
}

static MONITORING_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Calculate RMS (Root Mean Square) value from audio samples
pub fn calculate_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    
    let sum_of_squares: f32 = samples.iter().map(|&x| x * x).sum();
    (sum_of_squares / samples.len() as f32).sqrt()
}

/// Find peak amplitude in audio samples
pub fn find_peak(samples: &[f32]) -> f32 {
    samples.iter()
        .map(|&x| x.abs())
        .fold(0.0_f32, |a, b| a.max(b))
}

/// Calculate audio levels from buffer
pub fn calculate_audio_levels(buffer: &[f32]) -> AudioLevels {
    AudioLevels {
        rms: calculate_rms(buffer),
        peak: find_peak(buffer),
    }
}

/// Start monitoring audio levels
pub fn start_level_monitoring() {
    MONITORING_ACTIVE.store(true, Ordering::SeqCst);
}

/// Stop monitoring audio levels
pub fn stop_level_monitoring() {
    MONITORING_ACTIVE.store(false, Ordering::SeqCst);
}

/// Check if monitoring is active
pub fn is_monitoring_active() -> bool {
    MONITORING_ACTIVE.load(Ordering::SeqCst)
}

/// Process audio buffer and emit levels if monitoring is active
pub fn process_audio_with_levels<R: Runtime>(
    buffer: &[f32],
    app_handle: &AppHandle<R>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !is_monitoring_active() {
        return Ok(());
    }
    
    let levels = calculate_audio_levels(buffer);
    
    // Emit audio levels to frontend
    app_handle.emit("audio-levels", &levels)?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rms_calculation() {
        let samples = vec![0.5, -0.5, 0.3, -0.3, 0.1, -0.1];
        let rms = calculate_rms(&samples);
        assert!((rms - 0.3464).abs() < 0.001);
    }

    #[test]
    fn test_peak_detection() {
        let samples = vec![0.1, 0.5, 0.9, 0.3, -0.7, 0.2];
        let peak = find_peak(&samples);
        assert_eq!(peak, 0.9);
    }

    #[test]
    fn test_empty_samples() {
        let samples: Vec<f32> = vec![];
        assert_eq!(calculate_rms(&samples), 0.0);
        assert_eq!(find_peak(&samples), 0.0);
    }
}