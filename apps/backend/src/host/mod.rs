pub mod exec;
pub mod metrics;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use sysinfo::System;
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

use crate::host::metrics::Snapshot;

pub struct HostMonitor {
    pub system: Arc<Mutex<System>>,
    pub cache: AsyncMutex<Option<(Instant, Snapshot)>>,
    pub cwd: AsyncMutex<HashMap<Uuid, String>>,
}

impl HostMonitor {
    pub fn new() -> Self {
        Self {
            system: Arc::new(Mutex::new(System::new())),
            cache: AsyncMutex::new(None),
            cwd: AsyncMutex::new(HashMap::new()),
        }
    }
}
