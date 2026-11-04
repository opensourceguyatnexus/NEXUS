// Launching and opening.
//
// Three entry points, all guarded:
//  - launch_path: start a real executable after checking it exists.
//  - open_url: open an https link in the default browser.
//  - open_windows_tool: open a built-in Windows tool from a fixed allowlist.
//
// We never build a shell command string from caller-supplied text. Executables
// are spawned directly; the Windows tools come from a hardcoded match, so there
// is nothing to inject into.

use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// Launch an app through the shell with the "runas" verb, which triggers the
// standard UAC prompt. Used for programs whose manifest requires elevation
// (for example HWiNFO), where a plain CreateProcess fails with error 740.
fn launch_elevated(path: &str, dir: Option<&Path>) -> Result<(), String> {
    let file = to_wide(path);
    let verb = to_wide("runas");
    let dir_wide = dir.map(|d| to_wide(&d.to_string_lossy()));
    let dir_ptr = dir_wide
        .as_ref()
        .map(|v| PCWSTR(v.as_ptr()))
        .unwrap_or_else(PCWSTR::null);

    let result = unsafe {
        ShellExecuteW(
            HWND(std::ptr::null_mut()),
            PCWSTR(verb.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR::null(),
            dir_ptr,
            SW_SHOWNORMAL,
        )
    };
    // ShellExecuteW returns a value greater than 32 on success.
    if result.0 as isize > 32 {
        Ok(())
    } else {
        Err("Launch was cancelled or blocked (administrator rights needed).".into())
    }
}

pub fn launch_path(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(format!("Path does not exist: {}", path));
    }
    if !p.is_file() {
        return Err(format!("Not a file: {}", path));
    }
    let dir = p.parent();
    let mut cmd = Command::new(path);
    if let Some(d) = dir {
        cmd.current_dir(d);
    }
    match cmd.spawn() {
        Ok(_) => Ok(()),
        Err(e) => {
            // 740 = the application requires elevation. Retry via UAC.
            if e.raw_os_error() == Some(740) {
                launch_elevated(path, dir)
            } else {
                Err(format!("Failed to launch: {}", e))
            }
        }
    }
}

// Launch a Microsoft Store / packaged app by its AppUserModelID. We hand the
// AUMID to explorer via the AppsFolder shell path. The AUMID comes from our own
// Get-StartApps query, not from user text.
pub fn launch_store(aumid: &str) -> Result<(), String> {
    if aumid.is_empty() || aumid.contains(['\r', '\n']) {
        return Err("Invalid app id.".into());
    }
    Command::new("explorer.exe")
        .arg(format!("shell:AppsFolder\\{}", aumid))
        .spawn()
        .map_err(|e| format!("Failed to launch app: {}", e))?;
    Ok(())
}

pub fn open_url(url: &str) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Only http(s) links can be opened.".into());
    }
    // ShellExecute opens the default browser directly. Going through cmd would
    // treat an & in the URL as a command separator.
    let file = to_wide(url);
    let verb = to_wide("open");
    let result = unsafe {
        ShellExecuteW(
            HWND(std::ptr::null_mut()),
            PCWSTR(verb.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize > 32 {
        Ok(())
    } else {
        Err("Windows could not open that link.".into())
    }
}

// Open a built-in Windows tool. The id must be one we recognize; anything else
// is rejected. Each arm is a fixed command with fixed arguments.
pub fn open_windows_tool(id: &str) -> Result<(), String> {
    let mut cmd = match id {
        "task_manager" => {
            let mut c = Command::new("cmd");
            c.args(["/C", "start", "", "taskmgr.exe"]);
            c
        }
        "device_manager" => {
            let mut c = Command::new("cmd");
            c.args(["/C", "start", "", "devmgmt.msc"]);
            c
        }
        "disk_cleanup" => Command::new("cleanmgr.exe"),
        "system_info" => Command::new("msinfo32.exe"),
        "windows_settings" => {
            let mut c = Command::new("cmd");
            c.args(["/C", "start", "", "ms-settings:"]);
            c
        }
        "startup_apps" => {
            let mut c = Command::new("cmd");
            c.args(["/C", "start", "", "ms-settings:startupapps"]);
            c
        }
        "apps_features" => {
            let mut c = Command::new("cmd");
            c.args(["/C", "start", "", "ms-settings:appsfeatures"]);
            c
        }
        "storage_settings" => {
            let mut c = Command::new("cmd");
            c.args(["/C", "start", "", "ms-settings:storagesense"]);
            c
        }
        "network" => {
            let mut c = Command::new("cmd");
            c.args(["/C", "start", "", "ncpa.cpl"]);
            c
        }
        other => return Err(format!("Unknown tool: {}", other)),
    };
    cmd.creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Failed to open tool: {}", e))?;
    Ok(())
}
