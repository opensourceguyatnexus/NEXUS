// Update Center. Lists apps with available upgrades via winget and updates them.
//
// winget has no machine-readable output for `upgrade`, so we parse the text
// table. We use the header row to find each column's start, which handles names
// and ids that contain spaces. The header is found by the dashes line under it,
// not by its wording, so this also works on a non-English Windows. If winget
// fails and prints no table, that is reported instead of claiming everything is
// current.

use crate::models::AppUpdate;
use crate::runtimes::InstallResult;
use std::os::windows::process::CommandExt;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
// winget's "no package found" exit code (0x8A150014).
const NO_PACKAGES_FOUND: i32 = 0x8A15_0014_u32 as i32;

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
    if let Some(list) = parse_upgrade_table(&text) {
        return Ok(list);
    }
    // No table at all: winget prints only a sentence when nothing needs updating.
    let code = output.status.code().unwrap_or(-1);
    if code == 0 || code == NO_PACKAGES_FOUND || text.contains("No installed package found") {
        return Ok(Vec::new());
    }
    Err(format!("Could not read winget's update list (exit code {}).", code))
}

fn parse_upgrade_table(text: &str) -> Option<Vec<AppUpdate>> {
    // winget redraws its progress spinner with bare carriage returns, so keep
    // only what is left on screen after the last one.
    let lines: Vec<&str> = text
        .lines()
        .map(|l| {
            let l = l.trim_end_matches('\r');
            l.rsplit('\r').next().unwrap_or(l)
        })
        .collect();

    let is_rule = |l: &str| {
        let t = l.trim();
        t.len() >= 10 && t.chars().all(|c| c == '-')
    };
    // The header is the line above the first line made only of dashes.
    let dashes_idx = lines.iter().position(|l| is_rule(l))?;
    let header: Vec<char> = lines[dashes_idx.checked_sub(1)?].chars().collect();

    // Each column starts where a header word starts. Counted in characters,
    // because translated headers are not always ASCII.
    let starts: Vec<usize> = (0..header.len())
        .filter(|&i| !header[i].is_whitespace() && (i == 0 || header[i - 1].is_whitespace()))
        .collect();
    // Name, Id, Version, Available, and usually Source.
    if starts.len() < 4 {
        return None;
    }
    let (name_i, id_i, ver_i, avail_i) = (starts[0], starts[1], starts[2], starts[3]);
    let src_i = starts.get(4).copied().unwrap_or(usize::MAX);

    let slice = |chars: &[char], start: usize, end: usize| -> String {
        if start >= chars.len() {
            return String::new();
        }
        let end = end.min(chars.len());
        chars[start..end].iter().collect::<String>().trim().to_string()
    };

    let mut result = Vec::new();
    for line in lines.iter().skip(dashes_idx + 1) {
        let chars: Vec<char> = line.chars().collect();
        // A blank line, a short summary line or the start of a second table
        // ends the first table block.
        if chars.len() <= avail_i || is_rule(line) {
            break;
        }
        let name = slice(&chars, name_i, id_i);
        let id = slice(&chars, id_i, ver_i);
        let current = slice(&chars, ver_i, avail_i);
        let available = slice(&chars, avail_i, src_i);

        // A real row has an id (winget ids contain a dot and no spaces) and an
        // available version. Longer summary sentences fail this.
        if id.contains('.') && !id.contains(char::is_whitespace) && !available.is_empty() {
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

    #[test]
    fn reads_a_translated_table_after_spinner_frames() {
        let text = [
            "   -\r   \\\r     \rNom       ID           Versión  Disponible  Origen",
            "-------------------------------------------------",
            "Mi Año    Vendor.App   1.0      2.0         winget",
            "1 actualizaciones disponibles.",
            "",
        ]
        .join("\r\n");
        let list = parse_upgrade_table(&text).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Mi Año");
        assert_eq!(list[0].id, "Vendor.App");
        assert_eq!(list[0].current, "1.0");
        assert_eq!(list[0].available, "2.0");
    }

    // Read-only: `winget upgrade` with no id only lists, it installs nothing.
    #[test]
    fn lists_updates_on_this_pc() {
        let list = list_updates().unwrap();
        println!("updates={}", list.len());
        assert!(list.iter().all(|u| u.id.contains('.') && !u.available.is_empty()));
    }
}
