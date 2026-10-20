use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::Serialize;
use sysinfo::{Disks, ProcessesToUpdate, System, MINIMUM_CPU_UPDATE_INTERVAL};

use crate::{
    error::{AppError, AppResult},
    host::HostMonitor,
};

const CACHE_TTL: Duration = Duration::from_secs(4);
pub const POLL_INTERVAL_SEC: u64 = 5;

#[derive(Debug, Clone, Serialize)]
pub struct CoreStat {
    pub name: String,
    pub usage_percent: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskStat {
    pub name: String,
    pub mount: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    /// `host` is the OCI VM filesystem mounted at `/host/root`. `local` is what the API process sees.
    pub scope: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub mode: String,
    pub recommended_poll_sec: u64,
    pub exec_mode: String,
    pub hostname: String,
    pub os: String,
    pub kernel: String,
    pub arch: String,
    pub uptime_sec: u64,
    pub load_1: f64,
    pub load_5: f64,
    pub load_15: f64,
    pub cpu_count: usize,
    pub cpu_usage_percent: f32,
    pub cores: Vec<CoreStat>,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub memory_available_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
    pub disks: Vec<DiskStat>,
    pub process_count: usize,
    pub collected_at: chrono::DateTime<Utc>,
}

pub async fn collect(monitor: &HostMonitor, exec_mode: &str) -> AppResult<Snapshot> {
    if let Some(hit) = cached(monitor).await {
        return Ok(hit);
    }

    let system = Arc::clone(&monitor.system);
    let exec_mode = exec_mode.to_string();
    let snapshot = tokio::task::spawn_blocking(move || sample(&system, &exec_mode))
        .await
        .map_err(|e| AppError::internal(format!("host sample join: {e}")))?;

    let mut cache = monitor.cache.lock().await;
    *cache = Some((Instant::now(), snapshot.clone()));
    Ok(snapshot)
}

async fn cached(monitor: &HostMonitor) -> Option<Snapshot> {
    let guard = monitor.cache.lock().await;
    guard.as_ref().and_then(|(at, snap)| {
        if at.elapsed() < CACHE_TTL {
            Some(snap.clone())
        } else {
            None
        }
    })
}

fn sample(system: &Mutex<System>, exec_mode: &str) -> Snapshot {
    let mut sys = system.lock().unwrap_or_else(|err| err.into_inner());
    sys.refresh_cpu_usage();
    std::thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL);
    sys.refresh_cpu_usage();
    sys.refresh_memory();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let cores: Vec<CoreStat> = sys
        .cpus()
        .iter()
        .map(|cpu| CoreStat {
            name: cpu.name().to_string(),
            usage_percent: cpu.cpu_usage(),
        })
        .collect();
    let cpu_usage_percent = if cores.is_empty() {
        0.0
    } else {
        cores.iter().map(|c| c.usage_percent).sum::<f32>() / cores.len() as f32
    };
    let process_count = sys.processes().len();
    let memory_total_bytes = sys.total_memory();
    let memory_used_bytes = sys.used_memory();
    let memory_available_bytes = sys.available_memory();
    let swap_total_bytes = sys.total_swap();
    let swap_used_bytes = sys.used_swap();
    drop(sys);

    let mut disks: Vec<DiskStat> = Disks::new_with_refreshed_list()
        .iter()
        .filter_map(|disk| {
            let total = disk.total_space();
            if total == 0 {
                return None;
            }
            let available = disk.available_space();
            let used = total.saturating_sub(available);
            let mount = disk.mount_point().to_string_lossy().to_string();
            let scope = if mount == "/host/root" || mount.starts_with("/host/root/") {
                "host"
            } else {
                "local"
            };
            Some(DiskStat {
                name: disk.name().to_string_lossy().to_string(),
                mount,
                total_bytes: total,
                available_bytes: available,
                used_bytes: used,
                scope: scope.to_string(),
            })
        })
        .collect();
    disks.sort_by(|a, b| b.total_bytes.cmp(&a.total_bytes));

    let load = System::load_average();
    let os = format!(
        "{} {}",
        System::name().unwrap_or_default(),
        System::os_version().unwrap_or_default()
    )
    .trim()
    .to_string();

    Snapshot {
        mode: "snapshot".into(),
        recommended_poll_sec: POLL_INTERVAL_SEC,
        exec_mode: exec_mode.to_string(),
        hostname: System::host_name().unwrap_or_else(|| "unknown".into()),
        os,
        kernel: System::kernel_version().unwrap_or_default(),
        arch: System::cpu_arch(),
        uptime_sec: System::uptime(),
        load_1: load.one,
        load_5: load.five,
        load_15: load.fifteen,
        cpu_count: cores.len(),
        cpu_usage_percent,
        cores,
        memory_total_bytes,
        memory_used_bytes,
        memory_available_bytes,
        swap_total_bytes,
        swap_used_bytes,
        disks,
        process_count,
        collected_at: Utc::now(),
    }
}
