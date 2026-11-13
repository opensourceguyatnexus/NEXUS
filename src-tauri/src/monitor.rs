// System Monitor.
//
// CPU usage, memory, and disk usage come from sysinfo. CPU temperature is read
// from sysinfo components when the platform exposes them, which Windows often
// does not, so it may be None. GPU name comes from WMI; GPU utilization and
// temperature need vendor libraries (NVML or ADL) that are out of scope for the
// MVP, so they are reported as None rather than guessed.

use crate::models::{DiskUsage, MonitorSnapshot, ProcInfo};
use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use nvml_wrapper::Nvml;
use serde::Deserialize;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use sysinfo::{Components, Disks, ProcessesToUpdate, System};
use wmi::{COMLibrary, WMIConnection};

#[derive(Deserialize)]
#[serde(rename = "Win32_VideoController")]
#[serde(rename_all = "PascalCase")]
struct Win32VideoController {
    name: Option<String>,
}

// The adapter name does not change while the app runs, so once WMI has
// answered it is not asked again.
fn gpu_name_cached() -> Option<String> {
    static GPU_NAME: OnceLock<String> = OnceLock::new();
    if let Some(n) = GPU_NAME.get() {
        return Some(n.clone());
    }
    let n = gpu_name_wmi()?;
    Some(GPU_NAME.get_or_init(|| n).clone())
}

fn gpu_name_wmi() -> Option<String> {
    let wmi = COMLibrary::new().ok().and_then(|c| WMIConnection::new(c).ok())?;
    let rows = wmi.query::<Win32VideoController>().ok()?;
    rows.into_iter().find_map(|r| r.name)
}

// Live GPU stats from NVML (NVIDIA only). Each field is optional because some
// metrics are not exposed on every card. Returns None entirely when NVML is not
// available (no NVIDIA driver, or an AMD/Intel GPU).
struct GpuStats {
    name: Option<String>,
    usage: Option<f32>,
    temp: Option<f32>,
    mem_used: Option<u64>,
    mem_total: Option<u64>,
    fan: Option<f32>,
}

// Starting NVML is slow, so a working handle is kept between polls. It is
// dropped when the card cannot be read (for example after a driver update), so
// the next poll starts fresh.
fn nvml_stats() -> Option<GpuStats> {
    static NVML: Mutex<Option<Nvml>> = Mutex::new(None);
    let mut slot = NVML.lock().unwrap_or_else(|e| e.into_inner());
    if slot.is_none() {
        *slot = Nvml::init().ok();
    }
    let stats = read_gpu(slot.as_ref()?);
    if stats.is_none() {
        *slot = None;
    }
    stats
}

fn read_gpu(nvml: &Nvml) -> Option<GpuStats> {
    let device = nvml.device_by_index(0).ok()?;
    let mem = device.memory_info().ok();
    Some(GpuStats {
        name: device.name().ok(),
        usage: device.utilization_rates().ok().map(|u| u.gpu as f32),
        temp: device
            .temperature(TemperatureSensor::Gpu)
            .ok()
            .map(|t| t as f32),
        mem_used: mem.as_ref().map(|m| m.used),
        mem_total: mem.as_ref().map(|m| m.total),
        fan: device.fan_speed(0).ok().map(|f| f as f32),
    })
}

fn cpu_temp() -> Option<f32> {
    let components = Components::new_with_refreshed_list();
    for c in &components {
        let label = c.label().to_lowercase();
        if label.contains("cpu") || label.contains("package") || label.contains("tctl") {
            if let Some(t) = c.temperature() {
                return Some(t);
            }
        }
    }
    None
}

pub fn snapshot() -> MonitorSnapshot {
    let mut sys = System::new();
    // CPU usage needs two samples spaced by a short interval.
    sys.refresh_cpu_usage();
    thread::sleep(Duration::from_millis(250));
    sys.refresh_cpu_usage();
    sys.refresh_memory();

    let cpu_usage = Some(sys.global_cpu_usage());
    let mem_used = sys.used_memory();
    let mem_total = sys.total_memory();

    let disks_info = Disks::new_with_refreshed_list();
    let mut disks = Vec::new();
    for d in &disks_info {
        let total = d.total_space();
        let avail = d.available_space();
        disks.push(DiskUsage {
            mount: d.mount_point().display().to_string(),
            used: total.saturating_sub(avail),
            total,
        });
    }

    // GPU: prefer NVML live stats, fall back to the WMI adapter name.
    let gpu = nvml_stats();
    let gpu_name = gpu
        .as_ref()
        .and_then(|g| g.name.clone())
        .or_else(gpu_name_cached);

    MonitorSnapshot {
        cpu_usage,
        cpu_temp: cpu_temp(),
        mem_used,
        mem_total,
        gpu_name,
        gpu_usage: gpu.as_ref().and_then(|g| g.usage),
        gpu_temp: gpu.as_ref().and_then(|g| g.temp),
        gpu_mem_used: gpu.as_ref().and_then(|g| g.mem_used),
        gpu_mem_total: gpu.as_ref().and_then(|g| g.mem_total),
        gpu_fan: gpu.as_ref().and_then(|g| g.fan),
        disks,
    }
}

// Top processes by CPU. Two refreshes spaced by a short interval are needed for
// a meaningful CPU reading. CPU is normalized to a share of total capacity.
pub fn top_processes() -> Vec<ProcInfo> {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    thread::sleep(Duration::from_millis(300));
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let cores = std::thread::available_parallelism()
        .map(|n| n.get() as f32)
        .unwrap_or(1.0);

    let mut list: Vec<ProcInfo> = sys
        .processes()
        .values()
        .map(|p| ProcInfo {
            name: p.name().to_string_lossy().to_string(),
            pid: p.pid().as_u32(),
            cpu: p.cpu_usage() / cores,
            mem: p.memory(),
        })
        .collect();

    list.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
    list.truncate(8);
    list
}
