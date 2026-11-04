// Windows Toolkit: a temp-file cleaner that always scans before it cleans and
// asks for confirmation, plus one-click shortcuts to built-in Windows tools.

import { useState } from "react";
import {
  Trash2,
  Search,
  Loader2,
  HardDrive,
  Gauge,
  MonitorCog,
  Boxes,
  Info,
  Settings as SettingsIcon,
  Network,
  Trash,
  Folder,
  File as FileIcon,
  CornerLeftUp,
  PieChart,
  Shield,
  ShieldCheck,
} from "lucide-react";
import { api, fmtBytes } from "../api";
import type { CleanResult, FolderEntry, ScanResult } from "../types";
import { ErrorBanner, Loading } from "../components/ui";
import { Modal } from "../components/Modal";
import { useNexus } from "../state";

const TOOLS = [
  { id: "disk_cleanup", t: "Disk Cleanup", icon: HardDrive },
  { id: "task_manager", t: "Task Manager", icon: Gauge },
  { id: "device_manager", t: "Device Manager", icon: MonitorCog },
  { id: "storage_settings", t: "Storage", icon: Boxes },
  { id: "apps_features", t: "Apps & Features", icon: Boxes },
  { id: "network", t: "Network", icon: Network },
  { id: "system_info", t: "System Info", icon: Info },
  { id: "windows_settings", t: "Windows Settings", icon: SettingsIcon },
];

export function Toolkit() {
  const { openTool, elevated, restartAsAdmin, setNotice } = useNexus();
  const [scan, setScan] = useState<ScanResult | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [scanning, setScanning] = useState(false);
  const [cleaning, setCleaning] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [result, setResult] = useState<CleanResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Quick maintenance actions
  const [recycleBusy, setRecycleBusy] = useState(false);
  const [dnsBusy, setDnsBusy] = useState(false);
  const [confirmRecycle, setConfirmRecycle] = useState(false);

  // Disk analyzer
  const [diskPath, setDiskPath] = useState("C:\\");
  const [diskEntries, setDiskEntries] = useState<FolderEntry[]>([]);
  const [diskLoading, setDiskLoading] = useState(false);
  const [diskError, setDiskError] = useState<string | null>(null);
  const [analyzedPath, setAnalyzedPath] = useState<string | null>(null);

  const runScan = async () => {
    setScanning(true);
    setError(null);
    setResult(null);
    try {
      const r = await api.scanTemp();
      setScan(r);
      setSelected(new Set(r.locations.filter((l) => l.size_bytes > 0).map((l) => l.id)));
    } catch (e) {
      setError(String(e));
    } finally {
      setScanning(false);
    }
  };

  const toggle = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const selectedBytes =
    scan?.locations
      .filter((l) => selected.has(l.id))
      .reduce((sum, l) => sum + l.size_bytes, 0) ?? 0;

  const doClean = async () => {
    setConfirmOpen(false);
    setCleaning(true);
    setError(null);
    try {
      const r = await api.cleanTemp([...selected]);
      await runScan();
      setResult(r);
    } catch (e) {
      setError(String(e));
    } finally {
      setCleaning(false);
    }
  };

  // A friendly summary of what the clean actually did.
  const resultMessage = (r: CleanResult): string => {
    if (r.removed_files === 0 && r.skipped_files > 0) {
      return (
        `Nothing could be removed right now. ${r.skipped_files.toLocaleString()} ` +
        `files are in use or less than a day old, and were skipped to keep running ` +
        `programs safe. Close other apps and try again later to clear more.`
      );
    }
    let msg = `Freed ${fmtBytes(r.freed_bytes)} by removing ${r.removed_files.toLocaleString()} files.`;
    if (r.skipped_files > 0) {
      msg += ` ${r.skipped_files.toLocaleString()} files were in use or less than a day old, so they were kept.`;
    }
    return msg;
  };

  const doFlushDns = async () => {
    setDnsBusy(true);
    try {
      await api.flushDns();
      setNotice("DNS cache flushed.");
    } catch (e) {
      setNotice(`DNS flush failed: ${e}`);
    } finally {
      setDnsBusy(false);
    }
  };

  const doEmptyRecycle = async () => {
    setConfirmRecycle(false);
    setRecycleBusy(true);
    try {
      await api.emptyRecycleBin();
      setNotice("Recycle Bin emptied.");
    } catch (e) {
      setNotice(`${e}`);
    } finally {
      setRecycleBusy(false);
    }
  };

  const analyze = async (path: string) => {
    setDiskLoading(true);
    setDiskError(null);
    try {
      const r = await api.analyzeDisk(path);
      setDiskEntries(r);
      setDiskPath(path);
      setAnalyzedPath(path);
    } catch (e) {
      setDiskError(String(e));
      setDiskEntries([]);
    } finally {
      setDiskLoading(false);
    }
  };

  const goUp = () => {
    const trimmed = (analyzedPath ?? diskPath).replace(/[\\/]+$/, "");
    const idx = Math.max(trimmed.lastIndexOf("\\"), trimmed.lastIndexOf("/"));
    if (idx <= 2) {
      analyze(trimmed.slice(0, 2) + "\\");
    } else {
      analyze(trimmed.slice(0, idx));
    }
  };

  const diskMax = diskEntries.length ? diskEntries[0].size : 1;

  return (
    <div>
      <div className="page-head">
        <h1>Windows Toolkit</h1>
        <p>Clean up, free disk space, and reach common system tools.</p>
      </div>

      {error && <ErrorBanner message={error} />}

      {!elevated && (
        <div className="note-banner" style={{ justifyContent: "space-between" }}>
          <span style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <Shield size={16} />
            Some cleanups (Prefetch, Windows Update cache) need administrator rights.
            Without them, those files are skipped.
          </span>
          <button className="btn sm" onClick={restartAsAdmin} style={{ flexShrink: 0 }}>
            <ShieldCheck size={14} /> Restart as admin
          </button>
        </div>
      )}

      <div className="section-title">Temporary File Cleaner</div>
      <div className="card pad-lg">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 12, flexWrap: "wrap" }}>
          <div>
            <div style={{ fontWeight: 600, fontSize: 15 }}>
              {scan ? `${fmtBytes(scan.total_bytes)} found` : "Scan to see what can be freed"}
            </div>
            <div className="muted" style={{ fontSize: 12.5 }}>
              {scan
                ? `${scan.total_files.toLocaleString()} files across ${scan.locations.length} locations`
                : "Only known temp folders are scanned. Nothing is deleted until you confirm."}
            </div>
          </div>
          <button className="btn" onClick={runScan} disabled={scanning}>
            {scanning ? <Loader2 size={14} className="spin" /> : <Search size={14} />}
            {scanning ? "Scanning" : "Scan now"}
          </button>
        </div>

        {scan && (
          <div style={{ marginTop: 16 }}>
            {scan.locations.map((l) => (
              <div key={l.id} className="row">
                <label className="checkbox" style={{ minWidth: 0 }}>
                  <input
                    type="checkbox"
                    checked={selected.has(l.id)}
                    disabled={l.size_bytes === 0}
                    onChange={() => toggle(l.id)}
                  />
                  <span className="meta">
                    <span className="name">{l.label}</span>
                    <span className="path">{l.path}</span>
                  </span>
                </label>
                <span className="amount">
                  {fmtBytes(l.size_bytes)} · {l.file_count.toLocaleString()} files
                </span>
              </div>
            ))}

            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginTop: 16 }}>
              <div className="muted" style={{ fontSize: 13 }}>
                Selected: <span className="mono">{fmtBytes(selectedBytes)}</span>
              </div>
              <button
                className="btn primary"
                onClick={() => setConfirmOpen(true)}
                disabled={cleaning || selected.size === 0}
              >
                {cleaning ? <Loader2 size={14} className="spin" /> : <Trash2 size={14} />}
                {cleaning ? "Cleaning" : "Clean selected"}
              </button>
            </div>

            {result && (
              <div className={`result-banner ${result.removed_files === 0 ? "warn" : ""}`}>
                {resultMessage(result)}
              </div>
            )}
          </div>
        )}
      </div>

      <div className="section-title">Quick Maintenance</div>
      <div className="grid cols-4">
        <button
          className="quick tool-card interactive"
          onClick={() => setConfirmRecycle(true)}
          disabled={recycleBusy}
        >
          <div className="chip">
            {recycleBusy ? <Loader2 size={18} className="spin" /> : <Trash2 size={18} />}
          </div>
          <div className="t">Empty Recycle Bin</div>
        </button>
        <button className="quick tool-card interactive" onClick={doFlushDns} disabled={dnsBusy}>
          <div className="chip">
            {dnsBusy ? <Loader2 size={18} className="spin" /> : <Network size={18} />}
          </div>
          <div className="t">Flush DNS Cache</div>
        </button>
      </div>

      <div className="section-title">Disk Space Analyzer</div>
      <div className="card pad-lg">
        <div style={{ display: "flex", gap: 9, marginBottom: 14 }}>
          <button className="btn icon" onClick={goUp} disabled={diskLoading} title="Up one folder">
            <CornerLeftUp size={16} />
          </button>
          <input
            type="text"
            value={diskPath}
            onChange={(e) => setDiskPath(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && !diskLoading && analyze(diskPath)}
            placeholder="C:\"
            style={{ flex: 1 }}
          />
          <button className="btn" onClick={() => analyze(diskPath)} disabled={diskLoading}>
            {diskLoading ? <Loader2 size={15} className="spin" /> : <PieChart size={15} />}
            Analyze
          </button>
        </div>

        {diskError && <ErrorBanner message={diskError} />}

        {diskLoading ? (
          <Loading label="Measuring folder sizes (this can take a while)" />
        ) : diskEntries.length > 0 ? (
          <div>
            <div className="muted" style={{ fontSize: "var(--text-meta)", marginBottom: 10 }}>
              Largest items in {analyzedPath}. Click a folder to go deeper.
            </div>
            {diskEntries.map((e) => (
              <div className="row" key={e.path}>
                <button
                  className="meta"
                  onClick={() => e.is_dir && analyze(e.path)}
                  disabled={!e.is_dir}
                  style={{
                    display: "flex",
                    flexDirection: "row",
                    alignItems: "center",
                    gap: 10,
                    background: "transparent",
                    border: "none",
                    color: "var(--fg)",
                    cursor: e.is_dir ? "pointer" : "default",
                    textAlign: "left",
                    minWidth: 0,
                    flex: 1,
                  }}
                >
                  {e.is_dir ? (
                    <Folder size={16} style={{ color: "var(--accent)", flexShrink: 0 }} />
                  ) : (
                    <FileIcon size={16} style={{ color: "var(--fg-faint)", flexShrink: 0 }} />
                  )}
                  <span
                    style={{
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                      fontSize: "var(--text-label)",
                      fontWeight: e.is_dir ? 600 : 400,
                    }}
                  >
                    {e.name}
                  </span>
                </button>
                <div style={{ display: "flex", alignItems: "center", gap: 12, width: "42%" }}>
                  <div className="meter" style={{ flex: 1, marginTop: 0 }}>
                    <span style={{ width: `${(e.size / diskMax) * 100}%` }} />
                  </div>
                  <span className="amount" style={{ minWidth: 72, textAlign: "right" }}>
                    {fmtBytes(e.size)}
                  </span>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="muted" style={{ fontSize: "var(--text-label)" }}>
            Enter a folder (like C:\ or C:\Users) and click Analyze to see what is using space.
          </div>
        )}
      </div>

      <div className="section-title">System Tools</div>
      <div className="grid cols-4">
        {TOOLS.map((t) => {
          const Icon = t.icon;
          return (
            <button key={t.id} className="quick tool-card interactive" onClick={() => openTool(t.id)}>
              <div className="chip">
                <Icon size={18} />
              </div>
              <div className="t">{t.t}</div>
            </button>
          );
        })}
      </div>

      {confirmOpen && (
        <Modal
          title="Clean temporary files?"
          danger
          icon={<Trash size={20} />}
          onClose={() => setConfirmOpen(false)}
          footer={
            <>
              <button className="btn ghost" onClick={() => setConfirmOpen(false)}>
                Cancel
              </button>
              <button className="btn primary" onClick={doClean}>
                <Trash2 size={15} /> Delete {fmtBytes(selectedBytes)}
              </button>
            </>
          }
        >
          <p>
            This removes temporary files from {selected.size} selected location
            {selected.size === 1 ? "" : "s"}, freeing about{" "}
            <strong>{fmtBytes(selectedBytes)}</strong>. Files that are currently in
            use are skipped automatically. This cannot be undone.
          </p>
        </Modal>
      )}

      {confirmRecycle && (
        <Modal
          title="Empty the Recycle Bin?"
          danger
          icon={<Trash size={20} />}
          onClose={() => setConfirmRecycle(false)}
          footer={
            <>
              <button className="btn ghost" onClick={() => setConfirmRecycle(false)}>
                Cancel
              </button>
              <button className="btn primary" onClick={doEmptyRecycle}>
                <Trash2 size={15} /> Empty it
              </button>
            </>
          }
        >
          <p>
            This permanently deletes everything in the Recycle Bin on all drives. This
            cannot be undone.
          </p>
        </Modal>
      )}
    </div>
  );
}
