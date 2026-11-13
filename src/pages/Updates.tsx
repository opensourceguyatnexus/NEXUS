// Update Center: shows apps with available upgrades (via winget) and updates
// them one at a time or all at once.

import { useEffect, useState } from "react";
import { Loader2, RefreshCw, ArrowUpCircle, CheckCircle2, PackageCheck } from "lucide-react";
import { api } from "../api";
import type { AppUpdate } from "../types";
import { useNexus } from "../state";
import { Loading, ErrorBanner, EmptyState } from "../components/ui";
import { Modal } from "../components/Modal";

const ALL_UPDATES = "all-updates";

export function Updates() {
  const { setNotice, job, runJob } = useNexus();
  const [items, setItems] = useState<AppUpdate[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const [confirmAll, setConfirmAll] = useState(false);

  const load = async () => {
    setChecking(true);
    setError(null);
    try {
      setItems(await api.getUpdates());
    } catch (e) {
      setError(String(e));
    } finally {
      setChecking(false);
    }
  };

  // Check on opening, and again when a running update finishes.
  const jobRunning = job !== null;
  useEffect(() => {
    if (!jobRunning || !items) load();
  }, [jobRunning]);

  const busy = jobRunning || checking;
  const allBusy = job?.id === ALL_UPDATES;

  const upgradeOne = async (item: AppUpdate) => {
    if (busy) return;
    await runJob({ id: item.id, label: `Updating ${item.name}` }, async () => {
      try {
        const res = await api.upgradeApp(item.id);
        setNotice(res.ok ? `${item.name}: ${res.message}` : `${item.name} failed: ${res.message}`);
      } catch (e) {
        setNotice(`${item.name} failed: ${e}`);
      }
    });
  };

  const upgradeEverything = async () => {
    setConfirmAll(false);
    await runJob({ id: ALL_UPDATES, label: "Updating all apps. This can take a few minutes." }, async () => {
      try {
        const res = await api.upgradeAll();
        setNotice(res.ok ? res.message : `Update all failed: ${res.message}`);
      } catch (e) {
        setNotice(`Update all failed: ${e}`);
      }
    });
  };

  if (!items && !error) return <Loading label="Checking for updates" />;

  const total = items?.length ?? 0;
  // winget cannot tell which version these have, so "Update all" leaves them out.
  const isUnknown = (i: AppUpdate) => i.current.trim().toLowerCase() === "unknown";
  const count = items?.filter((i) => !isUnknown(i)).length ?? 0;

  return (
    <div>
      <div className="page-head spread">
        <div>
          <h1>Update Center</h1>
          <p>Keep your installed apps up to date through winget.</p>
        </div>
        <div className="btn-row">
          <button className="btn" onClick={load} disabled={busy}>
            <RefreshCw size={15} className={checking ? "spin" : ""} /> Check again
          </button>
          <button
            className="btn primary"
            onClick={() => setConfirmAll(true)}
            disabled={busy || count === 0}
          >
            {allBusy ? <Loader2 size={15} className="spin" /> : <PackageCheck size={15} />}
            Update all ({count})
          </button>
        </div>
      </div>

      {error && <ErrorBanner message={error} />}

      {error && total === 0 ? null : total === 0 ? (
        <EmptyState
          icon={<CheckCircle2 size={30} />}
          title="Everything is up to date"
          hint="No winget-managed apps have updates available right now."
        />
      ) : (
        <div>
          {items?.map((item) => (
            <div className="row" key={item.id}>
              <span className="meta">
                <span className="name">{item.name}</span>
                <span className="path">{item.id}</span>
              </span>
              <div style={{ display: "flex", alignItems: "center", gap: 14 }}>
                <span className="mono muted" style={{ fontSize: "var(--text-meta)" }}>
                  {isUnknown(item) ? "version unknown" : item.current}{" "}
                  <span style={{ opacity: 0.6 }}>→</span>{" "}
                  <span style={{ color: "var(--accent-text)", fontWeight: 600 }}>{item.available}</span>
                </span>
                <button className="btn sm" onClick={() => upgradeOne(item)} disabled={busy}>
                  {job?.id === item.id ? (
                    <Loader2 size={14} className="spin" />
                  ) : (
                    <ArrowUpCircle size={14} />
                  )}
                  Update
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {confirmAll && (
        <Modal
          title="Update all apps?"
          icon={<PackageCheck size={20} />}
          onClose={() => setConfirmAll(false)}
          footer={
            <>
              <button className="btn ghost" onClick={() => setConfirmAll(false)}>
                Cancel
              </button>
              <button className="btn primary" onClick={upgradeEverything}>
                <PackageCheck size={15} /> Update {count}
              </button>
            </>
          }
        >
          <p>
            This updates {count} app{count === 1 ? "" : "s"} with winget. Some may ask for
            administrator permission. It can take a few minutes.
            {count < total &&
              " Apps whose current version is unknown are skipped. Use their own Update button."}
          </p>
        </Modal>
      )}
    </div>
  );
}
