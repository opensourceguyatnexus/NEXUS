// Dashboard: the live board, system overview, quick actions, favorites, and recent apps.

import {
  Trash2,
  ListChecks,
  Gauge,
  MonitorCog,
  Settings as SettingsIcon,
  Cpu as CpuIcon,
  AppWindow,
  Info,
  ChevronRight,
  MemoryStick,
  Boxes,
  Star,
} from "lucide-react";
import type { ComponentType } from "react";
import { useEffect, useRef, useState } from "react";
import type { PageId } from "../App";
import { useNexus } from "../state";
import { api, fmtBytes } from "../api";
import type { MonitorSnapshot, ProcInfo } from "../types";
import { AppCard } from "../components/AppCard";
import { Loading, EmptyState, AppLogo, Meter } from "../components/ui";
import { LogoMark } from "../components/LogoMark";

function Stat({
  icon: Icon,
  label,
  value,
  sub,
}: {
  icon: ComponentType<{ size?: number }>;
  label: string;
  value: string;
  sub?: string;
}) {
  return (
    <div className="stat">
      <div className="head">
        <div className="chip">
          <Icon size={17} />
        </div>
        <div className="label">{label}</div>
      </div>
      <div className="value">{value}</div>
      {sub && <div className="sub">{sub}</div>}
    </div>
  );
}

// One reading on the board. Its wire to the NEXUS chip brightens with the load.
function BoardChip({
  side,
  label,
  pct,
  unit = "%",
  detail,
}: {
  side: "left" | "right";
  label: string;
  pct: number | null;
  unit?: string;
  detail?: string;
}) {
  return (
    <div className={`board-chip ${side}`}>
      <div className="top">
        <span className="k">{label}</span>
        <span className="v">
          {pct != null ? pct.toFixed(0) : "--"}
          <small>{unit}</small>
        </span>
      </div>
      <Meter pct={pct ?? 0} warn={(pct ?? 0) > 90} />
      <div className="d" title={detail}>
        {detail || "\u00a0"}
      </div>
    </div>
  );
}

const wire = (pct: number | null) => 0.3 + 0.7 * Math.min(1, (pct ?? 0) / 100);

export function Dashboard({ navigate }: { navigate: (p: PageId) => void }) {
  const { apps, hardware, settings, loading, launch, openTool, iconFor } = useNexus();

  // Live performance, polled while the Dashboard is open.
  const [snap, setSnap] = useState<MonitorSnapshot | null>(null);
  const [procs, setProcs] = useState<ProcInfo[]>([]);
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    let timer: number;
    const tick = async () => {
      try {
        const [s, p] = await Promise.all([api.getMonitor(), api.getProcesses()]);
        if (alive.current) {
          setSnap(s);
          setProcs(p);
        }
      } catch {
        // ignore transient errors
      } finally {
        if (alive.current) timer = window.setTimeout(tick, 2500);
      }
    };
    tick();
    return () => {
      alive.current = false;
      window.clearTimeout(timer);
    };
  }, []);

  if (loading) return <Loading label="Scanning your PC" />;

  const cpu = snap?.cpu_usage ?? null;
  const gpu = snap?.gpu_usage ?? null;
  const memPctOrNull = snap && snap.mem_total > 0 ? (snap.mem_used / snap.mem_total) * 100 : null;
  const disk = snap?.disks[0];
  const diskPct = disk && disk.total > 0 ? (disk.used / disk.total) * 100 : null;

  const installedCount = apps.filter((a) => a.installed).length;
  const favApps = (settings?.favorites ?? [])
    .map((id) => apps.find((a) => a.id === id))
    .filter((a): a is NonNullable<typeof a> => Boolean(a));
  const recentApps = (settings?.recent ?? [])
    .map((id) => apps.find((a) => a.id === id))
    .filter((a): a is NonNullable<typeof a> => Boolean(a));

  const quickActions = [
    { t: "Scan Temp Files", s: "Review space you can free", icon: Trash2, run: () => navigate("toolkit") },
    { t: "Startup Manager", s: "Manage startup apps", icon: ListChecks, run: () => navigate("startup") },
    { t: "Task Manager", s: "Open Task Manager", icon: Gauge, run: () => openTool("task_manager") },
    { t: "Device Manager", s: "Open Device Manager", icon: MonitorCog, run: () => openTool("device_manager") },
    { t: "System Monitor", s: "CPU, memory and disks", icon: CpuIcon, run: () => navigate("monitor") },
    { t: "Windows Settings", s: "Open Settings", icon: SettingsIcon, run: () => openTool("windows_settings") },
    { t: "App Manager", s: "Organize installed apps", icon: AppWindow, run: () => navigate("apps") },
    { t: "System Info", s: "Open msinfo32", icon: Info, run: () => openTool("system_info") },
  ];

  return (
    <div>
      <section className="board" aria-label="Live performance">
        <svg className="board-traces" viewBox="0 0 1000 300" preserveAspectRatio="none" aria-hidden="true">
          <g style={{ opacity: wire(cpu) }}>
            <path pathLength={1} d="M270 60 H340 L372 92 H500" />
            <path pathLength={1} d="M270 84 H326 L354 112 H500" />
          </g>
          <g style={{ opacity: wire(memPctOrNull) }}>
            <path pathLength={1} d="M270 240 H340 L430 150 H500" />
            <path pathLength={1} d="M270 216 H326 L412 130 H500" />
          </g>
          <g style={{ opacity: wire(gpu) }}>
            <path pathLength={1} d="M730 60 H660 L628 92 H500" />
            <path pathLength={1} d="M730 84 H674 L646 112 H500" />
          </g>
          <g style={{ opacity: wire(diskPct) }}>
            <path pathLength={1} d="M730 240 H660 L570 150 H500" />
            <path pathLength={1} d="M730 216 H674 L588 130 H500" />
          </g>
        </svg>
        <BoardChip
          side="left"
          label="CPU"
          pct={cpu}
          detail={snap?.cpu_temp != null ? `${snap.cpu_temp.toFixed(0)} \u00b0C` : hardware?.cpu?.detail}
        />
        <div className="board-core">
          <LogoMark className="logo" />
          <strong>{installedCount} utilities ready</strong>
          <span>{hardware?.os?.name ?? "Windows"}</span>
        </div>
        <BoardChip
          side="right"
          label="GPU"
          pct={gpu}
          detail={
            !snap
              ? undefined
              : gpu == null
                ? "Live GPU stats need an NVIDIA card"
                : [
                    snap.gpu_temp != null ? `${snap.gpu_temp.toFixed(0)} \u00b0C` : "",
                    snap.gpu_mem_used != null && snap.gpu_mem_total != null
                      ? `${fmtBytes(snap.gpu_mem_used)} of ${fmtBytes(snap.gpu_mem_total)}`
                      : "",
                  ]
                    .filter(Boolean)
                    .join(", ")
          }
        />
        <BoardChip
          side="left"
          label="Memory"
          pct={memPctOrNull}
          detail={snap ? `${fmtBytes(snap.mem_used)} of ${fmtBytes(snap.mem_total)}` : undefined}
        />
        <BoardChip
          side="right"
          label={disk ? `Drive ${disk.mount.replace(/[:\\]/g, "")}` : "Drive"}
          pct={diskPct}
          detail={disk ? `${fmtBytes(disk.total - disk.used)} free` : undefined}
        />
      </section>

      <div className="section-title">System overview</div>
      <div className="grid cols-4">
        <Stat icon={CpuIcon} label="Processor" value={hardware?.cpu?.name ?? "Unavailable"} sub={hardware?.cpu?.detail} />
        <Stat
          icon={MonitorCog}
          label="Graphics"
          value={hardware?.gpus[0]?.name ?? "Unavailable"}
          sub={hardware?.gpus.length ? `${hardware.gpus.length} adapter(s)` : undefined}
        />
        <Stat icon={MemoryStick} label="Memory" value={hardware?.memory?.name ?? "Unavailable"} sub={hardware?.memory?.detail} />
        <Stat icon={Boxes} label="Utilities" value={`${installedCount} installed`} sub={`${apps.length} in catalog`} />
      </div>

      {procs.length > 0 && (
        <>
          <div className="section-title">Top processes</div>
          <div className="card pad-lg">
            {procs.map((p) => (
              <div className="row" key={p.pid}>
                <span className="meta">
                  <span className="name">{p.name}</span>
                  <span className="path">PID {p.pid}</span>
                </span>
                <div style={{ display: "flex", gap: 18, alignItems: "center" }}>
                  <span className="amount" style={{ minWidth: 72, textAlign: "right" }}>
                    {p.cpu.toFixed(0)}% CPU
                  </span>
                  <span className="amount" style={{ minWidth: 72, textAlign: "right" }}>
                    {fmtBytes(p.mem)}
                  </span>
                </div>
              </div>
            ))}
          </div>
        </>
      )}

      <div className="section-title">Quick actions</div>
      <div className="grid cols-2">
        {quickActions.map((q) => {
          const Icon = q.icon;
          return (
            <button key={q.t} className="quick interactive" onClick={q.run}>
              <div className="chip">
                <Icon size={19} />
              </div>
              <div>
                <div className="t">{q.t}</div>
                <div className="s">{q.s}</div>
              </div>
              <ChevronRight className="go" size={18} />
            </button>
          );
        })}
      </div>

      <div className="section-title">Favorites</div>
      {favApps.length ? (
        <div className="grid cols-2">
          {favApps.map((a) => (
            <AppCard key={a.id} app={a} />
          ))}
        </div>
      ) : (
        <EmptyState
          icon={<Star size={30} />}
          title="No favorites yet"
          hint="Star an app on the Applications page to pin it here."
        />
      )}

      {recentApps.length > 0 && (
        <>
          <div className="section-title">Recently used</div>
          <div className="grid cols-3">
            {recentApps.map((a) => (
              <div key={a.id} className="card">
                <div className="spread">
                  <div style={{ display: "flex", alignItems: "center", gap: 11, minWidth: 0 }}>
                    <AppLogo url={iconFor(a)} category={a.category} size={38} />
                    <div style={{ minWidth: 0 }}>
                      <div style={{ fontWeight: 650, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                        {a.name}
                      </div>
                      <div className="vendor">{a.vendor}</div>
                    </div>
                  </div>
                  <button
                    className="btn primary sm"
                    disabled={!a.launch_path && !a.launch_aumid}
                    onClick={() => launch(a)}
                  >
                    Launch
                  </button>
                </div>
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
