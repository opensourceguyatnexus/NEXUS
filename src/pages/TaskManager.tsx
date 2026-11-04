// Task Manager: see what is running and stop it. Processes are grouped by name
// (so "End" stops every chrome.exe at once) and refresh every 3 seconds while
// the tab is open. Services can be stopped or started; most need admin rights.
// Core Windows processes and services are protected in the backend.

import { useEffect, useMemo, useState } from "react";
import { Search, Square, Play, ShieldAlert, RefreshCw, Lock, ListTree, Cog } from "lucide-react";
import { api, fmtBytes } from "../api";
import type { ServiceInfo, TaskProc } from "../types";
import { useNexus } from "../state";
import { Loading, ErrorBanner, EmptyState } from "../components/ui";
import { Modal } from "../components/Modal";

type Tab = "processes" | "services";

interface ProcGroup {
  name: string;
  pids: number[];
  cpu: number;
  mem: number;
  protected: boolean;
}

type Pending =
  | { kind: "process"; group: ProcGroup }
  | { kind: "service"; svc: ServiceInfo; action: "stop" | "start" };

export function TaskManager() {
  const { setNotice, elevated, restartAsAdmin } = useNexus();
  const [tab, setTab] = useState<Tab>("processes");
  const [q, setQ] = useState("");
  const [procs, setProcs] = useState<TaskProc[] | null>(null);
  const [services, setServices] = useState<ServiceInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState<Pending | null>(null);
  const [busy, setBusy] = useState(false);

  const loadProcs = async (isCurrent: () => boolean = () => true) => {
    try {
      const p = await api.listProcesses();
      if (isCurrent()) {
        setProcs(p);
        setError(null);
      }
    } catch (e) {
      if (isCurrent()) setError(String(e));
    }
  };

  const loadServices = async (isCurrent: () => boolean = () => true) => {
    try {
      const s = await api.listServices();
      if (isCurrent()) {
        setServices(s);
        setError(null);
      }
    } catch (e) {
      if (isCurrent()) setError(String(e));
    }
  };

  // Processes poll every 3 seconds while their tab is open.
  useEffect(() => {
    // Belongs to this effect alone, so an old loop can never come back to life.
    let cancelled = false;
    const isCurrent = () => !cancelled;
    let timer: number;
    setError(null);
    if (tab === "processes") {
      const tick = async () => {
        await loadProcs(isCurrent);
        if (!cancelled) timer = window.setTimeout(tick, 3000);
      };
      tick();
    } else if (!services) {
      loadServices(isCurrent);
    }
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [tab]);

  const groups = useMemo(() => {
    const map = new Map<string, ProcGroup>();
    (procs ?? []).forEach((p) => {
      const key = p.name.toLowerCase();
      const g = map.get(key) ?? { name: p.name, pids: [], cpu: 0, mem: 0, protected: false };
      g.pids.push(p.pid);
      g.cpu += p.cpu;
      g.mem += p.mem;
      g.protected = g.protected || p.protected;
      map.set(key, g);
    });
    const needle = q.trim().toLowerCase();
    return [...map.values()]
      .filter((g) => !needle || g.name.toLowerCase().includes(needle))
      .sort((a, b) => b.mem - a.mem);
  }, [procs, q]);

  const shownServices = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return (services ?? []).filter(
      (s) => !needle || `${s.display_name} ${s.name}`.toLowerCase().includes(needle),
    );
  }, [services, q]);

  const confirm = async () => {
    if (!pending) return;
    setBusy(true);
    try {
      if (pending.kind === "process") {
        const g = pending.group;
        const results = await Promise.allSettled(g.pids.map((pid) => api.endProcess(pid)));
        const failed = results.filter((r): r is PromiseRejectedResult => r.status === "rejected");
        if (failed.length === 0) setNotice(`Ended ${g.name}.`);
        else if (failed.length === results.length) setNotice(`Could not end ${g.name}: ${failed[0].reason}`);
        else setNotice(`Ended ${results.length - failed.length} of ${results.length} ${g.name} processes.`);
        await loadProcs();
      } else {
        const { svc, action } = pending;
        if (action === "stop") await api.stopService(svc.name);
        else await api.startService(svc.name);
        setNotice(`${action === "stop" ? "Stopping" : "Starting"} ${svc.display_name}.`);
        // Services take a moment to change state.
        window.setTimeout(() => loadServices(), 1500);
      }
    } catch (e) {
      setNotice(`${e}`);
    } finally {
      setBusy(false);
      setPending(null);
    }
  };

  const loading = tab === "processes" ? !procs : !services;

  return (
    <div>
      <div className="page-head spread">
        <div>
          <h1>Task Manager</h1>
          <p>See what is running and stop apps or services you do not need.</p>
        </div>
        <button
          className="btn"
          onClick={() => (tab === "processes" ? loadProcs() : loadServices())}
        >
          <RefreshCw size={15} /> Refresh
        </button>
      </div>

      {error && <ErrorBanner message={error} />}

      {!elevated && (
        <div className="note-banner">
          <ShieldAlert size={16} />
          <span style={{ flex: 1 }}>
            Not running as administrator. You can end your own apps, but stopping most
            services and system-level processes needs admin rights.
          </span>
          <button className="btn sm" onClick={restartAsAdmin} style={{ flexShrink: 0 }}>
            Restart as admin
          </button>
        </div>
      )}

      <div className="filter-tabs">
        <button
          className={`chip-btn ${tab === "processes" ? "active" : ""}`}
          onClick={() => setTab("processes")}
        >
          <ListTree size={14} /> Processes
        </button>
        <button
          className={`chip-btn ${tab === "services" ? "active" : ""}`}
          onClick={() => setTab("services")}
        >
          <Cog size={14} /> Services
        </button>
      </div>

      <div className="search">
        <Search className="ic" size={16} />
        <input
          type="text"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder={tab === "processes" ? "Search processes" : "Search services"}
        />
      </div>

      {loading && !error ? (
        <Loading label={tab === "processes" ? "Reading processes" : "Reading services"} />
      ) : tab === "processes" ? (
        groups.length === 0 ? (
          <EmptyState title="Nothing matches" hint="Try a different search." />
        ) : (
          <div>
            {groups.map((g) => (
              <div className="row" key={g.name.toLowerCase()}>
                <span className="meta">
                  <span className="name">
                    {g.name}
                    {g.pids.length > 1 && <span className="muted"> ({g.pids.length})</span>}
                  </span>
                  <span className="path">
                    {fmtBytes(g.mem)} · CPU {g.cpu.toFixed(1)}% · PID {g.pids.slice(0, 3).join(", ")}
                    {g.pids.length > 3 ? "…" : ""}
                  </span>
                </span>
                {g.protected ? (
                  <span className="badge off" title="Core Windows process">
                    <Lock size={12} /> System
                  </span>
                ) : (
                  <button className="btn danger sm" onClick={() => setPending({ kind: "process", group: g })}>
                    <Square size={13} /> End
                  </button>
                )}
              </div>
            ))}
          </div>
        )
      ) : shownServices.length === 0 ? (
        <EmptyState title="Nothing matches" hint="Try a different search." />
      ) : (
        <div>
          {shownServices.map((s) => (
            <div className="row" key={s.name}>
              <span className="meta">
                <span className="name">{s.display_name}</span>
                <span className="path">
                  {s.name} · {s.start_mode}
                  {s.pid ? ` · PID ${s.pid}` : ""}
                </span>
              </span>
              <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
                <span className={`badge ${s.state === "Running" ? "ok" : "off"}`}>
                  <span className="dot" /> {s.state}
                </span>
                {s.can_stop ? (
                  <button
                    className="btn danger sm"
                    onClick={() => setPending({ kind: "service", svc: s, action: "stop" })}
                  >
                    <Square size={13} /> Stop
                  </button>
                ) : s.can_start ? (
                  <button
                    className="btn sm"
                    onClick={() => setPending({ kind: "service", svc: s, action: "start" })}
                  >
                    <Play size={13} /> Start
                  </button>
                ) : (
                  <span className="badge off" title="Windows does not allow this service to be stopped here">
                    <Lock size={12} /> Locked
                  </span>
                )}
              </div>
            </div>
          ))}
        </div>
      )}

      {pending && (
        <Modal
          title={
            pending.kind === "process"
              ? `End ${pending.group.name}?`
              : `${pending.action === "stop" ? "Stop" : "Start"} ${pending.svc.display_name}?`
          }
          icon={pending.kind === "service" && pending.action === "start" ? <Play size={20} /> : <Square size={20} />}
          onClose={() => !busy && setPending(null)}
          footer={
            <>
              <button className="btn ghost" onClick={() => setPending(null)} disabled={busy}>
                Cancel
              </button>
              <button
                className={`btn ${pending.kind === "service" && pending.action === "start" ? "primary" : "danger"}`}
                onClick={confirm}
                disabled={busy}
              >
                {pending.kind === "process"
                  ? "End task"
                  : pending.action === "stop"
                    ? "Stop service"
                    : "Start service"}
              </button>
            </>
          }
        >
          <p>
            {pending.kind === "process"
              ? `This closes ${pending.group.pids.length > 1 ? `all ${pending.group.pids.length} ` : ""}${pending.group.name} right away. Unsaved work in it will be lost.`
              : pending.action === "stop"
                ? "Anything that relies on this service may stop working until it is started again or you restart the PC."
                : "Starts the service now. Its startup type is not changed."}
          </p>
        </Modal>
      )}
    </div>
  );
}
