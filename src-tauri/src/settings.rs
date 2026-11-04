// User settings, stored as a single JSON file under the user's config folder
// (for example %APPDATA%\NEXUS\settings.json). All reads and writes are local;
// nothing leaves the machine.

use crate::models::{CustomApp, Settings};
use std::fs;
use std::path::PathBuf;

fn settings_dir() -> PathBuf {
    let mut dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("NEXUS");
    dir
}

fn settings_file() -> PathBuf {
    let mut f = settings_dir();
    f.push("settings.json");
    f
}

pub fn load_settings() -> Settings {
    let file = settings_file();
    let mut settings: Settings = match fs::read_to_string(&file) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|_| {
            // Keep the unreadable file, since the next save replaces it.
            let _ = fs::write(settings_dir().join("settings.bad.json"), &text);
            Settings::default()
        }),
        Err(_) => Settings::default(),
    };
    // Accent colors from older versions.
    if !matches!(settings.accent.as_str(), "blue" | "green" | "orange" | "rose") {
        settings.accent = "blue".into();
    }
    settings
}

pub fn save_settings(settings: &Settings) -> Result<(), String> {
    let dir = settings_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create settings folder: {}", e))?;
    let text = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Cannot serialize settings: {}", e))?;
    // Write then rename, so a crash mid-write cannot wipe the settings.
    let tmp = dir.join("settings.json.tmp");
    fs::write(&tmp, text).map_err(|e| format!("Cannot write settings: {}", e))?;
    fs::rename(&tmp, settings_file()).map_err(|e| format!("Cannot write settings: {}", e))?;
    Ok(())
}

pub fn set_theme(theme: &str) -> Result<Settings, String> {
    let mut s = load_settings();
    s.theme = theme.to_string();
    save_settings(&s)?;
    Ok(s)
}

pub fn set_accent(accent: &str) -> Result<Settings, String> {
    let mut s = load_settings();
    s.accent = accent.to_string();
    save_settings(&s)?;
    Ok(s)
}

pub fn toggle_favorite(app_id: &str) -> Result<Settings, String> {
    let mut s = load_settings();
    if let Some(pos) = s.favorites.iter().position(|x| x == app_id) {
        s.favorites.remove(pos);
    } else {
        s.favorites.push(app_id.to_string());
    }
    save_settings(&s)?;
    Ok(s)
}

// Record a recently launched app id, most recent first, capped at 8 entries.
pub fn push_recent(app_id: &str) -> Result<Settings, String> {
    let mut s = load_settings();
    s.recent.retain(|x| x != app_id);
    s.recent.insert(0, app_id.to_string());
    s.recent.truncate(8);
    save_settings(&s)?;
    Ok(s)
}

pub fn add_custom_app(app: CustomApp) -> Result<Settings, String> {
    let mut s = load_settings();
    // Replace an existing entry with the same id, otherwise append.
    if let Some(pos) = s.custom_apps.iter().position(|a| a.id == app.id) {
        s.custom_apps[pos] = app;
    } else {
        s.custom_apps.push(app);
    }
    save_settings(&s)?;
    Ok(s)
}

pub fn set_launch_override(app_id: &str, path: &str) -> Result<Settings, String> {
    let mut s = load_settings();
    s.launch_overrides
        .insert(app_id.to_string(), path.to_string());
    save_settings(&s)?;
    Ok(s)
}

pub fn remove_custom_app(app_id: &str) -> Result<Settings, String> {
    let mut s = load_settings();
    s.custom_apps.retain(|a| a.id != app_id);
    s.favorites.retain(|f| f != app_id);
    s.recent.retain(|r| r != app_id);
    save_settings(&s)?;
    Ok(s)
}
