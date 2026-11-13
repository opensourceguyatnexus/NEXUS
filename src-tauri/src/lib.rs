// NEXUS backend entry point. Declares the modules and exposes the Tauri
// commands the UI calls. Heavy or blocking work (detection, scanning, the
// monitor sample) runs on a blocking thread so the window stays responsive.

mod accounts;
mod catalog;
mod detect;
mod disk;
mod diskmgr;
mod elevation;
mod hardware;
mod icons;
mod launcher;
mod models;
mod monitor;
mod runtimes;
mod settings;
mod setup;
mod startup;
mod tasks;
mod toolkit;
mod updates;

use models::{
    AppUpdate, CleanResult, CustomApp, DetectedApp, FolderEntry, HardwareInfo, MonitorSnapshot,
    ProcInfo, ScanResult, Settings, StartupItem,
};
use runtimes::{InstallResult, RuntimeItem};
use diskmgr::{DiskInfo, SupportedSize};
use setup::EssentialItem;
use tasks::{ServiceInfo, TaskProc};
use std::collections::HashMap;
use tauri::async_runtime::spawn_blocking;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Manager,
};
use tauri_plugin_autostart::MacosLauncher;

async fn blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    spawn_blocking(f)
        .await
        .map_err(|e| format!("Background task failed: {}", e))
}

// ---- Catalog and detection ----

#[tauri::command]
async fn get_catalog_apps() -> Result<Vec<DetectedApp>, String> {
    blocking(|| {
        let overrides = settings::load_settings().launch_overrides;
        detect::detect_apps(&overrides)
    })
    .await
}

#[tauri::command]
async fn get_app_icons(paths: Vec<String>) -> Result<HashMap<String, String>, String> {
    blocking(move || icons::extract_icons(paths)).await
}

#[tauri::command]
async fn get_hardware() -> Result<HardwareInfo, String> {
    blocking(hardware::detect_hardware).await
}

// ---- Settings ----

#[tauri::command]
fn get_settings() -> Settings {
    settings::load_settings()
}

#[tauri::command]
fn set_theme(theme: String) -> Result<Settings, String> {
    settings::set_theme(&theme)
}

#[tauri::command]
fn set_accent(accent: String) -> Result<Settings, String> {
    settings::set_accent(&accent)
}

#[tauri::command]
fn toggle_favorite(app_id: String) -> Result<Settings, String> {
    settings::toggle_favorite(&app_id)
}

#[tauri::command]
fn add_custom_app(app: CustomApp) -> Result<Settings, String> {
    settings::add_custom_app(app)
}

#[tauri::command]
fn remove_custom_app(app_id: String) -> Result<Settings, String> {
    settings::remove_custom_app(&app_id)
}

#[tauri::command]
fn set_launch_override(app_id: String, path: String) -> Result<Settings, String> {
    settings::set_launch_override(&app_id, &path)
}

// ---- Launching ----

#[tauri::command]
fn launch_app(path: String, app_id: Option<String>) -> Result<Settings, String> {
    launcher::launch_path(&path)?;
    match app_id {
        Some(id) => settings::push_recent(&id),
        None => Ok(settings::load_settings()),
    }
}

#[tauri::command]
fn launch_store(aumid: String, app_id: Option<String>) -> Result<Settings, String> {
    launcher::launch_store(&aumid)?;
    match app_id {
        Some(id) => settings::push_recent(&id),
        None => Ok(settings::load_settings()),
    }
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    launcher::open_url(&url)
}

#[tauri::command]
fn open_tool(id: String) -> Result<(), String> {
    launcher::open_windows_tool(&id)
}

// ---- Windows Toolkit ----

#[tauri::command]
async fn scan_temp() -> Result<ScanResult, String> {
    blocking(toolkit::scan).await
}

#[tauri::command]
async fn clean_temp(ids: Vec<String>) -> Result<CleanResult, String> {
    blocking(move || toolkit::clean(ids)).await
}

#[tauri::command]
async fn empty_recycle_bin() -> Result<(), String> {
    blocking(toolkit::empty_recycle_bin).await?
}

#[tauri::command]
async fn flush_dns() -> Result<(), String> {
    blocking(toolkit::flush_dns).await?
}

#[tauri::command]
async fn analyze_disk(path: String) -> Result<Vec<FolderEntry>, String> {
    blocking(move || disk::analyze_path(&path)).await?
}

// ---- System Monitor ----

#[tauri::command]
async fn get_monitor() -> Result<MonitorSnapshot, String> {
    blocking(monitor::snapshot).await
}

#[tauri::command]
async fn get_processes() -> Result<Vec<ProcInfo>, String> {
    blocking(monitor::top_processes).await
}

// ---- Task Manager ----

#[tauri::command]
async fn list_processes() -> Result<Vec<TaskProc>, String> {
    blocking(tasks::list_processes).await
}

#[tauri::command]
async fn end_process(pid: u32) -> Result<(), String> {
    blocking(move || tasks::end_process(pid)).await?
}

#[tauri::command]
async fn list_services() -> Result<Vec<ServiceInfo>, String> {
    blocking(tasks::list_services).await?
}

#[tauri::command]
async fn stop_service(name: String) -> Result<(), String> {
    blocking(move || tasks::stop_service(&name)).await?
}

#[tauri::command]
async fn start_service(name: String) -> Result<(), String> {
    blocking(move || tasks::start_service(&name)).await?
}

// ---- Disk Manager ----

#[tauri::command]
async fn list_disks() -> Result<Vec<DiskInfo>, String> {
    blocking(diskmgr::list_disks).await?
}

#[tauri::command]
async fn partition_supported_size(disk: u32, part: u32) -> Result<SupportedSize, String> {
    blocking(move || diskmgr::supported_size(disk, part)).await?
}

#[tauri::command]
async fn partition_set_letter(disk: u32, part: u32, letter: Option<String>) -> Result<(), String> {
    blocking(move || diskmgr::set_letter(disk, part, letter)).await?
}

#[tauri::command]
async fn partition_set_label(disk: u32, part: u32, label: String) -> Result<(), String> {
    blocking(move || diskmgr::set_label(disk, part, label)).await?
}

#[tauri::command]
async fn partition_resize(
    disk: u32,
    part: u32,
    size: u64,
    expect_offset: u64,
    expect_size: u64,
) -> Result<(), String> {
    let expected = diskmgr::Expected { offset: expect_offset, size: expect_size };
    blocking(move || diskmgr::resize(disk, part, size, expected)).await?
}

#[tauri::command]
async fn partition_create(
    disk: u32,
    offset: u64,
    size: u64,
    fs: String,
    label: String,
) -> Result<(), String> {
    blocking(move || diskmgr::create(disk, offset, size, fs, label)).await?
}

#[tauri::command]
async fn partition_format(
    disk: u32,
    part: u32,
    fs: String,
    label: String,
    expect_offset: u64,
    expect_size: u64,
) -> Result<(), String> {
    let expected = diskmgr::Expected { offset: expect_offset, size: expect_size };
    blocking(move || diskmgr::format(disk, part, fs, label, expected)).await?
}

#[tauri::command]
async fn partition_delete(
    disk: u32,
    part: u32,
    expect_offset: u64,
    expect_size: u64,
) -> Result<(), String> {
    let expected = diskmgr::Expected { offset: expect_offset, size: expect_size };
    blocking(move || diskmgr::delete(disk, part, expected)).await?
}

// ---- Account Switcher ----

#[tauri::command]
async fn accounts_overview() -> Result<accounts::AccountsOverview, String> {
    blocking(accounts::overview).await
}

#[tauri::command]
async fn account_save(platform: String, name: String) -> Result<(), String> {
    blocking(move || accounts::save_current(&platform, &name)).await?
}

#[tauri::command]
async fn account_switch(platform: String, profile: String) -> Result<(), String> {
    blocking(move || accounts::switch_to(&platform, &profile)).await?
}

#[tauri::command]
async fn account_add_new(platform: String) -> Result<(), String> {
    blocking(move || accounts::add_new(&platform)).await?
}

#[tauri::command]
async fn account_delete(platform: String, profile: String) -> Result<(), String> {
    blocking(move || accounts::delete_saved(&platform, &profile)).await?
}

#[tauri::command]
async fn browser_open_profile(browser: String, profile: String) -> Result<(), String> {
    blocking(move || accounts::open_browser(&browser, &profile)).await?
}

// ---- Runtimes and redistributables ----

#[tauri::command]
async fn get_runtimes() -> Result<Vec<RuntimeItem>, String> {
    blocking(runtimes::list_runtimes).await
}

#[tauri::command]
async fn install_runtime(winget_id: String) -> Result<InstallResult, String> {
    blocking(move || runtimes::install_runtime(&winget_id)).await
}

// ---- Startup Manager ----

#[tauri::command]
async fn get_startup() -> Result<Vec<StartupItem>, String> {
    blocking(|| startup::list_startup(elevation::is_elevated())).await
}

// ---- Elevation ----

#[tauri::command]
fn is_elevated() -> bool {
    elevation::is_elevated()
}

#[tauri::command]
fn restart_as_admin(app: tauri::AppHandle) -> Result<(), String> {
    elevation::relaunch_as_admin()?;
    app.exit(0);
    Ok(())
}

#[tauri::command]
fn set_startup_enabled(source: String, name: String, enabled: bool) -> Result<(), String> {
    startup::set_startup_enabled(&source, &name, enabled)
}

// ---- New PC Setup ----

#[tauri::command]
async fn get_essentials() -> Result<Vec<EssentialItem>, String> {
    blocking(setup::list_essentials).await
}

#[tauri::command]
fn export_install_script(path: String, winget_ids: Vec<String>) -> Result<(), String> {
    setup::export_install_script(&path, &winget_ids)
}

// ---- Update Center ----

#[tauri::command]
async fn get_updates() -> Result<Vec<AppUpdate>, String> {
    blocking(updates::list_updates).await?
}

#[tauri::command]
async fn upgrade_app(id: String) -> Result<InstallResult, String> {
    blocking(move || updates::upgrade_app(&id)).await
}

#[tauri::command]
async fn upgrade_all() -> Result<InstallResult, String> {
    blocking(updates::upgrade_all).await
}

const START_HIDDEN_ARG: &str = "--minimized";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // A restart as admin starts this copy just before the old one exits. Wait so
    // the single-instance check below does not mistake it for a second launch.
    if std::env::args().any(|a| a == elevation::RELAUNCH_ARG) {
        std::thread::sleep(std::time::Duration::from_millis(1500));
    }

    tauri::Builder::default()
        // Must be first. NEXUS hides to the tray on close, so opening it again
        // should show that window, not start a second copy.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![START_HIDDEN_ARG]),
        ))
        .setup(|app| {
            elevation::allow_reopen_from_normal_copy(&app.config().identifier);

            // System tray: left-click shows the window, the menu has Show and Quit.
            let show = MenuItem::with_id(app, "show", "Show NEXUS", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("NEXUS")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.unminimize();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        ..
                    } = event
                    {
                        if let Some(w) = tray.app_handle().get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.unminimize();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;

            // Started by Windows at sign-in: stay in the tray.
            if std::env::args().any(|a| a == START_HIDDEN_ARG) {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window hides it to the tray instead of quitting. Use
            // the tray's Quit item to exit fully.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_catalog_apps,
            get_app_icons,
            get_hardware,
            get_settings,
            set_theme,
            set_accent,
            toggle_favorite,
            add_custom_app,
            remove_custom_app,
            set_launch_override,
            launch_app,
            launch_store,
            open_url,
            open_tool,
            scan_temp,
            clean_temp,
            empty_recycle_bin,
            flush_dns,
            analyze_disk,
            get_monitor,
            get_processes,
            list_processes,
            end_process,
            list_services,
            stop_service,
            start_service,
            list_disks,
            partition_supported_size,
            partition_set_letter,
            partition_set_label,
            partition_resize,
            partition_create,
            partition_format,
            partition_delete,
            accounts_overview,
            account_save,
            account_switch,
            account_add_new,
            account_delete,
            browser_open_profile,
            get_runtimes,
            install_runtime,
            get_startup,
            set_startup_enabled,
            is_elevated,
            restart_as_admin,
            get_essentials,
            export_install_script,
            get_updates,
            upgrade_app,
            upgrade_all,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
