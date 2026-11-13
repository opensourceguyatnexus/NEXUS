// Account Switcher: move between saved logins for game launchers and open
// browser profiles. NEXUS never sees or stores a password.
//
//  - Steam keeps every remembered account itself, so switching only changes which
//    one signs in next (registry + loginusers.vdf).
//  - Other launchers keep one login in files. Saving an account copies those files
//    to %LOCALAPPDATA%\NEXUS\accounts, and switching copies them back while the
//    launcher is closed. These copies are as sensitive as the originals and stay
//    on this PC, protected by the Windows user profile like the originals.
//  - Browsers already support profiles, so we list them and open the chosen one.

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sysinfo::{ProcessesToUpdate, System};
use winreg::enums::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Credentials::{
    CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST, CRED_TYPE_GENERIC,
};
use windows::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB};
use winreg::RegKey;

// How long a closed launcher gets to let go of its files.
const FILE_RELEASE_WAIT: Duration = Duration::from_secs(10);

// Only one switch or save may touch login files at a time.
static LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy)]
enum Root {
    Local,
    Roaming,
}

impl Root {
    fn folder(self) -> &'static str {
        match self {
            Root::Local => "Local",
            Root::Roaming => "Roaming",
        }
    }
}

struct Roots {
    local: PathBuf,
    roaming: PathBuf,
}

impl Roots {
    fn real() -> Result<Self, String> {
        Ok(Self {
            local: dirs::data_local_dir().ok_or("Cannot find the local app data folder.")?,
            roaming: dirs::data_dir().ok_or("Cannot find the app data folder.")?,
        })
    }

    fn path(&self, root: Root, rel: &str) -> PathBuf {
        match root {
            Root::Local => self.local.join(rel),
            Root::Roaming => self.roaming.join(rel),
        }
    }
}

// A launcher whose login lives in files.
struct Spec {
    id: &'static str,
    name: &'static str,
    // The launcher's own processes, closed before login files are touched.
    exes: &'static [&'static str],
    // Known game processes. NEXUS refuses to switch instead of killing a match.
    // Epic and Ubisoft games vary too much to list, so their dialog warns instead.
    games: &'static [&'static str],
    items: &'static [(Root, &'static str)],
    reg: Option<(&'static str, &'static str)>,
    // A Windows Credential Manager entry that is part of the login.
    cred: Option<&'static str>,
    // Leftovers of the previous account, removed on a switch and never saved.
    clear: &'static [(Root, &'static str)],
}

const EPIC_NEW_AUTH: &str = r"com.epicgames.eglv2\auth";

const SPECS: &[Spec] = &[
    Spec {
        id: "riot",
        name: "Riot Games",
        exes: &[
            "RiotClientServices.exe",
            "RiotClientUx.exe",
            "RiotClientUxRender.exe",
            "Riot Client.exe",
        ],
        games: &[
            "VALORANT-Win64-Shipping.exe",
            "VALORANT.exe",
            "LeagueClient.exe",
            "League of Legends.exe",
            "LoR.exe",
        ],
        items: &[
            (Root::Local, r"Riot Games\Riot Client\Data\Sessions"),
            (Root::Local, r"Riot Games\Riot Client\Data\Cookies"),
            (Root::Local, r"Riot Games\Riot Client\Data\RiotGamesPrivateSettings.yaml"),
            (Root::Local, r"Riot Games\Riot Client\Data\RiotClientPrivateSettings.yaml"),
            (Root::Local, r"Riot Games\Riot Client\Config\RiotClientSettings.yaml"),
        ],
        reg: None,
        cred: None,
        clear: &[],
    },
    Spec {
        id: "discord",
        name: "Discord",
        exes: &["Discord.exe"],
        games: &[],
        items: &[
            (Root::Roaming, r"discord\Local Storage\leveldb"),
            (Root::Roaming, r"discord\Session Storage"),
            (Root::Roaming, r"discord\Network"),
        ],
        reg: None,
        cred: None,
        clear: &[],
    },
    Spec {
        id: "epic",
        name: "Epic Games",
        exes: &["EpicGamesLauncher.exe", "EpicWebHelper.exe"],
        games: &[],
        items: &[
            (Root::Local, r"EpicGamesLauncher\Saved\Config\WindowsEditor\GameUserSettings.ini"),
            (Root::Local, r"EpicGamesLauncher\Saved\Config\Windows\GameUserSettings.ini"),
            // The newer launcher keeps its login here and in Credential Manager.
            (Root::Local, EPIC_NEW_AUTH),
        ],
        reg: Some((r"Software\Epic Games\Unreal Engine\Identifiers", "AccountId")),
        cred: Some("credentials.com.epicgames.eglv2.prod"),
        clear: &[(Root::Local, r"com.epicgames.eglv2\EBWebView")],
    },
    Spec {
        id: "ubisoft",
        name: "Ubisoft Connect",
        exes: &["upc.exe", "UplayService.exe", "UplayWebCore.exe"],
        games: &[],
        items: &[
            (Root::Local, r"Ubisoft Game Launcher\ConnectSecureStorage.dat"),
            (Root::Local, r"Ubisoft Game Launcher\settings.yaml"),
            (Root::Local, r"Ubisoft Game Launcher\user.dat"),
        ],
        reg: None,
        cred: None,
        clear: &[],
    },
];

fn spec(id: &str) -> Result<&'static Spec, String> {
    SPECS
        .iter()
        .find(|s| s.id == id)
        .ok_or_else(|| "Unknown launcher.".to_string())
}

// ---- What the UI receives ----

#[derive(Serialize)]
pub struct AccountEntry {
    pub id: String,
    pub name: String,
    pub detail: String,
    pub active: bool,
    pub avatar: Option<String>,
    // Steam can only switch silently when it remembers this account's login.
    pub remembered: bool,
}

#[derive(Serialize)]
pub struct AccountPlatform {
    pub id: String,
    pub name: String,
    pub running: bool,
    // Steam lists accounts itself, so there is nothing to save or delete.
    pub managed: bool,
    // Whether NEXUS can prove which account is signed in. Ubisoft does not expose
    // this, so its "active" only means the last one NEXUS switched to.
    pub verified: bool,
    // Saved accounts stay listed if the launcher is uninstalled, but can't be used.
    pub installed: bool,
    pub current: Option<String>,
    pub accounts: Vec<AccountEntry>,
    // Set when the saved account list could not be read.
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct BrowserProfile {
    pub id: String,
    pub name: String,
    pub detail: String,
}

#[derive(Serialize)]
pub struct BrowserInfo {
    pub id: String,
    pub name: String,
    pub profiles: Vec<BrowserProfile>,
}

#[derive(Serialize)]
pub struct AccountsOverview {
    pub platforms: Vec<AccountPlatform>,
    pub browsers: Vec<BrowserInfo>,
}

// ---- Saved account index ----

#[derive(Serialize, Deserialize, Default)]
struct PlatformState {
    active: Option<String>,
    profiles: Vec<SavedProfile>,
}

#[derive(Serialize, Deserialize, Clone)]
struct SavedProfile {
    id: String,
    name: String,
    saved_at: u64,
    // A stable account id when the launcher exposes one, never a token.
    identity: Option<String>,
}

fn store_dir(platform: &str) -> Result<PathBuf, String> {
    let base = dirs::data_local_dir().ok_or("Cannot find the local app data folder.")?;
    Ok(base.join("NEXUS").join("accounts").join(platform))
}

fn load_state(platform: &str) -> Result<PlatformState, String> {
    let file = store_dir(platform)?.join("state.json");
    match fs::read_to_string(file) {
        Ok(text) => serde_json::from_str(&text)
            .map_err(|e| format!("The saved account list is damaged: {}", e)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(PlatformState::default()),
        Err(e) => Err(format!("Cannot read the saved account list: {}", e)),
    }
}

fn save_state(platform: &str, state: &PlatformState) -> Result<(), String> {
    let dir = store_dir(platform)?;
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create the accounts folder: {}", e))?;
    let text = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    let tmp = dir.join("state.json.tmp");
    // Flushed to disk before the rename, so a power cut cannot leave an empty list.
    fs::File::create(&tmp)
        .and_then(|mut file| {
            file.write_all(text.as_bytes())?;
            file.sync_all()
        })
        .map_err(|e| format!("Cannot save the account list: {}", e))?;
    fs::rename(&tmp, dir.join("state.json"))
        .map_err(|e| format!("Cannot save the account list: {}", e))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn new_profile_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    millis.to_string()
}

fn clean_name(name: &str) -> Result<String, String> {
    let name: String = name.trim().chars().filter(|c| !c.is_control()).collect();
    if name.is_empty() {
        return Err("Enter a name for this account.".into());
    }
    if name.chars().count() > 60 {
        return Err("Use a name of 60 characters or fewer.".into());
    }
    Ok(name)
}

// ---- Who is signed in ----

struct Identity {
    id: String,
    label: String,
}

// Reads only the public claims of Riot's ID token. The refresh token is only
// checked for presence, so a signed-out file never counts as an account.
fn parse_riot_identity(yaml: &str) -> Option<Identity> {
    let signed_in = yaml.lines().any(|l| {
        l.trim()
            .strip_prefix("refresh_token:")
            .is_some_and(|v| !v.trim().trim_matches('"').is_empty())
    });
    if !signed_in {
        return None;
    }
    let token = yaml.lines().find_map(|l| {
        l.trim()
            .strip_prefix("id_token:")
            .map(|v| v.trim().trim_matches('"').to_string())
    })?;
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let id = claims.get("sub")?.as_str()?.to_string();
    let acct = claims.get("acct");
    let game = acct.and_then(|a| a.get("game_name")).and_then(|v| v.as_str());
    let tag = acct.and_then(|a| a.get("tag_line")).and_then(|v| v.as_str());
    let label = match (game, tag) {
        (Some(g), Some(t)) => format!("{}#{}", g, t),
        (Some(g), None) => g.to_string(),
        _ => "Riot account".to_string(),
    };
    Some(Identity { id, label })
}

const DISCORD_DB: &str = r"discord\Local Storage\leveldb";
const DISCORD_KEY_PREFIX: &[u8] = b"_https://discord.com\x00\x01";
const DISCORD_NO_LOGIN: &str = "Discord isn't signed in, or hasn't finished saving this login. Stay signed in for about a minute, then press Save current. Nothing was changed.";

#[derive(Debug, PartialEq)]
enum DiscordLogin {
    SignedIn(String),
    SignedOut,
    // The database could not be read, so nothing is known either way.
    Unknown,
}

// Reads Discord's local storage. Opening a database can change its files, so this
// always works on a temporary copy, which holds a login and is removed right after.
// Only the account id is read; the token is only checked for presence.
fn read_discord_login(leveldb: &Path, temp_parent: &Path) -> DiscordLogin {
    if !leveldb.is_dir() {
        return DiscordLogin::Unknown;
    }
    let copy = temp_parent.join(format!("_read_{}", new_profile_id()));
    let login = copy_path(leveldb, &copy)
        .ok()
        .and_then(|_| {
            let mut options = rusty_leveldb::Options::default();
            options.create_if_missing = false;
            rusty_leveldb::DB::open(&copy, options).ok()
        })
        .map(|mut db| {
            let mut get = |name: &str| {
                let mut key = DISCORD_KEY_PREFIX.to_vec();
                key.extend_from_slice(name.as_bytes());
                db.get(&key)
            };
            let has_token = get("token").is_some_and(|v| !v.is_empty());
            let user_id: String = get("user_id_cache")
                .unwrap_or_default()
                .iter()
                .filter(|b| b.is_ascii_digit())
                .map(|b| *b as char)
                .collect();
            if has_token && user_id.len() >= 15 {
                DiscordLogin::SignedIn(user_id)
            } else {
                DiscordLogin::SignedOut
            }
        })
        .unwrap_or(DiscordLogin::Unknown);
    let _ = remove_path(&copy);
    login
}

fn live_discord_login(roots: &Roots) -> DiscordLogin {
    match store_dir("discord") {
        Ok(store) if fs::create_dir_all(&store).is_ok() => {
            read_discord_login(&roots.path(Root::Roaming, DISCORD_DB), &store)
        }
        _ => DiscordLogin::Unknown,
    }
}

fn saved_discord_login(profile: &str) -> DiscordLogin {
    match (profile_dir("discord", profile), store_dir("discord")) {
        (Ok(dir), Ok(store)) => read_discord_login(&item_in_snapshot(&dir, Root::Roaming, DISCORD_DB), &store),
        _ => DiscordLogin::Unknown,
    }
}

// Accounts saved by older builds have no id yet. Fills in what can be read, and
// returns the copies that hold no login, which can never be switched to.
fn check_saved_discord_logins(state: &mut PlatformState) -> Vec<String> {
    let mut no_login = Vec::new();
    for profile in state.profiles.iter_mut().filter(|p| p.identity.is_none()) {
        match saved_discord_login(&profile.id) {
            DiscordLogin::SignedIn(id) => profile.identity = Some(id),
            DiscordLogin::SignedOut => no_login.push(profile.id.clone()),
            DiscordLogin::Unknown => {}
        }
    }
    no_login
}

// Epic keeps its remembered login in [RememberMe] Data. Only its presence is checked.
fn ini_has_epic_login(bytes: &[u8]) -> bool {
    // Epic may write this file as UTF-16; dropping the zero bytes reads both.
    let text = String::from_utf8_lossy(bytes).replace('\0', "");
    let mut in_section = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_section = line.eq_ignore_ascii_case("[RememberMe]");
        } else if in_section {
            if let Some(value) = line.strip_prefix("Data=") {
                return !value.trim().is_empty();
            }
        }
    }
    false
}

// The newer Epic launcher writes who is signed in as plain JSON. Its login also
// needs the Credential Manager entry, so both must be there.
fn parse_epic_new_identity(json: &str) -> Option<Identity> {
    let json: serde_json::Value = serde_json::from_str(json.trim_start_matches('\u{feff}')).ok()?;
    let info = json.pointer("/prod/account_info")?;
    let text = |key: &str| {
        info.get(key)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(String::from)
    };
    let label = text("display_name")?;
    let id = text("id")
        .or_else(|| text("account_id"))
        .unwrap_or_else(|| format!("name:{}", label));
    Some(Identity { label, id })
}

fn epic_new_identity(s: &Spec, roots: &Roots) -> Option<Identity> {
    let file = roots.path(Root::Local, EPIC_NEW_AUTH).join("auth.json");
    let identity = parse_epic_new_identity(&fs::read_to_string(file).ok()?)?;
    read_cred(s.cred?).map(|_| identity)
}

fn current_identity(s: &Spec, roots: &Roots) -> Option<Identity> {
    match s.id {
        "riot" => s.items.iter().find_map(|(root, rel)| {
            if !rel.ends_with("PrivateSettings.yaml") {
                return None;
            }
            let text = fs::read_to_string(roots.path(*root, rel)).ok()?;
            parse_riot_identity(&text)
        }),
        "epic" => {
            if let Some(identity) = epic_new_identity(s, roots) {
                return Some(identity);
            }
            let (key, value) = s.reg?;
            let id: String = RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey(key)
                .ok()?
                .get_value(value)
                .ok()?;
            // The account id can outlive a sign-out, so also require a saved login.
            let signed_in = s.items.iter().any(|(root, rel)| {
                fs::read(roots.path(*root, rel)).is_ok_and(|bytes| ini_has_epic_login(&bytes))
            });
            if id.trim().is_empty() || !signed_in {
                return None;
            }
            Some(Identity {
                label: "Epic account".into(),
                id,
            })
        }
        "discord" => match live_discord_login(roots) {
            DiscordLogin::SignedIn(id) => Some(Identity { label: String::new(), id }),
            _ => None,
        },
        _ => None,
    }
}

// Launchers that expose an account id let us verify who is signed in.
fn has_identity(s: &Spec) -> bool {
    matches!(s.id, "riot" | "epic" | "discord")
}

// ---- Finding and closing launchers ----

fn newest_discord_exe() -> Option<PathBuf> {
    let root = dirs::data_local_dir()?.join("Discord");
    let mut apps: Vec<PathBuf> = fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().starts_with("app-"))
                .unwrap_or(false)
                && p.join("Discord.exe").is_file()
        })
        .collect();
    apps.sort_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok());
    apps.pop().map(|p| p.join("Discord.exe"))
}

fn riot_exe() -> Option<PathBuf> {
    let data = std::env::var_os("ProgramData")?;
    let file = Path::new(&data).join(r"Riot Games\RiotClientInstalls.json");
    let json: serde_json::Value = serde_json::from_str(&fs::read_to_string(file).ok()?).ok()?;
    ["rc_default", "rc_live"].iter().find_map(|k| {
        let p = PathBuf::from(json.get(*k)?.as_str()?.replace('/', "\\"));
        p.is_file().then_some(p)
    })
}

fn program_files_x86() -> PathBuf {
    std::env::var_os("ProgramFiles(x86)")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)"))
}

// Epic can be installed anywhere, so ask Windows what opens its links first, then
// try the default folder, then a launcher that is running right now.
fn epic_exe_from_command(command: &str) -> Option<PathBuf> {
    let path = PathBuf::from(command.split('"').nth(1)?);
    let is_launcher = path
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("EpicGamesLauncher.exe"));
    (is_launcher && path.is_file()).then_some(path)
}

fn epic_exe() -> Option<PathBuf> {
    let existing = |p: PathBuf| p.is_file().then_some(p);
    let base = program_files_x86().join(r"Epic Games\Launcher\Portal\Binaries");
    let new_launcher = dirs::data_local_dir().map(|d| d.join(r"Epic Games\Launcher\EpicGamesLauncher.exe"));
    RegKey::predef(HKEY_CLASSES_ROOT)
        .open_subkey(r"com.epicgames.launcher\shell\open\command")
        .ok()
        .and_then(|k| k.get_value::<String, _>("").ok())
        .and_then(|command| epic_exe_from_command(&command))
        .or_else(|| existing(base.join(r"Win64\EpicGamesLauncher.exe")))
        .or_else(|| existing(base.join(r"Win32\EpicGamesLauncher.exe")))
        .or_else(|| new_launcher.and_then(existing))
        .or_else(|| {
            snapshot_processes().processes().values().find_map(|p| {
                if !p.name().eq_ignore_ascii_case("EpicGamesLauncher.exe") {
                    return None;
                }
                existing(p.exe()?.to_path_buf())
            })
        })
}

fn launcher_exe(id: &str) -> Option<PathBuf> {
    let existing = |p: PathBuf| p.is_file().then_some(p);
    match id {
        "riot" => riot_exe(),
        "discord" => newest_discord_exe(),
        "epic" => epic_exe(),
        "ubisoft" => {
            let from_reg: Option<String> = RegKey::predef(HKEY_LOCAL_MACHINE)
                .open_subkey(r"SOFTWARE\WOW6432Node\Ubisoft\Launcher")
                .ok()
                .and_then(|k| k.get_value("InstallDir").ok());
            from_reg
                .and_then(|d| existing(Path::new(&d).join("upc.exe")))
                .or_else(|| {
                    existing(program_files_x86().join(r"Ubisoft\Ubisoft Game Launcher\upc.exe"))
                })
        }
        _ => None,
    }
}

// Goes through Explorer so a launcher never inherits NEXUS's admin rights.
fn launch_unelevated(exe: &Path) -> Result<(), String> {
    Command::new("explorer.exe")
        .arg(exe)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not start the launcher: {}", e))
}

fn running_of(sys: &System, names: &[&str]) -> Vec<String> {
    let mut found: Vec<String> = sys
        .processes()
        .values()
        .map(|p| p.name().to_string_lossy().to_string())
        .filter(|n| names.iter().any(|x| x.eq_ignore_ascii_case(n)))
        .collect();
    found.sort();
    found.dedup();
    found
}

fn snapshot_processes() -> System {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    sys
}

fn close_processes(label: &str, names: &[&str], wait: Duration) -> Result<(), String> {
    let deadline = Instant::now() + wait;
    loop {
        let sys = snapshot_processes();
        let mut any = false;
        for p in sys.processes().values() {
            let name = p.name().to_string_lossy();
            if names.iter().any(|x| x.eq_ignore_ascii_case(&name)) {
                any = true;
                p.kill();
            }
        }
        if !any {
            // Give Windows a moment to release the file handles.
            thread::sleep(Duration::from_millis(800));
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "Could not close {}. Close it yourself, or restart NEXUS as administrator. Nothing was changed.",
                label
            ));
        }
        thread::sleep(Duration::from_millis(400));
    }
}

// True while another process has this file open. Windows error 32 and 33 are the
// sharing and lock violations; anything else is left for the copy to report.
fn is_file_locked(path: &Path) -> bool {
    let opened = fs::OpenOptions::new().read(true).share_mode(0).open(path);
    matches!(opened.err().and_then(|e| e.raw_os_error()), Some(32) | Some(33))
}

fn first_locked_file(path: &Path) -> Option<PathBuf> {
    let meta = fs::symlink_metadata(path).ok()?;
    if meta.is_dir() {
        return fs::read_dir(path)
            .ok()?
            .flatten()
            .find_map(|entry| first_locked_file(&entry.path()));
    }
    is_file_locked(path).then(|| path.to_path_buf())
}

// A process can be gone while Windows is still releasing its files.
fn wait_until_released(items: &[PathBuf], wait: Duration) -> Result<(), PathBuf> {
    let deadline = Instant::now() + wait;
    loop {
        let Some(locked) = items.iter().find_map(|item| first_locked_file(item)) else {
            return Ok(());
        };
        if Instant::now() >= deadline {
            return Err(locked);
        }
        thread::sleep(Duration::from_millis(200));
    }
}

// Returns whether the launcher was running, so it can be reopened afterwards.
fn prepare_launcher(s: &Spec, roots: &Roots) -> Result<bool, String> {
    let sys = snapshot_processes();
    let games = running_of(&sys, s.games);
    if let Some(game) = games.first() {
        return Err(format!(
            "{} is running. Close your game first. Nothing was changed.",
            game
        ));
    }
    let was_running = !running_of(&sys, s.exes).is_empty();
    // Found before closing: a launcher known only from its running process is gone afterwards.
    let exe = launcher_exe(s.id);
    if let Err(e) = close_processes(s.name, s.exes, Duration::from_secs(12)) {
        reopen_if_was_running(exe.as_deref(), was_running);
        return Err(e);
    }
    let items: Vec<PathBuf> = s.items.iter().map(|(root, rel)| roots.path(*root, rel)).collect();
    if let Err(locked) = wait_until_released(&items, FILE_RELEASE_WAIT) {
        reopen_if_was_running(exe.as_deref(), was_running);
        return Err(format!(
            "Another program is still using {}. Wait a few seconds and try again. Nothing was changed.",
            locked.display()
        ));
    }
    Ok(was_running)
}

fn reopen_if_was_running(exe: Option<&Path>, was_running: bool) {
    if !was_running {
        return;
    }
    if let Some(exe) = exe {
        let _ = launch_unelevated(exe);
    }
}

// ---- Copying login files ----

fn copy_path(src: &Path, dst: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(src).map_err(|e| format!("{}: {}", src.display(), e))?;
    if meta.file_type().is_symlink() {
        return Ok(());
    }
    if meta.is_dir() {
        fs::create_dir_all(dst).map_err(|e| format!("{}: {}", dst.display(), e))?;
        let entries = fs::read_dir(src).map_err(|e| format!("{}: {}", src.display(), e))?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            copy_path(&entry.path(), &dst.join(entry.file_name()))?;
        }
    } else {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {}", parent.display(), e))?;
        }
        fs::copy(src, dst).map_err(|e| format!("{}: {}", src.display(), e))?;
    }
    Ok(())
}

fn remove_path(path: &Path) -> Result<(), String> {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return Ok(());
    };
    let result = if meta.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    result.map_err(|e| format!("{}: {}", path.display(), e))
}

fn item_in_snapshot(dir: &Path, root: Root, rel: &str) -> PathBuf {
    dir.join("files").join(root.folder()).join(rel)
}

// Copy the live login into `dest`. The old copy is only removed once the new one
// is in place, so a failed copy can never damage an existing saved account.
// Finding nothing to copy is an error unless `allow_empty` is set.
fn capture(s: &Spec, roots: &Roots, dest: &Path, allow_empty: bool) -> Result<usize, String> {
    let tmp = dest.with_extension("tmp");
    let old = dest.with_extension("old");
    remove_path(&tmp)?;
    fs::create_dir_all(&tmp).map_err(|e| format!("{}: {}", tmp.display(), e))?;
    let mut copied = 0;
    let result = (|| -> Result<(), String> {
        for (root, rel) in s.items {
            let live = roots.path(*root, rel);
            if live.exists() {
                copy_path(&live, &item_in_snapshot(&tmp, *root, rel))?;
                copied += 1;
            }
        }
        if let Some(value) = read_reg(s) {
            fs::write(tmp.join("registry.txt"), value).map_err(|e| e.to_string())?;
        }
        if let Some(saved) = s.cred.and_then(read_cred) {
            fs::write(tmp.join(CRED_FILE), protect(&saved.to_bytes())?).map_err(|e| e.to_string())?;
        }
        if copied == 0 && !allow_empty {
            return Err(format!("No login was found for {}. Sign in first.", s.name));
        }
        remove_path(&old)?;
        if dest.exists() {
            fs::rename(dest, &old).map_err(|e| format!("{}: {}", dest.display(), e))?;
        }
        if let Err(e) = fs::rename(&tmp, dest) {
            let _ = fs::rename(&old, dest);
            return Err(format!("{}: {}", dest.display(), e));
        }
        let _ = remove_path(&old);
        Ok(())
    })();
    if let Err(e) = result {
        let _ = remove_path(&tmp);
        return Err(e);
    }
    Ok(copied)
}

// Make the live login match a snapshot. An item missing from it is removed, so
// nothing from the previous account stays behind. `None` signs out.
fn restore_files(s: &Spec, roots: &Roots, snapshot: Option<&Path>) -> Result<(), String> {
    for (root, rel) in s.items {
        let live = roots.path(*root, rel);
        remove_path(&live)?;
        if let Some(dir) = snapshot {
            let saved = item_in_snapshot(dir, *root, rel);
            if saved.exists() {
                copy_path(&saved, &live)?;
            }
        }
    }
    for (root, rel) in s.clear {
        // Only a cache: the launcher rebuilds it, so a leftover is not worth failing for.
        let _ = remove_path(&roots.path(*root, rel));
    }
    restore_cred(s, snapshot)
}

// ---- Credential Manager ----

const CRED_FILE: &str = "credential.bin";

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct SavedCred {
    user: String,
    persist: u32,
    blob: String,
}

impl SavedCred {
    fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn read_cred(target: &str) -> Option<SavedCred> {
    let name = wide(target);
    let mut found: *mut CREDENTIALW = std::ptr::null_mut();
    unsafe {
        CredReadW(PCWSTR(name.as_ptr()), CRED_TYPE_GENERIC, 0, &mut found).ok()?;
        let cred = &*found;
        let blob = if cred.CredentialBlob.is_null() {
            &[][..]
        } else {
            std::slice::from_raw_parts(cred.CredentialBlob, cred.CredentialBlobSize as usize)
        };
        let saved = SavedCred {
            user: if cred.UserName.is_null() {
                String::new()
            } else {
                cred.UserName.to_string().unwrap_or_default()
            },
            persist: cred.Persist.0,
            blob: STANDARD.encode(blob),
        };
        CredFree(found as *const std::ffi::c_void);
        Some(saved)
    }
}

fn write_cred(target: &str, saved: &SavedCred) -> Result<(), String> {
    let mut name = wide(target);
    let mut user = wide(&saved.user);
    let mut blob = STANDARD
        .decode(&saved.blob)
        .map_err(|_| "The saved login is damaged.".to_string())?;
    let cred = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(name.as_mut_ptr()),
        CredentialBlobSize: blob.len() as u32,
        CredentialBlob: blob.as_mut_ptr(),
        Persist: CRED_PERSIST(saved.persist),
        UserName: PWSTR(user.as_mut_ptr()),
        ..Default::default()
    };
    unsafe { CredWriteW(&cred, 0) }.map_err(|e| format!("Cannot write the saved login to Windows: {}", e))
}

fn delete_cred(target: &str) {
    let name = wide(target);
    // Already missing is the wanted result.
    let _ = unsafe { CredDeleteW(PCWSTR(name.as_ptr()), CRED_TYPE_GENERIC, 0) };
}

// Saved copies of a credential are encrypted for this Windows user.
fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
    crypt(data, true).ok_or_else(|| "Windows could not protect the saved login.".to_string())
}

fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
    crypt(data, false).ok_or_else(|| "Windows could not read the saved login.".to_string())
}

fn crypt(data: &[u8], encrypt: bool) -> Option<Vec<u8>> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        if encrypt {
            CryptProtectData(&input, PCWSTR::null(), None, None, None, 0, &mut output).ok()?;
        } else {
            CryptUnprotectData(&input, None, None, None, None, 0, &mut output).ok()?;
        }
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(output.pbData as *mut std::ffi::c_void));
        Some(bytes)
    }
}

// Make the live credential match a snapshot; one with no credential removes it.
fn restore_cred(s: &Spec, snapshot: Option<&Path>) -> Result<(), String> {
    let Some(target) = s.cred else {
        return Ok(());
    };
    let Some(bytes) = snapshot.and_then(|dir| fs::read(dir.join(CRED_FILE)).ok()) else {
        delete_cred(target);
        return Ok(());
    };
    let saved: SavedCred = serde_json::from_slice(&unprotect(&bytes)?)
        .map_err(|_| "The saved login is damaged.".to_string())?;
    write_cred(target, &saved)
}

fn read_reg(s: &Spec) -> Option<String> {
    let (key, value) = s.reg?;
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(key)
        .ok()?
        .get_value(value)
        .ok()
}

fn write_reg(s: &Spec, value: &str) -> Result<(), String> {
    let Some((key, name)) = s.reg else {
        return Ok(());
    };
    let (handle, _) = RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey(key)
        .map_err(|e| format!("Cannot open the registry: {}", e))?;
    handle
        .set_value(name, &value.to_string())
        .map_err(|e| format!("Cannot write the registry: {}", e))
}

// Replace the live login, returning to the backup if anything fails midway.
fn apply_login(s: &Spec, roots: &Roots, target: Option<&Path>) -> Result<(), String> {
    let backup = store_dir(s.id)?.join("_last_login");
    capture(s, roots, &backup, true)
        .map_err(|e| format!("Could not back up your current login, so nothing was changed. {}", e))?;
    let reg_value = target
        .map(|dir| fs::read_to_string(dir.join("registry.txt")).unwrap_or_default())
        .unwrap_or_default();
    let result = restore_files(s, roots, target).and_then(|_| write_reg(s, &reg_value));
    if let Err(e) = result {
        let undo = restore_files(s, roots, Some(&backup));
        if let Ok(old) = fs::read_to_string(backup.join("registry.txt")) {
            let _ = write_reg(s, &old);
        }
        return match undo {
            Ok(()) => Err(format!("Could not switch, so your login was put back. {}", e)),
            Err(undo_err) => {
                // Move it aside so a retry cannot overwrite the only good copy.
                let recovery = store_dir(s.id)?.join(format!("_recovery_{}", new_profile_id()));
                let kept = if fs::rename(&backup, &recovery).is_ok() { &recovery } else { &backup };
                Err(format!(
                    "Could not switch or put your login back ({}). A backup is in {}. {}",
                    undo_err,
                    kept.display(),
                    e
                ))
            }
        };
    }
    Ok(())
}

fn profile_dir(platform: &str, id: &str) -> Result<PathBuf, String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return Err("Invalid account.".into());
    }
    Ok(store_dir(platform)?.join(id))
}

// Keep the signed-in account's saved copy fresh before leaving it: Riot rotates its
// session, and Discord keeps its own account list in these files. It is matched by
// the signed-in account id, so a signed-out or different login can never overwrite
// a saved account. Epic's saved copy is left alone because its login is harder to prove.
fn refresh_saved_session(s: &Spec, roots: &Roots, state: &mut PlatformState) {
    if !matches!(s.id, "riot" | "discord") {
        return;
    }
    let Some(identity) = current_identity(s, roots) else {
        return;
    };
    let Some(profile) = state
        .profiles
        .iter_mut()
        .find(|p| p.identity.as_deref() == Some(identity.id.as_str()))
    else {
        return;
    };
    if let Ok(dir) = profile_dir(s.id, &profile.id) {
        // A failed refresh only leaves the older saved session in place.
        if capture(s, roots, &dir, false).is_ok() {
            profile.saved_at = now_secs();
        }
    }
}

// ---- Commands ----

pub fn save_current(platform: &str, name: &str) -> Result<(), String> {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let s = spec(platform)?;
    let name = clean_name(name)?;
    let roots = Roots::real()?;
    // Check before closing anything, so a login that isn't ready costs nothing.
    if s.id == "discord" && live_discord_login(&roots) == DiscordLogin::SignedOut {
        return Err(DISCORD_NO_LOGIN.into());
    }
    let mut state = load_state(s.id)?;
    let exe = launcher_exe(s.id);
    let was_running = prepare_launcher(s, &roots)?;
    let result = (|| -> Result<(), String> {
        let identity = current_identity(s, &roots);
        let unreadable = s.id == "discord" && live_discord_login(&roots) == DiscordLogin::Unknown;
        if has_identity(s) && identity.is_none() && !unreadable {
            return Err(format!(
                "No signed-in account was found. Sign in to {} and tick \"stay signed in\" first.",
                s.name
            ));
        }
        let identity_id = identity.map(|i| i.id);
        // Saving the same account again updates it instead of making a duplicate.
        // Without an account id, the same name is what marks it as the same one.
        let existing = state
            .profiles
            .iter()
            .find(|p| match &identity_id {
                // An old entry with no id can be replaced by saving under its name.
                Some(id) => {
                    p.identity.as_ref() == Some(id)
                        || (p.identity.is_none() && p.name.eq_ignore_ascii_case(&name))
                }
                None => p.name.eq_ignore_ascii_case(&name),
            })
            .map(|p| p.id.clone());
        let holds_known_account = state
            .profiles
            .iter()
            .any(|p| Some(&p.id) == existing.as_ref() && p.identity.is_some());
        if identity_id.is_none() && holds_known_account {
            return Err(format!(
                "{} hasn't finished saving this login, so NEXUS can't tell if it is the account already saved as \"{}\". Wait about a minute and try again, or use a different name. Nothing was changed.",
                s.name, name
            ));
        }
        let id = existing.clone().unwrap_or_else(new_profile_id);
        capture(s, &roots, &profile_dir(s.id, &id)?, false)?;
        if existing.is_some() {
            if let Some(p) = state.profiles.iter_mut().find(|p| p.id == id) {
                p.name = name.clone();
                p.saved_at = now_secs();
                if identity_id.is_some() {
                    p.identity = identity_id.clone();
                }
            }
        } else {
            state.profiles.push(SavedProfile {
                id: id.clone(),
                name: name.clone(),
                saved_at: now_secs(),
                identity: identity_id,
            });
        }
        state.active = Some(id);
        save_state(s.id, &state)
    })();
    reopen_if_was_running(exe.as_deref(), was_running);
    result
}

fn require_launcher(s: &Spec) -> Result<PathBuf, String> {
    launcher_exe(s.id).ok_or_else(|| format!("{} isn't installed. Nothing was changed.", s.name))
}

pub fn switch_to(platform: &str, profile: &str) -> Result<(), String> {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if platform == "steam" {
        return steam_switch(profile);
    }
    if platform == BNET_ID {
        return bnet_change(Some(profile));
    }
    let s = spec(platform)?;
    let roots = Roots::real()?;
    let mut state = load_state(s.id)?;
    if !state.profiles.iter().any(|p| p.id == profile) {
        return Err("That account is no longer saved.".into());
    }
    let dir = profile_dir(s.id, profile)?;
    if !dir.is_dir() {
        return Err("The saved login for this account is missing. Save it again.".into());
    }
    let exe = require_launcher(s)?;
    if s.id == "discord" {
        if saved_discord_login(profile) == DiscordLogin::SignedOut {
            return Err("This saved copy has no Discord login in it, so nothing was changed. Press Add account, sign in, wait about a minute, then press Save current with the same name.".into());
        }
        check_saved_discord_logins(&mut state);
    }
    let was_running = prepare_launcher(s, &roots)?;
    refresh_saved_session(s, &roots, &mut state);
    if let Err(e) = apply_login(s, &roots, Some(&dir)) {
        reopen_if_was_running(Some(&exe), was_running);
        return Err(e);
    }
    state.active = Some(profile.to_string());
    let saved = save_state(s.id, &state);
    launch_unelevated(&exe)?;
    saved.map_err(|e| format!("The account was switched, but the list could not be updated. {}", e))
}

// Sign out so a different account can be signed in. The previous login stays
// saved if it was saved, and a one-step backup is kept otherwise.
pub fn add_new(platform: &str) -> Result<(), String> {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if platform == "steam" {
        return steam_add_new();
    }
    if platform == BNET_ID {
        return bnet_change(None);
    }
    let s = spec(platform)?;
    let roots = Roots::real()?;
    let mut state = load_state(s.id)?;
    let exe = require_launcher(s)?;
    if s.id == "discord" {
        check_saved_discord_logins(&mut state);
    }
    let was_running = prepare_launcher(s, &roots)?;
    refresh_saved_session(s, &roots, &mut state);
    if let Err(e) = apply_login(s, &roots, None) {
        reopen_if_was_running(Some(&exe), was_running);
        return Err(e);
    }
    state.active = None;
    let saved = save_state(s.id, &state);
    launch_unelevated(&exe)?;
    saved.map_err(|e| format!("You were signed out, but the list could not be updated. {}", e))
}

pub fn delete_saved(platform: &str, profile: &str) -> Result<(), String> {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let s = spec(platform)?;
    let mut state = load_state(s.id)?;
    if !state.profiles.iter().any(|p| p.id == profile) {
        return Ok(());
    }
    remove_path(&profile_dir(s.id, profile)?)?;
    state.profiles.retain(|p| p.id != profile);
    if state.active.as_deref() == Some(profile) {
        state.active = None;
    }
    save_state(s.id, &state)
}

// ---- Steam ----

struct SteamAccount {
    id: String,
    account: String,
    persona: String,
    remember: bool,
}

// One `"key" "value"` pair, or a lone `"token"`, honoring backslash escapes.
fn vdf_tokens(line: &str) -> Vec<String> {
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

fn is_steam_id(token: &str) -> bool {
    token.len() >= 15 && token.chars().all(|c| c.is_ascii_digit())
}

fn parse_steam_users(text: &str) -> Vec<SteamAccount> {
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
        } else if inside && tokens.len() == 2 {
            if let Some(u) = users.last_mut() {
                match tokens[0].to_ascii_lowercase().as_str() {
                    "accountname" => u.account = tokens[1].clone(),
                    "personaname" => u.persona = tokens[1].clone(),
                    "rememberpassword" => u.remember = tokens[1] == "1",
                    _ => {}
                }
            }
        }
    }
    users.retain(|u| !u.account.is_empty());
    users
}

// Mark `target` as the account Steam should use, changing only the flags that
// decide this and leaving every other line as it was.
fn set_most_recent(text: &str, target: &str) -> String {
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

fn steam_key() -> Option<RegKey> {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(r"Software\Valve\Steam", KEY_READ | KEY_SET_VALUE)
        .ok()
}

fn steam_paths() -> Option<(PathBuf, PathBuf)> {
    let key = steam_key()?;
    let exe: String = key.get_value("SteamExe").ok()?;
    let path: String = key.get_value("SteamPath").ok()?;
    let exe = PathBuf::from(exe.replace('/', "\\"));
    exe.is_file()
        .then(|| (exe, PathBuf::from(path.replace('/', "\\"))))
}

// Steam records the game it is running, so a match is never killed by a switch.
fn refuse_if_steam_game_running() -> Result<(), String> {
    let app: u32 = steam_key()
        .and_then(|k| k.get_value("RunningAppID").ok())
        .unwrap_or(0);
    if app != 0 {
        return Err("A Steam game is running. Close your game first. Nothing was changed.".into());
    }
    Ok(())
}

fn close_steam(exe: &Path) -> Result<(), String> {
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

fn steam_switch(steam_id: &str) -> Result<(), String> {
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

fn steam_is_running() -> bool {
    !running_of(&snapshot_processes(), &["steam.exe"]).is_empty()
}

// The launcher is closed by now. After a failure it only comes back if it was open before.
fn reopen_after(exe: &Path, was_running: bool, result: Result<(), String>) -> Result<(), String> {
    if let Err(e) = result {
        if was_running {
            let _ = launch_unelevated(exe);
        }
        return Err(e);
    }
    launch_unelevated(exe)
}

// An empty auto-login user makes Steam show its own account picker.
fn steam_add_new() -> Result<(), String> {
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

fn steam_platform(sys: &System) -> Option<AccountPlatform> {
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

// ---- Battle.net ----
//
// Battle.net keeps every account that ticked "Keep me logged in" and signs in to
// the first name in its settings file, so switching only changes that order.
// NEXUS also remembers the names itself, because an empty list (used to reach the
// sign-in screen) would otherwise hide them.

const BNET_ID: &str = "battlenet";
const BNET_EXES: &[&str] = &["Battle.net.exe"];
const BNET_GAMES: &[&str] = &[
    "Overwatch.exe",
    "Wow.exe",
    "WowClassic.exe",
    "Diablo IV.exe",
    "D2R.exe",
    "Hearthstone.exe",
    "SC2_x64.exe",
    "cod.exe",
];
const BNET_NAMES: &str = "/Client/SavedAccountNames";

fn bnet_exe() -> Option<PathBuf> {
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

fn bnet_config_file() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join(r"Battle.net\Battle.net.config"))
}

fn bnet_read_config(file: &Path) -> Result<serde_json::Value, String> {
    let text = fs::read_to_string(file).map_err(|e| format!("Cannot read Battle.net's settings: {}", e))?;
    serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|_| "Battle.net's settings could not be understood. Nothing was changed.".to_string())
}

fn bnet_names(config: &serde_json::Value) -> Vec<String> {
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
fn bnet_reordered(names: &[String], first: Option<&str>) -> String {
    let Some(first) = first else {
        return String::new();
    };
    let mut ordered = vec![first.to_string()];
    ordered.extend(names.iter().filter(|n| !n.eq_ignore_ascii_case(first)).cloned());
    ordered.join(",")
}

fn bnet_remember(state: &mut PlatformState, names: &[String]) {
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

fn bnet_write_names(file: &Path, state: &mut PlatformState, first: Option<&str>) -> Result<(), String> {
    // Read after closing: Battle.net rewrites this file as it runs.
    let mut config = bnet_read_config(file)?;
    let names = bnet_names(&config);
    bnet_remember(state, &names);
    save_state(BNET_ID, state)?;
    if let Some(first) = first {
        if !state.profiles.iter().any(|p| p.name.eq_ignore_ascii_case(first)) {
            return Err("Battle.net no longer remembers this account.".into());
        }
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

fn bnet_change(first: Option<&str>) -> Result<(), String> {
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

fn bnet_platform(sys: &System) -> Option<AccountPlatform> {
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

// ---- Browsers ----

fn app_path(exe: &str) -> Option<PathBuf> {
    let sub = format!(r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{}", exe);
    [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER].iter().find_map(|hive| {
        let value: String = RegKey::predef(*hive).open_subkey(&sub).ok()?.get_value("").ok()?;
        let p = PathBuf::from(value.trim_matches('"'));
        p.is_file().then_some(p)
    })
}

const CHROMIUM: &[(&str, &str, &str, &str)] = &[
    ("chrome", "Google Chrome", "chrome.exe", r"Google\Chrome\User Data"),
    ("edge", "Microsoft Edge", "msedge.exe", r"Microsoft\Edge\User Data"),
    ("brave", "Brave", "brave.exe", r"BraveSoftware\Brave-Browser\User Data"),
];

fn chromium_profiles(user_data: &str) -> Option<Vec<BrowserProfile>> {
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
    list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Some(list)
}

fn firefox_profiles() -> Vec<BrowserProfile> {
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

fn list_browsers() -> Vec<BrowserInfo> {
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
fn safe_profile_arg(value: &str) -> bool {
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
fn open_unelevated_with_args(exe: &Path, args: &[String]) -> Result<(), String> {
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

// ---- Overview ----

pub fn overview() -> AccountsOverview {
    let sys = snapshot_processes();
    let mut platforms = Vec::new();
    if let Some(steam) = steam_platform(&sys) {
        platforms.push(steam);
    }
    if let Some(bnet) = bnet_platform(&sys) {
        platforms.push(bnet);
    }
    if let Ok(roots) = Roots::real() {
        for s in SPECS {
            let installed = launcher_exe(s.id).is_some();
            // An unreadable list is shown as an error and never written over.
            let (mut state, error) = match load_state(s.id) {
                Ok(state) => (state, None),
                Err(e) => (PlatformState::default(), Some(e)),
            };
            if !installed && state.profiles.is_empty() && error.is_none() {
                continue;
            }
            let mut no_login = Vec::new();
            if s.id == "discord" && error.is_none() {
                // Skip while a switch is running; the next refresh catches up.
                if let Ok(_guard) = LOCK.try_lock() {
                    let without_id = |state: &PlatformState| {
                        state.profiles.iter().filter(|p| p.identity.is_none()).count()
                    };
                    let before = without_id(&state);
                    no_login = check_saved_discord_logins(&mut state);
                    if without_id(&state) != before {
                        let _ = save_state(s.id, &state);
                    }
                }
            }
            let identity = current_identity(s, &roots);
            let active_id = match &identity {
                Some(i) => state
                    .profiles
                    .iter()
                    .find(|p| p.identity.as_deref() == Some(i.id.as_str()))
                    .map(|p| p.id.clone()),
                None if has_identity(s) => None,
                None => state.active.clone(),
            };
            let accounts = state
                .profiles
                .iter()
                .map(|p| {
                    let has_login = !no_login.contains(&p.id);
                    AccountEntry {
                        id: p.id.clone(),
                        name: p.name.clone(),
                        detail: if has_login {
                            String::new()
                        } else {
                            "No login saved. Sign in, then save it again with this name.".into()
                        },
                        active: active_id.as_deref() == Some(p.id.as_str()),
                        avatar: None,
                        remembered: has_login,
                    }
                })
                .collect();
            platforms.push(AccountPlatform {
                id: s.id.into(),
                name: s.name.into(),
                running: !running_of(&sys, s.exes).is_empty(),
                managed: true,
                verified: has_identity(s),
                installed,
                current: identity.map(|i| i.label).filter(|label| !label.is_empty()),
                accounts,
                error,
            });
        }
    }
    AccountsOverview {
        platforms,
        browsers: list_browsers(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VDF: &str = "\"users\"\n{\n\t\"111111111111111\"\n\t{\n\t\t\"AccountName\"\t\t\"alpha\"\n\t\t\"PersonaName\"\t\t\"Al \\\"Pro\\\"\"\n\t\t\"RememberPassword\"\t\t\"1\"\n\t\t\"MostRecent\"\t\t\"1\"\n\t\t\"AutoLogin\"\t\t\"1\"\n\t\t\"timestamp\"\t\t\"100\"\n\t}\n\t\"222222222222222\"\n\t{\n\t\t\"AccountName\"\t\t\"beta\"\n\t\t\"PersonaName\"\t\t\"Beta\"\n\t\t\"RememberPassword\"\t\t\"1\"\n\t\t\"timestamp\"\t\t\"200\"\n\t}\n}\n";

    fn temp_case(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nexus-accounts-test-{}", name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parses_steam_users_with_escaped_names() {
        let users = parse_steam_users(VDF);
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].account, "alpha");
        assert_eq!(users[0].persona, "Al \"Pro\"");
        assert!(users[1].remember);
    }

    #[test]
    fn set_most_recent_moves_the_flag_and_keeps_other_lines() {
        let out = set_most_recent(VDF, "222222222222222");
        let parts: Vec<&str> = out.split("\"222222222222222\"").collect();
        assert!(parts[0].contains("\"MostRecent\"\t\t\"0\""));
        assert!(parts[0].contains("\"AutoLogin\"\t\t\"0\""));
        assert!(parts[1].contains("\"MostRecent\"\t\t\"1\""));
        assert!(out.contains("\"PersonaName\"\t\t\"Al \\\"Pro\\\"\""));
        assert!(out.contains("\"timestamp\"\t\t\"200\""));
        assert!(out.ends_with('\n'));
        assert_eq!(parse_steam_users(&out).len(), 2);
    }

    #[test]
    fn riot_identity_reads_public_claims_only() {
        let claims = URL_SAFE_NO_PAD
            .encode(r#"{"sub":"abc","acct":{"game_name":"Player","tag_line":"EUW"}}"#);
        let yaml = format!("a:\n    id_token: \"h.{}.s\"\n    refresh_token: \"x\"\n", claims);
        let id = parse_riot_identity(&yaml).unwrap();
        assert_eq!(id.id, "abc");
        assert_eq!(id.label, "Player#EUW");
        assert!(parse_riot_identity("refresh_token: \"x\"\n").is_none());
        // An ID token left behind without a refresh token means signed out.
        let out = format!("a:\n    id_token: \"h.{}.s\"\n    refresh_token: \"\"\n", claims);
        assert!(parse_riot_identity(&out).is_none());
    }

    #[test]
    fn capture_and_restore_swap_logins_and_leave_nothing_behind() {
        let base = temp_case("swap");
        let roots = Roots {
            local: base.join("local"),
            roaming: base.join("roaming"),
        };
        let s = spec("riot").unwrap();
        let yaml = roots.path(Root::Local, s.items[2].1);
        let session = roots.path(Root::Local, s.items[0].1).join("a.dat");
        fs::create_dir_all(session.parent().unwrap()).unwrap();
        fs::create_dir_all(yaml.parent().unwrap()).unwrap();

        fs::write(&yaml, "account-a").unwrap();
        fs::write(&session, "session-a").unwrap();
        let saved_a = base.join("saved-a");
        assert_eq!(capture(s, &roots, &saved_a, false).unwrap(), 2);

        // Account B has no Sessions folder.
        fs::remove_dir_all(session.parent().unwrap()).unwrap();
        fs::write(&yaml, "account-b").unwrap();
        let saved_b = base.join("saved-b");
        assert_eq!(capture(s, &roots, &saved_b, false).unwrap(), 1);

        // A signed-out capture must fail and leave the saved account untouched.
        fs::remove_file(&yaml).unwrap();
        assert!(capture(s, &roots, &saved_b, false).is_err());
        assert!(saved_b.join("files").is_dir());
        assert!(!base.join("saved-b.tmp").exists());
        fs::write(&yaml, "account-b").unwrap();

        restore_files(s, &roots, Some(&saved_a)).unwrap();
        assert_eq!(fs::read_to_string(&yaml).unwrap(), "account-a");
        assert_eq!(fs::read_to_string(&session).unwrap(), "session-a");

        // Switching to B must not leave A's session behind.
        restore_files(s, &roots, Some(&saved_b)).unwrap();
        assert_eq!(fs::read_to_string(&yaml).unwrap(), "account-b");
        assert!(!session.exists());

        // Signing out removes the login, and the saved copies survive.
        restore_files(s, &roots, None).unwrap();
        assert!(!yaml.exists());
        assert!(saved_a.join("files").is_dir());
        let _ = fs::remove_dir_all(&base);
    }

    fn fake_discord_db(path: &Path, entries: &[(&str, &[u8])]) {
        let mut opts = rusty_leveldb::Options::default();
        opts.create_if_missing = true;
        let mut db = rusty_leveldb::DB::open(path, opts).unwrap();
        for (key, value) in entries {
            let mut full = DISCORD_KEY_PREFIX.to_vec();
            full.extend_from_slice(key.as_bytes());
            db.put(&full, value).unwrap();
        }
        db.flush().unwrap();
    }

    #[test]
    fn reads_who_is_signed_in_to_discord() {
        let base = temp_case("discord-login");

        let signed_in = base.join("signed-in");
        fake_discord_db(&signed_in, &[("token", b"secret"), ("user_id_cache", b"\x01\"123456789012345678\"")]);
        assert_eq!(read_discord_login(&signed_in, &base), DiscordLogin::SignedIn("123456789012345678".into()));

        // The real broken copy: Discord's account list is there, but no login.
        let signed_out = base.join("signed-out");
        fake_discord_db(&signed_out, &[("MultiAccountStore", b"{}")]);
        assert_eq!(read_discord_login(&signed_out, &base), DiscordLogin::SignedOut);

        let id_without_token = base.join("no-token");
        fake_discord_db(&id_without_token, &[("user_id_cache", b"\x01\"123456789012345678\"")]);
        assert_eq!(read_discord_login(&id_without_token, &base), DiscordLogin::SignedOut);

        assert_eq!(read_discord_login(&base.join("missing"), &base), DiscordLogin::Unknown);

        // Reading must leave no copy of the login behind.
        let leftovers = fs::read_dir(&base).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with("_read_")).count();
        assert_eq!(leftovers, 0);
        let _ = fs::remove_dir_all(&base);
    }

    // The real failure: a closed launcher's file stayed locked for a moment.
    #[test]
    fn waits_for_a_locked_file_and_gives_up_if_it_stays_locked() {
        let base = temp_case("locked");
        let nested = base.join("Network");
        fs::create_dir_all(&nested).unwrap();
        let file = nested.join("Cookies");
        fs::write(&file, "x").unwrap();
        let items = vec![base.clone()];

        assert!(wait_until_released(&items, Duration::ZERO).is_ok());

        let held = fs::OpenOptions::new().read(true).share_mode(0).open(&file).unwrap();
        assert_eq!(wait_until_released(&items, Duration::from_millis(300)), Err(file.clone()));

        // Released partway through the wait: the caller carries on normally.
        let release = thread::spawn(move || {
            thread::sleep(Duration::from_millis(500));
            drop(held);
        });
        assert!(wait_until_released(&items, Duration::from_secs(5)).is_ok());
        release.join().unwrap();
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn epic_login_needs_remember_me_data() {
        assert!(ini_has_epic_login(b"[RememberMe]\r\nEnable=True\r\nData=abc\r\n"));
        assert!(!ini_has_epic_login(b"[RememberMe]\nEnable=False\nData=\n"));
        assert!(!ini_has_epic_login(b"[Other]\nData=abc\n"));
        assert!(!ini_has_epic_login(b""));
        let utf16: Vec<u8> = "[RememberMe]\nData=abc\n".bytes().flat_map(|b| [b, 0]).collect();
        assert!(ini_has_epic_login(&utf16));
    }

    #[test]
    fn epic_path_is_only_taken_from_a_real_launcher_command() {
        let base = temp_case("epic-command");
        let exe = base.join("EpicGamesLauncher.exe");
        fs::write(&exe, "x").unwrap();
        let other = base.join("other.exe");
        fs::write(&other, "x").unwrap();
        let quoted = |p: &Path| format!("\"{}\" \"%1\"", p.display());
        assert_eq!(epic_exe_from_command(&quoted(&exe)), Some(exe.clone()));
        assert!(epic_exe_from_command(&quoted(&other)).is_none());
        assert!(epic_exe_from_command(&quoted(&base.join("gone").join("EpicGamesLauncher.exe"))).is_none());
        assert!(epic_exe_from_command("EpicGamesLauncher.exe").is_none());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn battle_net_switch_only_reorders_the_remembered_names() {
        let config: serde_json::Value = serde_json::from_str(
            r#"{"Client":{"SavedAccountNames":"a@x.com, b@x.com,,c@x.com","Other":1}}"#,
        )
        .unwrap();
        let names = bnet_names(&config);
        assert_eq!(names, ["a@x.com", "b@x.com", "c@x.com"]);
        assert_eq!(bnet_reordered(&names, Some("C@x.com")), "C@x.com,a@x.com,b@x.com");
        assert_eq!(bnet_reordered(&names, Some("new@x.com")), "new@x.com,a@x.com,b@x.com,c@x.com");
        assert_eq!(bnet_reordered(&names, None), "");
        assert!(bnet_names(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn new_epic_launcher_identity_needs_a_display_name() {
        let signed_in = r#"{"prod":{"account_info":{"display_name":"Player One","id":"abc123"}}}"#;
        let identity = parse_epic_new_identity(signed_in).unwrap();
        assert_eq!((identity.label.as_str(), identity.id.as_str()), ("Player One", "abc123"));
        let no_id = parse_epic_new_identity(r#"{"prod":{"account_info":{"display_name":"P"}}}"#).unwrap();
        assert_eq!(no_id.id, "name:P");
        assert!(parse_epic_new_identity(r#"{"prod":{"account_info":{"display_name":" "}}}"#).is_none());
        assert!(parse_epic_new_identity("{}").is_none());
    }

    #[test]
    fn a_saved_credential_comes_back_exactly_and_is_removed_when_missing() {
        let target = "NEXUS-test-credential";
        let saved = SavedCred {
            user: "tester".into(),
            persist: 2,
            blob: STANDARD.encode(b"not a real secret"),
        };
        let sealed = protect(&saved.to_bytes()).unwrap();
        assert_ne!(sealed, saved.to_bytes());
        let opened: SavedCred = serde_json::from_slice(&unprotect(&sealed).unwrap()).unwrap();
        assert_eq!(opened, saved);
        write_cred(target, &opened).unwrap();
        assert_eq!(read_cred(target), Some(saved));
        delete_cred(target);
        assert_eq!(read_cred(target), None);
    }

    #[test]
    fn rejects_unsafe_values() {
        assert!(profile_dir("riot", "..\\..\\x").is_err());
        assert!(profile_dir("riot", "").is_err());
        assert!(safe_profile_arg("Profile 1"));
        assert!(!safe_profile_arg("a\" --evil"));
        assert!(!safe_profile_arg("a%b"));
        assert!(clean_name("   ").is_err());
    }

    // Read-only check against this PC. Prints counts, never account details.
    #[test]
    fn overview_runs_on_this_pc() {
        let o = overview();
        for p in &o.platforms {
            println!(
                "{}: running={} managed={} accounts={} current_known={}",
                p.id,
                p.running,
                p.managed,
                p.accounts.len(),
                p.current.is_some()
            );
        }
        for b in &o.browsers {
            println!("{}: profiles={}", b.id, b.profiles.len());
        }
    }
}
