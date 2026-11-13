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
  lastJobResult: string | null;
  clearJobResult: () => void;
  runJob: (job: Job, work: (update: (label: string) => void) => Promise<void>) => Promise<void>;
  setupSelection: Set<string>;
  setSetupSelection: (next: Set<string>) => void;
  iconFor: (app: UnifiedApp) => string | undefined;
  refresh: () => Promise<void>;
  rescanApps: () => Promise<void>;
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

export function isFailure(message: string): boolean {
  return /fail|could not|cannot|can't|error/i.test(message);
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
  const jobNotice = useRef<string | null>(null);
  const [lastJobResult, setLastJobResult] = useState<string | null>(null);
  const [setupSelection, setSetupSelection] = useState<Set<string>>(new Set());

  const flashNotice = useCallback((s: string | null, fromPage = false) => {
    setNotice(s);
    if (noticeTimer.current) window.clearTimeout(noticeTimer.current);
    noticeTimer.current = null;
    if (!s) return;
    // A message a page sends while a long job runs is its result: it is kept
    // in the banner after the job ends, so the toast itself can fade.
    if (fromPage && jobRunning.current) jobNotice.current = s;
    else if (isFailure(s)) return;
    const ms = Math.min(15000, Math.max(3400, s.length * 70));
    noticeTimer.current = window.setTimeout(() => setNotice(null), ms);
  }, []);

  const pageNotice = useCallback((s: string | null) => flashNotice(s, true), [flashNotice]);

  const clearJobResult = useCallback(() => setLastJobResult(null), []);

  // Until settings load, keep whatever index.html restored from the last run.
  useEffect(() => {
    if (!settings) return;
    const t = settings.theme === "dark" ? "dark" : "bright";
    document.documentElement.setAttribute("data-theme", t);
    try {
      localStorage.setItem("nexus-theme", t);
    } catch {}
  }, [settings?.theme]);

  useEffect(() => {
    if (!settings) return;
    const a = settings.accent || "blue";
    document.documentElement.setAttribute("data-accent", a);
    try {
      localStorage.setItem("nexus-accent", a);
    } catch {}
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

  const rescanApps = useCallback(async () => {
    const found = await api.getCatalogApps();
    setDetected(found);
    loadIcons(pathsOf(found, settings?.custom_apps ?? []));
  }, [loadIcons, settings]);

  const toggleFavorite = useCallback(
    async (id: string) => {
      try {
        setSettings(await api.toggleFavorite(id));
      } catch (e) {
        flashNotice(`Could not save: ${e}`);
      }
    },
    [flashNotice]
  );

  const setTheme = useCallback(
    async (theme: string) => {
      try {
        setSettings(await api.setTheme(theme));
      } catch (e) {
        flashNotice(`Could not save: ${e}`);
      }
    },
    [flashNotice]
  );

  const setAccent = useCallback(
    async (accent: string) => {
      try {
        setSettings(await api.setAccent(accent));
      } catch (e) {
        flashNotice(`Could not save: ${e}`);
      }
    },
    [flashNotice]
  );

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
      try {
        setSettings(await api.removeCustomApp(id));
        flashNotice("Removed custom app.");
      } catch (e) {
        flashNotice(`Could not save: ${e}`);
      }
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
    if (jobRunning.current) {
      flashNotice("Wait for the current install or update to finish before restarting.");
      return;
    }
    try {
      await api.restartAsAdmin();
    } catch (e) {
      flashNotice(`Could not restart as administrator: ${e}`);
    }
  }, [flashNotice]);

  const runJob = useCallback<NexusState["runJob"]>(async (next, work) => {
    if (jobRunning.current) return;
    jobRunning.current = true;
    jobNotice.current = null;
    setLastJobResult(null);
    setJob(next);
    try {
      await work((label) => setJob({ id: next.id, label }));
    } finally {
      jobRunning.current = false;
      setJob(null);
      setLastJobResult(jobNotice.current);
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
    setNotice: pageNotice,
    restartAsAdmin,
    job,
    lastJobResult,
    clearJobResult,
    runJob,
    setupSelection,
    setSetupSelection,
    iconFor,
    refresh,
    rescanApps,
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
