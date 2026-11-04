// Applications: the single place to find, launch, and manage apps. It combines
// the hardware based suggestions with the full catalog and your custom apps,
// organized by clear category headers.

import { useMemo, useState } from "react";
import { Plus, Search, X, Sparkles } from "lucide-react";
import type { UnifiedApp } from "../state";
import { useNexus } from "../state";
import { AppCard } from "../components/AppCard";
import { Loading, EmptyState } from "../components/ui";
import { CATEGORY_LABELS } from "../types";

// Order used for both the filter chips and the grouped sections.
const CATEGORY_ORDER = [
  "mouse_keyboard",
  "audio",
  "gpu",
  "motherboard",
  "rgb_cooling",
  "benchmark",
  "custom",
];

const FILTERS: [string, string][] = [
  ["all", "All"],
  ...CATEGORY_ORDER.map((c) => [c, CATEGORY_LABELS[c]] as [string, string]),
];

function AddCustomForm({ onClose }: { onClose: () => void }) {
  const { addCustomApp } = useNexus();
  const [name, setName] = useState("");
  const [exec, setExec] = useState("");
  const [category, setCategory] = useState("custom");
  const [desc, setDesc] = useState("");
  const [url, setUrl] = useState("");
  const [err, setErr] = useState<string | null>(null);

  const save = async () => {
    if (!name.trim()) return setErr("Give the app a name.");
    if (!exec.trim()) return setErr("Enter the full path to the executable.");
    if (!/\.exe"?$/i.test(exec.trim())) return setErr("The path should point to an .exe file.");
    const id =
      "custom-" +
      name.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/(^-|-$)/g, "") +
      "-" +
      Date.now().toString(36);
    await addCustomApp({
      id,
      name: name.trim(),
      category,
      description: desc.trim() || "Custom application.",
      exec_path: exec.trim().replace(/^"|"$/g, ""),
      download_url: url.trim(),
    });
    onClose();
  };

  return (
    <div className="card pad-lg" style={{ marginBottom: 18 }}>
      <div className="spread" style={{ marginBottom: 14 }}>
        <strong>Add a custom app</strong>
        <button className="btn ghost icon" onClick={onClose}>
          <X size={16} />
        </button>
      </div>
      {err && <div className="err-banner">{err}</div>}
      <div className="grid cols-2">
        <div className="field">
          <label>Name</label>
          <input type="text" value={name} onChange={(e) => setName(e.target.value)} placeholder="For example, HWiNFO" />
        </div>
        <div className="field">
          <label>Category</label>
          <select value={category} onChange={(e) => setCategory(e.target.value)}>
            <option value="custom">Custom</option>
            <option value="mouse_keyboard">Mouse & Keyboard</option>
            <option value="audio">Headsets & Audio</option>
            <option value="gpu">GPU & Graphics</option>
            <option value="motherboard">Motherboard & System</option>
            <option value="rgb_cooling">RGB, Fans & Cooling</option>
            <option value="benchmark">Benchmark & Diagnostics</option>
          </select>
        </div>
      </div>
      <div className="field">
        <label>Executable path</label>
        <input
          type="text"
          value={exec}
          onChange={(e) => setExec(e.target.value)}
          placeholder="C:\Program Files\Vendor\App.exe"
        />
      </div>
      <div className="field">
        <label>Description (optional)</label>
        <input type="text" value={desc} onChange={(e) => setDesc(e.target.value)} />
      </div>
      <div className="field">
        <label>Official page (optional, https)</label>
        <input type="text" value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://" />
      </div>
      <div className="btn-row">
        <button className="btn primary" onClick={save}>
          Save app
        </button>
        <button className="btn ghost" onClick={onClose}>
          Cancel
        </button>
      </div>
    </div>
  );
}

export function Applications() {
  const { apps, hardware, loading } = useNexus();
  const [q, setQ] = useState("");
  const [filter, setFilter] = useState<string>("all");
  const [installedOnly, setInstalledOnly] = useState(false);
  const [showForm, setShowForm] = useState(false);

  // Hardware based suggestions, shown only on the unfiltered view.
  const hwBlob = useMemo(() => {
    if (!hardware) return "";
    const parts: string[] = [];
    if (hardware.cpu) parts.push(hardware.cpu.name);
    hardware.gpus.forEach((g) => parts.push(g.name));
    hardware.peripherals.forEach((p) => parts.push(p.name, p.detail));
    return parts.join(" ").toLowerCase();
  }, [hardware]);

  const recommended = useMemo(() => {
    if (!hwBlob) return [] as UnifiedApp[];
    const vendorKeys: Record<string, string[]> = {
      "nvidia-app": ["nvidia", "geforce", "rtx", "gtx"],
      "amd-adrenalin": ["radeon", "amd "],
      "razer-synapse": ["razer"],
      "logitech-ghub": ["logitech"],
      "steelseries-gg": ["steelseries"],
      "corsair-icue": ["corsair"],
      "hyperx-ngenuity": ["hyperx"],
      "armoury-crate": ["asus", "rog"],
      "msi-center": ["msi"],
    };
    return apps.filter((a) => {
      const keys = vendorKeys[a.id];
      return keys && keys.some((k) => hwBlob.includes(k));
    });
  }, [apps, hwBlob]);

  const filtered = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return apps.filter((a) => {
      if (filter === "custom" && !a.custom) return false;
      if (filter !== "all" && filter !== "custom" && a.category !== filter) return false;
      if (installedOnly && !a.installed) return false;
      if (needle && !(`${a.name} ${a.vendor} ${a.description}`.toLowerCase().includes(needle)))
        return false;
      return true;
    });
  }, [apps, q, filter, installedOnly]);

  if (loading) return <Loading label="Loading applications" />;

  const searching = q.trim().length > 0 || installedOnly;
  const showGrouped = filter === "all";
  const showRecommended = filter === "all" && !searching && recommended.length > 0;

  return (
    <div>
      <div className="page-head spread">
        <div>
          <h1>Applications</h1>
          <p>Find, launch, and manage your hardware and system apps in one place.</p>
        </div>
        <button className="btn primary" onClick={() => setShowForm((s) => !s)}>
          <Plus size={15} /> Add custom app
        </button>
      </div>

      {showForm && <AddCustomForm onClose={() => setShowForm(false)} />}

      <div className="search">
        <Search className="ic" size={16} />
        <input
          type="text"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder="Search apps by name, vendor, or description"
        />
      </div>

      <div className="filter-tabs">
        {FILTERS.map(([id, label]) => (
          <button
            key={id}
            className={`chip-btn ${filter === id ? "active" : ""}`}
            onClick={() => setFilter(id)}
          >
            {label}
          </button>
        ))}
        <label
          className="checkbox"
          style={{ marginLeft: 6, fontSize: 12.5, fontWeight: 600, color: "var(--fg-dim)" }}
        >
          <input
            type="checkbox"
            checked={installedOnly}
            onChange={(e) => setInstalledOnly(e.target.checked)}
          />
          Installed only
        </label>
      </div>

      {showRecommended && (
        <>
          <div className="section-title">
            <Sparkles size={13} style={{ verticalAlign: "-2px", marginRight: 6 }} />
            Suggested for your hardware
          </div>
          <div className="grid cols-2">
            {recommended.map((a) => (
              <AppCard key={`rec-${a.id}`} app={a} />
            ))}
          </div>
        </>
      )}

      {filtered.length === 0 && (
        <EmptyState title="Nothing matches" hint="Try a different search or filter, or add a custom app." />
      )}

      {showGrouped
        ? CATEGORY_ORDER.map((cat) => {
            const list =
              cat === "custom"
                ? filtered.filter((a) => a.custom)
                : filtered.filter((a) => a.category === cat && !a.custom);
            if (list.length === 0) return null;
            return (
              <div key={cat}>
                <div className="section-title">{CATEGORY_LABELS[cat]}</div>
                <div className="grid cols-2">
                  {list.map((a) => (
                    <AppCard key={a.id} app={a} />
                  ))}
                </div>
              </div>
            );
          })
        : filtered.length > 0 && (
            <>
              <div className="section-title">{CATEGORY_LABELS[filter] ?? "Results"}</div>
              <div className="grid cols-2">
                {filtered.map((a) => (
                  <AppCard key={a.id} app={a} />
                ))}
              </div>
            </>
          )}
    </div>
  );
}
