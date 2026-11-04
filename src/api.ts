// Thin wrappers around the Tauri commands exposed by the Rust backend.

import { invoke } from "@tauri-apps/api/core";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import type {
  AccountsOverview,
  AppUpdate,
  CleanResult,
  CustomApp,
  DetectedApp,
  DiskInfo,
  EssentialItem,
  FolderEntry,
  HardwareInfo,
  InstallResult,
  MonitorSnapshot,
  ProcInfo,
  RuntimeItem,
  ScanResult,
  ServiceInfo,
  Settings,
  StartupItem,
  SupportedSize,
  TaskProc,
} from "./types";

export const api = {
  getCatalogApps: () => invoke<DetectedApp[]>("get_catalog_apps"),
  getAppIcons: (paths: string[]) =>
    invoke<Record<string, string>>("get_app_icons", { paths }),
  getHardware: () => invoke<HardwareInfo>("get_hardware"),

  getSettings: () => invoke<Settings>("get_settings"),
  setTheme: (theme: string) => invoke<Settings>("set_theme", { theme }),
  setAccent: (accent: string) => invoke<Settings>("set_accent", { accent }),
  toggleFavorite: (appId: string) => invoke<Settings>("toggle_favorite", { appId }),
  addCustomApp: (app: CustomApp) => invoke<Settings>("add_custom_app", { app }),
  removeCustomApp: (appId: string) => invoke<Settings>("remove_custom_app", { appId }),
  setLaunchOverride: (appId: string, path: string) =>
    invoke<Settings>("set_launch_override", { appId, path }),

  launchApp: (path: string, appId?: string) =>
    invoke<Settings>("launch_app", { path, appId: appId ?? null }),
  launchStore: (aumid: string, appId?: string) =>
    invoke<Settings>("launch_store", { aumid, appId: appId ?? null }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  openTool: (id: string) => invoke<void>("open_tool", { id }),

  scanTemp: () => invoke<ScanResult>("scan_temp"),
  cleanTemp: (ids: string[]) => invoke<CleanResult>("clean_temp", { ids }),
  emptyRecycleBin: () => invoke<void>("empty_recycle_bin"),
  flushDns: () => invoke<void>("flush_dns"),
  analyzeDisk: (path: string) => invoke<FolderEntry[]>("analyze_disk", { path }),

  getMonitor: () => invoke<MonitorSnapshot>("get_monitor"),
  getProcesses: () => invoke<ProcInfo[]>("get_processes"),

  listProcesses: () => invoke<TaskProc[]>("list_processes"),
  endProcess: (pid: number) => invoke<void>("end_process", { pid }),
  listServices: () => invoke<ServiceInfo[]>("list_services"),
  stopService: (name: string) => invoke<void>("stop_service", { name }),
  startService: (name: string) => invoke<void>("start_service", { name }),

  listDisks: () => invoke<DiskInfo[]>("list_disks"),
  partitionSupportedSize: (disk: number, part: number) =>
    invoke<SupportedSize>("partition_supported_size", { disk, part }),
  partitionSetLetter: (disk: number, part: number, letter: string | null) =>
    invoke<void>("partition_set_letter", { disk, part, letter }),
  partitionSetLabel: (disk: number, part: number, label: string) =>
    invoke<void>("partition_set_label", { disk, part, label }),
  partitionResize: (disk: number, part: number, size: number, expectOffset: number, expectSize: number) =>
    invoke<void>("partition_resize", { disk, part, size, expectOffset, expectSize }),
  partitionCreate: (disk: number, offset: number, size: number, fs: string, label: string) =>
    invoke<void>("partition_create", { disk, offset, size, fs, label }),
  partitionFormat: (disk: number, part: number, fs: string, label: string, expectOffset: number, expectSize: number) =>
    invoke<void>("partition_format", { disk, part, fs, label, expectOffset, expectSize }),
  partitionDelete: (disk: number, part: number, expectOffset: number, expectSize: number) =>
    invoke<void>("partition_delete", { disk, part, expectOffset, expectSize }),

  accountsOverview: () => invoke<AccountsOverview>("accounts_overview"),
  accountSave: (platform: string, name: string) =>
    invoke<void>("account_save", { platform, name }),
  accountSwitch: (platform: string, profile: string) =>
    invoke<void>("account_switch", { platform, profile }),
  accountAddNew: (platform: string) => invoke<void>("account_add_new", { platform }),
  accountDelete: (platform: string, profile: string) =>
    invoke<void>("account_delete", { platform, profile }),
  browserOpenProfile: (browser: string, profile: string) =>
    invoke<void>("browser_open_profile", { browser, profile }),

  getRuntimes: () => invoke<RuntimeItem[]>("get_runtimes"),
  installRuntime: (wingetId: string) =>
    invoke<InstallResult>("install_runtime", { wingetId }),

  getStartup: () => invoke<StartupItem[]>("get_startup"),
  setStartupEnabled: (source: string, name: string, enabled: boolean) =>
    invoke<void>("set_startup_enabled", { source, name, enabled }),

  isElevated: () => invoke<boolean>("is_elevated"),
  restartAsAdmin: () => invoke<void>("restart_as_admin"),

  getEssentials: () => invoke<EssentialItem[]>("get_essentials"),
  exportInstallScript: (path: string, wingetIds: string[]) =>
    invoke<void>("export_install_script", { path, wingetIds }),

  getUpdates: () => invoke<AppUpdate[]>("get_updates"),
  upgradeApp: (id: string) => invoke<InstallResult>("upgrade_app", { id }),
  upgradeAll: () => invoke<InstallResult>("upgrade_all"),
};

// Native "save file" picker for the custom installer, returning the path or null.
export async function pickSavePath(defaultName: string): Promise<string | null> {
  const selected = await saveDialog({
    defaultPath: defaultName,
    filters: [{ name: "Batch installer", extensions: ["bat"] }],
  });
  return selected ?? null;
}

// Open a native file picker for a single .exe, returning the chosen path or null.
export async function pickExecutable(): Promise<string | null> {
  const selected = await openDialog({
    multiple: false,
    directory: false,
    filters: [{ name: "Programs", extensions: ["exe"] }],
  });
  if (typeof selected === "string") return selected;
  return null;
}

// A real app logo from a domain, via Google's favicon service (64px).
export function faviconUrl(domain: string): string {
  return `https://www.google.com/s2/favicons?sz=64&domain=${encodeURIComponent(domain)}`;
}

// Best-effort domain from an https URL, for logo lookup.
export function domainFromUrl(url: string): string | null {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return null;
  }
}

// Format bytes as a short human string, for example "1.4 GB".
export function fmtBytes(bytes: number): string {
  if (bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  const v = bytes / Math.pow(1024, i);
  return `${v.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}
