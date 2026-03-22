use std::{collections::VecDeque, sync::atomic::AtomicU64, time::Instant};

use tokio::sync::RwLock;

use crate::models::SystemSample;

#[derive(Debug, Default)]
pub struct RuntimeState {
    pub current: SystemSample,
    pub history: VecDeque<SystemSample>,
    pub sample_errors: u64,
}

#[derive(Debug)]
pub struct SharedState {
    pub hostname: String,
    pub started_at: Instant,
    pub history_size: usize,
    pub request_count: AtomicU64,
    pub inner: RwLock<RuntimeState>,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub shared: std::sync::Arc<SharedState>,
}

impl SharedState {
    #[must_use]
    pub fn new(hostname: String, history_size: usize) -> Self {
        Self {
            hostname,
            started_at: Instant::now(),
            history_size,
            request_count: AtomicU64::new(0),
            inner: RwLock::new(RuntimeState {
                history: VecDeque::with_capacity(history_size),
                ..RuntimeState::default()
            }),
        }
    }
}
