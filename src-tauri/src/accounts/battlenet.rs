// Battle.net
//
// Battle.net keeps every account that ticked "Keep me logged in" and signs in to
// the first name in its settings file, so switching only changes that order.
// NEXUS also remembers the names itself, because an empty list (used to reach the
// sign-in screen) would otherwise hide them.

use super::*;

pub(super) const BNET_ID: &str = "battlenet";
pub(super) const BNET_EXES: &[&str] = &["Battle.net.exe"];
pub(super) const BNET_GAMES: &[&str] = &[
    "Overwatch.exe",
    "Wow.exe",
    "WowClassic.exe",
    "Diablo IV.exe",
    "D2R.exe",
    "Hearthstone.exe",
    "SC2_x64.exe",
    "cod.exe",
];
pub(super) const BNET_NAMES: &str = "/Client/SavedAccountNames";

pub(super) fn bnet_exe() -> Option<PathBuf> {
    let in_dir = |dir: &Path| {
        ["Battle.net Launcher.exe", "Battle.net.exe"]
            .iter()
            .map(|name| dir.join(name))
            .find(|p| p.is_file())
    };
    let from_reg: Option<String> = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Battle.net")
        .ok()
        .and_then(|k| k.get_value("InstallLocation").ok());
    from_reg
        .and_then(|dir| in_dir(Path::new(&dir)))
        .or_else(|| in_dir(&program_files_x86().join("Battle.net")))
}

pub(super) fn bnet_config_file() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join(r"Battle.net\Battle.net.config"))
}

pub(super) fn bnet_read_config(file: &Path) -> Result<serde_json::Value, String> {
    let text = fs::read_to_string(file).map_err(|e| format!("Cannot read Battle.net's settings: {}", e))?;
    serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|_| "Battle.net's settings could not be understood. Nothing was changed.".to_string())
}

pub(super) fn bnet_names(config: &serde_json::Value) -> Vec<String> {
    config
        .pointer(BNET_NAMES)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(String::from)
        .collect()
}

// `first` signs in next; `None` empties the list so Battle.net asks who to sign in.
pub(super) fn bnet_reordered(names: &[String], first: Option<&str>) -> String {
    let Some(first) = first else {
        return String::new();
    };
    let mut ordered = vec![first.to_string()];
    ordered.extend(names.iter().filter(|n| !n.eq_ignore_ascii_case(first)).cloned());
    ordered.join(",")
}

pub(super) fn bnet_remember(state: &mut PlatformState, names: &[String]) {
    for name in names {
        if !state.profiles.iter().any(|p| p.name.eq_ignore_ascii_case(name)) {
            state.profiles.push(SavedProfile {
                id: new_profile_id(),
                name: name.clone(),
                saved_at: now_secs(),
                identity: None,
            });
        }
    }
}

pub(super) fn bnet_write_names(file: &Path, state: &mut PlatformState, first: Option<&str>) -> Result<(), String> {
    // Read after closing: Battle.net rewrites this file as it runs.
    let mut config = bnet_read_config(file)?;
    let names = bnet_names(&config);
    bnet_remember(state, &names);
    save_state(BNET_ID, state)?;
    if let Some(first) = first
        && !state.profiles.iter().any(|p| p.name.eq_ignore_ascii_case(first)) {
            return Err("Battle.net no longer remembers this account.".into());
        }
    let value = config
        .pointer_mut("/Client")
        .and_then(|client| client.as_object_mut())
        .ok_or("Battle.net's settings could not be understood. Nothing was changed.")?;
    value.insert(
        "SavedAccountNames".into(),
        serde_json::Value::String(bnet_reordered(&names, first)),
    );
    let text = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    fs::copy(file, file.with_extension("config.nexus-backup"))
        .map_err(|e| format!("Cannot back up Battle.net's settings: {}", e))?;
    let tmp = file.with_extension("config.nexus-tmp");
    fs::write(&tmp, text).map_err(|e| format!("Cannot update Battle.net's settings: {}", e))?;
    fs::rename(&tmp, file).map_err(|e| format!("Cannot update Battle.net's settings: {}", e))
}

pub(super) fn bnet_change(first: Option<&str>) -> Result<(), String> {
    let exe = bnet_exe().ok_or("Battle.net isn't installed. Nothing was changed.")?;
    let file = bnet_config_file().ok_or("Cannot find the app data folder.")?;
    let mut state = load_state(BNET_ID)?;
    let sys = snapshot_processes();
    if let Some(game) = running_of(&sys, BNET_GAMES).first() {
        return Err(format!("{} is running. Close your game first. Nothing was changed.", game));
    }
    let was_running = !running_of(&sys, BNET_EXES).is_empty();
    if let Err(e) = close_processes("Battle.net", BNET_EXES, Duration::from_secs(12)) {
        reopen_if_was_running(Some(&exe), was_running);
        return Err(e);
    }
    let result = bnet_write_names(&file, &mut state, first);
    reopen_after(&exe, was_running, result)
}

pub(super) fn bnet_platform(sys: &System) -> Option<AccountPlatform> {
    let installed = bnet_exe().is_some();
    let (state, error) = match load_state(BNET_ID) {
        Ok(state) => (state, None),
        Err(e) => (PlatformState::default(), Some(e)),
    };
    let listed = bnet_config_file()
        .and_then(|file| bnet_read_config(&file).ok())
        .map(|config| bnet_names(&config))
        .unwrap_or_default();
    if !installed && listed.is_empty() && state.profiles.is_empty() && error.is_none() {
        return None;
    }
    let is_listed = |name: &str| listed.iter().any(|n| n.eq_ignore_ascii_case(name));
    let mut names = listed.clone();
    names.extend(state.profiles.iter().map(|p| p.name.clone()).filter(|n| !is_listed(n)));
    let current = listed.first().cloned();
    let accounts = names
        .into_iter()
        .map(|name| AccountEntry {
            active: current.as_deref().is_some_and(|c| c.eq_ignore_ascii_case(&name)),
            remembered: is_listed(&name),
            id: name.clone(),
            name,
            detail: String::new(),
            avatar: None,
        })
        .collect();
    Some(AccountPlatform {
        id: BNET_ID.into(),
        name: "Battle.net".into(),
        running: !running_of(sys, BNET_EXES).is_empty(),
        managed: false,
        verified: true,
        installed,
        current,
        accounts,
        error,
    })
}
