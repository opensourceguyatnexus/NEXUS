// Disk space analyzer. Given a folder, it returns the immediate children (files
// and subfolders) with each subfolder's total recursive size, sorted largest
// first. The UI drills down by analyzing a chosen subfolder. Read-only: it never
// deletes anything. Access errors on protected paths are ignored.

use crate::models::FolderEntry;
use std::path::Path;

// Total size of everything under a directory, in bytes. Iterative to avoid deep
// recursion, and tolerant of folders it cannot read.
fn dir_size(root: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            match entry.file_type() {
                Ok(ft) if ft.is_dir() => stack.push(entry.path()),
                Ok(ft) if ft.is_file() => {
                    if let Ok(meta) = entry.metadata() {
                        total += meta.len();
                    }
                }
                _ => {}
            }
        }
    }
    total
}

pub fn analyze_path(path: &str) -> Result<Vec<FolderEntry>, String> {
    let p = Path::new(path);
    if !p.is_dir() {
        return Err(format!("Not a folder: {}", path));
    }
    let entries = std::fs::read_dir(p).map_err(|e| format!("Cannot read folder: {}", e))?;

    let mut out = Vec::new();
    for entry in entries.flatten() {
        let ep = entry.path();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let size = if is_dir {
            dir_size(&ep)
        } else {
            entry.metadata().map(|m| m.len()).unwrap_or(0)
        };
        let name = ep
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        out.push(FolderEntry {
            name,
            path: ep.to_string_lossy().to_string(),
            size,
            is_dir,
        });
    }
    out.sort_by_key(|entry| std::cmp::Reverse(entry.size));
    out.truncate(100);
    Ok(out)
}
