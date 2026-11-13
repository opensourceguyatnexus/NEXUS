// New PC Setup: a large, Ninite-style catalog of common Windows apps grouped by
// category, exported as a .bat installer that runs through winget. Each app carries
// a domain so the UI can show a real logo.

use crate::detect::{installed_display_names, start_app_names};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EssentialItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub group: String,
    pub winget_id: String,
    pub domain: String,
    pub installed: bool,
}

struct Spec {
    id: &'static str,
    name: &'static str,
    group: &'static str,
    winget_id: &'static str,
    domain: &'static str,
    matches: &'static [&'static str],
}

fn sp(
    id: &'static str,
    name: &'static str,
    group: &'static str,
    winget_id: &'static str,
    domain: &'static str,
    matches: &'static [&'static str],
) -> Spec {
    Spec { id, name, group, winget_id, domain, matches }
}

fn specs() -> Vec<Spec> {
    vec![
        // Web Browsers
        sp("chrome", "Google Chrome", "Web Browsers", "Google.Chrome", "google.com", &["google chrome"]),
        sp("firefox", "Mozilla Firefox", "Web Browsers", "Mozilla.Firefox", "mozilla.org", &["mozilla firefox"]),
        sp("brave", "Brave", "Web Browsers", "Brave.Brave", "brave.com", &["brave"]),
        sp("opera", "Opera", "Web Browsers", "Opera.Opera", "opera.com", &["opera "]),
        sp("vivaldi", "Vivaldi", "Web Browsers", "Vivaldi.Vivaldi", "vivaldi.com", &["vivaldi"]),
        sp("edge", "Microsoft Edge", "Web Browsers", "Microsoft.Edge", "microsoft.com", &["microsoft edge"]),

        // Messaging
        sp("discord", "Discord", "Messaging", "Discord.Discord", "discord.com", &["discord"]),
        sp("zoom", "Zoom", "Messaging", "Zoom.Zoom", "zoom.us", &["zoom"]),
        sp("telegram", "Telegram", "Messaging", "Telegram.TelegramDesktop", "telegram.org", &["telegram"]),
        sp("slack", "Slack", "Messaging", "SlackTechnologies.Slack", "slack.com", &["slack"]),
        sp("teams", "Microsoft Teams", "Messaging", "Microsoft.Teams", "microsoft.com", &["microsoft teams"]),
        sp("whatsapp", "WhatsApp", "Messaging", "WhatsApp.WhatsApp", "whatsapp.com", &["whatsapp"]),
        sp("thunderbird", "Thunderbird", "Messaging", "Mozilla.Thunderbird", "thunderbird.net", &["thunderbird"]),

        // Media
        sp("vlc", "VLC Media Player", "Media", "VideoLAN.VLC", "videolan.org", &["vlc media player"]),
        sp("spotify", "Spotify", "Media", "Spotify.Spotify", "spotify.com", &["spotify"]),
        sp("itunes", "iTunes", "Media", "Apple.iTunes", "apple.com", &["itunes"]),
        sp("foobar2000", "foobar2000", "Media", "PeterPawlowski.foobar2000", "foobar2000.org", &["foobar2000"]),
        sp("aimp", "AIMP", "Media", "AIMP.AIMP", "aimp.ru", &["aimp"]),
        sp("musicbee", "MusicBee", "Media", "MusicBee.MusicBee", "getmusicbee.com", &["musicbee"]),
        sp("audacity", "Audacity", "Media", "Audacity.Audacity", "audacityteam.org", &["audacity"]),
        sp("handbrake", "HandBrake", "Media", "HandBrake.HandBrake", "handbrake.fr", &["handbrake"]),
        sp("obs", "OBS Studio", "Media", "OBSProject.OBSStudio", "obsproject.com", &["obs studio"]),
        sp("klite", "K-Lite Codec Pack", "Media", "CodecGuide.K-LiteCodecPack.Standard", "codecguide.com", &["k-lite codec"]),

        // Developer Tools
        sp("vscode", "Visual Studio Code", "Developer Tools", "Microsoft.VisualStudioCode", "visualstudio.com", &["microsoft visual studio code"]),
        sp("git", "Git", "Developer Tools", "Git.Git", "git-scm.com", &["git version"]),
        sp("python", "Python 3", "Developer Tools", "Python.Python.3.12", "python.org", &["python 3"]),
        sp("nodejs", "Node.js (LTS)", "Developer Tools", "OpenJS.NodeJS.LTS", "nodejs.org", &["node.js"]),
        sp("github-desktop", "GitHub Desktop", "Developer Tools", "GitHub.GitHubDesktop", "github.com", &["github desktop"]),
        sp("docker", "Docker Desktop", "Developer Tools", "Docker.DockerDesktop", "docker.com", &["docker desktop"]),
        sp("notepadpp", "Notepad++", "Developer Tools", "Notepad++.Notepad++", "notepad-plus-plus.org", &["notepad++"]),
        sp("sublime", "Sublime Text 4", "Developer Tools", "SublimeHQ.SublimeText.4", "sublimetext.com", &["sublime text"]),
        sp("filezilla", "FileZilla", "Developer Tools", "TimKosse.FileZilla.Client", "filezilla-project.org", &["filezilla"]),
        sp("winscp", "WinSCP", "Developer Tools", "WinSCP.WinSCP", "winscp.net", &["winscp"]),
        sp("putty", "PuTTY", "Developer Tools", "PuTTY.PuTTY", "putty.org", &["putty"]),
        sp("winmerge", "WinMerge", "Developer Tools", "WinMerge.WinMerge", "winmerge.org", &["winmerge"]),

        // Utilities
        sp("powertoys", "Microsoft PowerToys", "Utilities", "Microsoft.PowerToys", "microsoft.com", &["powertoys"]),
        sp("7zip", "7-Zip", "Utilities", "7zip.7zip", "7-zip.org", &["7-zip"]),
        sp("everything", "Everything", "Utilities", "voidtools.Everything", "voidtools.com", &["everything"]),
        sp("wireshark", "Wireshark", "Utilities", "WiresharkFoundation.Wireshark", "wireshark.org", &["wireshark"]),
        sp("anydesk", "AnyDesk", "Utilities", "AnyDeskSoftwareGmbH.AnyDesk", "anydesk.com", &["anydesk"]),
        sp("teamviewer", "TeamViewer", "Utilities", "TeamViewer.TeamViewer", "teamviewer.com", &["teamviewer"]),
        sp("rustdesk", "RustDesk", "Utilities", "RustDesk.RustDesk", "rustdesk.com", &["rustdesk"]),
        sp("tailscale", "Tailscale", "Utilities", "tailscale.tailscale", "tailscale.com", &["tailscale"]),
        sp("wireguard", "WireGuard", "Utilities", "WireGuard.WireGuard", "wireguard.com", &["wireguard"]),
        sp("teracopy", "TeraCopy", "Utilities", "CodeSector.TeraCopy", "codesector.com", &["teracopy"]),
        sp("revo", "Revo Uninstaller", "Utilities", "RevoUninstaller.RevoUninstaller", "revouninstaller.com", &["revo uninstaller"]),
        sp("windirstat", "WinDirStat", "Utilities", "WinDirStat.WinDirStat", "windirstat.net", &["windirstat"]),
        sp("wiztree", "WizTree", "Utilities", "AntibodySoftware.WizTree", "diskanalyzer.com", &["wiztree"]),
        sp("ipscanner", "Advanced IP Scanner", "Utilities", "Famatech.AdvancedIPScanner", "advanced-ip-scanner.com", &["advanced ip scanner"]),
        sp("ccleaner", "CCleaner", "Utilities", "Piriform.CCleaner", "ccleaner.com", &["ccleaner"]),
        sp("openshell", "Open-Shell", "Utilities", "Open-Shell.Open-Shell-Menu", "github.com", &["open-shell"]),
        sp("glary", "Glary Utilities", "Utilities", "Glarysoft.GlaryUtilities", "glarysoft.com", &["glary utilities"]),

        // Security
        sp("malwarebytes", "Malwarebytes", "Security", "Malwarebytes.Malwarebytes", "malwarebytes.com", &["malwarebytes"]),
        sp("keepassxc", "KeePassXC", "Security", "KeePassXCTeam.KeePassXC", "keepassxc.org", &["keepassxc"]),
        sp("keepass", "KeePass 2", "Security", "DominikReichl.KeePass", "keepass.info", &["keepass password safe"]),
        sp("bitwarden", "Bitwarden", "Security", "Bitwarden.Bitwarden", "bitwarden.com", &["bitwarden"]),
        sp("1password", "1Password", "Security", "AgileBits.1Password", "1password.com", &["1password"]),

        // File Sharing
        sp("qbittorrent", "qBittorrent", "File Sharing", "qBittorrent.qBittorrent", "qbittorrent.org", &["qbittorrent"]),

        // Imaging
        sp("gimp", "GIMP", "Imaging", "GIMP.GIMP", "gimp.org", &["gimp"]),
        sp("krita", "Krita", "Imaging", "KDE.Krita", "krita.org", &["krita"]),
        sp("blender", "Blender", "Imaging", "BlenderFoundation.Blender", "blender.org", &["blender"]),
        sp("paintnet", "Paint.NET", "Imaging", "dotPDN.PaintDotNet", "getpaint.net", &["paint.net"]),
        sp("irfanview", "IrfanView", "Imaging", "IrfanSkiljan.IrfanView", "irfanview.com", &["irfanview"]),
        sp("inkscape", "Inkscape", "Imaging", "Inkscape.Inkscape", "inkscape.org", &["inkscape"]),
        sp("sharex", "ShareX", "Imaging", "ShareX.ShareX", "getsharex.com", &["sharex"]),
        sp("greenshot", "Greenshot", "Imaging", "Greenshot.Greenshot", "getgreenshot.org", &["greenshot"]),
        sp("xnview", "XnView MP", "Imaging", "XnSoft.XnViewMP", "xnview.com", &["xnview"]),

        // Documents
        sp("libreoffice", "LibreOffice", "Documents", "TheDocumentFoundation.LibreOffice", "libreoffice.org", &["libreoffice"]),
        sp("sumatrapdf", "SumatraPDF", "Documents", "SumatraPDF.SumatraPDF", "sumatrapdfreader.org", &["sumatra"]),
        sp("foxit", "Foxit Reader", "Documents", "Foxit.FoxitReader", "foxit.com", &["foxit"]),
        sp("openoffice", "Apache OpenOffice", "Documents", "Apache.OpenOffice", "openoffice.org", &["openoffice"]),

        // Compression
        sp("peazip", "PeaZip", "Compression", "Giorgiotani.Peazip", "peazip.github.io", &["peazip"]),
        sp("winrar", "WinRAR", "Compression", "RARLab.WinRAR", "win-rar.com", &["winrar"]),

        // Online Storage
        sp("dropbox", "Dropbox", "Online Storage", "Dropbox.Dropbox", "dropbox.com", &["dropbox"]),
        sp("googledrive", "Google Drive", "Online Storage", "Google.GoogleDrive", "google.com", &["google drive"]),
        sp("onedrive", "OneDrive", "Online Storage", "Microsoft.OneDrive", "microsoft.com", &["onedrive"]),

        // Gaming
        sp("steam", "Steam", "Gaming", "Valve.Steam", "steampowered.com", &["steam"]),
        sp("epic", "Epic Games Launcher", "Gaming", "EpicGames.EpicGamesLauncher", "epicgames.com", &["epic games launcher"]),
    ]
}

fn describe(group: &str) -> String {
    match group {
        "Web Browsers" => "Web browser.",
        "Messaging" => "Chat and messaging.",
        "Media" => "Audio and video.",
        "Developer Tools" => "For developers.",
        "Utilities" => "System utility.",
        "Security" => "Security and passwords.",
        "File Sharing" => "File sharing.",
        "Imaging" => "Images and graphics.",
        "Documents" => "Documents and PDFs.",
        "Compression" => "Archive and compression.",
        "Online Storage" => "Cloud storage sync.",
        "Gaming" => "Game store or launcher.",
        _ => "Application.",
    }
    .to_string()
}

pub fn list_essentials() -> Vec<EssentialItem> {
    let names = installed_display_names();
    let start = start_app_names();
    specs()
        .into_iter()
        .map(|sp| {
            let installed = names.iter().any(|n| sp.matches.iter().any(|m| n.contains(m)))
                || start.iter().any(|n| sp.matches.iter().any(|m| n.contains(m)));
            EssentialItem {
                installed,
                description: describe(sp.group),
                id: sp.id.into(),
                name: sp.name.into(),
                group: sp.group.into(),
                winget_id: sp.winget_id.into(),
                domain: sp.domain.into(),
            }
        })
        .collect()
}

// Escape text for a batch `echo` / `title` line.
fn bat_text(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '%' => out.push_str("%%"),
            '^' | '&' | '|' | '<' | '>' | '(' | ')' => {
                out.push('^');
                out.push(c);
            }
            '\r' | '\n' | '"' => {}
            _ => out.push(c),
        }
    }
    out
}

// Text progress bar such as [######--------------] 30%%, already batch-escaped.
fn bat_bar(done: usize, total: usize) -> String {
    let pct = (done * 100).checked_div(total).unwrap_or(100);
    let filled = pct / 5;
    format!("[{}{}] {}%%", "#".repeat(filled), "-".repeat(20 - filled), pct)
}

// Build a .bat installer for the chosen apps. Only known winget ids are written.
// It prints the overall percentage before each app, and winget shows its own
// download progress bar for each one.
fn build_install_script(winget_ids: &[String]) -> String {
    let all = specs();
    let apps: Vec<&Spec> = winget_ids
        .iter()
        .filter_map(|id| all.iter().find(|sp| sp.winget_id == id))
        .collect();
    let total = apps.len();

    let mut out = String::new();
    out.push_str("@echo off\r\n");
    out.push_str("setlocal\r\n");
    out.push_str(":: Custom installer generated by NEXUS. Run on any Windows PC with winget.\r\n");
    out.push_str("title NEXUS Setup - 0%%\r\n");
    out.push_str("set /a OK=0, FAIL=0\r\n");
    out.push_str("where winget >nul 2>nul || (echo winget was not found. Install \"App Installer\" from the Microsoft Store first. & pause & exit /b 1)\r\n");
    out.push_str(&format!("echo NEXUS Setup: installing {} app(s).\r\necho.\r\n\r\n", total));

    for (i, sp) in apps.iter().enumerate() {
        let name = bat_text(sp.name);
        out.push_str("echo ============================================================\r\n");
        out.push_str(&format!("echo {}  [{}/{}]  {}\r\n", bat_bar(i, total), i + 1, total, name));
        out.push_str("echo ============================================================\r\n");
        out.push_str(&format!("title NEXUS Setup - {}%% - {}\r\n", i * 100 / total, name));
        out.push_str(&format!(
            "winget install --id {} -e --accept-source-agreements --accept-package-agreements\r\n",
            sp.winget_id
        ));
        // winget failure codes are negative, so compare instead of `if errorlevel 1`.
        out.push_str("if %errorlevel% equ 0 (set /a OK+=1) else (set /a FAIL+=1)\r\necho.\r\n\r\n");
    }

    out.push_str("title NEXUS Setup - 100%%\r\n");
    out.push_str(&format!("echo {}  All done.\r\n", bat_bar(total, total)));
    out.push_str("echo Installed: %OK%   Failed or already installed: %FAIL%\r\n");
    out.push_str("pause\r\n");
    out
}

pub fn export_install_script(path: &str, winget_ids: &[String]) -> Result<(), String> {
    if !path.to_lowercase().ends_with(".bat") {
        return Err("The installer must be saved as a .bat file.".into());
    }
    let content = build_install_script(winget_ids);
    std::fs::write(path, content).map_err(|e| format!("Could not write installer: {}", e))?;
    Ok(())
}
