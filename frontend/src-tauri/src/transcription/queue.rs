use super::types::AudioChunk;
use log::info;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub struct AudioQueue {
    inner: Arc<Mutex<VecDeque<AudioChunk>>>,
    max_size: usize,
    dropped_counter: Arc<AtomicU64>,
}

impl AudioQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::new())),
            max_size,
            dropped_counter: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn push(&self, chunk: AudioChunk) -> Option<AudioChunk> {
        let mut queue = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                // Handle poisoned mutex by recovering the guard
                poisoned.into_inner()
            }
        };

        let dropped = if queue.len() >= self.max_size {
            let dropped_chunk = queue.pop_front();
            if dropped_chunk.is_some() {
                let count = self.dropped_counter.fetch_add(1, Ordering::SeqCst) + 1;
                info!(
                    "Dropped audio chunk due to queue overflow (total drops: {})",
                    count
                );
            }
            dropped_chunk
        } else {
            None
        };

        let chunk_id = chunk.chunk_id;
        queue.push_back(chunk);
        info!(
            "Added chunk {} to queue (queue size: {})",
            chunk_id,
            queue.len()
        );

        dropped
    }

    pub fn pop(&self) -> Option<AudioChunk> {
        match self.inner.lock() {
            Ok(mut guard) => guard.pop_front(),
            Err(poisoned) => poisoned.into_inner().pop_front(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self.inner.lock() {
            Ok(guard) => guard.is_empty(),
            Err(poisoned) => poisoned.into_inner().is_empty(),
        }
    }

    pub fn len(&self) -> usize {
        match self.inner.lock() {
            Ok(guard) => guard.len(),
            Err(poisoned) => poisoned.into_inner().len(),
        }
    }

    pub fn clear(&self) {
        match self.inner.lock() {
            Ok(mut guard) => guard.clear(),
            Err(poisoned) => poisoned.into_inner().clear(),
        }
    }

    pub fn dropped_count(&self) -> u64 {
        self.dropped_counter.load(Ordering::SeqCst)
    }

    pub fn reset_dropped_count(&self) {
        self.dropped_counter.store(0, Ordering::SeqCst);
    }
}

impl Clone for AudioQueue {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            max_size: self.max_size,
            dropped_counter: self.dropped_counter.clone(),
        }
    }
}