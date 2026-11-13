// Task Manager: list running processes and Windows services, and stop them.
//
// Processes come from sysinfo and are ended with TerminateProcess. Core Windows
// processes (and NEXUS itself) are protected so a click cannot crash the
// session. Services come from WMI and are stopped or started with sc.exe, which
// needs administrator rights for most services.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::thread;
use std::time::Duration;
use sysinfo::{Pid, ProcessesToUpdate, System};
use wmi::{COMLibrary, WMIConnection};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// Ending any of these logs you out, blue-screens, or breaks the desktop.
const PROTECTED_PROCS: &[&str] = &[
    "system",
    "system idle process",
    "idle",
    "registry",
    "secure system",
    "memory compression",
    "smss.exe",
    "csrss.exe",
    "wininit.exe",
    "winlogon.exe",
    "services.exe",
    "lsass.exe",
    "lsaiso.exe",
    "svchost.exe",
    "fontdrvhost.exe",
    "dwm.exe",
    "explorer.exe",
];

// Services Windows itself (or NEXUS, via WMI) needs to keep running.
const PROTECTED_SERVICES: &[&str] = &[
    "rpcss",
    "rpceptmapper",
    "dcomlaunch",
    "lsm",
    "brokerinfrastructure",
    "samss",
    "power",
    "plugplay",
    "coremessagingregistrar",
    "systemeventsbroker",
    "eventlog",
    "winmgmt",
    "gpsvc",
];

#[derive(Serialize)]
pub struct TaskProc {
    pub name: String,
    pub pid: u32,
    pub cpu: f32,
    pub mem: u64,
    pub protected: bool,
}

#[derive(Serialize)]
pub struct ServiceInfo {
    pub name: String,
    pub display_name: String,
    pub state: String,
    pub start_mode: String,
    pub pid: u32,
    pub can_stop: bool,
    pub can_start: bool,
}

#[derive(Deserialize)]
#[serde(rename = "Win32_Service")]
#[serde(rename_all = "PascalCase")]
struct Win32Service {
    name: String,
    display_name: Option<String>,
    state: Option<String>,
    start_mode: Option<String>,
    process_id: Option<u32>,
    accept_stop: Option<bool>,
}

fn is_protected_proc(name: &str, pid: u32) -> bool {
    pid <= 4 || pid == std::process::id() || PROTECTED_PROCS.contains(&name.to_lowercase().as_str())
}

// NEXUS's own child processes (its WebView2 window). Ending one blanks the app.
fn own_process_tree(sys: &System) -> HashSet<u32> {
    let mut tree: HashSet<u32> = HashSet::from([std::process::id()]);
    loop {
        let before = tree.len();
        for p in sys.processes().values() {
            if p.parent().is_some_and(|parent| tree.contains(&parent.as_u32())) {
                tree.insert(p.pid().as_u32());
            }
        }
        if tree.len() == before {
            return tree;
        }
    }
}

fn is_protected_service(name: &str) -> bool {
    PROTECTED_SERVICES.contains(&name.to_lowercase().as_str())
}

// All processes, heaviest memory first. CPU is a share of total capacity.
pub fn list_processes() -> Vec<TaskProc> {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    thread::sleep(Duration::from_millis(300));
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let cores = std::thread::available_parallelism()
        .map(|n| n.get() as f32)
        .unwrap_or(1.0);

    let own = own_process_tree(&sys);
    let mut list: Vec<TaskProc> = sys
        .processes()
        .values()
        .map(|p| {
            let name = p.name().to_string_lossy().to_string();
            let pid = p.pid().as_u32();
            TaskProc {
                protected: is_protected_proc(&name, pid) || own.contains(&pid),
                name,
                pid,
                cpu: p.cpu_usage() / cores,
                mem: p.memory(),
            }
        })
        .collect();

    list.sort_by_key(|task| std::cmp::Reverse(task.mem));
    list
}

pub fn end_process(pid: u32) -> Result<(), String> {
    let mut sys = System::new();
    let target = Pid::from_u32(pid);
    sys.refresh_processes(ProcessesToUpdate::All, true);
    // Already gone counts as ended: children often exit along with their parent.
    let Some(proc_) = sys.process(target) else {
        return Ok(());
    };
    let name = proc_.name().to_string_lossy().to_string();
    if is_protected_proc(&name, pid) || own_process_tree(&sys).contains(&pid) {
        return Err(format!("{} is protected because Windows or NEXUS needs it.", name));
    }
    if proc_.kill() {
        Ok(())
    } else {
        Err(format!(
            "Windows refused to end {}. It may need administrator rights.",
            name
        ))
    }
}

pub fn list_services() -> Result<Vec<ServiceInfo>, String> {
    let wmi = COMLibrary::new()
        .and_then(WMIConnection::new)
        .map_err(|e| format!("Could not read services: {}", e))?;
    let rows: Vec<Win32Service> = wmi
        .query()
        .map_err(|e| format!("Could not read services: {}", e))?;

    let mut list: Vec<ServiceInfo> = rows
        .into_iter()
        .map(|s| {
            let state = s.state.unwrap_or_default();
            let start_mode = s.start_mode.unwrap_or_default();
            let protected = is_protected_service(&s.name);
            ServiceInfo {
                can_stop: !protected && state == "Running" && s.accept_stop.unwrap_or(false),
                can_start: state == "Stopped" && start_mode != "Disabled",
                display_name: s.display_name.unwrap_or_else(|| s.name.clone()),
                name: s.name,
                state,
                start_mode,
                pid: s.process_id.unwrap_or(0),
            }
        })
        .collect();

    // Running first, then alphabetical.
    list.sort_by(|a, b| {
        (a.state != "Running")
            .cmp(&(b.state != "Running"))
            .then_with(|| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()))
    });
    Ok(list)
}

// Service names can contain spaces ("Steam Client Service"). The name goes straight
// to sc.exe with no shell, so only reject what can never be part of one.
fn valid_service_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 256
        && !name.chars().any(|c| c.is_control() || matches!(c, '"' | '/' | '\\'))
}

fn sc(action: &str, name: &str) -> Result<(), String> {
    if !valid_service_name(name) {
        return Err("Invalid service name.".into());
    }
    let out = Command::new("sc.exe")
        .args([action, name])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Could not run sc.exe: {}", e))?;
    // sc.exe exits with the Windows error code.
    match out.status.code().unwrap_or(-1) {
        0 => Ok(()),
        5 => Err("Access denied. Restart NEXUS as administrator to control services.".into()),
        1051 => Err("Other running services depend on it. Stop those first.".into()),
        1052 => Err("This service does not accept that request.".into()),
        1056 => Err("The service is already running.".into()),
        1058 => Err("The service is disabled.".into()),
        1060 => Err("That service does not exist.".into()),
        1062 => Err("The service is not running.".into()),
        code => Err(format!(
            "sc.exe failed (code {}): {}",
            code,
            String::from_utf8_lossy(&out.stdout).trim()
        )),
    }
}

pub fn stop_service(name: &str) -> Result<(), String> {
    if is_protected_service(name) {
        return Err(format!("{} is a core Windows service and is protected.", name));
    }
    sc("stop", name)
}

pub fn start_service(name: &str) -> Result<(), String> {
    sc("start", name)
}
