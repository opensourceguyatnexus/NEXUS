// Shared data structures used across the NEXUS backend.
// Everything here is serializable so it can cross the Tauri boundary to the UI.

use serde::{Deserialize, Serialize};

// A single utility defined in the curated catalog. These are the apps NEXUS
// knows how to detect and link to. Detection rules are kept simple on purpose:
// a registry display-name match, known executable paths, or executable names.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogApp {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub category: String,
    pub description: String,
    pub download_url: String,
    // Notes about which hardware this utility actually supports, so the UI can
    // avoid claiming a device is supported just because a vendor name matched.
    pub compatibility: String,
    // Lowercase substrings matched against installed-program display names.
    pub registry_names: Vec<String>,
    // Absolute paths (may contain %ENV% tokens) checked for existence.
    pub paths: Vec<String>,
    // Executable file names used as a fallback match against installed programs.
    pub exe_names: Vec<String>,
}

// The result of checking a catalog app against the current machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedApp {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub category: String,
    pub description: String,
    pub download_url: String,
    pub compatibility: String,
    pub installed: bool,
    // The path we will launch, when we found one.
    pub launch_path: Option<String>,
    // For Microsoft Store / packaged apps: the AppUserModelID to launch instead
    // of a file path.
    pub launch_aumid: Option<String>,
}

// A user-defined app that NEXUS did not detect automatically.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomApp {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub exec_path: String,
    pub download_url: String,
}

// Persisted user preferences. Kept small and local.
// New fields use serde defaults so older settings files keep loading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub theme: String,
    #[serde(default = "default_accent")]
    pub accent: String,
    pub favorites: Vec<String>,
    pub recent: Vec<String>,
    pub custom_apps: Vec<CustomApp>,
    // Manually chosen executable paths, keyed by catalog app id. Used when
    // automatic detection cannot resolve a launch path.
    #[serde(default)]
    pub launch_overrides: std::collections::HashMap<String, String>,
}

fn default_accent() -> String {
    "blue".into()
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "dark".into(),
            accent: default_accent(),
            favorites: Vec::new(),
            recent: Vec::new(),
            custom_apps: Vec::new(),
            launch_overrides: std::collections::HashMap::new(),
        }
    }
}

// A detected hardware device, grouped by kind in the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareDevice {
    pub kind: String,
    pub name: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub cpu: Option<HardwareDevice>,
    pub gpus: Vec<HardwareDevice>,
    pub memory: Option<HardwareDevice>,
    pub disks: Vec<HardwareDevice>,
    pub peripherals: Vec<HardwareDevice>,
    pub os: Option<HardwareDevice>,
}

// A live snapshot for the System Monitor. Values that cannot be read on this
// machine are returned as None so the UI can say "unavailable" instead of
// inventing a number.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorSnapshot {
    pub cpu_usage: Option<f32>,
    pub cpu_temp: Option<f32>,
    pub mem_used: u64,
    pub mem_total: u64,
    pub gpu_name: Option<String>,
    pub gpu_usage: Option<f32>,
    pub gpu_temp: Option<f32>,
    pub gpu_mem_used: Option<u64>,
    pub gpu_mem_total: Option<u64>,
    pub gpu_fan: Option<f32>,
    pub disks: Vec<DiskUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskUsage {
    pub mount: String,
    pub used: u64,
    pub total: u64,
}

// A running process for the Dashboard's top-processes list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcInfo {
    pub name: String,
    pub pid: u32,
    pub cpu: f32,
    pub mem: u64,
}

// Temp-file cleaner types. Scanning only ever reports; cleaning is a separate,
// explicit step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanLocation {
    pub id: String,
    pub label: String,
    pub path: String,
    pub file_count: u64,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub locations: Vec<CleanLocation>,
    pub total_bytes: u64,
    pub total_files: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanResult {
    pub freed_bytes: u64,
    pub removed_files: u64,
    pub skipped_files: u64,
    pub denied_files: u64,
}

// An app with an available upgrade, from winget.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppUpdate {
    pub name: String,
    pub id: String,
    pub current: String,
    pub available: String,
}

// One entry (file or folder) in the disk space analyzer, with its total size.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
}

// A Windows startup entry (registry Run key or Startup folder).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupItem {
    pub name: String,
    pub command: String,
    // Human label for where it comes from.
    pub location: String,
    // Internal source tag used for toggling: hkcu_run, hklm_run, hklm_run32, user_folder, common_folder.
    pub source: String,
    pub enabled: bool,
    // False when toggling would need admin (system-wide entries).
    pub can_toggle: bool,
}
