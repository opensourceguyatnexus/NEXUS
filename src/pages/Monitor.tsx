// System Monitor: a light live view. Polls a snapshot every two seconds while
// the page is open. Metrics the machine does not expose show as "Unavailable".

import { useEffect, useRef, useState } from "react";
import { Cpu, MemoryStick, MonitorCog, HardDrive } from "lucide-react";
import { api, fmtBytes } from "../api";
import type { MonitorSnapshot } from "../types";
import { Meter, Loading, ErrorBanner } from "../components/ui";

function pct(used: number, total: number): number {
  if (total <= 0) return 0;
  return (used / total) * 100;
}

export function Monitor() {
  const [snap, setSnap] = useState<MonitorSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const alive = useRef(true);

  useEffect(() => {
    alive.current = true;
    let timer: number;
    const tick = async () => {
      try {
        const s = await api.getMonitor();
        if (alive.current) {
          setSnap(s);
          setError(null);
        }
      } catch (e) {
        if (alive.current) setError(String(e));
      } finally {
        if (alive.current) timer = window.setTimeout(tick, 2000);
      }
    };
    tick();
    return () => {
      alive.current = false;
      window.clearTimeout(timer);
    };
  }, []);

  if (!snap && !error) return <Loading label="Reading sensors" />;

  const cpu = snap?.cpu_usage ?? null;
  const memPct = snap ? pct(snap.mem_used, snap.mem_total) : 0;

  return (
    <div>
      <div className="page-head">
        <h1>System Monitor</h1>
        <p>Live CPU, memory, and disk usage. Updates every 2 seconds.</p>
      </div>

      {error && <ErrorBanner message={error} />}

      <div className="grid cols-2">
        <div className="stat">
          <div className="head">
            <div className="chip">
              <Cpu size={17} />
            </div>
            <div className="label">CPU Usage</div>
          </div>
          <div className="value big">
            {cpu !== null ? cpu.toFixed(0) : "n/a"}
            <span className="unit">%</span>
          </div>
          <Meter pct={cpu ?? 0} warn={(cpu ?? 0) > 85} />
          <div className="sub">
            Temperature:{" "}
            {snap?.cpu_temp != null ? `${snap.cpu_temp.toFixed(0)} C` : "Unavailable on this system"}
          </div>
        </div>

        <div className="stat">
          <div className="head">
            <div className="chip">
              <MemoryStick size={17} />
            </div>
            <div className="label">Memory</div>
          </div>
          <div className="value big">
            {snap ? fmtBytes(snap.mem_used) : "n/a"}
            <span className="unit">of {snap ? fmtBytes(snap.mem_total) : ""}</span>
          </div>
          <Meter pct={memPct} warn={memPct > 90} />
          <div className="sub">{memPct.toFixed(0)}% used</div>
        </div>

        <div className="stat">
          <div className="head">
            <div className="chip">
              <MonitorCog size={17} />
            </div>
            <div className="label">Graphics</div>
          </div>
          <div className="value" style={{ fontSize: "var(--text-body)" }}>
            {snap?.gpu_name ?? "Unavailable"}
          </div>
          {snap?.gpu_usage != null ? (
            <>
              <div className="value big" style={{ marginTop: 8 }}>
                {snap.gpu_usage.toFixed(0)}
                <span className="unit">% usage</span>
              </div>
              <Meter pct={snap.gpu_usage} warn={snap.gpu_usage > 90} />
              <div className="gpu-grid">
                <div>
                  <span className="k">Temp</span>
                  <span className="v">{snap.gpu_temp != null ? `${snap.gpu_temp.toFixed(0)} C` : "n/a"}</span>
                </div>
                <div>
                  <span className="k">VRAM</span>
                  <span className="v">
                    {snap.gpu_mem_used != null && snap.gpu_mem_total != null
                      ? `${fmtBytes(snap.gpu_mem_used)} / ${fmtBytes(snap.gpu_mem_total)}`
                      : "n/a"}
                  </span>
                </div>
                <div>
                  <span className="k">Fan</span>
                  <span className="v">{snap.gpu_fan != null ? `${snap.gpu_fan.toFixed(0)} %` : "n/a"}</span>
                </div>
              </div>
            </>
          ) : (
            <div className="sub">
              Live GPU stats use NVML and are available on NVIDIA cards. Not available
              for this GPU or driver.
            </div>
          )}
        </div>

        <div className="stat">
          <div className="head">
            <div className="chip">
              <HardDrive size={17} />
            </div>
            <div className="label">Disks</div>
          </div>
          <div style={{ marginTop: 4, display: "flex", flexDirection: "column", gap: 13 }}>
            {snap?.disks.map((d) => (
              <div key={d.mount}>
                <div className="spread" style={{ fontSize: 13 }}>
                  <span className="mono" style={{ fontWeight: 600 }}>{d.mount}</span>
                  <span className="muted">
                    {fmtBytes(d.used)} / {fmtBytes(d.total)}
                  </span>
                </div>
                <Meter pct={pct(d.used, d.total)} warn={pct(d.used, d.total) > 90} />
              </div>
            ))}
            {snap && snap.disks.length === 0 && (
              <span className="muted" style={{ fontSize: 13 }}>
                No fixed disks reported.
              </span>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
