// Application discovery.
//
// We learn what is installed from several Windows sources and match each catalog
// entry against them, then resolve a real executable to launch. Order of truth
// for a launch path:
//   1. a manual override the user picked
//   2. an explicit catalog path that exists
//   3. the "App Paths" registry (the canonical exe location Windows uses)
//   4. the uninstall entry's DisplayIcon (usually the main exe)
//   5. a shallow search of the install folder for a known exe name
// We only ever read here; nothing is launched or modified.

use crate::catalog::catalog;
use crate::models::DetectedApp;
use std::collections::HashMap;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use winreg::enums::*;
use winreg::RegKey;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// Start menu apps (name + AppUserModelID), from Get-StartApps. This covers both
// desktop and Microsoft Store apps, and is how we find packaged apps like
// HyperX NGENUITY that are not in the uninstall registry. Launching uses the
// returned AUMID via shell:AppsFolder.
fn read_start_apps() -> Vec<(String, String)> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-StartApps | Select-Object Name,AppID | ConvertTo-Json -Compress",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    let out = match output {
        Ok(o) if o.status.success() => o.stdout,
        _ => return Vec::new(),
    };
    let text = String::from_utf8_lossy(&out);
    let value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut result = Vec::new();
    let mut push = |v: &serde_json::Value| {
        let name = v.get("Name").and_then(|x| x.as_str());
        let aumid = v.get("AppID").and_then(|x| x.as_str());
        if let (Some(name), Some(aumid)) = (name, aumid) {
            result.push((name.to_lowercase(), aumid.to_string()));
        }
    };
    match value {
        serde_json::Value::Array(arr) => arr.iter().for_each(&mut push),
        serde_json::Value::Object(_) => push(&value),
        _ => {}
    }
    result
}

struct InstalledProgram {
    display_name_lc: String,
    install_location: Option<String>,
    display_icon: Option<String>,
}

fn expand_env(input: &str) -> String {
    let mut out = String::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(end) = input[i + 1..].find('%') {
                let name = &input[i + 1..i + 1 + end];
                match std::env::var(name) {
                    Ok(val) => out.push_str(&val),
                    Err(_) => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                i = i + 1 + end + 1;
                continue;
            }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn read_uninstall_key(hive: RegKey, subkey: &str, out: &mut Vec<InstalledProgram>) {
    let base = match hive.open_subkey_with_flags(subkey, KEY_READ | KEY_WOW64_64KEY) {
        Ok(k) => k,
        Err(_) => return,
    };
    for name in base.enum_keys().flatten() {
        if let Ok(entry) = base.open_subkey(&name) {
            let display_name: Option<String> = entry.get_value("DisplayName").ok();
            if let Some(dn) = display_name {
                out.push(InstalledProgram {
                    display_name_lc: dn.to_lowercase(),
                    install_location: entry.get_value("InstallLocation").ok(),
                    display_icon: entry.get_value("DisplayIcon").ok(),
                });
            }
        }
    }
}

fn read_installed_programs() -> Vec<InstalledProgram> {
    let mut out = Vec::new();
    let machine_paths = [
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
        "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
    ];
    for p in machine_paths {
        read_uninstall_key(RegKey::predef(HKEY_LOCAL_MACHINE), p, &mut out);
    }
    read_uninstall_key(
        RegKey::predef(HKEY_CURRENT_USER),
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
        &mut out,
    );
    out
}

// App Paths maps an executable file name to its full path. This is how Windows
// resolves things like "start msiafterburner.exe", so it is a reliable source.
fn read_app_paths() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let sub = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths";
    for hive in [
        RegKey::predef(HKEY_LOCAL_MACHINE),
        RegKey::predef(HKEY_CURRENT_USER),
    ] {
        if let Ok(base) = hive.open_subkey_with_flags(sub, KEY_READ | KEY_WOW64_64KEY) {
            for name in base.enum_keys().flatten() {
                if let Ok(entry) = base.open_subkey(&name) {
                    // The default value holds the full path.
                    let full: Result<String, _> = entry.get_value("");
                    if let Ok(full) = full {
                        let cleaned = full.trim_matches('"').to_string();
                        if !cleaned.is_empty() {
                            map.insert(name.to_lowercase(), cleaned);
                        }
                    }
                }
            }
        }
    }
    map
}

// Shallow recursive search for any of exe_names under root, up to a small depth.
fn search_dir_for_exe(root: &Path, exe_names: &[String], depth: u32) -> Option<String> {
    if depth == 0 {
        return None;
    }
    let entries = std::fs::read_dir(root).ok()?;
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(ft) if ft.is_file() => {
                if let Some(fname) = path.file_name().and_then(|f| f.to_str())
                    && exe_names.contains(&fname.to_lowercase()) {
                        return Some(path.to_string_lossy().to_string());
                    }
            }
            Ok(ft) if ft.is_dir() => subdirs.push(path),
            _ => {}
        }
    }
    for d in subdirs {
        if let Some(found) = search_dir_for_exe(&d, exe_names, depth - 1) {
            return Some(found);
        }
    }
    None
}

fn resolve_launch_path(
    paths: &[String],
    exe_names: &[String],
    program: Option<&InstalledProgram>,
    app_paths: &HashMap<String, String>,
) -> Option<String> {
    // Explicit catalog paths.
    for p in paths {
        let expanded = expand_env(p);
        if PathBuf::from(&expanded).exists() {
            return Some(expanded);
        }
    }
    // App Paths registry by exe name.
    for exe in exe_names {
        if let Some(full) = app_paths.get(exe)
            && Path::new(full).exists() {
                return Some(full.clone());
            }
    }
    if let Some(prog) = program {
        // DisplayIcon usually points at the main exe.
        if let Some(icon) = &prog.display_icon {
            let cleaned = icon.split(',').next().unwrap_or(icon).trim_matches('"');
            if cleaned.to_lowercase().ends_with(".exe") && Path::new(cleaned).exists() {
                return Some(cleaned.to_string());
            }
        }
        // InstallLocation: direct join, then a shallow search.
        if let Some(loc) = &prog.install_location {
            let loc = loc.trim_matches('"');
            if !loc.is_empty() {
                for exe in exe_names {
                    let candidate = PathBuf::from(loc).join(exe);
                    if candidate.exists() {
                        return Some(candidate.to_string_lossy().to_string());
                    }
                }
                if let Some(found) = search_dir_for_exe(Path::new(loc), exe_names, 3) {
                    return Some(found);
                }
            }
        }
    }
    None
}

// Lowercased display names of everything in the uninstall registry. Used by the
// runtimes module to detect installed redistributables.
pub fn installed_display_names() -> Vec<String> {
    read_installed_programs()
        .into_iter()
        .map(|p| p.display_name_lc)
        .collect()
}

// Lowercased names of Start menu apps, which includes Microsoft Store apps.
// Used to detect Store installs (for example Spotify) that are not in the
// uninstall registry.
pub fn start_app_names() -> Vec<String> {
    read_start_apps().into_iter().map(|(n, _)| n).collect()
}

pub fn detect_apps(overrides: &HashMap<String, String>) -> Vec<DetectedApp> {
    let installed = read_installed_programs();
    let app_paths = read_app_paths();
    let start_apps = read_start_apps();
    let mut results = Vec::new();

    for app in catalog() {
        let matched = installed.iter().find(|p| {
            app.registry_names
                .iter()
                .any(|rn| p.display_name_lc.contains(rn))
        });

        // A manual override wins when the file still exists.
        let override_path = overrides
            .get(&app.id)
            .filter(|p| Path::new(p).exists())
            .cloned();

        let launch_path = override_path
            .or_else(|| resolve_launch_path(&app.paths, &app.exe_names, matched, &app_paths));

        // If no desktop exe resolved, try to find it as a Start menu / Store app
        // by matching the catalog's name fragments against Start app names.
        let launch_aumid = if launch_path.is_none() {
            start_apps
                .iter()
                .find(|(name, _)| app.registry_names.iter().any(|rn| name.contains(rn)))
                .map(|(_, aumid)| aumid.clone())
        } else {
            None
        };

        // Installed if a registry entry matched, we resolved a path, or it was
        // found as a Store app.
        let installed_flag = matched.is_some() || launch_path.is_some() || launch_aumid.is_some();

        results.push(DetectedApp {
            id: app.id,
            name: app.name,
            vendor: app.vendor,
            category: app.category,
            description: app.description,
            download_url: app.download_url,
            compatibility: app.compatibility,
            installed: installed_flag,
            launch_path,
            launch_aumid,
        });
    }
    results
}
