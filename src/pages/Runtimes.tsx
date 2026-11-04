// Runtimes: install common redistributables a PC needs (Visual C++, DirectX,
// .NET) through winget. Installing is a real system change and triggers a UAC
// prompt, so the bulk action confirms first. Detection reads the registry.

import { useEffect, useMemo, useState } from "react";
import { Loader2, Download, RefreshCw, CheckCircle2, PackageCheck } from "lucide-react";
import { api } from "../api";
import type { RuntimeItem } from "../types";
import { useNexus } from "../state";
import { Loading, ErrorBanner } from "../components/ui";
import { Modal } from "../components/Modal";

function StatusBadge({ installed }: { installed: boolean | null }) {
  if (installed === true)
    return (
      <span className="badge ok">
        <span className="dot" /> Installed
      </span>
    );
  if (installed === false)
    return (
      <span className="badge off">
        <span className="dot" /> Not installed
      </span>
    );
  return <span className="badge">Cannot detect</span>;
}

export function Runtimes() {
  const { setNotice, job, runJob } = useNexus();
  const [items, setItems] = useState<RuntimeItem[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirmAll, setConfirmAll] = useState(false);

  const load = async () => {
    setError(null);
    try {
      setItems(await api.getRuntimes());
    } catch (e) {
      setError(String(e));
    }
  };

  // Check on opening, and again when a running install finishes.
  const busy = job !== null;
  useEffect(() => {
    if (!busy || !items) load();
  }, [busy]);

  const groups = useMemo(() => {
    const map = new Map<string, RuntimeItem[]>();
    (items ?? []).forEach((it) => {
      const arr = map.get(it.group) ?? [];
      arr.push(it);
      map.set(it.group, arr);
    });
    return [...map.entries()];
  }, [items]);

  const missing = (items ?? []).filter((i) => i.installed !== true);

  const installOne = async (item: RuntimeItem) => {
    await runJob({ id: item.winget_id, label: `Installing ${item.name}` }, async () => {
      try {
        const res = await api.installRuntime(item.winget_id);
        setNotice(res.ok ? `${item.name}: ${res.message}` : `${item.name} failed: ${res.message}`);
      } catch (e) {
        setNotice(`${item.name} failed: ${e}`);
      }
    });
  };

  const installAllMissing = async () => {
    setConfirmAll(false);
    if (missing.length === 0) return;
    const toInstall = missing;
    await runJob({ id: "all-runtimes", label: "Installing runtimes" }, async (update) => {
      const failed: string[] = [];
      for (let i = 0; i < toInstall.length; i++) {
        const m = toInstall[i];
        update(`Installing ${i + 1} of ${toInstall.length}: ${m.name}`);
        // One failure shouldn't stop the rest of the list.
        const ok = await api.installRuntime(m.winget_id).then((res) => res.ok, () => false);
        if (!ok) failed.push(m.name);
      }
      setNotice(
        failed.length === 0
          ? `Installed ${toInstall.length} runtime${toInstall.length === 1 ? "" : "s"}.`
          : `${toInstall.length - failed.length} of ${toInstall.length} installed. Failed: ${failed.join(", ")}.`,
      );
    });
  };

  if (!items && !error) return <Loading label="Checking installed runtimes" />;

  return (
    <div>
      <div className="page-head spread">
        <div>
          <h1>Runtimes & Redistributables</h1>
          <p>Install the common runtimes a Windows PC needs for apps and games.</p>
        </div>
        <div className="btn-row">
          <button className="btn" onClick={load} disabled={busy}>
            <RefreshCw size={15} className={busy ? "spin" : ""} /> Recheck
          </button>
          <button
            className="btn primary"
            onClick={() => setConfirmAll(true)}
            disabled={busy || missing.length === 0}
          >
            <PackageCheck size={15} /> Install all missing ({missing.length})
          </button>
        </div>
      </div>

      {error && <ErrorBanner message={error} />}

      <div className="note-banner">
        <Download size={16} />
        <span>
          Installs run through winget and each one asks for administrator permission
          (UAC). DirectX cannot be detected reliably, so it always shows as installable.
        </span>
      </div>

      {groups.map(([group, list]) => (
        <div key={group}>
          <div className="section-title">{group}</div>
          <div>
            {list.map((item) => (
              <div key={item.id} className="row">
                <span className="meta">
                  <span className="name">{item.name}</span>
                  <span className="path" style={{ fontFamily: "var(--font)", color: "var(--fg-dim)" }}>
                    {item.description}
                  </span>
                </span>
                <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
                  <StatusBadge installed={item.installed} />
                  {item.installed === true ? (
                    <button className="btn ghost sm" onClick={() => installOne(item)} disabled={busy}>
                      {job?.id === item.winget_id ? (
                        <Loader2 size={14} className="spin" />
                      ) : (
                        <RefreshCw size={14} />
                      )}
                      Reinstall
                    </button>
                  ) : (
                    <button className="btn primary sm" onClick={() => installOne(item)} disabled={busy}>
                      {job?.id === item.winget_id ? (
                        <Loader2 size={14} className="spin" />
                      ) : (
                        <Download size={14} />
                      )}
                      Install
                    </button>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      ))}

      {confirmAll && (
        <Modal
          title="Install all missing runtimes?"
          icon={<CheckCircle2 size={20} />}
          onClose={() => setConfirmAll(false)}
          footer={
            <>
              <button className="btn ghost" onClick={() => setConfirmAll(false)}>
                Cancel
              </button>
              <button className="btn primary" onClick={installAllMissing}>
                <PackageCheck size={15} /> Install {missing.length}
              </button>
            </>
          }
        >
          <p>
            This installs {missing.length} runtime{missing.length === 1 ? "" : "s"} using
            winget. Windows will ask for administrator permission for each one. You can
            keep using the rest of NEXUS while it runs.
          </p>
        </Modal>
      )}
    </div>
  );
}
