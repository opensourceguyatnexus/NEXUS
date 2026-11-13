// Browser profiles: browsers already support them, so NEXUS only lists them
// and opens the chosen one.

use super::*;

pub(super) fn app_path(exe: &str) -> Option<PathBuf> {
    let sub = format!(r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{}", exe);
    [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER].iter().find_map(|hive| {
        let value: String = RegKey::predef(*hive).open_subkey(&sub).ok()?.get_value("").ok()?;
        let p = PathBuf::from(value.trim_matches('"'));
        p.is_file().then_some(p)
    })
}

pub(super) const CHROMIUM: &[(&str, &str, &str, &str)] = &[
    ("chrome", "Google Chrome", "chrome.exe", r"Google\Chrome\User Data"),
    ("edge", "Microsoft Edge", "msedge.exe", r"Microsoft\Edge\User Data"),
    ("brave", "Brave", "brave.exe", r"BraveSoftware\Brave-Browser\User Data"),
];

pub(super) fn chromium_profiles(user_data: &str) -> Option<Vec<BrowserProfile>> {
    let file = dirs::data_local_dir()?.join(user_data).join("Local State");
    let json: serde_json::Value = serde_json::from_str(&fs::read_to_string(file).ok()?).ok()?;
    let cache = json.get("profile")?.get("info_cache")?.as_object()?;
    let text = |v: &serde_json::Value, k: &str| {
        v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
    };
    let mut list: Vec<BrowserProfile> = cache
        .iter()
        .map(|(dir, info)| {
            let name = text(info, "name");
            BrowserProfile {
                id: dir.clone(),
                name: if name.is_empty() { dir.clone() } else { name },
                detail: text(info, "user_name"),
            }
        })
        .collect();
    list.sort_by_key(|a| a.name.to_lowercase());
    Some(list)
}

pub(super) fn firefox_profiles() -> Vec<BrowserProfile> {
    let Some(file) = dirs::data_dir().map(|d| d.join(r"Mozilla\Firefox\profiles.ini")) else {
        return Vec::new();
    };
    fs::read_to_string(file)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.trim().strip_prefix("Name="))
        .map(|name| BrowserProfile {
            id: name.to_string(),
            name: name.to_string(),
            detail: String::new(),
        })
        .collect()
}

pub(super) fn list_browsers() -> Vec<BrowserInfo> {
    let mut browsers: Vec<BrowserInfo> = CHROMIUM
        .iter()
        .filter(|(_, _, exe, _)| app_path(exe).is_some())
        .filter_map(|(id, name, _, data)| {
            Some(BrowserInfo {
                id: id.to_string(),
                name: name.to_string(),
                profiles: chromium_profiles(data)?,
            })
        })
        .collect();
    if app_path("firefox.exe").is_some() {
        let profiles = firefox_profiles();
        if !profiles.is_empty() {
            browsers.push(BrowserInfo {
                id: "firefox".into(),
                name: "Mozilla Firefox".into(),
                profiles,
            });
        }
    }
    browsers
}

// Profile names reach a command line, so allow only plain characters.
pub(super) fn safe_profile_arg(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'))
}

pub fn open_browser(browser: &str, profile: &str) -> Result<(), String> {
    // Only a profile the browser itself reports can be opened.
    let info = list_browsers()
        .into_iter()
        .find(|b| b.id == browser)
        .ok_or("That browser was not found.")?;
    if !info.profiles.iter().any(|p| p.id == profile) || !safe_profile_arg(profile) {
        return Err("That browser profile was not found.".into());
    }
    let exe_name = if browser == "firefox" {
        "firefox.exe"
    } else {
        CHROMIUM
            .iter()
            .find(|c| c.0 == browser)
            .map(|c| c.2)
            .ok_or("That browser was not found.")?
    };
    let exe = app_path(exe_name).ok_or("That browser was not found.")?;
    let args: Vec<String> = if browser == "firefox" {
        vec!["-P".into(), profile.to_string()]
    } else {
        vec![format!("--profile-directory={}", profile)]
    };
    if crate::elevation::is_elevated() {
        return open_unelevated_with_args(&exe, &args);
    }
    Command::new(&exe)
        .args(&args)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not open {}: {}", info.name, e))
}

// A browser should never run as administrator. Explorer cannot pass arguments,
// so when NEXUS is elevated it starts a small self-deleting script for us.
pub(super) fn open_unelevated_with_args(exe: &Path, args: &[String]) -> Result<(), String> {
    let exe_text = exe.to_string_lossy();
    if exe_text.contains(['"', '%']) {
        return Err("The browser path contains unsupported characters.".into());
    }
    let quoted: Vec<String> = args.iter().map(|a| format!("\"{}\"", a)).collect();
    let script = format!(
        "@start \"\" \"{}\" {}\r\n@del \"%~f0\"\r\n",
        exe_text,
        quoted.join(" ")
    );
    let file = std::env::temp_dir().join(format!("nexus-open-{}.bat", new_profile_id()));
    fs::write(&file, script).map_err(|e| format!("Could not prepare the browser launch: {}", e))?;
    launch_unelevated(&file)
}
