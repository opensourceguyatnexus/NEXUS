// Elevation helpers: check whether NEXUS is running as administrator, and
// relaunch it elevated through the standard UAC prompt.

use std::ffi::c_void;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND};
use windows::Win32::Security::{
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    ChangeWindowMessageFilterEx, FindWindowW, MSGFLT_ALLOW, SW_SHOWNORMAL, WM_COPYDATA,
};

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// Tells the new copy that the old one is about to exit.
pub const RELAUNCH_ARG: &str = "--nexus-relaunched";

pub fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut ret_len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret_len,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elevation.TokenIsElevated != 0
    }
}

// Windows blocks messages from normal apps to admin ones, so opening NEXUS normally
// could not reach a hidden admin copy and nothing would appear. This allows only
// that one message, only on the single-instance plugin's own window. The plugin
// names that window from the app identifier, and the message only shows NEXUS.
pub fn allow_reopen_from_normal_copy(identifier: &str) {
    if !is_elevated() {
        return;
    }
    let class = to_wide(&format!("{identifier}-sic"));
    let title = to_wide(&format!("{identifier}-siw"));
    unsafe {
        if let Ok(window) = FindWindowW(PCWSTR(class.as_ptr()), PCWSTR(title.as_ptr())) {
            let _ = ChangeWindowMessageFilterEx(window, WM_COPYDATA, MSGFLT_ALLOW, None);
        }
    }
}

// Launch a fresh, elevated copy of NEXUS. The caller should exit the current
// (non-elevated) instance afterwards.
pub fn relaunch_as_admin() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("Cannot find NEXUS: {}", e))?;
    let exe_wide = to_wide(&exe.to_string_lossy());
    let verb = to_wide("runas");
    let args = to_wide(RELAUNCH_ARG);
    let result = unsafe {
        ShellExecuteW(
            HWND(std::ptr::null_mut()),
            PCWSTR(verb.as_ptr()),
            PCWSTR(exe_wide.as_ptr()),
            PCWSTR(args.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize > 32 {
        Ok(())
    } else {
        Err("Elevation was cancelled.".into())
    }
}
