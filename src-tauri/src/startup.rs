// Startup Manager.
//
// Lists programs that run at sign-in, from the Run registry keys (per-user and
// system) and the Startup folders. Enable/disable uses the same StartupApproved
// mechanism Task Manager uses: a small binary value whose first byte is 0x02
// (enabled) or 0x03 (disabled). We only toggle user-scope entries, which need no
// admin rights; system-wide entries are listed read-only.

use crate::models::StartupItem;
use std::path::PathBuf;
use winreg::enums::*;
use winreg::{RegKey, RegValue};

const RUN: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run";
const APPROVED_RUN: &str =
    "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run";
const APPROVED_FOLDER: &str =
    "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\StartupFolder";

// Read the enabled flag for one entry name from a StartupApproved subkey.
// Missing value means the entry is enabled.
fn approved_enabled(hive: &RegKey, approved_path: &str, name: &str) -> bool {
    if let Ok(key) = hive.open_subkey(approved_path) {
        if let Ok(val) = key.get_raw_value(name) {
            if let Some(first) = val.bytes.first() {
                return first % 2 == 0; // 0x02 enabled, 0x03 disabled
            }
        }
    }
    true
}

fn read_run_key(hive: &RegKey, approved_path: &str, source: &str, location: &str, can_toggle: bool, out: &mut Vec<StartupItem>) {
    let run = match hive.open_subkey(RUN) {
        Ok(k) => k,
        Err(_) => return,
    };
    for (name, value) in run.enum_values().flatten() {
        let command = value.to_string();
        let enabled = approved_enabled(hive, approved_path, &name);
        out.push(StartupItem {
            name,
            command,
            location: location.into(),
            source: source.into(),
            enabled,
            can_toggle,
        });
    }
}

fn read_folder(dir: Option<PathBuf>, approved_hive: &RegKey, source: &str, location: &str, can_toggle: bool, out: &mut Vec<StartupItem>) {
    let dir = match dir {
        Some(d) if d.exists() => d,
        _ => return,
    };
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !ext.eq_ignore_ascii_case("lnk") && !ext.eq_ignore_ascii_case("exe") {
            continue;
        }
        let file_name = match path.file_name().and_then(|f| f.to_str()) {
            Some(f) => f.to_string(),
            None => continue,
        };
        let display = path.file_stem().and_then(|f| f.to_str()).unwrap_or(&file_name).to_string();
        let enabled = approved_enabled(approved_hive, APPROVED_FOLDER, &file_name);
        out.push(StartupItem {
            name: display,
            command: path.to_string_lossy().to_string(),
            location: location.into(),
            source: source.into(),
            enabled,
            can_toggle,
        });
    }
}

fn user_startup_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|mut p| {
        p.push("Microsoft\\Windows\\Start Menu\\Programs\\Startup");
        p
    })
}

fn common_startup_dir() -> Option<PathBuf> {
    std::env::var("ProgramData")
        .ok()
        .map(|pd| PathBuf::from(pd).join("Microsoft\\Windows\\Start Menu\\Programs\\Startup"))
}

// `elevated` controls whether system-wide (HKLM) entries can be toggled, since
// that needs administrator rights.
pub fn list_startup(elevated: bool) -> Vec<StartupItem> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut out = Vec::new();

    read_run_key(&hkcu, APPROVED_RUN, "hkcu_run", "This user (registry)", true, &mut out);
    read_run_key(&hklm, APPROVED_RUN, "hklm_run", "All users (registry)", elevated, &mut out);

    read_folder(user_startup_dir(), &hkcu, "user_folder", "This user (Startup folder)", true, &mut out);
    read_folder(common_startup_dir(), &hklm, "common_folder", "All users (Startup folder)", elevated, &mut out);

    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

// Write the StartupApproved binary flag. 12 bytes: first byte 0x02 enabled or
// 0x03 disabled, the rest a timestamp we leave as zero.
fn write_approved(hive: &RegKey, approved_path: &str, name: &str, enabled: bool) -> Result<(), String> {
    let (key, _) = hive
        .create_subkey(approved_path)
        .map_err(|e| format!("Cannot open approval key: {}", e))?;
    let mut bytes = vec![0u8; 12];
    bytes[0] = if enabled { 0x02 } else { 0x03 };
    let val = RegValue {
        bytes,
        vtype: REG_BINARY,
    };
    key.set_raw_value(name, &val)
        .map_err(|e| format!("Cannot write approval: {}", e))?;
    Ok(())
}

// For a Startup folder entry, the approval value name is the file name, which we
// no longer have from the UI (we pass the display name). Re-resolve it by
// matching the display name against the folder contents.
fn folder_value_name(dir: Option<PathBuf>, display: &str) -> Option<String> {
    let dir = dir?;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        let stem = path.file_stem().and_then(|f| f.to_str()).unwrap_or("");
        if stem.eq_ignore_ascii_case(display) {
            return path.file_name().and_then(|f| f.to_str()).map(|s| s.to_string());
        }
    }
    None
}

pub fn set_startup_enabled(source: &str, name: &str, enabled: bool) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    match source {
        "hkcu_run" => write_approved(&hkcu, APPROVED_RUN, name, enabled),
        "hklm_run" => write_approved(&hklm, APPROVED_RUN, name, enabled),
        "user_folder" => {
            let value_name = folder_value_name(user_startup_dir(), name)
                .ok_or_else(|| "Could not find the startup file.".to_string())?;
            write_approved(&hkcu, APPROVED_FOLDER, &value_name, enabled)
        }
        "common_folder" => {
            let value_name = folder_value_name(common_startup_dir(), name)
                .ok_or_else(|| "Could not find the startup file.".to_string())?;
            write_approved(&hklm, APPROVED_FOLDER, &value_name, enabled)
        }
        _ => Err("Unknown startup entry.".into()),
    }
}
