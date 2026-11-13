// New PC Setup: pick essential apps (with real logos) and export them as a .bat
// installer to run on any PC. The installer shows overall progress plus winget's
// download progress for each app. Pairs with the Runtimes page.

import { useEffect, useMemo, useState } from "react";
import { FileDown, ListChecks, X, Download } from "lucide-react";
import { api, pickSavePath } from "../api";
import type { EssentialItem } from "../types";
import { useNexus } from "../state";
import { Loading, ErrorBanner, AppLogo } from "../components/ui";

export function Setup() {
  const { setNotice, setupSelection: selected, setSetupSelection: setSelected } = useNexus();
  const [items, setItems] = useState<EssentialItem[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .getEssentials()
      .then(setItems)
      .catch((e) => setError(String(e)));
  }, []);

  const groups = useMemo(() => {
    const map = new Map<string, EssentialItem[]>();
    (items ?? []).forEach((it) => {
      const arr = map.get(it.group) ?? [];
      arr.push(it);
      map.set(it.group, arr);
    });
    return [...map.entries()];
  }, [items]);

  const toggle = (id: string) => {
    const next = new Set(selected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setSelected(next);
  };

  const selectAll = () => setSelected(new Set((items ?? []).map((i) => i.winget_id)));
  const selectMissing = () =>
    setSelected(new Set((items ?? []).filter((i) => !i.installed).map((i) => i.winget_id)));
  const clearSelection = () => setSelected(new Set());

  const saveInstaller = async (wingetIds: string[], fileName: string) => {
    if (wingetIds.length === 0) return;
    try {
      const path = await pickSavePath(fileName);
      if (!path) return;
      await api.exportInstallScript(path, wingetIds);
      setNotice(`Saved a custom installer with ${wingetIds.length} app${wingetIds.length === 1 ? "" : "s"}.`);
    } catch (e) {
      setNotice(`Could not export installer: ${e}`);
    }
  };

  // Keep the catalog order so the installer runs in the same order as the list.
  const exportInstaller = () =>
    saveInstaller(
      (items ?? []).filter((i) => selected.has(i.winget_id)).map((i) => i.winget_id),
      "nexus-setup.bat",
    );

  const exportOne = (item: EssentialItem) =>
    saveInstaller([item.winget_id], `install-${item.id}.bat`);

  if (!items && !error) return <Loading label="Loading essential apps" />;

  const selectedCount = selected.size;

  return (
    <div>
      <div className="page-head spread">
        <div>
          <h1>New PC Setup</h1>
          <p>Pick the essentials and export one installer for a fresh Windows install.</p>
        </div>
        <div className="btn-row" style={{ flexWrap: "wrap", justifyContent: "flex-end" }}>
          <button className="btn" onClick={selectAll}>
            <ListChecks size={15} /> Select all
          </button>
          <button className="btn" onClick={selectMissing} title="Select only the apps not found on this PC">
            <ListChecks size={15} /> Select missing
          </button>
          <button className="btn" onClick={clearSelection} disabled={selectedCount === 0}>
            <X size={15} /> Clear
          </button>
          <button className="btn primary" onClick={exportInstaller} disabled={selectedCount === 0}>
            <FileDown size={15} /> Export installer ({selectedCount})
          </button>
        </div>
      </div>

      {error && <ErrorBanner message={error} />}

      <div className="note-banner">
        <Download size={16} />
        <span>
          Saves a .bat you can run on any PC with winget. It shows the overall percentage
          and each app's download progress as it goes. For Visual C++, DirectX, and .NET,
          use the Runtimes page.
        </span>
      </div>

      {groups.map(([group, list]) => (
        <div key={group}>
          <div className="section-title">{group}</div>
          <div>
            {list.map((item) => (
              <div key={item.id} className="row">
                <label className="checkbox" style={{ minWidth: 0, gap: 12 }}>
                  <input
                    type="checkbox"
                    checked={selected.has(item.winget_id)}
                    onChange={() => toggle(item.winget_id)}
                  />
                  <AppLogo domain={item.domain} size={32} />
                  <span className="meta">
                    <span className="name">
                      {item.name}
                      {item.installed && (
                        <span className="badge ok" style={{ marginLeft: 8 }} title="Found on this PC">
                          Installed
                        </span>
                      )}
                    </span>
                    <span className="path" style={{ fontFamily: "var(--font)", color: "var(--fg-dim)" }}>
                      {item.description}
                    </span>
                  </span>
                </label>
                <button className="btn sm" onClick={() => exportOne(item)}>
                  <FileDown size={14} /> Export
                </button>
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
