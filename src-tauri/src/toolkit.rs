// Windows Toolkit: temporary-file cleaner.
//
// Safety rules baked in:
//  - Only a fixed allowlist of temp locations is ever touched.
//  - Scanning only measures; it never deletes.
//  - Cleaning removes files inside the allowlisted roots only, skips anything
//    in use or inaccessible, and reports exactly what was freed.
//  - The root folders themselves are never removed.

use crate::models::{CleanLocation, CleanResult, ScanResult};
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// The allowlist. Each entry is (id, label, path with %ENV% tokens). Anything not
// on this list is never touched.
fn location_specs() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("user_temp", "User temporary files", "%LOCALAPPDATA%\\Temp"),
        ("windows_temp", "Windows temporary files", "%SystemRoot%\\Temp"),
        ("crash_dumps", "Crash dumps", "%LOCALAPPDATA%\\CrashDumps"),
        ("prefetch", "Windows Prefetch", "%SystemRoot%\\Prefetch"),
        (
            "win_update",
            "Windows Update cache",
            "%SystemRoot%\\SoftwareDistribution\\Download",
        ),
        (
            "thumbnails",
            "Thumbnail and icon cache",
            "%LOCALAPPDATA%\\Microsoft\\Windows\\Explorer",
        ),
        (
            "chrome_cache",
            "Chrome cache",
            "%LOCALAPPDATA%\\Google\\Chrome\\User Data\\Default\\Cache",
        ),
        (
            "edge_cache",
            "Edge cache",
            "%LOCALAPPDATA%\\Microsoft\\Edge\\User Data\\Default\\Cache",
        ),
    ]
}

fn expand_env(input: &str) -> String {
    let mut out = String::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(end) = input[i + 1..].find('%') {
                let name = &input[i + 1..i + 1 + end];
                if let Ok(val) = std::env::var(name) {
                    out.push_str(&val);
                    i = i + 1 + end + 1;
                    continue;
                }
            }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

// Resolve the allowlist to existing, de-duplicated roots.
fn resolved_roots() -> Vec<(String, String, PathBuf)> {
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut out = Vec::new();
    for (id, label, raw) in location_specs() {
        let path = PathBuf::from(expand_env(raw));
        if !path.exists() {
            continue;
        }
        let canon = path.canonicalize().unwrap_or_else(|_| path.clone());
        if seen.iter().any(|s| s == &canon) {
            continue;
        }
        seen.push(canon);
        out.push((id.to_string(), label.to_string(), path));
    }
    out
}

// Walk a directory, returning (file_count, total_bytes). Errors on individual
// entries are ignored so one locked file cannot abort the whole scan.
fn measure_dir(root: &Path) -> (u64, u64) {
    let mut files = 0u64;
    let mut bytes = 0u64;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            match entry.file_type() {
                Ok(ft) if ft.is_dir() => stack.push(path),
                Ok(ft) if ft.is_file() => {
                    if let Ok(meta) = entry.metadata() {
                        files += 1;
                        bytes += meta.len();
                    }
                }
                _ => {}
            }
        }
    }
    (files, bytes)
}

pub fn scan() -> ScanResult {
    let mut locations = Vec::new();
    let mut total_bytes = 0u64;
    let mut total_files = 0u64;
    for (id, label, root) in resolved_roots() {
        let (files, bytes) = measure_dir(&root);
        total_bytes += bytes;
        total_files += files;
        locations.push(CleanLocation {
            id,
            label,
            path: root.to_string_lossy().to_string(),
            file_count: files,
            size_bytes: bytes,
        });
    }
    ScanResult {
        locations,
        total_bytes,
        total_files,
    }
}

// Installers and running apps unpack into Temp, so leave files that were changed or
// that arrived in the last day. Unpacked files often keep an old modified time, so
// the creation time is checked too. Caches can be rebuilt at any time, so only the
// Temp folders need this.
const TEMP_MIN_AGE: Duration = Duration::from_secs(24 * 60 * 60);

fn is_recent(meta: &fs::Metadata) -> bool {
    let young = |t: std::io::Result<std::time::SystemTime>| {
        t.ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age < TEMP_MIN_AGE)
    };
    young(meta.modified()) || young(meta.created())
}

// Delete files inside one root. Returns (freed_bytes, removed, skipped, denied).
// `denied` counts files in Windows folders that could not be removed because the
// app is not running as administrator.
fn clean_dir(root: &Path) -> (u64, u64, u64, u64) {
    let keep_recent = root.ends_with("Temp");
    let needs_admin = !crate::elevation::is_elevated()
        && std::env::var("SystemRoot").is_ok_and(|w| root.starts_with(w));
    let mut denied = 0u64;
    let mut freed = 0u64;
    let mut removed = 0u64;
    let mut skipped = 0u64;
    let mut dirs_seen: Vec<PathBuf> = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            match entry.file_type() {
                Ok(ft) if ft.is_dir() => {
                    dirs_seen.push(path.clone());
                    stack.push(path);
                }
                Ok(ft) if ft.is_file() => {
                    let meta = entry.metadata().ok();
                    if keep_recent && meta.as_ref().is_some_and(is_recent) {
                        skipped += 1;
                        continue;
                    }
                    let size = meta.map(|m| m.len()).unwrap_or(0);
                    match fs::remove_file(&path) {
                        Ok(_) => {
                            freed += size;
                            removed += 1;
                        }
                        Err(e) if needs_admin && e.kind() == std::io::ErrorKind::PermissionDenied => {
                            denied += 1
                        }
                        Err(_) => skipped += 1,
                    }
                }
                _ => {}
            }
        }
    }
    // Try to remove now-empty subdirectories, deepest first. Never the root.
    dirs_seen.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for d in dirs_seen {
        let _ = fs::remove_dir(&d);
    }
    (freed, removed, skipped, denied)
}

// Empty the Recycle Bin for all drives via PowerShell (no extra prompts).
pub fn empty_recycle_bin() -> Result<(), String> {
    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Clear-RecycleBin -Force -ErrorAction SilentlyContinue",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("Could not empty Recycle Bin: {}", e))?;
    if status.success() {
        Ok(())
    } else {
        Err("Recycle Bin could not be emptied (it may already be empty).".into())
    }
}

#[link(name = "dnsapi")]
unsafe extern "system" {
    fn DnsFlushResolverCache() -> i32;
}

// Flush the DNS resolver cache. Windows is asked directly: `ipconfig /flushdns`
// needs administrator rights on newer Windows builds, and this call does not.
pub fn flush_dns() -> Result<(), String> {
    if unsafe { DnsFlushResolverCache() } != 0 {
        return Ok(());
    }
    Err("Windows did not clear the DNS cache. Use Restart as admin and try again.".into())
}

pub fn clean(selected_ids: Vec<String>) -> CleanResult {
    let mut freed_bytes = 0u64;
    let mut removed_files = 0u64;
    let mut skipped_files = 0u64;
    let mut denied_files = 0u64;
    for (id, _label, root) in resolved_roots() {
        if !selected_ids.contains(&id) {
            continue;
        }
        let (freed, removed, skipped, denied) = clean_dir(&root);
        freed_bytes += freed;
        removed_files += removed;
        skipped_files += skipped;
        denied_files += denied;
    }
    CleanResult {
        freed_bytes,
        removed_files,
        skipped_files,
        denied_files,
    }
}
