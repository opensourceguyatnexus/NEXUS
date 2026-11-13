// Hardware detection.
//
// CPU, memory, and disks come from sysinfo. GPUs and USB peripherals come from
// WMI, which is the standard Windows source for that information. If WMI is
// unavailable the related lists simply come back empty rather than failing the
// whole call.

use crate::models::{HardwareDevice, HardwareInfo};
use nvml_wrapper::Nvml;
use serde::Deserialize;
use sysinfo::{Disks, System};
use wmi::{COMLibrary, WMIConnection};

// Only string fields here. Numeric WMI fields (like AdapterRAM) vary in type and
// can fail deserialization, which would drop the whole row including the name.
// Accurate VRAM comes from NVML instead.
#[derive(Deserialize)]
#[serde(rename = "Win32_VideoController")]
#[serde(rename_all = "PascalCase")]
struct Win32VideoController {
    name: Option<String>,
    driver_version: Option<String>,
}

// The primary NVIDIA GPU as a device entry, via NVML. This is the reliable path
// on NVIDIA cards (the same source the System Monitor uses). None on other GPUs.
fn nvml_gpu_device() -> Option<HardwareDevice> {
    let nvml = Nvml::init().ok()?;
    let device = nvml.device_by_index(0).ok()?;
    let name = device.name().ok()?;
    let mut detail = String::new();
    if let Ok(mem) = device.memory_info() {
        detail.push_str(&format!("{} VRAM", gib(mem.total)));
    }
    if let Ok(driver) = nvml.sys_driver_version() {
        if !detail.is_empty() {
            detail.push_str("  |  ");
        }
        detail.push_str(&format!("driver {}", driver));
    }
    Some(HardwareDevice {
        kind: "gpu".into(),
        name,
        detail,
    })
}

#[derive(Deserialize)]
#[serde(rename = "Win32_OperatingSystem")]
#[serde(rename_all = "PascalCase")]
struct Win32OperatingSystem {
    caption: Option<String>,
    version: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename = "Win32_PointingDevice")]
#[serde(rename_all = "PascalCase")]
struct Win32PointingDevice {
    name: Option<String>,
    manufacturer: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename = "Win32_Keyboard")]
#[serde(rename_all = "PascalCase")]
struct Win32Keyboard {
    name: Option<String>,
    description: Option<String>,
}

fn gib(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
}

fn query_gpus_wmi(wmi: &WMIConnection) -> Vec<HardwareDevice> {
    let mut out = Vec::new();
    if let Ok(rows) = wmi.query::<Win32VideoController>() {
        for r in rows {
            let name = r.name.unwrap_or_else(|| "Unknown display adapter".into());
            let detail = r
                .driver_version
                .map(|dv| format!("driver {}", dv))
                .unwrap_or_default();
            out.push(HardwareDevice {
                kind: "gpu".into(),
                name,
                detail,
            });
        }
    }
    out
}

fn query_peripherals(wmi: &WMIConnection) -> Vec<HardwareDevice> {
    let mut out = Vec::new();
    if let Ok(rows) = wmi.query::<Win32PointingDevice>() {
        for r in rows {
            if let Some(name) = r.name {
                let detail = r.manufacturer.unwrap_or_default();
                out.push(HardwareDevice {
                    kind: "pointing".into(),
                    name,
                    detail,
                });
            }
        }
    }
    if let Ok(rows) = wmi.query::<Win32Keyboard>() {
        for r in rows {
            if let Some(name) = r.name {
                let detail = r.description.unwrap_or_default();
                out.push(HardwareDevice {
                    kind: "keyboard".into(),
                    name,
                    detail,
                });
            }
        }
    }
    out
}

fn query_os(wmi: &WMIConnection) -> Option<HardwareDevice> {
    if let Ok(rows) = wmi.query::<Win32OperatingSystem>()
        && let Some(r) = rows.into_iter().next() {
            let name = r.caption.unwrap_or_else(|| "Windows".into());
            let detail = r.version.map(|v| format!("version {}", v)).unwrap_or_default();
            return Some(HardwareDevice {
                kind: "os".into(),
                name,
                detail,
            });
        }
    None
}

pub fn detect_hardware() -> HardwareInfo {
    let mut sys = System::new_all();
    sys.refresh_all();

    // CPU
    let cpu = sys.cpus().first().map(|c| {
        let count = sys.cpus().len();
        HardwareDevice {
            kind: "cpu".into(),
            name: c.brand().trim().to_string(),
            detail: format!("{} logical cores", count),
        }
    });

    // Memory (RAM)
    let memory = Some(HardwareDevice {
        kind: "memory".into(),
        name: format!("{} total", gib(sys.total_memory())),
        detail: format!("RAM · {} free", gib(sys.available_memory())),
    });

    // Disks
    let disks_info = Disks::new_with_refreshed_list();
    let mut disks = Vec::new();
    for d in &disks_info {
        let total = d.total_space();
        let avail = d.available_space();
        let used = total.saturating_sub(avail);
        disks.push(HardwareDevice {
            kind: "disk".into(),
            name: format!("{}", d.mount_point().display()),
            detail: format!("{} used of {}", gib(used), gib(total)),
        });
    }

    // NVML is the reliable source for an NVIDIA GPU, independent of WMI.
    let nvml_gpu = nvml_gpu_device();

    // WMI-backed data. If COM or WMI is not available, degrade gracefully.
    let (wmi_gpus, peripherals, os) = match COMLibrary::new()
        .and_then(WMIConnection::new)
    {
        Ok(wmi) => (query_gpus_wmi(&wmi), query_peripherals(&wmi), query_os(&wmi)),
        Err(_) => (Vec::new(), Vec::new(), None),
    };

    // Prefer the NVML entry; add any non-NVIDIA WMI adapters (e.g. integrated).
    let gpus = match nvml_gpu {
        Some(primary) => {
            let mut v = vec![primary];
            for w in wmi_gpus {
                if !w.name.to_lowercase().contains("nvidia") {
                    v.push(w);
                }
            }
            v
        }
        None => wmi_gpus,
    };

    HardwareInfo {
        cpu,
        gpus,
        memory,
        disks,
        peripherals,
        os,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Read-only check against this PC. Prints counts and the OS name only.
    #[test]
    fn wmi_hardware_is_found_on_this_pc() {
        let info = detect_hardware();
        println!(
            "os={:?} gpus={} peripherals={}",
            info.os.as_ref().map(|o| o.name.clone()),
            info.gpus.len(),
            info.peripherals.len()
        );
        assert!(info.os.is_some(), "WMI should report the OS");
        assert!(!info.gpus.is_empty(), "WMI or NVML should report a GPU");
        assert!(!info.peripherals.is_empty(), "WMI should report a keyboard or mouse");
    }
}
