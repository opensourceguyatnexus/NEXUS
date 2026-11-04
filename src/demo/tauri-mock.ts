// Stands in for the Tauri backend in the website's live demo and in screenshots.
// Everything here is sample data, and no action touches a real PC. It is only
// used by vite.demo.config.ts and is never part of the desktop app.

const GB = 1024 ** 3;

const settings = {
  theme: new URLSearchParams(location.search).get("theme") ?? "dark",
  accent: "blue",
  favorites: ["msi_afterburner", "hwinfo"],
  recent: ["msi_afterburner", "hwinfo"],
  custom_apps: [] as unknown[],
};

const app = (id: string, name: string, vendor: string, category: string, description: string, installed = true) => ({
  id,
  name,
  vendor,
  category,
  description,
  download_url: "https://example.com",
  compatibility: "Works on this PC.",
  installed,
  launch_path: installed ? `C:\\Demo\\${id}.exe` : null,
  launch_aumid: null,
});

const part = (
  disk: number,
  number: number,
  letter: string | null,
  offset: number,
  size: number,
  kind: string,
  fs: string | null,
  label: string | null,
  free: number | null,
  flags: Partial<{ is_boot: boolean; is_system: boolean; locked: boolean; resizable: boolean }> = {},
) => ({
  disk,
  number,
  letter,
  offset,
  size,
  kind,
  file_system: fs,
  label,
  free,
  is_boot: false,
  is_system: false,
  locked: false,
  resizable: fs === "NTFS",
  ...flags,
});

const essential = (id: string, name: string, description: string, group: string, domain: string) => ({
  id,
  name,
  description,
  group,
  winget_id: `Demo.${id}`,
  domain,
  installed: false,
});

const proc = (name: string, pid: number, cpu: number, mem: number, isProtected = false) => ({
  name,
  pid,
  cpu,
  mem,
  protected: isProtected,
});

const service = (name: string, display_name: string, running: boolean) => ({
  name,
  display_name,
  state: running ? "Running" : "Stopped",
  start_mode: running ? "Auto" : "Manual",
  pid: running ? 2200 : 0,
  can_stop: running,
  can_start: !running,
});

let tick = 0;

function monitor() {
  tick += 1;
  const wave = (base: number, spread: number) => base + Math.sin(tick / 1.7) * spread;
  return {
    cpu_usage: wave(34, 9),
    cpu_temp: wave(58, 3),
    mem_used: 19.4 * GB,
    mem_total: 32 * GB,
    gpu_name: "Sample RTX GPU",
    gpu_usage: wave(62, 14),
    gpu_temp: wave(66, 2),
    gpu_mem_used: 7.2 * GB,
    gpu_mem_total: 12 * GB,
    gpu_fan: wave(48, 4),
    disks: [
      { mount: "C:\\", used: 612 * GB, total: 931 * GB },
      { mount: "D:\\", used: 1210 * GB, total: 1863 * GB },
    ],
  };
}

const RESPONSES: Record<string, () => unknown> = {
  get_settings: () => settings,
  is_elevated: () => false,
  get_app_icons: () => ({}),
  get_catalog_apps: () => [
    app("msi_afterburner", "MSI Afterburner", "MSI", "gpu", "Overclock your graphics card and watch it in game."),
    app("hwinfo", "HWiNFO", "REALiX", "benchmark", "Every sensor in your PC, in one list."),
    app("gg", "Logitech G HUB", "Logitech", "mouse_keyboard", "Set up your mouse, keyboard and headset."),
    app("icue", "iCUE", "Corsair", "rgb_cooling", "Control RGB lighting and fan speeds."),
    app("cinebench", "Cinebench", "Maxon", "benchmark", "Benchmark your processor.", false),
    app("nahimic", "Nahimic", "A-Volute", "audio", "Tune game audio for your headset.", false),
  ],
  get_hardware: () => ({
    cpu: { kind: "cpu", name: "Sample 8-Core Processor", detail: "16 logical cores" },
    gpus: [{ kind: "gpu", name: "Sample RTX GPU", detail: "12.0 GB VRAM" }],
    memory: { kind: "memory", name: "32.0 GB total", detail: "RAM, 12.6 GB free" },
    disks: [
      { kind: "disk", name: "C:\\", detail: "612.0 GB used of 931.0 GB" },
      { kind: "disk", name: "D:\\", detail: "1210.0 GB used of 1863.0 GB" },
    ],
    peripherals: [
      { kind: "keyboard", name: "Sample mechanical keyboard", detail: "USB" },
      { kind: "pointing", name: "Sample gaming mouse", detail: "USB" },
    ],
    os: { kind: "os", name: "Microsoft Windows 11", detail: "version 10.0.26200" },
  }),
  get_monitor: monitor,
  get_processes: () => [
    { name: "game.exe", pid: 8120, cpu: 21, mem: 6.1 * GB },
    { name: "chrome.exe", pid: 4120, cpu: 4, mem: 2.3 * GB },
    { name: "discord.exe", pid: 7628, cpu: 1, mem: 0.6 * GB },
  ],
  list_processes: () => [
    proc("game.exe", 8120, 21.4, 6.1 * GB),
    proc("chrome.exe", 4120, 2.2, 1.4 * GB),
    proc("chrome.exe", 4188, 1.8, 0.9 * GB),
    proc("discord.exe", 7628, 0.9, 0.6 * GB),
    proc("steam.exe", 6204, 0.4, 0.3 * GB),
    proc("explorer.exe", 3312, 0.3, 0.2 * GB, true),
    proc("svchost.exe", 1204, 0.2, 0.1 * GB, true),
  ],
  list_services: () => [
    service("Spooler", "Print Spooler", true),
    service("Steam Client Service", "Steam Client Service", true),
    service("Fax", "Fax", false),
    service("WSearch", "Windows Search", true),
  ],
  scan_temp: () => ({
    locations: [
      { id: "user_temp", label: "User temporary files", path: "C:\\Users\\Player\\AppData\\Local\\Temp", file_count: 4120, size_bytes: 3.8 * GB },
      { id: "win_update", label: "Windows Update cache", path: "C:\\Windows\\SoftwareDistribution\\Download", file_count: 310, size_bytes: 2.1 * GB },
      { id: "crash_dumps", label: "Crash dumps", path: "C:\\Users\\Player\\AppData\\Local\\CrashDumps", file_count: 12, size_bytes: 0.6 * GB },
    ],
    total_bytes: 6.5 * GB,
    total_files: 4442,
  }),
  clean_temp: () => ({ freed_bytes: 6.1 * GB, removed_files: 4310, skipped_files: 132 }),
  analyze_disk: () => [
    { name: "Games", path: "D:\\Games", size: 820 * GB, is_dir: true },
    { name: "Videos", path: "D:\\Videos", size: 240 * GB, is_dir: true },
    { name: "Installers", path: "D:\\Installers", size: 74 * GB, is_dir: true },
  ],
  list_disks: () => [
    {
      number: 0,
      name: "Sample NVMe SSD",
      style: "GPT",
      bus: "NVMe",
      size: 931 * GB,
      health: "Healthy",
      is_boot: true,
      partitions: [
        part(0, 1, null, 1024 ** 2, 0.1 * GB, "System", null, null, null, { is_system: true, locked: true, resizable: false }),
        part(0, 2, "C", 0.2 * GB, 930 * GB, "Basic", "NTFS", "Windows", 319 * GB, { is_boot: true }),
      ],
      free: [],
    },
    {
      number: 1,
      name: "Sample Game SSD",
      style: "GPT",
      bus: "NVMe",
      size: 1863 * GB,
      health: "Healthy",
      is_boot: false,
      partitions: [part(1, 1, "D", 1024 ** 2, 1500 * GB, "Basic", "NTFS", "Games", 290 * GB)],
      free: [{ offset: 1500 * GB, size: 363 * GB }],
    },
  ],
  partition_supported_size: () => ({ min: 700 * GB, max: 1863 * GB }),
  accounts_overview: () => ({
    platforms: [
      {
        id: "steam",
        name: "Steam",
        running: true,
        managed: false,
        verified: true,
        installed: true,
        current: "MainPlayer",
        accounts: [
          { id: "1", name: "MainPlayer", detail: "main_player", active: true, avatar: null, remembered: true },
          { id: "2", name: "SmurfAccount", detail: "smurf_account", active: false, avatar: null, remembered: true },
        ],
      },
      {
        id: "riot",
        name: "Riot Games",
        running: false,
        managed: true,
        verified: true,
        installed: true,
        current: "MainPlayer#EUW",
        accounts: [
          { id: "3", name: "MainPlayer#EUW", detail: "", active: true, avatar: null, remembered: true },
          { id: "4", name: "Ranked Alt", detail: "", active: false, avatar: null, remembered: true },
        ],
      },
      {
        id: "discord",
        name: "Discord",
        running: true,
        managed: true,
        verified: false,
        installed: true,
        current: null,
        accounts: [
          { id: "5", name: "Personal", detail: "", active: true, avatar: null, remembered: true },
          { id: "6", name: "Clan server", detail: "", active: false, avatar: null, remembered: true },
        ],
      },
    ],
    browsers: [
      {
        id: "brave",
        name: "Brave",
        profiles: [
          { id: "Default", name: "Personal", detail: "" },
          { id: "Profile 1", name: "Gaming", detail: "" },
        ],
      },
    ],
  }),
  get_essentials: () => [
    essential("chrome", "Google Chrome", "Web browser.", "Browsers", "google.com"),
    essential("firefox", "Mozilla Firefox", "Web browser.", "Browsers", "firefox.com"),
    essential("steam", "Steam", "Games library and store.", "Gaming", "steampowered.com"),
    essential("discord", "Discord", "Voice and text chat.", "Gaming", "discord.com"),
    essential("epic", "Epic Games Launcher", "Games library and store.", "Gaming", "epicgames.com"),
    essential("sevenzip", "7-Zip", "Opens archives.", "Utilities", "7-zip.org"),
    essential("vlc", "VLC", "Plays any video.", "Utilities", "videolan.org"),
  ],
  get_runtimes: () => [
    { id: "vcredist", name: "Visual C++ Redistributable", description: "Most games need this.", group: "Microsoft", winget_id: "Demo.VC", installed: true },
    { id: "dotnet", name: ".NET Desktop Runtime", description: "Needed by many launchers.", group: "Microsoft", winget_id: "Demo.NET", installed: false },
    { id: "directx", name: "DirectX End-User Runtime", description: "Older games need this.", group: "Microsoft", winget_id: "Demo.DX", installed: null },
  ],
  install_runtime: () => ({ ok: true, message: "Demo only, nothing was installed." }),
  get_startup: () => [
    { name: "Discord", command: "C:\\Demo\\Discord.exe", location: "Current user", source: "hkcu", enabled: true, can_toggle: true },
    { name: "Steam", command: "C:\\Demo\\steam.exe -silent", location: "Current user", source: "hkcu", enabled: false, can_toggle: true },
    { name: "Graphics driver helper", command: "C:\\Demo\\helper.exe", location: "All users", source: "hklm", enabled: true, can_toggle: false },
  ],
  get_updates: () => [
    { name: "7-Zip", id: "7zip.7zip", current: "26.01", available: "26.05" },
    { name: "Discord", id: "Discord.Discord", current: "1.0.9266", available: "1.0.9270" },
    { name: "VLC media player", id: "VideoLAN.VLC", current: "3.0.21", available: "3.0.23" },
  ],
  upgrade_app: () => ({ ok: true, message: "Demo only, nothing was changed." }),
  upgrade_all: () => ({ ok: true, message: "Demo only, nothing was changed." }),
};

export async function invoke(cmd: string, args?: Record<string, unknown>): Promise<unknown> {
  if (cmd === "set_theme") settings.theme = String(args?.theme);
  if (cmd === "set_accent") settings.accent = String(args?.accent);
  if (cmd === "toggle_favorite") {
    const id = String(args?.appId);
    settings.favorites = settings.favorites.includes(id)
      ? settings.favorites.filter((x) => x !== id)
      : [...settings.favorites, id];
  }
  const settingsCommands = ["set_theme", "set_accent", "toggle_favorite", "launch_app", "launch_store"];
  if (settingsCommands.includes(cmd)) return settings;
  // Every other action resolves without doing anything: there is no PC behind the demo.
  await new Promise((r) => setTimeout(r, 220));
  return RESPONSES[cmd]?.();
}
