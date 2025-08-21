use super::types::AudioChunk;
use std::collections::VecDeque;
use std::sync::Mutex;

pub struct AudioQueue {
    queue: Mutex<VecDeque<AudioChunk>>,
}

impl AudioQueue {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
        }
    }

    pub fn push(&self, chunk: AudioChunk) -> Result<(), String> {
        self.queue
            .lock()
            .map_err(|e| format!("Failed to lock queue: {}", e))?
            .push_back(chunk);
        Ok(())
    }

    pub fn pop(&self) -> Option<AudioChunk> {
        self.queue.lock().ok()?.pop_front()
    }

    pub fn len(&self) -> usize {
        self.queue.lock().ok().map(|q| q.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&self) {
        if let Ok(mut queue) = self.queue.lock() {
            queue.clear();
        }
    }
}
