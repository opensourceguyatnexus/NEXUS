// Startup Manager: list programs that run at sign-in and toggle them on or off.
// System-wide entries are read-only unless NEXUS runs as administrator.

import { useEffect, useState } from "react";
import { Power, RefreshCw } from "lucide-react";
import { api } from "../api";
import type { StartupItem } from "../types";
import { useNexus } from "../state";
import { Loading, ErrorBanner, EmptyState } from "../components/ui";

export function Startup() {
  const { setNotice, elevated, restartAsAdmin } = useNexus();
  const [items, setItems] = useState<StartupItem[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = async () => {
    setError(null);
    try {
      setItems(await api.getStartup());
    } catch (e) {
      setError(String(e));
    }
  };

  useEffect(() => {
    load();
  }, []);

  const toggle = async (it: StartupItem) => {
    const key = `${it.source}:${it.name}`;
    setBusy(key);
    try {
      await api.setStartupEnabled(it.source, it.name, !it.enabled);
      await load();
      setNotice(`${it.name} ${!it.enabled ? "enabled" : "disabled"} at startup.`);
    } catch (e) {
      setNotice(`Could not change ${it.name}: ${e}`);
    } finally {
      setBusy(null);
    }
  };

  if (!items && !error) return <Loading label="Reading startup programs" />;

  const enabledCount = items?.filter((i) => i.enabled).length ?? 0;

  return (
    <div>
      <div className="page-head spread">
        <div>
          <h1>Startup Manager</h1>
          <p>Choose which programs run when you sign in. Fewer means a faster boot.</p>
        </div>
        <button className="btn" onClick={load}>
          <RefreshCw size={15} /> Refresh
        </button>
      </div>

      {error && <ErrorBanner message={error} />}

      <div className="note-banner">
        <Power size={16} />
        <span style={{ flex: 1 }}>
          {enabledCount} of {items?.length ?? 0} startup programs are enabled.
          {!elevated &&
            " System-wide entries are read-only here because changing them needs administrator rights."}
        </span>
        {!elevated && (
          <button className="btn sm" onClick={restartAsAdmin} style={{ flexShrink: 0 }}>
            Restart as admin
          </button>
        )}
      </div>

      {items && items.length === 0 ? (
        <EmptyState title="No startup programs found" hint="Nothing is set to run at sign-in." />
      ) : (
        <div>
          {items?.map((it) => {
            const key = `${it.source}:${it.name}`;
            return (
              <div className="row" key={key}>
                <span className="meta">
                  <span className="name">{it.name}</span>
                  <span className="path">{it.command}</span>
                </span>
                <div style={{ display: "flex", alignItems: "center", gap: 14 }}>
                  <span className="badge">{it.location}</span>
                  {it.can_toggle ? (
                    <label className="switch" title={it.enabled ? "Disable" : "Enable"}>
                      <input
                        type="checkbox"
                        checked={it.enabled}
                        disabled={busy === key}
                        onChange={() => toggle(it)}
                      />
                      <span className="track" />
                    </label>
                  ) : (
                    <span className="badge off" title="Needs administrator rights">
                      {it.enabled ? "On" : "Off"} · locked
                    </span>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
