// System runtimes and redistributables (Visual C++, DirectX, .NET).
//
// Detection reads the uninstall registry for known display names. Installation
// is delegated to winget using official Microsoft package ids. Installing is a
// privileged action: it triggers a UAC prompt, which the user approves. We never
// build a command from user text; winget ids come from this fixed list.

use crate::detect::installed_display_names;
use serde::{Deserialize, Serialize};
use std::os::windows::process::CommandExt;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub group: String,
    pub winget_id: String,
    // True when we can detect it, false when not found, null when detection is
    // not reliable for this item (for example DirectX components).
    pub installed: Option<bool>,
}

struct Spec {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    group: &'static str,
    winget_id: &'static str,
    // Lowercased substrings matched against installed display names. Empty means
    // detection is unreliable and we report Unknown.
    matches: &'static [&'static str],
}

fn specs() -> Vec<Spec> {
    vec![
        // Visual C++ (modern)
        Spec { id: "vc-2015-2022-x64", name: "Visual C++ 2015-2022 (x64)", description: "Runtime for most modern apps and games (64-bit).", group: "Visual C++", winget_id: "Microsoft.VCRedist.2015+.x64", matches: &["2015-2022 redistributable (x64)", "2022 redistributable (x64)", "2019 redistributable (x64)"] },
        Spec { id: "vc-2015-2022-x86", name: "Visual C++ 2015-2022 (x86)", description: "Runtime for most modern apps and games (32-bit).", group: "Visual C++", winget_id: "Microsoft.VCRedist.2015+.x86", matches: &["2015-2022 redistributable (x86)", "2022 redistributable (x86)", "2019 redistributable (x86)"] },
        // Visual C++ (legacy, still needed by many games)
        Spec { id: "vc-2013-x64", name: "Visual C++ 2013 (x64)", description: "Legacy runtime required by some older games.", group: "Visual C++", winget_id: "Microsoft.VCRedist.2013.x64", matches: &["2013 redistributable (x64)", "2013 x64"] },
        Spec { id: "vc-2013-x86", name: "Visual C++ 2013 (x86)", description: "Legacy runtime required by some older games.", group: "Visual C++", winget_id: "Microsoft.VCRedist.2013.x86", matches: &["2013 redistributable (x86)", "2013 x86"] },
        Spec { id: "vc-2012-x64", name: "Visual C++ 2012 (x64)", description: "Legacy runtime.", group: "Visual C++", winget_id: "Microsoft.VCRedist.2012.x64", matches: &["2012 redistributable (x64)", "2012 x64"] },
        Spec { id: "vc-2012-x86", name: "Visual C++ 2012 (x86)", description: "Legacy runtime.", group: "Visual C++", winget_id: "Microsoft.VCRedist.2012.x86", matches: &["2012 redistributable (x86)", "2012 x86"] },
        Spec { id: "vc-2010-x64", name: "Visual C++ 2010 (x64)", description: "Legacy runtime.", group: "Visual C++", winget_id: "Microsoft.VCRedist.2010.x64", matches: &["2010 redistributable (x64)", "2010 x64"] },
        Spec { id: "vc-2010-x86", name: "Visual C++ 2010 (x86)", description: "Legacy runtime.", group: "Visual C++", winget_id: "Microsoft.VCRedist.2010.x86", matches: &["2010 redistributable (x86)", "2010 x86"] },
        Spec { id: "vc-2008-x64", name: "Visual C++ 2008 (x64)", description: "Legacy runtime.", group: "Visual C++", winget_id: "Microsoft.VCRedist.2008.x64", matches: &["2008 redistributable (x64)", "2008 x64"] },
        Spec { id: "vc-2008-x86", name: "Visual C++ 2008 (x86)", description: "Legacy runtime.", group: "Visual C++", winget_id: "Microsoft.VCRedist.2008.x86", matches: &["2008 redistributable (x86)", "2008 x86"] },
        // DirectX
        Spec { id: "directx", name: "DirectX End-User Runtime", description: "Legacy DirectX components (d3dx9 and others) many games need.", group: "DirectX", winget_id: "Microsoft.DirectX", matches: &[] },
        // .NET
        Spec { id: "dotnet-desktop-8", name: ".NET Desktop Runtime 8", description: "Runtime for modern .NET desktop applications.", group: ".NET", winget_id: "Microsoft.DotNet.DesktopRuntime.8", matches: &["windows desktop runtime - 8"] },
        Spec { id: "dotnet-desktop-6", name: ".NET Desktop Runtime 6", description: "Runtime for .NET 6 desktop applications.", group: ".NET", winget_id: "Microsoft.DotNet.DesktopRuntime.6", matches: &["windows desktop runtime - 6"] },
    ]
}

pub fn list_runtimes() -> Vec<RuntimeItem> {
    let names = installed_display_names();
    specs()
        .into_iter()
        .map(|s| {
            let installed = if s.matches.is_empty() {
                None
            } else {
                Some(names.iter().any(|n| s.matches.iter().any(|m| n.contains(m))))
            };
            RuntimeItem {
                id: s.id.into(),
                name: s.name.into(),
                description: s.description.into(),
                group: s.group.into(),
                winget_id: s.winget_id.into(),
                installed,
            }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallResult {
    pub ok: bool,
    pub message: String,
}

// Only winget ids that appear in our spec list may be installed.
fn is_known_winget_id(id: &str) -> bool {
    specs().iter().any(|s| s.winget_id == id)
}

pub fn install_runtime(winget_id: &str) -> InstallResult {
    if !is_known_winget_id(winget_id) {
        return InstallResult {
            ok: false,
            message: "Unknown package.".into(),
        };
    }
    winget_install(winget_id)
}

// What winget's documented exit codes mean, or None for a generic failure. Failures
// such as "package not found" must never be reported as success.
pub fn winget_outcome(code: i32, success: &str) -> Option<(bool, String)> {
    let (ok, message) = match code {
        0 => (true, success),
        -1978335135 | -1978334963 => (true, "Already installed."),
        -1978334967 => (true, "Done. Restart your PC to finish."),
        -1978335189 => (true, "No newer version applies to this install."),
        -1978335212 => (false, "winget could not find this package."),
        -1978335215 => (false, "The download failed its safety check (hash mismatch). Try again later."),
        _ => return None,
    };
    Some((ok, message.to_string()))
}

// The caller validates the id against the allowlist first.
pub fn winget_install(winget_id: &str) -> InstallResult {
    let output = Command::new("winget")
        .args([
            "install",
            "--id",
            winget_id,
            "-e",
            "--accept-source-agreements",
            "--accept-package-agreements",
            "--disable-interactivity",
            "--silent",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match output {
        Ok(o) => {
            let code = o.status.code().unwrap_or(-1);
            if let Some((ok, message)) = winget_outcome(code, "Installed.") {
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
            message: format!("Could not run winget: {}. Is App Installer present?", e),
        },
    }
}
