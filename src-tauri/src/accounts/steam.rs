// Steam: it remembers every account itself, so switching only changes which
// one signs in next (the registry and loginusers.vdf).

use super::*;

pub(super) struct SteamAccount {
    pub(super) id: String,
    pub(super) account: String,
    pub(super) persona: String,
    pub(super) remember: bool,
}

// One `"key" "value"` pair, or a lone `"token"`, honoring backslash escapes.
pub(super) fn vdf_tokens(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut token = String::new();
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    if let Some(next) = chars.next() {
                        token.push(next);
                    }
                }
                '"' => break,
                _ => token.push(c),
            }
        }
        tokens.push(token);
    }
    tokens
}

pub(super) fn is_steam_id(token: &str) -> bool {
    token.len() >= 15 && token.chars().all(|c| c.is_ascii_digit())
}

pub(super) fn parse_steam_users(text: &str) -> Vec<SteamAccount> {
    let mut users: Vec<SteamAccount> = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let tokens = vdf_tokens(line);
        if line.trim() == "}" {
            inside = false;
        } else if tokens.len() == 1 && is_steam_id(&tokens[0]) {
            users.push(SteamAccount {
                id: tokens[0].clone(),
                account: String::new(),
                persona: String::new(),
                remember: false,
            });
            inside = true;
        } else if inside && tokens.len() == 2
            && let Some(u) = users.last_mut() {
                match tokens[0].to_ascii_lowercase().as_str() {
                    "accountname" => u.account = tokens[1].clone(),
                    "personaname" => u.persona = tokens[1].clone(),
                    "rememberpassword" => u.remember = tokens[1] == "1",
                    _ => {}
                }
            }
    }
    users.retain(|u| !u.account.is_empty());
    users
}

// Mark `target` as the account Steam should use, changing only the flags that
// decide this and leaving every other line as it was.
pub(super) fn set_most_recent(text: &str, target: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut current: Option<String> = None;
    let mut seen_recent = false;
    for line in text.lines() {
        let tokens = vdf_tokens(line);
        if tokens.len() == 1 && is_steam_id(&tokens[0]) {
            current = Some(tokens[0].clone());
            seen_recent = false;
            out.push(line.to_string());
            continue;
        }
        let Some(id) = current.clone() else {
            out.push(line.to_string());
            continue;
        };
        let is_target = id == target;
        if line.trim() == "}" {
            if is_target && !seen_recent {
                out.push("\t\t\"MostRecent\"\t\t\"1\"".to_string());
            }
            current = None;
            out.push(line.to_string());
            continue;
        }
        if tokens.len() == 2 {
            let key = tokens[0].to_ascii_lowercase();
            if key == "mostrecent" {
                seen_recent = true;
            }
            let value = match key.as_str() {
                "mostrecent" | "autologin" => Some(if is_target { "1" } else { "0" }),
                "allowautologin" if is_target => Some("1"),
                _ => None,
            };
            if let Some(value) = value {
                let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
                out.push(format!("{}\"{}\"\t\t\"{}\"", indent, tokens[0], value));
                continue;
            }
        }
        out.push(line.to_string());
    }
    let mut joined = out.join("\n");
    if text.ends_with('\n') {
        joined.push('\n');
    }
    joined
}

pub(super) fn steam_key() -> Option<RegKey> {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(r"Software\Valve\Steam", KEY_READ | KEY_SET_VALUE)
        .ok()
}

pub(super) fn steam_paths() -> Option<(PathBuf, PathBuf)> {
    let key = steam_key()?;
    let exe: String = key.get_value("SteamExe").ok()?;
    let path: String = key.get_value("SteamPath").ok()?;
    let exe = PathBuf::from(exe.replace('/', "\\"));
    exe.is_file()
        .then(|| (exe, PathBuf::from(path.replace('/', "\\"))))
}

// Steam records the game it is running, so a match is never killed by a switch.
pub(super) fn refuse_if_steam_game_running() -> Result<(), String> {
    let app: u32 = steam_key()
        .and_then(|k| k.get_value("RunningAppID").ok())
        .unwrap_or(0);
    if app != 0 {
        return Err("A Steam game is running. Close your game first. Nothing was changed.".into());
    }
    Ok(())
}

pub(super) fn close_steam(exe: &Path) -> Result<(), String> {
    let names = ["steam.exe"];
    if !running_of(&snapshot_processes(), &names).is_empty() {
        // Ask Steam to exit by itself first so it can save its own state.
        let _ = Command::new(exe).arg("-shutdown").spawn();
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline && !running_of(&snapshot_processes(), &names).is_empty() {
            thread::sleep(Duration::from_millis(500));
        }
    }
    close_processes("Steam", &names, Duration::from_secs(10))
}

pub(super) fn steam_switch(steam_id: &str) -> Result<(), String> {
    let (exe, root) = steam_paths().ok_or("Steam was not found.")?;
    let vdf = root.join(r"config\loginusers.vdf");
    let text = fs::read_to_string(&vdf).map_err(|e| format!("Cannot read Steam's accounts: {}", e))?;
    let account = parse_steam_users(&text)
        .into_iter()
        .find(|u| u.id == steam_id)
        .ok_or("Steam no longer remembers this account.")?;
    refuse_if_steam_game_running()?;
    let was_running = steam_is_running();
    close_steam(&exe)?;
    let result = (|| -> Result<(), String> {
        // Read again: Steam rewrites this file as it exits.
        let text = fs::read_to_string(&vdf).map_err(|e| format!("Cannot read Steam's accounts: {}", e))?;
        fs::write(root.join(r"config\loginusers.vdf.nexus-backup"), &text)
            .map_err(|e| format!("Cannot back up Steam's accounts: {}", e))?;
        let tmp = root.join(r"config\loginusers.vdf.nexus-tmp");
        fs::write(&tmp, set_most_recent(&text, steam_id))
            .map_err(|e| format!("Cannot update Steam's accounts: {}", e))?;
        fs::rename(&tmp, &vdf).map_err(|e| format!("Cannot update Steam's accounts: {}", e))?;
        let key = steam_key().ok_or("Cannot open Steam's registry settings.")?;
        key.set_value("AutoLoginUser", &account.account)
            .map_err(|e| format!("Cannot update Steam's settings: {}", e))?;
        key.set_value("RememberPassword", &1u32)
            .map_err(|e| format!("Cannot update Steam's settings: {}", e))
    })();
    reopen_after(&exe, was_running, result)
}

pub(super) fn steam_is_running() -> bool {
    !running_of(&snapshot_processes(), &["steam.exe"]).is_empty()
}

// An empty auto-login user makes Steam show its own account picker.
pub(super) fn steam_add_new() -> Result<(), String> {
    let (exe, _) = steam_paths().ok_or("Steam was not found.")?;
    refuse_if_steam_game_running()?;
    let was_running = steam_is_running();
    close_steam(&exe)?;
    let result = steam_key()
        .ok_or_else(|| "Cannot open Steam's registry settings.".to_string())
        .and_then(|key| {
            key.set_value("AutoLoginUser", &String::new())
                .map_err(|e| format!("Cannot update Steam's settings: {}", e))
        });
    reopen_after(&exe, was_running, result)
}

pub(super) fn steam_platform(sys: &System) -> Option<AccountPlatform> {
    let (_, root) = steam_paths()?;
    let text = fs::read_to_string(root.join(r"config\loginusers.vdf")).unwrap_or_default();
    let auto: String = steam_key()?.get_value("AutoLoginUser").unwrap_or_default();
    let accounts: Vec<AccountEntry> = parse_steam_users(&text)
        .into_iter()
        .map(|u| {
            let avatar = fs::read(root.join(format!(r"config\avatarcache\{}.png", u.id)))
                .ok()
                .map(|b| format!("data:image/png;base64,{}", STANDARD.encode(b)));
            AccountEntry {
                active: u.account.eq_ignore_ascii_case(&auto),
                name: if u.persona.is_empty() {
                    u.account.clone()
                } else {
                    u.persona.clone()
                },
                detail: u.account,
                id: u.id,
                avatar,
                remembered: u.remember,
            }
        })
        .collect();
    let current = accounts.iter().find(|a| a.active).map(|a| a.name.clone());
    Some(AccountPlatform {
        id: "steam".into(),
        name: "Steam".into(),
        running: !running_of(sys, &["steam.exe"]).is_empty(),
        managed: false,
        verified: true,
        installed: true,
        current,
        accounts,
        error: None,
    })
}
