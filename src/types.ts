// TypeScript mirrors of the Rust structs returned by the backend.

export interface DetectedApp {
  id: string;
  name: string;
  vendor: string;
  category: string;
  description: string;
  download_url: string;
  compatibility: string;
  installed: boolean;
  launch_path: string | null;
  launch_aumid: string | null;
}

export interface CustomApp {
  id: string;
  name: string;
  category: string;
  description: string;
  exec_path: string;
  download_url: string;
}

export interface Settings {
  theme: string;
  accent: string;
  favorites: string[];
  recent: string[];
  custom_apps: CustomApp[];
}

export interface HardwareDevice {
  kind: string;
  name: string;
  detail: string;
}

export interface HardwareInfo {
  cpu: HardwareDevice | null;
  gpus: HardwareDevice[];
  memory: HardwareDevice | null;
  disks: HardwareDevice[];
  peripherals: HardwareDevice[];
  os: HardwareDevice | null;
}

export interface DiskUsage {
  mount: string;
  used: number;
  total: number;
}

export interface ProcInfo {
  name: string;
  pid: number;
  cpu: number;
  mem: number;
}

export interface TaskProc {
  name: string;
  pid: number;
  cpu: number;
  mem: number;
  protected: boolean;
}

export interface ServiceInfo {
  name: string;
  display_name: string;
  state: string;
  start_mode: string;
  pid: number;
  can_stop: boolean;
  can_start: boolean;
}

export interface PartitionInfo {
  disk: number;
  number: number;
  letter: string | null;
  offset: number;
  size: number;
  kind: string;
  file_system: string | null;
  label: string | null;
  free: number | null;
  is_boot: boolean;
  is_system: boolean;
  locked: boolean;
  resizable: boolean;
}

export interface FreeSpace {
  offset: number;
  size: number;
}

export interface DiskInfo {
  number: number;
  name: string;
  style: string;
  bus: string;
  size: number;
  health: string;
  is_boot: boolean;
  is_offline?: boolean;
  partitions: PartitionInfo[];
  free: FreeSpace[];
}

export interface SupportedSize {
  min: number;
  max: number;
}

export interface AccountEntry {
  id: string;
  name: string;
  detail: string;
  active: boolean;
  avatar: string | null;
  remembered: boolean;
}

export interface AccountPlatform {
  id: string;
  name: string;
  running: boolean;
  managed: boolean;
  verified: boolean;
  installed: boolean;
  current: string | null;
  accounts: AccountEntry[];
  error?: string | null;
}

export interface BrowserProfile {
  id: string;
  name: string;
  detail: string;
}

export interface BrowserInfo {
  id: string;
  name: string;
  profiles: BrowserProfile[];
}

export interface AccountsOverview {
  platforms: AccountPlatform[];
  browsers: BrowserInfo[];
}

export interface MonitorSnapshot {
  cpu_usage: number | null;
  cpu_temp: number | null;
  mem_used: number;
  mem_total: number;
  gpu_name: string | null;
  gpu_usage: number | null;
  gpu_temp: number | null;
  gpu_mem_used: number | null;
  gpu_mem_total: number | null;
  gpu_fan: number | null;
  disks: DiskUsage[];
}

export interface CleanLocation {
  id: string;
  label: string;
  path: string;
  file_count: number;
  size_bytes: number;
}

export interface ScanResult {
  locations: CleanLocation[];
  total_bytes: number;
  total_files: number;
}

export interface CleanResult {
  freed_bytes: number;
  removed_files: number;
  skipped_files: number;
  denied_files: number;
}

export interface RuntimeItem {
  id: string;
  name: string;
  description: string;
  group: string;
  winget_id: string;
  installed: boolean | null;
}

export interface InstallResult {
  ok: boolean;
  message: string;
}

export interface StartupItem {
  name: string;
  command: string;
  location: string;
  source: string;
  enabled: boolean;
  can_toggle: boolean;
}

export interface AppUpdate {
  name: string;
  id: string;
  current: string;
  available: string;
}

export interface FolderEntry {
  name: string;
  path: string;
  size: number;
  is_dir: boolean;
}

export interface EssentialItem {
  id: string;
  name: string;
  description: string;
  group: string;
  winget_id: string;
  domain: string;
  installed: boolean;
}

export const CATEGORY_LABELS: Record<string, string> = {
  mouse_keyboard: "Mouse & Keyboard",
  audio: "Headsets & Audio",
  gpu: "GPU & Graphics",
  motherboard: "Motherboard & System",
  rgb_cooling: "RGB, Fans & Cooling",
  benchmark: "Benchmark & Diagnostics",
  custom: "Custom",
};
