// Update Center. Lists apps with available upgrades via winget and updates them.
//
// winget has no machine-readable output for `upgrade`, so we parse the text
// table. We use the header row to find each column's start, which handles names
// and ids that contain spaces. If winget's output cannot be read (for example a
// localized Windows), that is reported instead of claiming everything is current.

use crate::models::AppUpdate;
use crate::runtimes::InstallResult;
use std::os::windows::process::CommandExt;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn list_updates() -> Result<Vec<AppUpdate>, String> {
    let output = Command::new("winget")
        .args([
            "upgrade",
            "--include-unknown",
            "--accept-source-agreements",
            "--disable-interactivity",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Could not run winget: {}. Is App Installer installed?", e))?;
    let text = String::from_utf8_lossy(&output.stdout);
    if text.contains("No installed package found") {
        return Ok(Vec::new());
    }
    parse_upgrade_table(&text).ok_or_else(|| {
        format!(
            "Could not read winget's update list (exit code {}).",
            output.status.code().unwrap_or(-1)
        )
    })
}

fn parse_upgrade_table(text: &str) -> Option<Vec<AppUpdate>> {
    let lines: Vec<&str> = text.lines().collect();

    // Find the header row (English Windows).
    let header_idx = lines.iter().position(|l| {
        l.contains("Name") && l.contains("Id") && l.contains("Version") && l.contains("Available")
    })?;
    let header = lines[header_idx];

    // The header is ASCII, so byte offsets equal char offsets here.
    let id_i = header.find("Id")?;
    let ver_i = header.find("Version")?;
    let avail_i = header.find("Available")?;
    let src_i = header.find("Source").unwrap_or(usize::MAX);

    let slice = |chars: &[char], start: usize, end: usize| -> String {
        if start >= chars.len() {
            return String::new();
        }
        let end = end.min(chars.len());
        chars[start..end].iter().collect::<String>().trim().to_string()
    };

    let mut result = Vec::new();
    // Data rows start two lines after the header (skip the dashes line).
    for line in lines.iter().skip(header_idx + 2) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break; // end of the first table block
        }
        // Stop at the summary line(s).
        let lower = trimmed.to_lowercase();
        if lower.contains("upgrades available") || lower.contains("package(s) have") {
            break;
        }
        let chars: Vec<char> = line.chars().collect();
        let name = slice(&chars, 0, id_i);
        let id = slice(&chars, id_i, ver_i);
        let current = slice(&chars, ver_i, avail_i);
        let available = slice(&chars, avail_i, src_i);

        // A real row has an id (winget ids contain a dot) and an available version.
        if id.contains('.') && !available.is_empty() {
            result.push(AppUpdate {
                name,
                id,
                current,
                available,
            });
        }
    }
    Some(result)
}

fn run_winget(args: &[&str], success: &str) -> InstallResult {
    let output = Command::new("winget")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    match output {
        Ok(o) => {
            let code = o.status.code().unwrap_or(-1);
            if let Some((ok, message)) = crate::runtimes::winget_outcome(code, success) {
                InstallResult { ok, message }
            } else {
                let err = String::from_utf8_lossy(&o.stderr);
                let out = String::from_utf8_lossy(&o.stdout);
                let tail: String = out
                    .lines()
                    .chain(err.lines())
                    .rev()
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(" ");
                InstallResult {
                    ok: false,
                    message: format!("winget exited with code {}. {}", code, tail.trim()),
                }
            }
        }
        Err(e) => InstallResult {
            ok: false,
            message: format!("Could not run winget: {}", e),
        },
    }
}

pub fn upgrade_app(id: &str) -> InstallResult {
    if id.is_empty() || id.contains(['\r', '\n', ' ']) {
        return InstallResult {
            ok: false,
            message: "Invalid package id.".into(),
        };
    }
    run_winget(&[
        "upgrade",
        "--id",
        id,
        "-e",
        "--include-unknown",
        "--accept-source-agreements",
        "--accept-package-agreements",
        "--silent",
        "--disable-interactivity",
    ], "Updated.")
}

pub fn upgrade_all() -> InstallResult {
    run_winget(&[
        "upgrade",
        "--all",
        "--include-unknown",
        "--accept-source-agreements",
        "--accept-package-agreements",
        "--silent",
        "--disable-interactivity",
    ], "All updates finished.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unreadable_output_is_an_error_not_an_empty_list() {
        assert!(parse_upgrade_table("Nom  Identifiant  Version\n").is_none());
    }

    // Read-only: `winget upgrade` with no id only lists, it installs nothing.
    #[test]
    fn lists_updates_on_this_pc() {
        let list = list_updates().unwrap();
        println!("updates={}", list.len());
        assert!(list.iter().all(|u| u.id.contains('.') && !u.available.is_empty()));
    }
}
