// Central app state shared across pages: detected apps, custom apps, hardware,
// settings, real executable icons, and the actions that mutate them.

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type { ReactNode } from "react";
import { api, pickExecutable } from "./api";
import type { CustomApp, DetectedApp, HardwareInfo, Settings } from "./types";

export interface UnifiedApp {
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
  custom: boolean;
}

// A long install or update. Lives here, not in a page, so it keeps its progress
// when you change pages and a second one can't start on top of it.
export interface Job {
  id: string;
  label: string;
}

interface NexusState {
  settings: Settings | null;
  apps: UnifiedApp[];
  hardware: HardwareInfo | null;
  icons: Record<string, string>;
  elevated: boolean;
  loading: boolean;
  error: string | null;
  notice: string | null;
  setNotice: (s: string | null) => void;
  restartAsAdmin: () => Promise<void>;
  job: Job | null;
  runJob: (job: Job, work: (update: (label: string) => void) => Promise<void>) => Promise<void>;
  setupSelection: Set<string>;
  setSetupSelection: (next: Set<string>) => void;
  iconFor: (app: UnifiedApp) => string | undefined;
  refresh: () => Promise<void>;
  launch: (app: UnifiedApp) => Promise<void>;
  locateAndLaunch: (app: UnifiedApp) => Promise<void>;
  toggleFavorite: (id: string) => Promise<void>;
  setTheme: (theme: string) => Promise<void>;
  setAccent: (accent: string) => Promise<void>;
  addCustomApp: (app: CustomApp) => Promise<void>;
  removeCustomApp: (id: string) => Promise<void>;
  openUrl: (url: string) => Promise<void>;
  openTool: (id: string) => Promise<void>;
}

const Ctx = createContext<NexusState | null>(null);

function toUnified(detected: DetectedApp[], custom: CustomApp[]): UnifiedApp[] {
  const a: UnifiedApp[] = detected.map((d) => ({
    id: d.id,
    name: d.name,
    vendor: d.vendor,
    category: d.category,
    description: d.description,
    download_url: d.download_url,
    compatibility: d.compatibility,
    installed: d.installed,
    launch_path: d.launch_path,
    launch_aumid: d.launch_aumid,
    custom: false,
  }));
  const c: UnifiedApp[] = custom.map((m) => ({
    id: m.id,
    name: m.name,
    vendor: "Custom entry",
    category: m.category || "custom",
    description: m.description,
    download_url: m.download_url,
    compatibility: "Added manually by you.",
    installed: true,
    launch_path: m.exec_path,
    launch_aumid: null,
    custom: true,
  }));
  return [...a, ...c];
}

function pathsOf(detected: DetectedApp[], custom: CustomApp[]): string[] {
  const set = new Set<string>();
  detected.forEach((d) => d.launch_path && set.add(d.launch_path));
  custom.forEach((m) => m.exec_path && set.add(m.exec_path));
  return [...set];
}

export function NexusProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [detected, setDetected] = useState<DetectedApp[]>([]);
  const [hardware, setHardware] = useState<HardwareInfo | null>(null);
  const [icons, setIcons] = useState<Record<string, string>>({});
  const [elevated, setElevated] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const noticeTimer = useRef<number | null>(null);
  const [job, setJob] = useState<Job | null>(null);
  const jobRunning = useRef(false);
  const [setupSelection, setSetupSelection] = useState<Set<string>>(new Set());

  const flashNotice = useCallback((s: string | null) => {
    setNotice(s);
    if (noticeTimer.current) window.clearTimeout(noticeTimer.current);
    if (s) noticeTimer.current = window.setTimeout(() => setNotice(null), 3400);
  }, []);

  useEffect(() => {
    const t = settings?.theme === "dark" ? "dark" : "bright";
    document.documentElement.setAttribute("data-theme", t);
  }, [settings?.theme]);

  useEffect(() => {
    document.documentElement.setAttribute("data-accent", settings?.accent ?? "blue");
  }, [settings?.accent]);

  const loadIcons = useCallback(async (paths: string[]) => {
    if (paths.length === 0) return;
    try {
      const map = await api.getAppIcons(paths);
      setIcons((prev) => ({ ...prev, ...map }));
    } catch {
      // Icons are best-effort; a failure just means fallbacks are used.
    }
  }, []);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      // Settings first, so the saved theme applies before the slower scans finish.
      const s = await api.getSettings();
      setSettings(s);
      const [apps, hw, elev] = await Promise.all([
        api.getCatalogApps(),
        api.getHardware(),
        api.isElevated(),
      ]);
      setDetected(apps);
      setHardware(hw);
      setElevated(elev);
      setLoading(false);
      // Icons load in the background so the UI appears immediately.
      loadIcons(pathsOf(apps, s.custom_apps));
    } catch (e) {
      setError(String(e));
      setLoading(false);
    }
  }, [loadIcons]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const apps = useMemo(
    () => toUnified(detected, settings?.custom_apps ?? []),
    [detected, settings]
  );

  const iconFor = useCallback(
    (app: UnifiedApp) => (app.launch_path ? icons[app.launch_path] : undefined),
    [icons]
  );

  const launch = useCallback(
    async (app: UnifiedApp) => {
      try {
        let s: Settings | null = null;
        if (app.launch_path) {
          s = await api.launchApp(app.launch_path, app.id);
        } else if (app.launch_aumid) {
          s = await api.launchStore(app.launch_aumid, app.id);
        } else {
          return;
        }
        setSettings(s);
        flashNotice(`Launched ${app.name}.`);
      } catch (e) {
        flashNotice(`Could not launch ${app.name}: ${e}`);
      }
    },
    [flashNotice]
  );

  // Used when an app is installed but we could not resolve its executable: the
  // user points us at the .exe, we save it, then launch.
  const locateAndLaunch = useCallback(
    async (app: UnifiedApp) => {
      try {
        const path = await pickExecutable();
        if (!path) return;
        const s = await api.setLaunchOverride(app.id, path);
        setSettings(s);
        const refreshed = await api.getCatalogApps();
        setDetected(refreshed);
        loadIcons([path]);
        const s2 = await api.launchApp(path, app.id);
        setSettings(s2);
        flashNotice(`Saved path and launched ${app.name}.`);
      } catch (e) {
        flashNotice(`Could not set path for ${app.name}: ${e}`);
      }
    },
    [flashNotice, loadIcons]
  );

  const toggleFavorite = useCallback(async (id: string) => {
    setSettings(await api.toggleFavorite(id));
  }, []);

  const setTheme = useCallback(async (theme: string) => {
    setSettings(await api.setTheme(theme));
  }, []);

  const setAccent = useCallback(async (accent: string) => {
    setSettings(await api.setAccent(accent));
  }, []);

  const addCustomApp = useCallback(
    async (app: CustomApp) => {
      const s = await api.addCustomApp(app);
      setSettings(s);
      loadIcons([app.exec_path]);
      flashNotice(`Saved ${app.name}.`);
    },
    [flashNotice, loadIcons]
  );

  const removeCustomApp = useCallback(
    async (id: string) => {
      setSettings(await api.removeCustomApp(id));
      flashNotice("Removed custom app.");
    },
    [flashNotice]
  );

  const openUrl = useCallback(
    async (url: string) => {
      try {
        await api.openUrl(url);
      } catch (e) {
        flashNotice(`Could not open link: ${e}`);
      }
    },
    [flashNotice]
  );

  const openTool = useCallback(
    async (id: string) => {
      try {
        await api.openTool(id);
        flashNotice("Opening Windows tool.");
      } catch (e) {
        flashNotice(`Could not open tool: ${e}`);
      }
    },
    [flashNotice]
  );

  const restartAsAdmin = useCallback(async () => {
    try {
      await api.restartAsAdmin();
    } catch (e) {
      flashNotice(`Could not restart as administrator: ${e}`);
    }
  }, [flashNotice]);

  const runJob = useCallback<NexusState["runJob"]>(async (next, work) => {
    if (jobRunning.current) return;
    jobRunning.current = true;
    setJob(next);
    try {
      await work((label) => setJob({ id: next.id, label }));
    } finally {
      jobRunning.current = false;
      setJob(null);
    }
  }, []);

  const value: NexusState = {
    settings,
    apps,
    hardware,
    icons,
    elevated,
    loading,
    error,
    notice,
    setNotice: flashNotice,
    restartAsAdmin,
    job,
    runJob,
    setupSelection,
    setSetupSelection,
    iconFor,
    refresh,
    launch,
    locateAndLaunch,
    toggleFavorite,
    setTheme,
    setAccent,
    addCustomApp,
    removeCustomApp,
    openUrl,
    openTool,
  };

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useNexus(): NexusState {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useNexus must be used inside NexusProvider");
  return ctx;
}
