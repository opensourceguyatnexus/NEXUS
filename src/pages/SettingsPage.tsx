// Settings: appearance (theme + accent), startup behavior, detection, the full
// detected-hardware list, and an honest note about what this build supports.

import { useEffect, useState } from "react";
import { RefreshCw, Moon, Sun, Check, ShieldCheck, Shield } from "lucide-react";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import { useNexus } from "../state";
import type { HardwareDevice } from "../types";

const ACCENTS: { id: string; label: string; swatch: string }[] = [
  { id: "blue", label: "Blue", swatch: "#1668c7" },
  { id: "green", label: "Green", swatch: "#2f7d12" },
  { id: "orange", label: "Orange", swatch: "#b5540b" },
  { id: "rose", label: "Rose", swatch: "#c41d5c" },
];

function HwRow({ d }: { d: HardwareDevice }) {
  return (
    <div className="row">
      <span className="meta">
        <span className="name">{d.name}</span>
        {d.detail && <span className="path">{d.detail}</span>}
      </span>
      <span className="badge">{d.kind}</span>
    </div>
  );
}

export function SettingsPage() {
  const { settings, hardware, refresh, setTheme, setAccent, loading, setNotice, elevated, restartAsAdmin, job } =
    useNexus();
  const isDark = settings?.theme === "dark";
  const currentAccent = settings?.accent ?? "blue";

  const [autostart, setAutostart] = useState<boolean | null>(null);

  useEffect(() => {
    isEnabled()
      .then(setAutostart)
      .catch(() => setAutostart(null));
  }, []);

  const toggleAutostart = async () => {
    try {
      if (autostart) {
        await disable();
        setAutostart(false);
        setNotice("NEXUS will no longer start with Windows.");
      } else {
        await enable();
        setAutostart(true);
        setNotice("NEXUS will start with Windows.");
      }
    } catch (e) {
      setNotice(`Could not change startup setting: ${e}`);
    }
  };

  const devices: HardwareDevice[] = [];
  if (hardware?.os) devices.push(hardware.os);
  if (hardware?.cpu) devices.push(hardware.cpu);
  hardware?.gpus.forEach((g) => devices.push(g));
  if (hardware?.memory) devices.push(hardware.memory);
  hardware?.disks.forEach((d) => devices.push(d));
  hardware?.peripherals.forEach((p) => devices.push(p));

  return (
    <div>
      <div className="page-head">
        <h1>Settings</h1>
        <p>Appearance, startup, detection, and what this build supports.</p>
      </div>

      <div className="section-title">Appearance</div>
      <div className="card pad-lg">
        <div className="field">
          <label>Theme</label>
          <div className="btn-row">
            <button className={`btn ${!isDark ? "primary" : ""}`} onClick={() => setTheme("bright")}>
              <Sun size={15} /> Bright
            </button>
            <button className={`btn ${isDark ? "primary" : ""}`} onClick={() => setTheme("dark")}>
              <Moon size={15} /> Dark
            </button>
          </div>
        </div>
        <div className="field" style={{ marginBottom: 0 }}>
          <label>Accent color</label>
          <div style={{ display: "flex", gap: 12, flexWrap: "wrap" }}>
            {ACCENTS.map((a) => (
              <button
                key={a.id}
                className="swatch"
                title={a.label}
                onClick={() => setAccent(a.id)}
                style={{ background: a.swatch }}
                aria-label={a.label}
              >
                {currentAccent === a.id && <Check size={16} color="#fff" strokeWidth={3} />}
              </button>
            ))}
          </div>
        </div>
      </div>

      <div className="section-title">Startup</div>
      <div className="card pad-lg">
        <div className="spread">
          <div>
            <div style={{ fontWeight: 600 }}>Start NEXUS with Windows</div>
            <div className="muted" style={{ fontSize: "var(--text-label)" }}>
              Launch automatically when you sign in. Closing the window keeps NEXUS in
              the system tray; use the tray icon to reopen or quit.
            </div>
          </div>
          <label className="switch" title="Start with Windows">
            <input
              type="checkbox"
              checked={!!autostart}
              disabled={autostart === null}
              onChange={toggleAutostart}
            />
            <span className="track" />
          </label>
        </div>
      </div>

      <div className="section-title">Permissions</div>
      <div className="card pad-lg">
        <div className="spread">
          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <div
              className="chip"
              style={{
                width: 36,
                height: 36,
                borderRadius: "var(--radius-sm)",
                display: "grid",
                placeItems: "center",
                background: elevated ? "var(--accent-soft)" : "var(--surface-2)",
                color: elevated ? "var(--accent-text)" : "var(--fg-faint)",
              }}
            >
              {elevated ? <ShieldCheck size={18} /> : <Shield size={18} />}
            </div>
            <div>
              <div style={{ fontWeight: 600 }}>
                {elevated ? "Running as administrator" : "Running as standard user"}
              </div>
              <div className="muted" style={{ fontSize: "var(--text-label)" }}>
                {elevated
                  ? "Protected cleanups and system-wide startup entries are available."
                  : "Restart as admin to clean protected folders (Prefetch, Update cache) and toggle system-wide startup."}
              </div>
            </div>
          </div>
          {!elevated && (
            <button
              className="btn primary"
              onClick={restartAsAdmin}
              disabled={job !== null}
              title={job ? "Wait for the current install or update to finish" : undefined}
            >
              <ShieldCheck size={15} /> Restart as administrator
            </button>
          )}
        </div>
      </div>

      <div className="section-title">Detection</div>
      <div className="card pad-lg">
        <div className="spread">
          <div className="muted" style={{ fontSize: "var(--text-label)" }}>
            Re-read hardware and re-check which apps are installed.
          </div>
          <button className="btn" onClick={refresh} disabled={loading}>
            <RefreshCw size={14} className={loading ? "spin" : ""} /> Rescan
          </button>
        </div>
      </div>

      <div className="section-title">Detected hardware</div>
      {devices.length ? (
        <div>
          {devices.map((d, i) => (
            <HwRow key={`${d.kind}-${i}`} d={d} />
          ))}
        </div>
      ) : (
        <div className="card muted">No hardware details available.</div>
      )}

      <div className="section-title">About this build</div>
      <div className="card pad-lg muted" style={{ fontSize: "var(--text-label)", lineHeight: 1.7 }}>
        NEXUS has no telemetry and no account. Saved game logins stay on this PC. App
        logos are fetched from Google's favicon service using the app's website name,
        so those requests leave your PC. Live GPU stats use NVML on NVIDIA cards. CPU temperature depends on
        sensors Windows may not expose without extra drivers. App detection relies on
        the Windows registry and known paths, so an unusual install can show as "not
        installed" until you locate it or add it as a custom app.
      </div>
    </div>
  );
}
